"""Emit test vectors for the save and load menus (src/savemenu.rs
`save_menu`, 25FD:0002..010F, and `load_menu`, 25FD:0242..028C, with the
slot picker 25FD:0718) from the original code.

Usage: gen_savemenu.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/savemenu.rs) the
operation (0 save, 1 load), the place (5E6, D48), the buttons (AFF9),
191D, 5E0, A902, 5C8, the slots (1A6C, 1A80), the font and bank words
(102C, 1036, 1030), 61E, 7A46, C995, what was at 9686, NAMES.DAT's ten
names (or no file), and a script of clicks (each with AFF9 after it) and
typed names (or Esc). Every routine the menus call out to is a stub
whose Python hook logs it with its registers and plays the script:
0000:8FC4 (clears AFF9), 9AE6/9AF8 (fonts), 5C86 (a bank), 7428 (a file
loaded: NAMES.DAT's names written at 9686), 5828, 9A6E, 5364, 9466, 541E
(drawing), 1194:564D (a click: AX, BX; 5C8 bit 1 cleared as its way
back clears it), 0000:9EE5 (the name typed at
1044, or Esc clearing A902 bit 1000h), 1194:59C8 (a message), 1194:4D91
(a deletion), 292B:095E, 23F1:0145, 2294:11F2. The save stops at
25FD:0110 (the writing), the load at 25FD:028D (the load); a cancelled or
refused menu returns.

  savemenu | seed | result

Results: "fault" / "hang", or the end ("slot" / "back"), AL, and the
FNV-1a digests of the changed DGROUP bytes and the stubs' log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_CX, UC_X86_REG_DX, UC_X86_REG_SI, UC_X86_REG_DI, UC_X86_REG_CS, UC_X86_REG_IP
from emu import Arena, Fault, LOAD_SEG, STUB_SEG
from emu import changes, fnv, mine
from gen_attack import stream


def setup(seed):
    """(op, writes, names, clicks, edits). Mirrored in tests/savemenu.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    op = byte() & 1
    w = []
    w.append((0x05E6, bytes([4 if byte() < 0x40 else byte() % 5])))
    w.append((0x0D48, bytes([byte() % 8])))
    w.append((0xAFF9, bytes([byte() & 3 if byte() < 0x40 else 0])))
    w.append((0x191D, bytes([byte() & 1])))
    for at in (0x05E0, 0xA902, 0x05C8, 0x102C, 0x1036, 0x1030, 0x061E, 0x7A46):
        w.append((at, pk(word())))
    w.append((0x1A6C, bytes([byte()])))
    w.append((0x1A80, bytes([byte() % 10])))
    w.append((0xC995, bytes([byte()])))
    w.append((0x9686, bytes(byte() for _ in range(0x1E0))))
    names = None
    if byte() >= 0x20:
        names = bytearray()
        for _ in range(10):
            e = bytearray(0x30)
            if byte() >= 0x60:
                n = byte() % 20
                e[:n] = bytes(0x41 + byte() % 26 for _ in range(n))
            names += e
    clicks = []
    for _ in range(1 + byte() % 4):
        k = byte() % 8
        if k == 0:
            ax, bx = 0xFFFF, byte()
        elif k == 1:
            ax, bx = (byte() % 5, 0x13B + byte() % 4)[byte() % 2], 4 + byte() % 0x80
        elif k == 2:
            ax, bx = 5 + byte() % 0x136, byte() % 4
        else:
            ax, bx = 5 + word() % 0x136, 4 + byte() % 0x9A
        buttons = byte() & 3 if byte() < 0x40 else 0
        clicks.append((ax, bx, buttons))
    edits = []
    for _ in range(1 + byte() % 3):
        if byte() < 0x40:
            edits.append(None)
        else:
            n = byte() % 0x30
            edits.append(bytes(0x61 + byte() % 26 for _ in range(n)))
    return op, w, names, clicks, edits


