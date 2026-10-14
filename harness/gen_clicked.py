"""Emit test vectors for the hit test's result made ready (src/talk.rs
`clicked_flat`, 2294:047B).

Usage: gen_clicked.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/clicked.rs) what
was clicked (CB5E: the scratch record D7DE, an actor slot, the slots'
edges 0AB2h/0AB3h/0D36h/0D37h, RECORD, 0, or any word) and the scratch
record's 1Ch bytes.

  clicked | seed | result

Results: "fault" / "hang", or the FNV-1a digest of the changed DGROUP
bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault
from gen_attack import fnv, stream


def setup(seed):
    """Writes. Mirrored in tests/clicked.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    k = byte() % 6
    v = (0xD7DE, 0x0AB3 + 0x1C * (byte() % 23), (0x0AB2, 0x0AB3, 0x0D36, 0x0D37)[byte() % 4], 0x3D96, 0, word())[k]
    w = [(0xCB5E, struct.pack('<H', v))]
    w.append((0xD7DE, bytes(byte() for _ in range(0x1C))))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_047B)
    for n in range(int(os.environ.get('N', 2000))):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            a.call_far(0x2294, 0x047B)
            after = a.seg_bytes(a.ds)
            res = f'{fnv(b"".join(struct.pack("<HB", i, after[i]) for i in range(0x10000) if after[i] != before[i])):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'clicked | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
