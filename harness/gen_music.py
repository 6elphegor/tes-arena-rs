"""Emit test vectors for the music's pickers (src/music.rs: `town_music`,
2294:04C0; `area_music`, 2294:04F5 with 10B6; `fm_song_name`, 0000:11DA).

Usage: gen_music.py ACD_unpacked.EXE OUT

The AIL calls are stubbed: 2F9F:0438 (stop) logs FFFFh, 2F9F:001F (start)
logs the name's offset (its first argument), both into a log of words at
DGROUP FFC0 (count in bytes at FFBE); 2F9F:000D (the sequence's status)
returns the byte at FFBC. The RNG (0000:6EA4) and 1194:559F run as they are.

A case fills from a seed (`setup`, mirrored in tests/music.rs): the op, the
map's outdoors bit (C398 bit 4), the level's water bit (C39A bit 10h), the
town's song flag (430F) and the status, the time of day (aimed at 10Ch/10Dh
and 2F3h/2F4h a quarter of the time), the weather, the area (5E6), the
building's song (FF0), the dungeon's seed byte against the one picked
(BB0A, 427D), the FM flag (1010), the driver (8F60), the song playing
(D56), the combat hold (430E), the town's size (A47) and the climate (DA8),
the RNG's seed (its table off), the hostile-near flag (101B); for 11DA a name at DGROUP F000 (with or
without ".xmi", other extensions, no dot).

  music | seed | op | result

Op: 0 `town_music`, 1 `area_music`, 2 `fm_song_name`. Results: "fault" /
"hang", or the FNV-1a digest of the changed DGROUP bytes below FFBC, the
log's words (comma-separated hex) and for op 2 the SI returned.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from unicorn.x86_const import UC_X86_REG_SI

# push bp; mov bp,sp; mov ax,[bp+6]; mov bx,[FFBE]; mov [bx+FFC0],ax;
# add word [FFBE],2; pop bp; retf
START = bytes.fromhex('5589e58b46068b1ebeff8987c0ff8306beff025dcb')
# mov bx,[FFBE]; mov word [bx+FFC0],FFFF; add word [FFBE],2; retf
STOP = bytes.fromhex('8b1ebeffc787c0ffffff8306beff02cb')
# mov al,[FFBC]; retf
STATUS = bytes.fromhex('a0bcffcb')

SONGS = [0x421C, 0x4226, 0x4233, 0x4240, 0x424C, 0x4259, 0x4265, 0x4272, 0x427E, 0x87B3, 0x87BE, 0x87C9,
         0x87D5, 0x87E2, 0x87ED, 0x87FA, 0x8805, 0xFFFF]
NAMES = [b'night.xmi', b'city.xmi', b'town.XMI', b'combat.xfm', b'a.xmi.b', b'noext', b'.xmi', b'x.xmid',
         b'dungeon1.xmi', b'snow.xm', b'a.b.xmi']


def setup(seed):
    """(writes, op). Mirrored in tests/music.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    op = byte() % 3
    w.append((0xC398, pk(word() & ~4 | (4 if byte() & 1 else 0))))
    w.append((0xC39A, pk(word() & ~0x10 | (0x10 if byte() < 0x30 else 0))))
    w.append((0x430F, pb(byte() & 1)))
    w.append((0xFFBC, pb((0, 1, 2, byte())[byte() % 4])))
    k = byte() % 4
    t = (word() & 0x3FF, 0x10B + byte() % 3, 0x2F3 + byte() % 2, word())[k]
    w.append((0xCCFE, pk(t)))
    w.append((0xBB09, pb((0, 1, 0x81, 2, 0x82, 3, byte())[byte() % 7])))
    w.append((0x05E6, pb((0, 0, 1, 3, 4, byte())[byte() % 6])))
    w.append((0x0FF0, pk(SONGS[byte() % len(SONGS)])))
    b = byte()
    w.append((0xBB0A, pb(b)))
    w.append((0x427D, pb(b if byte() < 0x60 else byte())))
    w.append((0x1010, pb(byte() & 1)))
    w.append((0x8F60, pk(0 if byte() < 0x30 else word())))
    w.append((0x0D56, pk(SONGS[byte() % len(SONGS)] if byte() < 0xC0 else word())))
    w.append((0x430E, pb(0 if byte() < 0xC0 else byte())))
    w.append((0x0A47, pb(byte() % 3 if byte() < 0xE0 else byte() % 6)))
    w.append((0x0DA8, pk((0, 4, 8, word())[byte() % 4])))
    w.append((0x91F0, pk(0)))
    w.append((0x91F6, struct.pack('<I', word() | word() << 16)))
    name = NAMES[byte() % len(NAMES)]
    w.append((0xF000, name + b'\0' + bytes([byte() for _ in range(4)])))
    w.append((0x101B, pb(byte())))
    return w, op


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0x2F9F, 0x001F, START), (0x2F9F, 0x0438, STOP), (0x2F9F, 0x000D, STATUS)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_04F5)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        writes, op = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        a.w16(0xFFBE, 0)
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                a.call_far(0x2294, 0x04C0)
            elif op == 1:
                a.call_far(0x2294, 0x04F5)
            else:
                a.call_far(0, 0x11DA, si=0xF000)
            after = a.seg_bytes(a.ds)
            cnt = struct.unpack_from('<H', after, 0xFFBE)[0]
            log = ','.join('%x' % struct.unpack_from('<H', after, 0xFFC0 + i)[0] for i in range(0, min(cnt, 0x30), 2))
            res = '%x %s' % (fnv(changes(before[:0xFFBC], after[:0xFFBC])), log or '-')
            if op == 2:
                res += ' %x' % a.mu.reg_read(UC_X86_REG_SI)
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'music | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
