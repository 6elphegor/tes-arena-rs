"""Emit the vectors for a worn item on the character sheet's doll
(1194:1F8D, `Machine::sheet_item`): its frame drawn, or coloured
through bank 4 (20BF, `item_coloured`), and a helmet by the race (2020,
`sheet_helmet`: the head's colours for races 1, 3 and 4, mliz?.img for
race 7).

Usage: gen_sheetitem.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The game started and set up as gen_fileloadems.py has it, then for each
case (`cases`, mirrored in tests/sheetitem.rs) from that state: the
player's race (1A8) and sex (1AB) and the item at the first slot
(0212: type, +0Eh flags, +10h colour) written, the clothes' names made
(1194:1D22), the equipment's frames loaded into bank 17h (2462) and
1F8D near-called with SI 0212 and the case's DI. One line a case:

  item | NAME | <runs> | <digest> | <mapped> <ems> <page>

what the case changed in DGROUP (the open file's name's segment, A90A,
taken 1000h lower as the port's), the bank left mapped (8FA8), the 32
EMS banks' digest and the page's ([A900], FA00h bytes).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv
from screens import ScreenArena
from gen_msgbox import runs


def cases():
    """(name, race, sex, type, flags, colour, DI). Mirrored in
    tests/sheetitem.rs."""
    out = []
    # Weapons (kind 1: flags 04h) and armour (kind 0) of each kind, plain
    # and coloured (flags 1) in each of the colours.
    for t in (0, 3, 7, 11, 15):
        out.append((f'weapon{t}', 0, 0, t, 0xC4, 0))
        for c in (0, 1, 2, 3, 5, 7):
            out.append((f'weapon{t}c{c}', 0, 0, t, 0xC5, c))
    # (Type 8 of kind 2, frame 2Ch, is past the file's frames: its header
    # is whatever follows, and 5B04 draws it on for ever.)
    for t in (0, 2, 4, 6, 8):
        for k in (0, 1, 2) if t != 8 else (0, 1):
            out.append((f'armour{t}k{k}', 0, 1, t, 0xC0 | k << 4, 0))
            out.append((f'armour{t}k{k}c2', 0, 1, t, 0xC1 | k << 4, 2))
    # A helmet (type 5, kind 0) for each race and sex, and race 7's by
    # its armour's kind; a coloured one draws no head.
    for race in range(8):
        for sex in (0, 1):
            out.append((f'helmet{race}s{sex}', race, sex, 5, 0xC0, 0))
    for k in (1, 2):
        out.append((f'helmet7k{k}', 7, 0, 5, 0xC0 | k << 4, 0))
    out.append(('helmet7c4', 7, 0, 5, 0xC1, 4))
    # An accessory (kind 2: no frame) and special items (type bit 80h).
    out.append(('ring', 0, 0, 3, 0xC8, 0))
    out.append(('special2', 0, 0, 0x82, 0xC4, 0))
    out.append(('special5', 3, 0, 0x85, 0xC0, 0))
    out = [c + (0,) for c in out]
    # Race 7's helmet loads its picture at DGROUP:DI, DI as 1F8D finds
    # it: after races 1, 3 and 4's helmet 929Eh.
    out.append(('helmet7di', 7, 1, 5, 0xC0, 0, 0x929E))
    return out


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    a = ScreenArena(EXE, GAME, SAVE)
    a.init_memory()
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    a.w16(0xCB8A, a.r16(0x9030))
    saved = a.stub()
    a.mu.mem_write(a.lin(a.ds, 0x1A80), b'\x40')
    a.mu.mem_write(a.lin(a.ds, 0x1A6C), b'\x40')
    assert a.run_screen(0x21EE, 0x016C) is None, 'the setup waited'
    a.unstub(saved)
    snap = a.snapshot()
    out = open(OUT, 'w')
    for name, race, sex, ty, flags, colour, di in cases():
        a.restore(snap)
        d0 = bytearray(a.seg_bytes(a.ds))
        a.mu.mem_write(a.lin(a.ds, 0x1A8), bytes([race]))
        a.mu.mem_write(a.lin(a.ds, 0x1AB), bytes([sex]))
        a.mu.mem_write(a.lin(a.ds, 0x212), bytes([ty]))
        a.mu.mem_write(a.lin(a.ds, 0x212 + 0x0E), bytes([flags]))
        a.mu.mem_write(a.lin(a.ds, 0x212 + 0x10), bytes([colour]))
        for seg, off, regs in ((0x1194, 0x1D22, {}), (0x1194, 0x2462, {}), (0x1194, 0x1F8D, dict(si=0x212, di=di))):
            assert a.run_screen(seg, off, regs, near=True) is None, f'{name}: {off:04X} waited'
        d1 = bytearray(a.seg_bytes(a.ds))
        for d in (d0, d1):
            if struct.unpack_from('<H', d, 0xA90A)[0] == a.ds:
                struct.pack_into('<H', d, 0xA90A, a.ds - 0x1000)
        offs, digest = runs(d0, d1)
        ems = fnv(b''.join(a.ems_bank(b) for b in range(0x20)))
        page = fnv(bytes(a.mu.mem_read(a.lin(a.r16(0xA900), 0), 0xFA00)))
        out.write(f'item | {name} | {offs} | {digest} | {a.r16(0x8FA8):x} {ems:x} {page:x}\n')
        out.flush()
    out.close()


if __name__ == '__main__':
    main()
