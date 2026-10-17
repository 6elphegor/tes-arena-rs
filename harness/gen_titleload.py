"""Emit test vectors for a saved game loaded from the title screen
(src/load.rs `title_load`: 25FD:0309..032D, then the world loop entered
afresh, 0000:A232..A2AB) from the original code.

Usage: gen_titleload.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

SAVE_DIR holds a saved game in slot 1 (the user's save); its SAVEGAME.01
and SAVEENGN.01 are loaded, and slot 65's files come from GAME_DIR, as
for gen_load.py. Vectors hold digests only.

A case fills from a seed (`setup`, mirrored in tests/titleload.rs) the
state before the load: 5C8 (bit 4 set: the title screen's way), 5E4,
5E0, C398, C39A, the light table's segment (CCC2: none, or SHADES with
a pattern), CB5C, the province (A46), the location (A4C), the working
slot (1A80), FF9, 6636, the walking speed (C390), the weather (BB09),
the reach (D37), D39, the first creature record and the others' +40Dh,
the creature slots' flags and the effects' slots. EMS is on, sound off
(8F64 0: the INF's sound loader 23BA:0217 returns at once), a missing
texture read from handle 2. The slot is 1 (25FD:028D run first), the
screen not read (C995), the entry and place hooks (C3B0, CD2A) unset
and the entered hook (CD42) 0000:468B. Then it runs from 25FD:0309
through the SAVEGAME load, its hook 2294:0006 (with the rebuild
25FD:034B), its tail (the level's INF), and the world loop's entry up
to 0000:A2AC, where the frames begin.

Counted in DGROUP (stubs): as gen_load.py's (0000:1AF9 FFE0, 1A5C FFE2,
23BA:01C7 FFE4, 0000:E5C9 FFE6, 0000:AAA2 FFEA, 25FD:056F FFEC,
1194:3AE8 FFEE, 292B:09AC FFF0, 2F9F:001F FFF2, 0000:3503 FFF4), and
0000:DA8E (FFF6, the view update), 25CA:01E6 (FFF8, the creature slots marked usable,
returning 0), 0000:17F9 (FFFA), 0000:068A (FFFC) and 0000:468B (FFFE,
the entered hook). Not counted, drawing only: 0000:DFDB, A561, 946A,
9B56.

  title | seed | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes (relocated segments put back), the creature records (8 x 41Eh),
the spells (3276:0000..3520), the 80h bytes at the far pointer 1020,
the walls' and floors' planes, and the 20h EMS banks
(dos.py `enable_ems`, the frame at the buffer segment 9030) the INF's
textures and flats are read to.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from gen_frame import counter

CSEG = 0x7000
BUFFER = 0x7400
SHADES = 0x6000  # a light table (CCC2) with gen_info.py's pattern
EXTRA = (0x8800, 0x0010)
PLAYER = 0x01A4
SLOTS, SLOT_SIZE = 0x0660, 0x3C
# Far pointers that may hold a code segment, written relocated.
HOOKS = (0xC3A4, 0xC3A8, 0xC3AC, 0xC3B0, 0xC3B4, 0xC3B8, 0xCD1A, 0xCD22, 0xCD26, 0xCD2A, 0xCD2E,
         0xCD32, 0xCD36, 0xCD3A, 0xCD42, 0xCD46, 0xCD4A, 0xCD4E, 0xCD52, 0xCD56, 0xCD5A, 0xCD62,
         0xCD66, 0xCD6A, 0xCD6E, 0xBA76, 0xBA7A)


def unrelocate(buf, ds):
    """The hooks' code segments in a DGROUP copy put back."""
    for at in HOOKS:
        sg = struct.unpack_from('<H', buf, at + 2)[0]
        if LOAD_SEG <= sg < ds:
            struct.pack_into('<H', buf, at + 2, sg - LOAD_SEG)


