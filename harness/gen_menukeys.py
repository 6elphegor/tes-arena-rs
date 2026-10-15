"""Emit test vectors for the menu keys of play (src/menukeys.rs
`menu_keys`, 2542:0100) from the original code.

Usage: gen_menukeys.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/menukeys.rs) the
state bytes of the eleven keys the check asks about and of both Shifts
(DS:ACB5 + scan; held now, at a density drawn per case), their latches
(9208 + scan), the words C39A, 5CA, 5E0, 1947, 5C8 and 61A..61E, the
byte 01AD, the CyberMan's buttons down (04E2's masks 20h, 10h, 08h, rarely)
and its DL answers (03ED: rarely bit 0 first, then some held), and the
carry of the touch cast. Every routine the check hands over to is a stub
that logs itself: 2542:000A (options), 056A (spellbook), 037C (status),
034D (use), 03BF (world map), 045A (automap), 02A3 (logbook), 01E6
(camp), 29CF:0000 (the touch cast: AX, SI and the drawn carry), 1194:0CF8
(the sheet) and the drawing around it (0000:9A98, 0000:A611, A6B5,
2433:0A39, unlogged), 2F26:04E2 (AL) and 03ED. 1194:5F13 before the sheet
runs, the planes it reads ([D806], [CCD0]) at the zeroed segments 7000h
and 8000h.

  menukeys | seed | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes (61C's DGROUP as the port's) and of the stubs' log.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_SI, UC_X86_REG_DX, UC_X86_REG_EFLAGS, UC_X86_REG_CS, UC_X86_REG_IP
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream

KEYS = [0x01, 0x1E, 0x2E, 0x19, 0x1F, 0x16, 0x32, 0x3B, 0x42, 0x26, 0x13]
SHIFTS = [0x2A, 0x36]


def setup(seed):
    """(writes, cyber masks, DL answers, cast carry). Mirrored in
    tests/menukeys.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    density = 1 + byte() % 0x40
    for scan in KEYS + SHIFTS:
        w.append((0xACB5 + scan, bytes([1 if byte() < density else 0])))
        w.append((0x9208 + scan, bytes([byte() & 1])))
    for at in (0xC39A, 0x05CA, 0x05E0, 0x1947, 0x05C8, 0x061A, 0x061C, 0x061E):
        w.append((at, pk(word())))
    w.append((0x01AD, bytes([byte() if byte() < 0xC0 else 0])))
    masks = byte() if byte() < 0x10 else 0
    dl = []
    if byte() < 0x10:
        dl = [1] * (1 + byte() % 3)
    dl.append(0)
    carry = byte() < 0x80
    w.append((0xD806, pk(0x7000)))
    w.append((0xCCD0, pk(0x8000)))
    return w, masks, dl, carry


# (name, segment, offset, far)
STUBS = [
    ('options', 0x2542, 0x000A, False), ('spellbook', 0x2542, 0x056A, False), ('status', 0x2542, 0x037C, False),
    ('use', 0x2542, 0x034D, False), ('worldmap', 0x2542, 0x03BF, False), ('automap', 0x2542, 0x045A, False),
    ('log', 0x2542, 0x02A3, False), ('camp', 0x2542, 0x01E6, False), ('cast', 0x29CF, 0x0000, True),
    ('sheet', 0x1194, 0x0CF8, True), ('fill', 0, 0x9A98, True),
    ('a611', 0, 0xA611, True), ('a6b5', 0, 0xA6B5, True), ('a39', 0x2433, 0x0A39, True),
    ('cyber', 0x2F26, 0x04E2, True), ('buttons', 0x2F26, 0x03ED, True),
]
CODES = {'options': 1, 'spellbook': 2, 'status': 3, 'use': 4, 'worldmap': 5, 'automap': 6, 'log': 7,
         'camp': 8, 'cast': 9, 'sheet': 10, 'cyber': 11, 'buttons': 12}


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for _, seg, off, far in STUBS:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb' if far else b'\xc3')
    state = {}
    reg = lambda r: a.mu.reg_read(r)

    def carry(mu, cf):
        fl = mu.reg_read(UC_X86_REG_EFLAGS) & ~1
        mu.reg_write(UC_X86_REG_EFLAGS, fl | (1 if cf else 0))

    def hook(mu, address, size, name):
        log = state['log']
        code = CODES.get(name)
        if name == 'cast':
            log.append((code, reg(UC_X86_REG_AX), reg(UC_X86_REG_SI), int(state['carry'])))
            carry(mu, state['carry'])
        elif name == 'cyber':
            al = reg(UC_X86_REG_AX) & 0xFF
            log.append((code, al))
            carry(mu, state['masks'] & al != 0)
        elif name == 'buttons':
            d = state['dl']
            v = d.pop(0) if d else 0
            log.append((code, v))
            mu.reg_write(UC_X86_REG_DX, v)
        elif code is not None:
            log.append((code, 0))

    for name, seg, off, _ in STUBS:
        at = a.lin(LOAD_SEG + seg, off)
        a.mu.hook_add(UC_HOOK_CODE, hook, name, at, at)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_0100)
    total = int(os.environ.get('N', 6000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, masks, dl, cf = setup(seed)
        for at, data in writes:
            a.mu.mem_write(a.lin(a.ds, at), data)
        state.update(log=[], masks=masks, dl=list(dl), carry=cf)
        before = a.seg_bytes(a.ds)
        try:
            a._setup([0xFFF0, LOAD_SEG + 0x2542], {})
            a.mu.reg_write(UC_X86_REG_CS, LOAD_SEG + 0x2542)
            a.mu.reg_write(UC_X86_REG_IP, 0x0100)
            a.fault = None
            a.mu.emu_start(a.lin(LOAD_SEG + 0x2542, 0x0100), a.lin(LOAD_SEG + 0x2542, 0xFFF0), count=200_000)
            if a.fault is not None:
                raise Fault(a.fault)
            if (reg(UC_X86_REG_CS), reg(UC_X86_REG_IP)) != (LOAD_SEG + 0x2542, 0xFFF0):
                raise AssertionError('did not return')
            after = bytearray(a.seg_bytes(a.ds))
            # The message line's segment (61C) as picking pockets stores
            # it, relocated in the emulator.
            if struct.unpack_from('<H', after, 0x061C)[0] == a.ds and before[0x061C:0x061E] != after[0x061C:0x061E]:
                after[0x061C:0x061E] = struct.pack('<H', DGROUP)
            log = b''.join(struct.pack(f'<{len(e)}H', *e) for e in state['log'])
            res = f'{fnv(changes(before, after)):x} {fnv(log):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'menukeys | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
