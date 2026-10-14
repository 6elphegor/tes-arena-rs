"""Emit test vectors for src/spell.rs (29CF:0941, a cast applied to its
targets) from the original code.

Usage: gen_spell.py ACD_unpacked.EXE OUT

A case fills from a seed (`setup`, mirrored in tests/spell.rs) the
player, eight creature records (41Eh apart, as reflection expects), the
eight creature actor slots, a spell record (in its own segment SSEG, so
no relocated segment is involved), a cast record, the target list, the
timed-flag table (TSEG), the effect slots and a few variables, then runs
29CF:0941 on the cast. (The buffer segments lie above 5BA0h: below that
they would overlap DGROUP in the emulator, which loads the image at
1000h.)

  cast | seed | result

Stubbed: 29CF:0270 (a projectile from the player, not ported) with an
INT 3, so a case reaching it is a "fault". So is a successful instant
death on a creature: 2CDD:03C1 leaves DS at the creature's segment, and
the original then reads its tables through it and runs into data. The DGROUP segment is written
as 3BA0 in the target list, the cast's caster and the effect slots, and
compared image-relative.

Results as gen_attack.py: "fault" / "hang", or FNV-1a digests of the
changed DGROUP bytes, the creature segment (32 KB), the timer table
(TSEG, 1 KB) and the wall plane (WSEG, 32 KB).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import UC_X86_REG_DS
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes
from gen_attack import fnv, stream

CSEG, SSEG, TSEG, WSEG, FRAME = 0x7800, 0x6000, 0x5C00, 0x6800, 0x8800
PLAYER, CSIZE = 0x01A4, 0x41E
CASTS, SLOTS = 0x7F86, 0x0660


def setup(seed):
    """(segment or None for DGROUP, offset, bytes) writes, in order, and
    the cast record's offset. Mirrored in tests/spell.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    raw = lambda n: bytes(byte() for _ in range(n))
    w = []
    pk = lambda v: struct.pack('<H', v & 0xFFFF)

    def target():
        """A far pointer: the player or one of the creatures."""
        k = byte() % 9
        return (PLAYER, DGROUP) if k == 8 else (k * CSIZE, CSEG)

    def person(seg, off, creature):
        w.append((seg, off, raw(0x6E)))
        w.append((seg, off + 4, bytes([byte() % 0x18 if creature else byte() % 8])))
        c = byte()
        w.append((seg, off + 5, bytes([0x23 if c < 0x20 else 0x11 if c < 0x30 else 0x0F if c < 0x40 else c])))
        w.append((seg, off + 6, bytes([byte() % 30])))
        # (Creatures never have 0x4000: with it a curse rolls the dice with
        # DS at the creature's segment, which the random generator reads.)
        f = word() & (0x00D0 if creature else 0x40D0) | (0x1000 if creature else 0)
        w.append((seg, off + 0x40D, pk(f)))
        w.append((seg, off + 0x40F, raw(9)))
        w.append((seg, off + 0x41A, pk(word() if byte() < 0x80 else 0)))
        w.append((seg, off + 0x41C, pk(0 if byte() < 0xA0 else byte())))

    person(None, PLAYER, False)
    for k in range(8):
        person(CSEG, k * CSIZE, True)
        si = 0x0C57 + k * 0x1C
        w.append((None, si + 0xF, pk(word() & 0x3FFF)))
        w.append((None, si + 0x12, pk(k * CSIZE)))
    # The spell.
    sp = [byte() % 8 if byte() < 0xF0 else 0 for _ in range(18)]
    w.append((SSEG, 0, b''.join(pk(v) for v in sp)))
    w.append((SSEG, 0x24, bytes([(byte() % 6 - 1) & 0xFF, 0, byte() % 6])))
    w.append((SSEG, 0x27, pk(word())))
    kinds = []
    for _ in range(3):
        t = byte()
        kinds.append(0xFF if t < 0x18 else 0x19 + t % 8 if t < 0x20 else t % 0x19)
    subs = [byte() % 6 for _ in range(3)]
    subs = [v % 2 if k == 13 else v for k, v in zip(kinds, subs)]
    w.append((SSEG, 0x29, bytes(kinds + subs + [byte() % 16 for _ in range(3)])))
    w.append((SSEG, 0x32, pk(word())))
    # The cast and its targets.
    cast = CASTS + (byte() % 8) * 0x16
    off, seg = target()
    w.append((None, cast, struct.pack('<HHHHHH', 0, SSEG, off, seg, 1 + byte() % 30, word())))
    # The list runs on past its count when reflections repeat (each one
    # writes the caster at the same index): stale entries are targets too.
    n = 1 + byte() % 3
    w.append((None, 0x0D5A, pk(n)))
    for i in range(16):
        off, seg = target()
        w.append((None, 0x0D5C + 4 * i, struct.pack('<HH', off, seg)))
    # The timed-flag table, the effect slots and the rest.
    w.append((None, 0x1020, struct.pack('<HH', 0, TSEG)))
    w.append((TSEG, 0, raw(0x240)))
    w.append((None, SLOTS, raw(16 * 0x3C)))
    for i in range(16):
        at = SLOTS + i * 0x3C
        t = byte()
        if t < 0x80:
            w.append((None, at + 0x3B, b'\xff'))
        else:
            off, seg = target()
            w.append((None, at + 0x28, struct.pack('<HH', off, seg)))
            w.append((None, at + 0x3A, bytes([byte() % 12])))
    w.append((None, 0x1292, pk(1 + byte() % 30)))
    w.append((None, 0x05E6, bytes([byte() & 1])))
    w.append((None, 0x0DC0, raw(10)))
    w.append((None, 0xC3D2, pk(0x3C if byte() < 0x80 else word())))
    sq = (byte() % 64) << 8 | (byte() % 64) << 1
    w.append((None, 0xCB90, pk(sq)))
    w.append((WSEG, sq, pk(0 if byte() < 0x80 else word())))
    w.append((None, 0xC39A, pk(word())))
    w.append((None, 0xC942, bytes([byte()])))
    w.append((None, 0xD0B8 + 4 * 0x19, struct.pack('<HH', byte(), 0)))
    w.append((FRAME, 0, raw(0x110)))
    w.append((None, 0x91F6, raw(4)))
    return w, cast