def setup(seed):
    """Writes (segment None: DGROUP). Mirrored in tests/titleload.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    w.append((None, 0x05C8, pk(word() | 4)))
    w.append((None, 0x05E4, pb(byte())))
    w.append((None, 0x05E0, pk(word())))
    w.append((None, 0xC398, pk(word() & 0x7FFB | (4 if byte() < 0x80 else 0) | (0x8000 if byte() < 0x40 else 0))))
    w.append((None, 0xC39A, pk(word() & ~0x200 | (0x200 if byte() < 0x40 else 0))))
    w.append((None, 0xCCC2, pk(0 if byte() < 0x80 else SHADES)))
    w.append((None, 0xCB5C, pb(byte())))
    w.append((None, 0x0A46, pb(byte() % 9 if byte() < 0xE0 else byte())))
    w.append((None, 0x0A4C, pb(byte())))
    w.append((None, 0x1A80, pb((65, byte())[byte() & 1])))
    w.append((None, 0x0FF9, pk(word())))
    w.append((None, 0x6636, pb(byte())))
    w.append((None, 0xC390, pk(word())))
    w.append((None, 0xBB09, pb((0, 1, 2, 0x81, 0x82)[byte() % 5])))
    w.append((None, 0x0D37, pk(word())))
    w.append((None, 0x0D39, pb(byte())))
    rec = bytearray(byte() for _ in range(0x41E))
    rec[4] = 0x2A if byte() < 0x20 else 1 + byte() % 0x29
    w.append((CSEG, 0, bytes(rec)))
    for k in range(1, 8):
        w.append((CSEG, 0x41E * k + 0x40D, pk(word())))
    for i in range(8):
        w.append((None, 0x0C57 + 0x1C * i + 0xF, pk(word() & 0x3FFF | (0x4000 if byte() < 0xA0 else 0))))
    for i in range(16):
        sl = SLOTS + SLOT_SIZE * i
        w.append((None, sl + 0x3B, pb(0xFF if byte() < 0x80 else byte())))
        w.append((None, sl + 0x28, pk(PLAYER if byte() < 0x80 else word())))
    return w


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME, SAVE)
    a.std_devices = True  # a missing texture is read from handle 2 (see src/info.rs)
    slots = b'\xff\x06\xf8\xff\x31\xc0\xcb'  # inc word [FFF8]; xor ax, ax; retf
    for seg, off, code in [(0, 0x1AF9, counter(0xFFE0, True)), (0, 0x1A5C, counter(0xFFE2, True)),
                           (0x23BA, 0x01C7, counter(0xFFE4, True)), (0, 0xE5C9, counter(0xFFE6, True)),
                           (0, 0xAAA2, counter(0xFFEA, True)), (0x25FD, 0x056F, counter(0xFFEC, True)),
                           (0x1194, 0x3AE8, counter(0xFFEE, True)), (0x292B, 0x09AC, counter(0xFFF0, True)),
                           (0x2F9F, 0x001F, counter(0xFFF2, True)), (0, 0x3503, counter(0xFFF4, True)),
                           (0, 0xDA8E, counter(0xFFF6, False)), (0x25CA, 0x01E6, slots),
                           (0, 0x17F9, counter(0xFFFA, True)), (0, 0x068A, counter(0xFFFC, True)),
                           (0, 0x468B, counter(0xFFFE, True)),
                           (0, 0xDFDB, b'\xc3'), (0, 0xA561, b'\xc3'),
                           (0, 0x946A, b'\xc3'), (0, 0x9B56, b'\xc3')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.run_range(0x25FD, 0x028D, 0x02A0, eax=1)  # slot 1's names
    a.w16(0x1042, CSEG)
    a.enable_ems(BUFFER)  # the frame is the buffer segment (9030)
    a.mu.mem_write(SHADES * 16, bytes((i * 7 + (i >> 8)) & 0xFF for i in range(0x1000)))
    a.w16(0x1020, EXTRA[1])
    a.w16(0x1022, EXTRA[0])
    a.mu.mem_write(a.lin(a.ds, 0xC995), b'\x01')
    for at in (0xC3B0, 0xCD2A):
        a.w16(at, 0)
        a.w16(at + 2, 0)
    a.w16(0xCD42, 0x468B)
    a.w16(0xCD44, LOAD_SEG)
    base = a.snapshot()
    zero = bytes(0x41E * 8)
    rng = random.Random(0x1994_0309)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(base)
        a.mu.mem_write(a.lin(CSEG, 0), zero)
        a.mu.mem_write(a.lin(EXTRA[0], EXTRA[1]), bytes(0x80))
        for seg, off, data in setup(seed):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        before = bytearray(a.seg_bytes(a.ds))
        unrelocate(before, a.ds)
        try:
            a._setup([], {})
            a._run(0x25FD, 0x0309, LOAD_SEG, 0xA2AC)
            for i in range(16):
                at = SLOTS + SLOT_SIZE * i + 0x2A
                if a.r16(at) == a.ds:
                    a.w16(at, a.ds - LOAD_SEG)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = bytearray(a.seg_bytes(a.ds))
            unrelocate(after, a.ds)
            parts = [changes(before, after), a.mu.mem_read(a.lin(CSEG, 0), 0x41E * 8),
                     a.mu.mem_read(a.lin(LOAD_SEG + 0x3276, 0), 0x3520),
                     a.mu.mem_read(a.lin(EXTRA[0], EXTRA[1]), 0x80)]
            for var in (0xD806, 0xCCD0):
                sg = a.r16(var)
                parts.append(a.mu.mem_read(a.lin(sg, 0), 0x8000) if sg else b'')
            parts.append(b''.join(a.ems_bank(b) for b in range(0x20)))
            res = ' '.join(f'{fnv(p):x}' for p in parts)
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'title | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