STUBS = [
    ('wait', 0, 0x8FC4), ('font0', 0, 0x9AE6), ('font1', 0, 0x9AF8), ('bank', 0, 0x5C86),
    ('load', 0, 0x7428), ('draw', 0, 0x5828), ('fill', 0, 0x9A6E), ('centred', 0, 0x5364),
    ('show', 0, 0x9466), ('show', 0, 0x541E), ('click', 0x1194, 0x564D), ('type', 0, 0x9EE5),
    ('message', 0x1194, 0x59C8), ('delete', 0x1194, 0x4D91), ('states', 0x292B, 0x095E),
    ('automap', 0x23F1, 0x0145), ('screen', 0x2294, 0x11F2), ('cyber', 0x2F26, 0x02A1),
]


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for _, seg, off in STUBS:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
    state = {}
    reg = lambda r: a.mu.reg_read(r)

    def hook(mu, address, size, name):
        log, ds = state['log'], a.ds
        if name == 'wait':
            log.append((1, 0))
            mu.mem_write(a.lin(ds, 0xAFF9), b'\0')
        elif name in ('font0', 'font1'):
            log.append((2 if name == 'font0' else 3, reg(UC_X86_REG_AX)))
        elif name == 'bank':
            log.append((4, reg(UC_X86_REG_BX)))
        elif name == 'load':
            di, si = reg(UC_X86_REG_DI), reg(UC_X86_REG_SI)
            if di == 0x9686:
                log.append((5, si))
                if state['names'] is not None:
                    mu.mem_write(a.lin(ds, 0x9686), bytes(state['names']))
            else:
                log.append((6, si))
        elif name == 'fill':
            log.append((7, reg(UC_X86_REG_AX), reg(UC_X86_REG_BX), reg(UC_X86_REG_CX), reg(UC_X86_REG_DX)))
        elif name == 'centred':
            log.append((8, reg(UC_X86_REG_SI), reg(UC_X86_REG_AX), reg(UC_X86_REG_BX)))
        elif name == 'show':
            log.append((9, 0))
        elif name == 'click':
            c = state['clicks']
            ax, bx, buttons = c.pop(0) if c else (0xFFFF, 0, 0)
            log.append((10, ax, bx))
            mu.reg_write(UC_X86_REG_AX, ax)
            mu.reg_write(UC_X86_REG_BX, bx)
            mu.mem_write(a.lin(ds, 0xAFF9), bytes([buttons]))
            # 1194:567E, 5692: 5C8 bit 1 cleared on the way back.
            f = struct.unpack('<H', mu.mem_read(a.lin(ds, 0x05C8), 2))[0] & 0xFFFD
            mu.mem_write(a.lin(ds, 0x05C8), struct.pack('<H', f))
        elif name == 'type':
            e = state['edits']
            t = e.pop(0) if e else b''
            log.append((11, 0))
            if t is None:
                f = struct.unpack('<H', mu.mem_read(a.lin(ds, 0xA902), 2))[0] & 0xEFFF
                mu.mem_write(a.lin(ds, 0xA902), struct.pack('<H', f))
            else:
                mu.mem_write(a.lin(ds, 0x1044), t + b'\0')
        elif name == 'message':
            log.append((12, reg(UC_X86_REG_SI)))
        elif name == 'delete':
            log.append((13, reg(UC_X86_REG_SI)))
        elif name == 'states':
            log.append((14, 0))
        elif name == 'automap':
            log.append((15, 0))
        elif name == 'screen':
            log.append((16, 0))
        elif name == 'cyber':
            pass

    for name, seg, off in STUBS:
        at = a.lin(LOAD_SEG + seg, off)
        a.mu.hook_add(UC_HOOK_CODE, hook, name, at, at)
    stops = {0: 0x0110, 1: 0x028D}
    for off in stops.values():
        at = a.lin(LOAD_SEG + 0x25FD, off)
        a.mu.hook_add(UC_HOOK_CODE, lambda mu, *_: mu.emu_stop(), None, at, at)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0718)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        op, writes, names, clicks, edits = setup(seed)
        for at, data in writes:
            a.mu.mem_write(a.lin(a.ds, at), data)
        state.update(log=[], names=names, clicks=list(clicks), edits=list(edits))
        before = a.seg_bytes(a.ds)
        try:
            a._setup([0, STUB_SEG], {})
            start = (0x0002, 0x0242)[op]
            a.mu.reg_write(UC_X86_REG_CS, LOAD_SEG + 0x25FD)
            a.mu.reg_write(UC_X86_REG_IP, start)
            a.fault = None
            a.mu.emu_start(a.lin(LOAD_SEG + 0x25FD, start), a.lin(STUB_SEG, 0), count=2_000_000)
            if a.fault is not None:
                raise Fault(a.fault)
            cs, ip = reg(UC_X86_REG_CS), reg(UC_X86_REG_IP)
            if (cs, ip) == (LOAD_SEG + 0x25FD, stops[op]):
                end = 'slot'
            elif cs == STUB_SEG:
                end = 'back'
            else:
                raise AssertionError('did not return')
            after = a.seg_bytes(a.ds)
            log = b''.join(struct.pack(f'<{len(e)}H', *e) for e in state['log'])
            res = f'{end} {reg(UC_X86_REG_AX) & 0xFF:x} {fnv(changes(before, after)):x} {fnv(log):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'savemenu | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
