"""Emit test vectors for the player's strike (src/strike.rs) from the
original code.

Usage: gen_strike.py ACD_unpacked.EXE OUT

A case fills the player and a combatant as gen_attack.py does (`setup`),
then from the same stream (`strike`, mirrored in tests/strike.rs) the
actor slot the player strikes, the squares under it, the flats' table
and data in the EMS frame, the quest records and the list at 0FD4, and
runs 0000:3B6D (the player hits that actor):

  strike | seed k | result
  quest | seed k stage | result   the same, made a quest case (`quest`)

Stubbed: the sound (23BA:002A), the message box of a broken special
item (1194:A36C) and the text of a quest target giving up (0000:9AE6,
9CA0, 2DA2:000A, 1194:59C8). EMS is off.

Results as gen_attack.py: "fault" / "hang" or digests of the changed
DGROUP bytes and of the creature segment, plus one of the wall plane
(the first 32 KB of WSEG).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG, DGROUP
from gen_attack import setup, fnv, CSEG, PLAYER, RSIZE

WSEG, FRAME = 0x6800, 0x8800
ACTORS, SIZE = 0x0AB3, 0x1C


def strike(byte, word, w, rec):
    """The rest of a case after `setup`, appended to its writes `w`.
    The struck slot is a creature slot (15..22); the caller points it at
    record k, at `rec`. Returns the slot."""
    j = 15 + byte() % 8
    si = ACTORS + j * SIZE
    w.append((None, si, bytes(byte() for _ in range(SIZE))))
    x, y = word() & 0x1FFF, word() & 0x1FFF
    w.append((None, si, struct.pack('<HHH', x, 0, y)))
    w.append((None, si + 6, struct.pack('<H', 0xC3D0)))
    w.append((None, si + 0xA, struct.pack('<H', word() & 0x1FF)))
    w.append((None, si + 0xF, struct.pack('<H', word() & 0x3FFF)))
    px, py = (x + byte() - 0x80) & 0xFFFF, (y + byte() - 0x80) & 0xFFFF
    w.append((None, 0xC3D0, struct.pack('<H', px)))
    w.append((None, 0xC3D4, struct.pack('<H', py)))
    # The actor's square and the one north of it (0000:2858).
    sq = (y >> 7 << 8) | (x >> 7 << 1)
    t = byte()
    wall = 0xB000 | word() & 0xFFF if t < 0x80 else 0 if t < 0xC0 else word()
    w.append((WSEG, sq, struct.pack('<H', wall)))
    w.append((WSEG, (sq - 0x100) & 0xFFFF, struct.pack('<H', 0 if byte() < 0x80 else word())))
    # Flats: offsets into the frame, and the frame's bytes.
    for i in range(0x30):
        w.append((None, 0xD0B8 + 4 * i, struct.pack('<HH', byte(), 0)))
    w.append((FRAME, 0, bytes(byte() for _ in range(0x110))))
    # Quests: record 0 sometimes waits for this combatant's death.
    w.append((None, 0x0E6E, bytes(byte() for _ in range(4 * 0x34))))
    t = byte()
    if t < 0x60:
        stage = 6 if t < 0x30 else 4
        w.append((None, 0x0E6E + 4, struct.pack('<H', 0x40 | stage << 9 | word() & 0xF13F)))
        w.append((None, 0x0E6E + 0x2F, bytes([0x18 + byte() % 0x20, byte() % 0x18])))
    w.append((None, 0x0FD4, bytes(byte() % 10 if byte() < 0xC0 else 0xFF for _ in range(9))))
    w.append((None, 0x102C, struct.pack('<H', word())))
    v = word()
    w.append((None, 0x05CA, struct.pack('<H', v & 0x0160)))
    w.append((None, 0x653E, struct.pack('<H', word())))
    w.append((None, 0x05C2, bytes(byte() for _ in range(4))))
    # Odd records: already dead or flagged 0x80, no maximum HP, types 17h
    # and 2Ah; a second quest waiting for a death.
    if byte() < 0x40:
        w.append((CSEG, rec + 0x40D, struct.pack('<H', word() & 0x1190)))
    if byte() < 0x30:
        w.append((CSEG, rec + 0x5B, struct.pack('<H', 0 if byte() & 1 else 0xFFF0)))
    # (A kill clears the low byte of the flags first, unless the creature
    # is of type 0Eh or 16h.)
    t = byte()
    if t < 0x50:
        ty = 0x17 if t < 0x20 else 0x2A if t < 0x30 else 0x0E if t & 1 else 0x16
        w.append((CSEG, rec + 4, bytes([ty])))
    if byte() < 0x40:
        q = byte() % 0x18
        w.append((None, 0x0E6E + 0x34 + 4, struct.pack('<H', 0x40 | 6 << 9)))
        w.append((None, 0x0E6E + 0x34 + 0x30, bytes([q])))
        if byte() < 0x80:
            w.append((CSEG, rec + 4, bytes([q])))
    return si


def quest(a, k, stage):
    """A quest case: the record is a quest target (05CA bit 0x40) on 1 HP
    of 40, and quest record 0 is of kind 1 at `stage` (6: waiting for a
    creature of its type to die; 4: for a person of its class to give
    up). Mirrored in tests/strike.rs."""
    rec = k * RSIZE
    a.w16(0x05CA, a.r16(0x05CA) | 0x40)
    a.w16(rec + 0x59, 1, CSEG)
    a.w16(rec + 0x5B, 40, CSEG)
    a.w16(0x0E6E + 4, 0x40 | stage << 9)
    ty = a.mu.mem_read(a.lin(CSEG, rec + 4), 1)[0]
    cl = a.mu.mem_read(a.lin(CSEG, rec + 5), 1)[0]
    a.mu.mem_write(a.lin(a.ds, 0x0E6E + 0x2F), bytes([(cl & 0x1F) + 0x18, ty]))


RELOC = [0x061C] + [0x0660 + i * 0x3C + 0x2A for i in range(16)]


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off in [(0x23BA, 0x002A), (0x1194, 0xA36C), (0, 0x9AE6), (0, 0x9CA0), (0x2DA2, 0x000A),
                     (0x1194, 0x59C8)]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), b'\xcb')
    a.w16(0x1042, CSEG)
    a.w16(0xD806, WSEG)
    a.w16(0x9030, FRAME)
    a.w16(0x91F0, 0)
    classes = list(a.mu.mem_read(a.lin(a.ds, 0x2A50), 18))
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)

    def unrelocated(mem):
        mem = bytearray(mem)
        for o in RELOC:
            if struct.unpack_from('<H', mem, o)[0] == a.ds:
                struct.pack_into('<H', mem, o, DGROUP)
        return bytes(mem)

    rng = __import__('random').Random(0x1994_3B6D)
    for n in range(4000):
        seed = rng.getrandbits(32)
        k = rng.randrange(8)
        stage = rng.choice([4, 6]) if n >= 3000 else None
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (CSEG, WSEG, FRAME):
            a.mu.mem_write(a.lin(seg, 0), zero)
        slot = []
        w = setup(seed, k, classes, lambda byte, word, w: slot.append(strike(byte, word, w, k * RSIZE)))
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        a.w16(slot[0] + 0x12, k * RSIZE)
        if stage is not None:
            quest(a, k, stage)
        for o in RELOC:
            if a.r16(o) == DGROUP:
                a.w16(o, a.ds)
        before = unrelocated(a.seg_bytes(a.ds))
        try:
            a._setup([0, STUB_SEG], {'si': slot[0]})
            a._run(0, 0x3B6D, STUB_SEG, 0)
            after = unrelocated(a.seg_bytes(a.ds))
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        if stage is None:
            out.write(f'strike | {seed:x} {k} | {res}\n')
        else:
            out.write(f'quest | {seed:x} {k} {stage} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
