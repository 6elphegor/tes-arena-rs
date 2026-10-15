"""Emit test vectors for the world loop's frame (src/engine.rs
`Engine::frame`, the body of 0000:A240's loop, A2AC..A453) from the
original code: its order and its own state.

Usage: gen_world.py ACD_unpacked.EXE OUT

The frame's big steps are stubbed, each logging its call (`STUBS`, and
the hooks through 0000:A8EF pointed at far stubs in segment 5000h): the
log at DGROUP FE00 (three bytes a call: the step's number and a word,
the eye's x C3D0 by default, else SI, BX or the argument on the stack;
the count of bytes at FDFE; the view's stub logs its reach C394). The
physics' stub moves x on one and the frame body's the window's bottom
C3A2, so that the position kept after them and the second window show. The rest runs: the window (A5D1), the clip
put back (A606), the eye moved back (A919), the position kept around
the physics (BA62), the reach doubled below the floor, the caches
emptied (C019), the stage marks (BA4B). The tests stub the same steps
(`EngineHooks::step`).

A case fills from a seed (`setup`, mirrored in tests/world.rs) the
position, height and heading, the kept position, the reach, the area
and level flags, fog, dry ground and the weather, the level's bank, a
waiting INF, the window, the clip and its copy, the timers and the
counters, the draw lists' counts and some of the caches, and each hook
set or not. It runs A2AC to the timer's wait (A428), then from after it
(A431) to the loop's test (A453); a frame whose area is left (C398 bit
80h before it, which the stubs keep) runs to A45D.

  world | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digest of the
changed DGROUP bytes.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream

HOOK_SEG = 0x5000
LOG, COUNT = 0xFE00, 0xFDFE

# (offset in segment 0000, step number, what is logged).
STUBS = [
    (0xFCF6, 2, 'x'),     # the walk's bob
    (0xE12E, 3, 'x'),     # the lights near the eye
    (0xDFDB, 4, 'si'),    # the textures put back
    (0x5C8A, 5, 'bx'),    # the level's bank mapped
    (0xE106, 6, 'x'),     # the moving lights
    (0xD4B4, 7, 'arg'),   # the drawing's set-up
    (0xC923, 8, 'reach'), # the view drawn (its reach logged)
    (0xB4F5, 9, 'x'),     # the draw lists
    (0xB358, 10, 'x'),    # the flats drawn
    (0xE26B, 11, 'x'),    # the snow
    (0xE1A2, 12, 'x'),    # the rain
    (0xF57C, 13, 'x'),    # the physics
    (0xD476, 14, 'x'),    # the changed squares
    (0xEC03, 15, 'x'),    # a level's INF
    (0xA7B4, 16, 'x'),    # the picture shown
]
# The hooks through A8EF: (DGROUP offset, step number, stub offset in
# HOOK_SEG).
HOOKS = [(0xC3A4, 1, 0x000), (0xCD16, 17, 0x040), (0xCD66, 18, 0x080), (0xCD1A, 19, 0x0C0), (0xC3A8, 20, 0x100)]


def stub(number, what, far, moves=None):
    """push bx; push ax; AX = the value; log [FDFE] += 3 bytes; pop; ret.
    With `moves` that word in DGROUP goes on one too: the physics' stand-in
    moves the eye's x (C3D0), so that keeping the position after them
    shows; the frame body's the window's bottom (C3A2), so that the second
    window (A5D1 at A3F9) shows."""
    load = {
        'x': b'\xA1\xD0\xC3',                          # mov ax, [C3D0]
        'si': b'\x89\xF0',                             # mov ax, si
        'bx': b'\x89\xD8',                             # mov ax, bx
        'arg': b'\x55\x89\xE5\x8B\x46\x08\x5D',        # push bp; mov bp, sp; mov ax, [bp+8]; pop bp
        'reach': b'\xA1\x94\xC3',                      # mov ax, [C394]
    }[what]
    return (b'\x53\x50' + load
            + b'\x8B\x1E' + struct.pack('<H', COUNT)                       # mov bx, [COUNT]
            + b'\xC6\x87' + struct.pack('<H', LOG) + bytes([number])       # mov byte [bx+LOG], n
            + b'\x89\x87' + struct.pack('<H', LOG + 1)                     # mov [bx+LOG+1], ax
            + b'\x83\x06' + struct.pack('<H', COUNT) + b'\x03'             # add word [COUNT], 3
            + (b'\xFF\x06' + struct.pack('<H', moves) if moves else b'')   # inc word [moves]
            + b'\x58\x5B' + (b'\xCB' if far else b'\xC3'))


def setup(seed):
    """Writes (DGROUP offset, bytes), in order. Mirrored in tests/world.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    for at in (0xC3D0, 0xC3D4, 0xBA62, 0xBA64, 0xBA66):
        w.append((at, pk(word())))
    w.append((0xC3D2, pk(word() if byte() < 0x40 else 0x3C)))
    w.append((0xC3DA, pk(word() & 0x1FF if byte() < 0xE0 else word())))
    w.append((0xC394, pk(byte() % 8 if byte() < 0xE0 else word())))
    flags = word() & ~0x80 | (0x80 if byte() < 0x20 else 0)
    w.append((0xC398, pk(flags)))
    w.append((0xC39A, pk(word())))
    w.append((0xCB5C, bytes([0 if byte() < 0xA0 else byte()])))
    w.append((0xCB5B, bytes([1 if byte() < 0x80 else byte()])))
    w.append((0xBB09, bytes([(0, 0x81, 0x82, byte())[byte() % 4]])))
    w.append((0xC8F0, bytes([byte()])))
    w.append((0xCB84, pk(0 if byte() < 0x80 else word())))
    for at in range(0xBA58, 0xBA60, 2):
        w.append((at, pk(word())))
    for at in range(0xA914, 0xA91C, 2):
        w.append((at, pk(word())))
    for at in range(0xC39C, 0xC3A4, 2):
        w.append((at, pk(word())))
    for at in (0xCB86, 0x0151, 0xBA60, 0xCCBC, 0xBAC8, 0xCD7A, 0xCB5E, 0xC3DE, 0xC3E0):
        w.append((at, pk(word())))
    w.append((0xBA4B, bytes([byte()])))
    for _ in range(6):
        k = byte()
        w.append((0xD330 + 2 * (k & 0x7F), pk(word())))
        k = byte()
        w.append((0xD440 + 6 * (k & 0x7F), pk(word())))
    for at, _, off in HOOKS:
        w.append((at, struct.pack('<HH', off, HOOK_SEG) if byte() < 0xC0 else bytes(4)))
    w.append((COUNT, pk(0)))
    return w


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for off, n, what in STUBS:
        a.mu.mem_write(a.lin(LOAD_SEG, off), stub(n, what, False, moves=0xC3D0 if off == 0xF57C else None))
    for _, n, off in HOOKS:
        a.mu.mem_write(a.lin(HOOK_SEG, off), stub(n, 'x', True, moves=0xC3A2 if n == 20 else None))
    base = bytes(a.mu.mem_read(0, 0x110000))
    rng = random.Random(0x1994_A2AC)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(0, base)
        writes = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        left = a.r16(0xC398) & 0x80 != 0
        try:
            if left:
                a.run_range(0, 0xA2AC, 0xA45D)
            else:
                a.run_range(0, 0xA2AC, 0xA428)
                a.run_range(0, 0xA431, 0xA453)
            res = f'{fnv(changes(before, a.seg_bytes(a.ds))):x}'
        except (Fault, AssertionError):
            res = 'stuck'
        out.write(f'world | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
