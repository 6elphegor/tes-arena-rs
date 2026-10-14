"""Emit test vectors for a door stepping as the renderer meets it
(src/click.rs `door_drawn`, 0000:CD66 up to the shared tail at CF20).

Usage: gen_door.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/door.rs) the
player's position (C3D0, C3D4) a few squares from the door's, the door's
word (kind 0Bh, any state and low byte), the renderer's variables the
step sets, C398, the sound tables (C3BC, C3C8) and the door hook (CD46):
none, or a stub (by either of two far pointers) that keeps the AX it is called with.

  door | seed | sound | result

Sound: the AX the hook got ("-" none). Results: "fault" / "hang", or the
FNV-1a digest of the changed DGROUP bytes and the door's word.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import changes
from emu import mine
from gen_attack import fnv, stream

WSEG = 0x6800
STUB = 0x7000


def setup(seed):
    """(writes, door offset). Mirrored in tests/door.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    col, row = byte() & 0x7F, byte()
    sq = (row << 8) | (col << 1)
    # The player within a few squares, anywhere in them.
    px = ((col + (byte() % 7) - 3) << 7 | byte() & 0x7F) & 0xFFFF
    py = ((row + (byte() % 7) - 3) << 7 | byte() & 0x7F) & 0xFFFF
    w.append((None, 0xC3D0, pk(px)))
    w.append((None, 0xC3D4, pk(py)))
    w.append((None, 0xC398, pk(word())))
    for at in (0xCCC4, 0xCB37, 0xCB38, 0xBA7F, 0xC3BC, 0xC3BE, 0xC3C8, 0xC3CA):
        w.append((None, at, pk(word())))
    # The hook: none, or the stub by two far pointers (the original
    # tests the segment only).
    off, seg = ((0, 0), (0x10, STUB), (0, STUB + 1))[byte() % 3]
    w.append((None, 0xCD46, pk(off)))
    w.append((None, 0xCD48, pk(seg)))
    w.append((WSEG, sq, pk(0xB000 | word() & 0x0FFF)))
    return w, sq


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.w16(0xD806, WSEG)
    # push ds; push STUB; pop ds; mov [0], ax; mov byte [2], 1; pop ds;
    # retf
    a.mu.mem_write(a.lin(STUB, 0x10), b'\x1e\x68\x00\x70\x1f\xa3\x00\x00\xc6\x06\x02\x00\x01\x1f\xcb')
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_CD66)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(STUB, 0), bytes(4))
        writes, sq = setup(seed)
        for seg, off, data in writes:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = a.seg_bytes(a.ds)
        sound = '-'
        try:
            a.run_range(0, 0xCD66, 0xCF20, es=WSEG, di=sq)
            after = a.seg_bytes(a.ds)
            changed = changes(before, after)
            res = f'{fnv(changed + struct.pack("<H", a.r16(sq, seg=WSEG))):x}'
            if a.mu.mem_read(a.lin(STUB, 2), 1)[0]:
                sound = f'{a.r16(0, seg=STUB):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'door | {seed:x} | {sound} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
