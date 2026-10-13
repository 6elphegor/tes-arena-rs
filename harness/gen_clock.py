"""Emit test vectors for the game clock (src/clock.rs, 0000:142D) from the
original code.

Usage: gen_clock.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/clock.rs) the
calendar, the player (fatigue, class, race, level, experience, items),
eight creatures and their actor slots (some near the map's corner, where
the aura check looks), the effect slots, the timed-flag table (TSEG),
the countdowns and the flags the handlers test, then advances the clock
by a seeded number of minutes (EAX).

  clock | seed | result

Stubbed with a RETF, as `ClockHooks` (the port's NoClockHooks do
nothing): quests (1194:99F8, 98E8), encounters (1194:887A, 86F2), the
level-up screen (1194:1034) and its text (0000:9AF8), and the
exhaustion path of 2901:0002 (from 2901:0014).

Results: "fault" / "hang", or FNV-1a digests of the changed DGROUP
bytes, the creature segment (32 KB) and the timer table (1 KB).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG, DGROUP
from gen_attack import fnv, stream

CSEG, TSEG = 0x7800, 0x5C00
PLAYER, CSIZE = 0x01A4, 0x41E
SLOTS = 0x0660


def setup(seed, auras):
    """(segment or None for DGROUP, offset, bytes) writes, in order, and
    the minutes to advance. `auras` lists the creature types whose
    special-attack byte (2DE6) is FEh. Mirrored in tests/clock.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    raw = lambda n: bytes(byte() for _ in range(n))
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []

    # The calendar: a time, sometimes just before a new day, month or year.
    t = word() | byte() << 16
    k = byte()
    if k < 0x40:
        t = t - t % 1440 + 1440 - 1 - byte() % 4
    elif k < 0x50:
        t = t - t % 518400 + 518400 - 1 - byte() % 4
    w.append((None, 0x05CC, struct.pack('<I', t & 0xFFFFFFFF)))
    w.append((None, 0x05D0, raw(10)))
    w.append((None, 0x0A20, bytes([byte()])))
    # The player.
    w.append((None, PLAYER, raw(0x6E)))
    c = byte()
    w.append((None, PLAYER + 5, bytes([0x11 if c < 0x40 else c % 0x12])))
    w.append((None, PLAYER + 4, bytes([7 if byte() < 0x30 else byte() % 8])))
    w.append((None, PLAYER + 6, bytes([1 + byte() % 20])))
    w.append((None, PLAYER + 0x5D, pk(byte() * 4 if byte() < 0xC0 else word())))
    w.append((None, PLAYER + 0x409, struct.pack('<I', word() * byte())))
    w.append((None, PLAYER + 0x40D, pk(word() & 0x00D0)))
    w.append((None, PLAYER + 0x41A, pk(word() if byte() < 0x40 else 0)))
    for i in range(40):
        it = 0x0212 + i * 19
        if byte() < 0x40:
            continue
        b = bytearray(raw(19))
        b[14] = b[14] & 0xB3 | 0x40 | (byte() % 4) << 2
        w.append((None, it, bytes(b)))
    # Creatures and their slots.
    for k in range(8):
        rec = k * CSIZE
        w.append((CSEG, rec, raw(0x6E)))
        ty = auras[byte() % len(auras)] if byte() < 0x80 else 1 + byte() % 0x18
        w.append((CSEG, rec + 4, bytes([ty])))
        w.append((CSEG, rec + 0x40D, pk(word() & 0x00D0 | (0x1000 if byte() < 0xE0 else 0))))
        si = 0x0C57 + k * 0x1C
        near = byte() < 0x60
        x, y = (byte(), byte()) if near else (word(), word())
        t2 = byte()
        sprite = 0xC5 if t2 < 0x10 else 0xC0 + t2 % 5
        w.append((None, si, struct.pack('<HHH', x, 0, y)))
        w.append((None, si + 0xC, bytes([sprite])))
        w.append((None, si + 0xF, pk(word() & 0x7FFF if byte() < 0x30 else word() & 0x3FFF)))
        w.append((None, si + 0x12, pk(rec)))
    # Effects, timers, countdowns.
    w.append((None, 0x1020, struct.pack('<HH', 0, TSEG)))
    w.append((TSEG, 0, raw(0x240)))
    w.append((None, SLOTS, raw(16 * 0x3C)))
    for i in range(16):
        at = SLOTS + i * 0x3C
        if byte() < 0xA0:
            w.append((None, at + 0x3B, b'\xff'))
        else:
            k = byte() % 9
            off, seg = (PLAYER, DGROUP) if k == 8 else (k * CSIZE, CSEG)
            w.append((None, at + 0x28, struct.pack('<HH', off, seg)))
            w.append((None, at + 0x3A, bytes([byte() % 12])))
    w.append((None, 0x1378, raw(5)))
    # Flags the handlers test.
    w.append((None, 0xC398, pk(word() & 0x0084 if byte() < 0x20 else word() & 0x0004)))
    w.append((None, 0xC39A, pk(word() & 0x4210)))
    w.append((None, 0x05C8, pk(word() & 0x0020 if byte() < 0x30 else 0)))
    w.append((None, 0x1013, bytes([byte() if byte() < 0x30 else 0])))
    w.append((None, 0x05E6, bytes([byte() % 5])))
    w.append((None, 0x137F, bytes([0xFF if byte() < 0x80 else byte()])))
    w.append((None, 0x0FFB, pk(0 if byte() < 0xC0 else byte())))
    w.append((None, 0x05CA, pk(word() & 0x0100)))
    w.append((None, 0xCB5C, bytes([byte() if byte() < 0x20 else 0])))
    w.append((None, 0x91F6, raw(4)))
    w.append((None, 0x05E0, raw(2)))
    w.append((None, 0x05C6, raw(2)))
    k = byte()
    minutes = 1 + byte() % 30 if k < 0x80 else 1 + word() % 400 if k < 0xF0 else 0x10001 + byte() % 2
    return w, minutes


RELOC = [0x061C] + [SLOTS + i * 0x3C + 0x2A for i in range(16)]


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off in [(0x1194, 0x99F8), (0x1194, 0x98E8), (0x1194, 0x887A), (0x1194, 0x86F2), (0x1194, 0x1034),
                     (0, 0x9AF8), (0x2901, 0x0014)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
    a.w16(0x1042, CSEG)
    a.w16(0x91F0, 0)
    auras = [t + 1 for t in range(0x30) if a.mu.mem_read(a.lin(a.ds, 0x2DE6 + t), 1)[0] == 0xFE]
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)

    def unrelocated(mem):
        mem = bytearray(mem)
        for o in RELOC:
            if struct.unpack_from('<H', mem, o)[0] == a.ds:
                struct.pack_into('<H', mem, o, DGROUP)
        return bytes(mem)

    rng = __import__('random').Random(0x1994_142D)
    for n in range(1500):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (CSEG, TSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        w, minutes = setup(seed, auras)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        for o in RELOC:
            if a.r16(o) == DGROUP:
                a.w16(o, a.ds)
        before = unrelocated(a.seg_bytes(a.ds))
        try:
            a._setup([0, STUB_SEG], {'eax': minutes})
            a._run(0, 0x142D, STUB_SEG, 0)
            after = unrelocated(a.seg_bytes(a.ds))
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} {fnv(a.seg_bytes(TSEG)[:0x400]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'clock | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
