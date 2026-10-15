"""Emit test vectors for the sound played (src/sound.rs `sound_at`,
23BA:002A, with its volume 23BA:00E8).

Usage: gen_sound.py ACD_unpacked.EXE OUT

The sound driver (segment 0FFD) is stubbed: its status (0D52) returns
the case's from DGROUP FFE0, the volume call (0D70) keeps the volume
pushed at FFE4 and counts at FFE6, the play and its partners (0D46, 0D58,
0D5E) count at FFE8; the EMS bank map (0000:5C86) does nothing. Each
case starts from zeroed registers and flags.

A case fills from a seed (`setup`, mirrored in tests/sound.rs) sound on
(8F64), the footsteps' mutes (5E3, 5BE bit 80h), water (C39A bit 10h),
the volume percent (D3B, now and then 200..203: the divide's edge), the player's square
(CB90), the slots (C867, 40h bytes, FFh none), the priorities (46F4),
the slots' records (44F0, 8 bytes each: a handle, a far pointer into a
scratch segment, a length), the driver's status, AX (a sound number,
FFh, or any word) and CX (a square near the player's or any word).

  sound | seed | result

Results: "fault" (the volume's divide), or the FNV-1a digests of the
changed DGROUP bytes below FFE0 and of the stubs' records (FFE4..FFE9).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from emu import changes, fnv, mine
from gen_attack import stream

SAMPLE_SEG = 0x7000
BUFFER_SEG = 0x7800

STATUS = bytes([0xA1, 0xE0, 0xFF, 0xCB])                       # mov ax,[FFE0]; retf
VOLUME = bytes([0x55, 0x89, 0xE5, 0x8B, 0x46, 0x08, 0xA3, 0xE4, 0xFF,
                0xFF, 0x06, 0xE6, 0xFF, 0x5D, 0xCB])           # [FFE4] = [bp+8]; inc [FFE6]
COUNT = bytes([0xFF, 0x06, 0xE8, 0xFF, 0xCB])                  # inc [FFE8]; retf


def setup(seed):
    """(writes, ax, cx). Mirrored in tests/sound.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0x8F64, pk(0 if byte() < 0x20 else word() | 1)))
    w.append((0x05E3, bytes([0 if byte() < 0xC0 else byte()])))
    w.append((0x05BE, pk(word())))
    w.append((0xC39A, pk(word())))
    # Past 100 aimed at the divide's edge: 7Fh x 202 / 100 is 100h.
    w.append((0x0D3B, bytes([byte() % 101 if byte() < 0xE8 else 200 + byte() % 4])))
    me = word()
    w.append((0xCB90, pk(me)))
    w.append((0xC867, bytes(0xFF if byte() < 0x30 else byte() % 0x20 for _ in range(0x40))))
    w.append((0x46F4, bytes(byte() for _ in range(0x40))))
    recs = b''
    for _ in range(0x20):
        recs += struct.pack('<HHHH', word() & 0x7FFF, byte() & 0xF0, SAMPLE_SEG, byte() % 0x40)
    w.append((0x44F0, recs))
    w.append((0xEB89, struct.pack('<HH', 0, BUFFER_SEG)))
    w.append((0xFFE0, pk(byte() % 3)))
    k = byte() % 4
    ax = (byte() % 0x30, 0xFF, word(), byte() & 1)[k]
    k = byte() % 3
    near = (me & 0xFF00 | (me + (byte() % 9) * 2 - 8) & 0xFF) + ((byte() % 9) - 4 << 8)
    cx = (me, near, word())[k]
    return w, ax & 0xFFFF, cx & 0xFFFF


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for off, code in ((0x0D52, STATUS), (0x0D70, VOLUME), (0x0D46, COUNT), (0x0D58, COUNT), (0x0D5E, COUNT)):
        a.mu.mem_write(a.lin(0x1000 + 0x0FFD, off), code)
    a.mu.mem_write(a.lin(0x1000, 0x5C86), b'\xcb')
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_002A)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, ax, cx = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x23BA, 0x002A, eax=ax, ecx=cx, ebx=0, edx=0, esi=0, edi=0, ebp=0, eflags=2)
            if a.fault is not None:
                raise Fault(a.fault)
            after = a.seg_bytes(a.ds)
            res = f'{fnv(changes(before[:0xFFE0], after[:0xFFE0])):x} {fnv(after[0xFFE4:0xFFEA]):x}'
        except (Fault, AssertionError):
            res = 'fault'
        out.write(f'sound | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
