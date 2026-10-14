"""Emit test vectors for resting (src/rest.rs): the regeneration
(2901:016C), the camp check (2433:12F5), the rest's start (1194:424E) and
a frame of it (1194:4635).

Usage: gen_rest.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/rest.rs) the player
(health, fatigue, spell points, class, level, attributes and their bases,
the heal modifier), the rest's state (1013, 1014, 1016, 1018, 1019,
49E6, 13C8), enemies (FF9, FFB), a key (AFF9), where the player is (C398,
C39A, 5E6, D48, 191D), the room (12C4, 12CA, the clock 5CC), the dream
(F76, F77, F7A), the actor slots' flags, and the scripts the stubs read:
up to 8 menu picks (words from FF00) and 8 typed texts (4 bytes each from
FF20); then runs one of the four.

Stubbed (the port's `RestHooks`):
- 0000:142D, the clock: the minutes (EAX) added to the dword at FFE0;
- 1194:020E, a menu: the next word of the picks (FFF8 points at it);
- 1194:4800, the hours typed: the next 4 bytes of texts to 4980 (FFFA);
- 1194:59CC, a message: its text at FFF6, a count at FFF4;
- 1194:D45D, the dream: its text number at FFEE, a count at FFEC;
- 2294:0341, the sky: a count at FFEA;
- 1194:C04C, the room's file read: the carry from FFE8 bit 0;
- 1194:BA91, the room's file written: a count at FFE6;
- 0000:5364, the hours drawn: a count at FFE4;
- the screen's routines (0000:9AE6, 9AF8, 9466, 9426, 6BDE, 4EFD, A611,
  A6B5, 9A98, 2294:10EC, 2542:0446, 1194:55B2): returns.

  rest | seed | op | result

Op: 0 regeneration (AX hours), 1 camp check, 2 start, 3 frame. Results:
"fault" / "hang", or the carry, the FNV-1a digest of the changed DGROUP
bytes and that of the creature segment (64 KB).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from unicorn.x86_const import UC_X86_REG_EFLAGS

PLAYER = 0x01A4
CSEG = 0x7800
TEXTS = [b'', b'0', b'1', b'2', b'3', b'12', b'999', b'5a', b'-3', b'00', b'ab', b'24', b'100', b'7']
CLASSES = [0x00, 0x0F, 0x20, 0x21, 0x23, 0x24, 0x25, 0x2A, 0x05, 0x37]

RETF, RET = b'\xcb', b'\xc3'
STUBS = [
    # add [FFE0], eax; retf
    (0, 0x142D, bytes.fromhex('660106e0ffcb')),
    # push bx; mov bx,[FFF8]; mov ax,[bx]; add word [FFF8],2; pop bx; retf
    (0x1194, 0x020E, bytes.fromhex('538b1ef8ff8b078306f8ff025bcb')),
    # push si; mov si,[FFFA]; mov eax,[si]; mov [4980],eax; add word [FFFA],4; pop si; ret
    (0x1194, 0x4800, bytes.fromhex('568b36faff668b0466a380498306faff045ec3')),
    # mov [FFF6],si; inc word [FFF4]; ret
    (0x1194, 0x59CC, bytes.fromhex('8936f6ffff06f4ffc3')),
    # mov [FFEE],ax; inc word [FFEC]; ret
    (0x1194, 0xD45D, bytes.fromhex('a3eeffff06ecffc3')),
    # inc word [FFEA]; retf
    (0x2294, 0x0341, bytes.fromhex('ff06eaffcb')),
    # test byte [FFE8],1; jz +2; stc; ret; clc; ret
    (0x1194, 0xC04C, bytes.fromhex('f606e8ff017402f9c3f8c3')),
    # inc word [FFE6]; ret
    (0x1194, 0xBA91, bytes.fromhex('ff06e6ffc3')),
    # inc word [FFE4]; retf
    (0, 0x5364, bytes.fromhex('ff06e4ffcb')),
] + [(s, o, RETF) for s, o in [
    (0, 0x9AE6), (0, 0x9AF8), (0, 0x9466), (0, 0x9426), (0, 0x6BDE), (0, 0x4EFD), (0, 0xA611),
    (0, 0xA6B5), (0, 0x9A98), (0x2294, 0x10EC), (0x2542, 0x0446), (0x1194, 0x55B2)]]


def setup(seed):
    """(writes, op, ax). Writes are (offset, bytes) in DGROUP. Mirrored in
    tests/rest.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    op = byte() % 4
    # The player: a whole record, then the fields resting reads.
    w.append((PLAYER, bytes(byte() for _ in range(0x6E))))
    w.append((0x01A9, pb(CLASSES[byte() % len(CLASSES)])))
    w.append((0x01AA, pb(byte() % 4)))
    cur = []
    for i in range(8):
        base = byte()
        w.append((0x01D5 + i, pb(base)))
        cur.append((base - byte() % 40 if byte() < 0xC0 else byte()) & 0xFF)
        w.append((0x01CD + i, pb(cur[-1])))
    max_hp = word() % 300 + 1
    w.append((0x01FF, pk(max_hp)))
    k = byte()
    hp = max_hp if k < 0x50 else (word() % (max_hp + 1) if k < 0xF0 else word())
    w.append((0x01FD, pk(hp)))
    w.append((0x01F7, pk((byte() % 12) - 4 if byte() < 0xF0 else word())))
    w.append((0x0201, pk(word() & 0x7FFF if byte() < 0xC0 else 0x7F00 + byte())))
    max_sp = word() % 400
    w.append((0x020C, pk(max_sp)))
    w.append((0x020A, pk(max_sp if byte() < 0x60 else word() % (max_sp + 1))))
    # The rest's state.
    w.append((0x1013, pb(byte() % 2 if op != 3 else (1 if byte() < 0xE0 else 0))))
    w.append((0x1014, pk(word() & 0x7FFF)))
    w.append((0x1016, pk((byte() % 5) if byte() < 0xE0 else word())))
    w.append((0x1018, pb(byte() % 2)))
    w.append((0x1019, pk((byte() % 30) + 1 if byte() < 0xE8 else byte() % 3)))
    w.append((0x49E6, pb(byte() % 3)))
    w.append((0x13C8, pb(byte() % 2 if byte() < 0xF0 else byte())))
    w.append((0x0FF9, pk(word())))
    w.append((0x0FFB, pk(0 if byte() < 0xD0 else word())))
    w.append((0xAFF9, pb(0 if byte() < 0xE0 else 1)))
    # Where the player is.
    w.append((0xC398, pk(word() & ~4 | (4 if byte() < 0x30 else 0))))
    w.append((0xC39A, pk(word() & ~0x210 | (0x10 if byte() < 0x20 else 0) | (0x200 if byte() < 0x30 else 0))))
    w.append((0x05E6, pb(4 if byte() < 0x90 else byte() % 6)))
    w.append((0x0D48, pb((4, 4, 0xB, 8, 5, 7, 6)[byte() % 7])))
    w.append((0x191D, pb(byte() % 2)))
    w.append((0x0D39, pb(byte())))
    # The room and the clock.
    now = word() | word() << 16
    w.append((0x05CC, struct.pack('<I', now)))
    end = (now + (word() % 3000) - 300) & 0xFFFFFFFF
    w.append((0x12CA, struct.pack('<I', end)))
    w.append((0x12C4, pk(word())))
    w.append((0x12BC, pb(byte())))
    w.append((0x12BE, pb(byte())))
    # The dream.
    w.append((0x0F76, pb(0 if byte() < 0xA0 else 1)))
    w.append((0x0F77, pb(byte() % 10)))
    w.append((0x0F7A, pb(byte() % 2)))
    # The actor slots' flags.
    for i in range(0x17):
        w.append((0x0AB3 + i * 0x1C + 0xF, pk(word() | (0x4000 if byte() < 0x80 else 0))))
    # The scripts.
    picks = [(0xFFFF, 0, 1, 2, word())[byte() % 5] for _ in range(8)]
    w.append((0xFF00, b''.join(pk(p) for p in picks)))
    texts = [TEXTS[byte() % len(TEXTS)] for _ in range(4)]
    w.append((0xFF20, b''.join(t.ljust(4, b'\0')[:4] for t in texts) + bytes(16)))
    w.append((0xFFF8, pk(0xFF00)))
    w.append((0xFFFA, pk(0xFF20)))
    w.append((0xFFE8, pb(byte() % 2)))
    ax = (byte() % 4 + 1 if byte() < 0xD0 else word()) if op == 0 else 0
    # Aimed: rested exactly at the fatigue margin (fatigue + 60 against
    # the limit the start sets, strength + endurance times 64).
    if byte() < 0x40:
        lim = ((cur[0] + cur[5]) << 6) & 0xFFFF
        w.append((0x1014, pk(lim)))
        w.append((0x0201, pk(lim - 0x3C + byte() % 3 - 1)))
        w.append((0x01FD, pk(max_hp)))
        w.append((0x020A, pk(max_sp)))
    # Spell effects on creatures' records, which forgetting them ends.
    for _ in range(byte() % 4):
        slot = 15 + byte() % 8
        rec = word()
        w.append((0x0AB3 + slot * 0x1C + 0x12, pk(rec)))
        e = byte() % 16
        w.append((0x0660 + e * 0x3C + 0x28, struct.pack('<HH', rec, CSEG)))
    return w, op, ax


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in STUBS:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.w16(0x1042, CSEG)
    base_ds = a.seg_bytes(a.ds)
    rng = random.Random(0x1994_016C)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(CSEG, 0), bytes(0x10000))
        writes, op, ax = setup(seed)
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = a.seg_bytes(a.ds)
        try:
            entry = [(0x2901, 0x016C), (0x2433, 0x12F5), (0x1194, 0x424E), (0x1194, 0x4635)][op]
            a.call_far(*entry, eax=ax)
            carry = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
            after = a.seg_bytes(a.ds)
            cseg = fnv(bytes(a.mu.mem_read(a.lin(CSEG, 0), 0x10000)))
            res = f'{carry} {fnv(changes(before, after)):x} {cseg:x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'rest | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
