"""Emit test vectors for the options screen Esc brings up in play
(src/options.rs `options_screen`, 2542:000A with its handlers and the
drawing 0808/0848) from the original code.

Usage: gen_options.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/options.rs) the
volumes (D3B, D3A, around their ends), the detail (D37), the font and bank
words (102C, 1026, 1036, 1030), the music driver's words (EB93, EB00),
5C8, 5CA, A902, C398, FCE, and a script of clicks (Esc, a point in one of
the table's boxes, or anywhere; each with the CyberMan's button, rarely)
and of answers to "Are you sure ?". Every routine the screen calls out to
is a stub whose Python hook logs it with its registers and plays the
script: 0000:9AE6/9AF8 (fonts), 23F1:0145 (the automap), 5C86 (a bank),
7428 (OP.IMG), 5828, 9A6E and 5364 (drawing; both clear the carry as the
real ones do), 541E, 5454, 9466, 1194:564D (a click: AX, BX, the mouse's
8F92/8F94, 5C8 bit 2 cleared), 2F26:04E2 (the CyberMan's button: the
carry), 25FD:0002 and 0242 (the menus), 1194:48DF (the answer), 1194:4D91
(a deletion), 0FFD:0DE8 (the music's volume: its four arguments). 0000:7052
(the number written) is the original's. The script's end clicks Resume.
21EE:01ED (the program shut down) stops the case.

  options | seed | result

Results: "fault" / "hang", or the end ("resume" / "new" / "load" / "save"
/ "quit" / "none", the CyberMan's leaving), and the FNV-1a digests of the
changed DGROUP bytes and the stubs' log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_AX, UC_X86_REG_BX, UC_X86_REG_CX, UC_X86_REG_DX, UC_X86_REG_SI,
                               UC_X86_REG_SP, UC_X86_REG_SS, UC_X86_REG_CS, UC_X86_REG_IP, UC_X86_REG_EFLAGS)
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream

# The click table at 5C2C: (x0, y0, x1, y1).
BOXES = [(0, 0x76, 0x40, 0x92), (0x41, 0x76, 0x80, 0x92), (0x81, 0x76, 0xC0, 0x92), (0xC1, 0x76, 0x100, 0x92),
         (0x101, 0x76, 0x13F, 0x92), (0x2D, 0x4F, 0x3E, 0x57), (0x77, 0x4F, 0x87, 0x57), (0x2D, 0x68, 0x3E, 0x6F),
         (0x77, 0x68, 0x87, 0x6F), (0xCB, 0x5C, 0x12E, 0x63)]
# Box 0..4 are the bottom row; the handlers that leave are rarer.
PICKS = [0, 1, 2, 3, 3, 4, 5, 5, 5, 6, 6, 6, 7, 7, 7, 8, 8, 8, 9, 9, 9, 9]


def setup(seed):
    """(writes, clicks, answers). Mirrored in tests/options.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []

    def volume():
        k = byte() % 6
        return (0, 1, 0x63, 0x64, byte(), byte() % 0x65)[k]
    w.append((0x0D3B, bytes([volume()])))
    w.append((0x0D3A, bytes([volume()])))
    w.append((0x0D37, pk(word() if byte() < 0x20 else 6 + byte() % 0x14)))
    for at in (0x102C, 0x1026, 0x1036, 0x1030, 0xEB93, 0xEB00, 0x05C8, 0x05CA, 0xA902, 0xC398, 0x0FCE):
        w.append((at, pk(word())))
    clicks = []
    for _ in range(1 + byte() % 8):
        k = byte() % 8
        if k == 0:
            ax, bx = 0xFFFF, 0xFFFF
        elif k == 1:
            ax, bx = word() % 0x140, byte() % 0xC8
        else:
            x0, y0, x1, y1 = BOXES[PICKS[byte() % len(PICKS)]]
            ax = (x0, x1, x0 + byte() % (x1 - x0 + 1))[byte() % 3]
            bx = (y0, y1, y0 + byte() % (y1 - y0 + 1))[byte() % 3]
        cyber = byte() < 0x08
        clicks.append((ax, bx, cyber))
    answers = [(0, 1, 0xFFFF)[byte() % 3] for _ in range(3)]
    return w, clicks, answers