# The handlers 2CDD:000C reaches through its tables. A table read that
# lands elsewhere (a sub-kind past the tables) jumps into data; the port
# returns None, and `guard_dispatch` stops such a case as a fault.
HANDLERS = {0x004F, 0x0142, 0x01B3, 0x020C, 0x0278, 0x030F, 0x037D, 0x03C1, 0x03F1, 0x03F2, 0x0447, 0x0497,
            0x04FA, 0x05CE, 0x0712, 0x0713, 0x0723, 0x075A, 0x078C, 0x0793, 0x07A3, 0x07DB, 0x0822, 0x0841,
            0x0863, 0x00CB, 0x00CC, 0x0061, 0x00A5, 0x010C, 0x012F, 0x0154, 0x0180, 0x019B, 0x01BC, 0x01D6,
            0x01F1, 0x0386, 0x03A3, 0x03C0, 0x021C, 0x022F, 0x024E, 0x0263, 0x0288, 0x02F0, 0x02FC, 0x0404,
            0x04A6, 0x050F, 0x0548, 0x0571, 0x058F, 0x05F7, 0x0604}
# Call sites `call word ptr [di + table]` and their displacements.
DISPATCH = [(0x003F, 0x83E2), (0x005A, 0x8414), (0x014D, 0x841E), (0x01B7, 0x8424), (0x0216, 0x842A),
            (0x0282, 0x8432), (0x0381, 0x8438), (0x03FE, 0x843E), (0x0504, 0x8442), (0x05DB, 0x844A)]


def guard_dispatch(a):
    from unicorn import UC_HOOK_CODE
    from unicorn.x86_const import UC_X86_REG_DS, UC_X86_REG_DI

    def check(mu, addr, size, table):
        ds, di = mu.reg_read(UC_X86_REG_DS), mu.reg_read(UC_X86_REG_DI)
        target = struct.unpack('<H', mu.mem_read(ds * 16 + ((di + table) & 0xFFFF), 2))[0]
        if target not in HANDLERS:
            a.fault = 'dispatch'
            mu.emu_stop()
    for off, table in DISPATCH:
        lin = (LOAD_SEG + 0x2CDD) * 16 + off
        a.mu.hook_add(UC_HOOK_CODE, check, table, lin, lin)


RELOC = [0x061C] + [0x0D5E + 4 * i for i in range(16)] + [CASTS + 0x16 * i + 6 for i in range(8)] + \
    [SLOTS + i * 0x3C + 0x2A for i in range(16)]


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.mu.mem_write(a.lin(LOAD_SEG + 0x29CF, 0x0270), b'\xcc')

    # 2CDD:03C1 (instant death) leaves DS at the target's segment, and
    # what follows reads DGROUP's tables through it, running into data:
    # a case that gets there with a target outside DGROUP stops as a
    # fault (the port returns None there).
    def leak(mu, addr, size, _):
        if mu.reg_read(UC_X86_REG_DS) != a.ds:
            a.fault = 'ds'
            mu.emu_stop()
    # Kinds 2 and 6 leave DS there too through their sub-kind tables:
    # stop at the next dispatch or the return into 29CF:0941.
    for seg, off in [(0x2CDD, 0x03E5), (0x2CDD, 0x003F), (0x29CF, 0x09A3)]:
        lin = (LOAD_SEG + seg) * 16 + off
        a.mu.hook_add(UC_HOOK_CODE, leak, None, lin, lin)
    guard_dispatch(a)
    a.w16(0x1042, CSEG)
    a.w16(0xD806, WSEG)
    a.w16(0x9030, FRAME)
    a.w16(0x91F0, 0)
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)

    def unrelocated(mem):
        mem = bytearray(mem)
        for o in RELOC:
            if struct.unpack_from('<H', mem, o)[0] == a.ds:
                struct.pack_into('<H', mem, o, DGROUP)
        return bytes(mem)

    rng = __import__('random').Random(0x1994_0941)
    for n in range(4000):
        seed = rng.getrandbits(32)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (CSEG, SSEG, TSEG, WSEG, FRAME):
            a.mu.mem_write(a.lin(seg, 0), zero)
        w, cast = setup(seed)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        for o in RELOC:
            if a.r16(o) == DGROUP:
                a.w16(o, a.ds)
        before = unrelocated(a.seg_bytes(a.ds))
        try:
            a.call_near(0x29CF, 0x0941, si=cast)
            after = unrelocated(a.seg_bytes(a.ds))
            # Messages written with DS at the creature segment.
            if a.r16(0x061C, CSEG) == a.ds:
                a.w16(0x061C, DGROUP, CSEG)
            changed = changes(before, after)
            res = (f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} '
                   f'{fnv(a.seg_bytes(TSEG)[:0x400]):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}')
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        except UcError:
            res = 'invalid'
        out.write(f'cast | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
