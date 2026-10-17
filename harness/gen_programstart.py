"""Emit the vectors for the program's start (src/start.rs: `memory_init`,
`program_start`, 21EE:0000) and the game's set-up after it (21EE:016C,
`game_setup`), run as harness/screens.py's `start` runs them.

Usage: gen_programstart.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The memory as screens.py's `init_memory` leaves it, then 21EE:0000, the
texts' segment the frame's (main's 0000:108B), the working slot 40h,
then 21EE:016C. After each stage:

  heap | STAGE | <segment size flags> ...
  dgroup | STAGE | <start+length ...> | <digest>

  ems | STAGE | <digest>

the heap's blocks from the first header, the EMS banks 0..1Fh (end to
end), and what the stage changed in
DGROUP from the memory's start (the offsets as runs, the digest of their
bytes after). The image is relocated 1000h higher here than the port's
unrelocated image: the words SEGMENTS names (each an image segment the
start stores) are taken 1000h lower before the digest, as the port has
them. The heap lies where the port's does (`memory_init`).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv
from screens import ScreenArena

# DGROUP words holding a segment of the image, by stage (the harness's
# 1000h higher than the port's).
SEGMENTS = {
    'start': {
        0x8F72: "the timer's hook's segment (21EE:028A)",
        0xA90A: "the open file's name's segment (DGROUP)",
    },
    'setup': {
        0xCD74: "the sky's hook's segment (2294:0CDE)",
    },
}


def heap_line(a, stage):
    off, seg = a.r16(0x157), a.r16(0x159)
    out = []
    while (seg or off) and len(out) < 500:
        _, nxt, _, prev, size, used = struct.unpack('<HHHHHB', bytes(a.mu.mem_read(seg * 16 + off, 11)))
        out.append(f'{seg:x} {size:x} {used:x}')
        seg, off = nxt, 0
    return f'heap | {stage} | {" ".join(out)}\n'


def ems_line(a, stage):
    return f'ems | {stage} | {fnv(b"".join(a.ems_bank(b) for b in range(0x20))):x}\n'


def dgroup_line(d0, d1, stage):
    d1 = bytearray(d1)
    segs = {}
    for s in SEGMENTS:
        segs.update(SEGMENTS[s])
        if s == stage:
            break
    for at in segs:
        w = struct.unpack_from('<H', d1, at)[0]
        struct.pack_into('<H', d1, at, (w - 0x1000) & 0xFFFF)
    changed = [i for i in range(0x10000) if d0[i] != d1[i]]
    runs = []
    for i in changed:
        if runs and runs[-1][0] + runs[-1][1] == i:
            runs[-1][1] += 1
        else:
            runs.append([i, 1])
    offs = ' '.join(f'{s:x}+{n:x}' for s, n in runs)
    return f'dgroup | {stage} | {offs} | {fnv(bytes(d1[i] for i in changed)):x}\n'


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    a = ScreenArena(EXE, GAME, SAVE)
    a.init_memory()
    d0 = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    out = open(OUT, 'w')
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    out.write(heap_line(a, 'start'))
    out.write(ems_line(a, 'start'))
    out.write(dgroup_line(d0, bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000)), 'start'))
    a.w16(0xCB8A, a.r16(0x9030))
    saved = a.stub()
    a.mu.mem_write(a.lin(a.ds, 0x1A80), b'\x40')
    a.mu.mem_write(a.lin(a.ds, 0x1A6C), b'\x40')
    assert a.run_screen(0x21EE, 0x016C) is None, 'the setup waited'
    a.unstub(saved)
    out.write(heap_line(a, 'setup'))
    out.write(ems_line(a, 'setup'))
    out.write(dgroup_line(d0, bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000)), 'setup'))
    out.close()


if __name__ == '__main__':
    main()