STUBS = [
    ('font0', 0, 0x9AE6), ('font1', 0, 0x9AF8), ('automap', 0x23F1, 0x0145), ('bank', 0, 0x5C86),
    ('load', 0, 0x7428), ('draw', 0, 0x5828), ('fill', 0, 0x9A6E), ('centred', 0, 0x5364),
    ('hide', 0, 0x541E), ('unhide', 0, 0x5454), ('show', 0, 0x9466), ('click', 0x1194, 0x564D),
    ('cyber', 0x2F26, 0x04E2), ('savemenu', 0x25FD, 0x0002), ('loadmenu', 0x25FD, 0x0242),
    ('confirm', 0x1194, 0x48DF), ('delete', 0x1194, 0x4D91), ('music', 0x0FFD, 0x0DE8), ('quit', 0x21EE, 0x01ED),
]


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for _, seg, off in STUBS:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
    state = {}
    reg = lambda r: a.mu.reg_read(r)

    def carry(mu, cf):
        fl = mu.reg_read(UC_X86_REG_EFLAGS) & ~1
        mu.reg_write(UC_X86_REG_EFLAGS, fl | (1 if cf else 0))

    def hook(mu, address, size, name):
        log, ds = state['log'], a.ds
        if name in ('font0', 'font1'):
            log.append((2 if name == 'font0' else 3, reg(UC_X86_REG_AX)))
        elif name == 'automap':
            log.append((15, 0))
        elif name == 'bank':
            log.append((4, reg(UC_X86_REG_BX)))
        elif name == 'load':
            log.append((6, reg(UC_X86_REG_SI)))
        elif name == 'fill':
            log.append((7, reg(UC_X86_REG_AX), reg(UC_X86_REG_BX), reg(UC_X86_REG_CX), reg(UC_X86_REG_DX)))
            carry(mu, False)
        elif name == 'centred':
            log.append((8, reg(UC_X86_REG_SI), reg(UC_X86_REG_AX), reg(UC_X86_REG_BX)))
            carry(mu, False)
        elif name == 'hide':
            log.append((17, 0))
        elif name == 'unhide':
            log.append((18, 0))
        elif name == 'show':
            log.append((9, 0))
        elif name == 'click':
            c = state['clicks']
            ax, bx, cyber = c.pop(0) if c else (0x120, 0x80, False)
            state['cyber'] = cyber
            log.append((10, ax, bx))
            mu.reg_write(UC_X86_REG_AX, ax)
            mu.reg_write(UC_X86_REG_BX, bx)
            a.w16(0x8F92, ax)
            a.w16(0x8F94, bx)
            a.w16(0x05C8, a.r16(0x05C8) & ~2 & 0xFFFF)
        elif name == 'cyber':
            if state['cyber']:
                state['end'] = 'none'
                mu.emu_stop()
            carry(mu, False)
        elif name == 'savemenu':
            log.append((20, 0))
        elif name == 'loadmenu':
            log.append((21, 0))
        elif name == 'confirm':
            c = state['answers']
            v = c.pop(0) if c else 1
            log.append((22, v))
            mu.reg_write(UC_X86_REG_AX, v)
        elif name == 'delete':
            log.append((13, reg(UC_X86_REG_SI)))
        elif name == 'music':
            sp, ss = reg(UC_X86_REG_SP), reg(UC_X86_REG_SS)
            log.append((23,) + struct.unpack('<4H', mu.mem_read(a.lin(ss, sp + 4), 8)))
        elif name == 'quit':
            state['end'] = 'quit'
            mu.emu_stop()

    for name, seg, off in STUBS:
        at = a.lin(LOAD_SEG + seg, off)
        a.mu.hook_add(UC_HOOK_CODE, hook, name, at, at)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0E5C)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, clicks, answers = setup(seed)
        for at, data in writes:
            a.mu.mem_write(a.lin(a.ds, at), data)
        state.update(log=[], clicks=list(clicks), answers=list(answers), end=None, cyber=False)
        before = a.seg_bytes(a.ds)
        try:
            a._setup([0xFFF0], {})
            a.mu.reg_write(UC_X86_REG_CS, LOAD_SEG + 0x2542)
            a.mu.reg_write(UC_X86_REG_IP, 0x000A)
            a.fault = None
            a.mu.emu_start(a.lin(LOAD_SEG + 0x2542, 0x000A), a.lin(LOAD_SEG + 0x2542, 0xFFF0), count=2_000_000)
            if a.fault is not None:
                raise Fault(a.fault)
            cs, ip = reg(UC_X86_REG_CS), reg(UC_X86_REG_IP)
            if state['end'] is not None:
                end = state['end']
            elif (cs, ip) == (LOAD_SEG + 0x2542, 0xFFF0):
                kinds = {e[0] for e in state['log']}
                end = 'load' if 21 in kinds else 'save' if 20 in kinds else \
                    'new' if any(e[0] == 13 for e in state['log']) else 'resume'
            else:
                raise AssertionError('did not return')
            after = a.seg_bytes(a.ds)
            log = b''.join(struct.pack(f'<{len(e)}H', *e) for e in state['log'])
            res = f'{end} {fnv(changes(before, after)):x} {fnv(log):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'options | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
