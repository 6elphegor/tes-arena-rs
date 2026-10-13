"""Emit test vectors for projectiles (src/projectile.rs) from the
original code.

Usage: gen_projectile.py ACD_unpacked.EXE OUT

A case is a strike case (gen_strike.py: `setup`, then `strike`), then
from the same stream (`flight`, mirrored in tests/projectile.rs): the 23
actors around the player and their squares (1951), walls on the 16 x 16
squares around the player, the light list, a few used object nodes,
flat kinds (CA97), eight spells in SPSEG and the cast records pointing
at them (7F86), the projectile slots (some flying, some arrows), the
occupant (7A44) and the projectile count; the timed-spell-flag table is
TSEG, zeroed. Then maybe a launch (29CF:02E2 with SI the cast, BX the
actor slot, DX), then N frames of 29CF:0441.

  flight | seed k | result

A word access at DGROUP offset FFFF counts as a fault (a 386 raises #GP
in real mode; unicorn reads on past the segment). Stubbed: sounds (23BA:002A), a projectile spawned by
a spell effect (29CF:0270, an INT 3: a "fault", as the port returns None)
and the UI of gen_strike.py. A spell effect leaving DS at the creature
segment, or dispatching outside its handlers, stops a case, as in
gen_spell.py. Results as gen_strike.py.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG, DGROUP
from gen_attack import setup, fnv, CSEG, RSIZE
from gen_strike import strike, WSEG, FRAME, ACTORS, SIZE
from gen_spell import guard_dispatch

SPSEG = 0x6000
TSEG = 0x5C00  # the timed-spell-flag table, zeroed
RELOC = [0x061C] + [0x0660 + i * 0x3C + 0x2A for i in range(16)] + [0x0D5E + 4 * i for i in range(9)]


def flight(byte, word, w):
    """The projectile world; returns (launch, actor, cast, dx, frames)."""
    # The player mid-map: projectiles must stay within the 128 columns
    # of the object lists (beyond, the lists run into memory holding
    # relocated segments, which can't be compared).
    px, py = 0x2000 + (word() & 0xFFF), 0x2000 + (word() & 0xFFF)
    w.append((None, 0xC3D0, struct.pack('<H', px)))
    w.append((None, 0xC3D4, struct.pack('<H', py)))
    spots = []
    for i in range(23):
        si = ACTORS + i * SIZE
        x = (px + byte() * 4 - 0x200) & 0xFFFF
        y = (py + byte() * 4 - 0x200) & 0xFFFF
        spots.append((x, y))
        z = word() & 0x3FF
        t = byte()
        sprite = 0xC5 if t < 0x10 else 0xC0 + t % 5
        flags = word() & 0x3FFF | (0x4000 if byte() < 0x20 else 0)
        w.append((None, si, struct.pack('<HHHH', x, z, y, 0xC3D0)))
        w.append((None, si + 0xA, struct.pack('<H', word() & 0x1FF)))
        w.append((None, si + 0xC, bytes([sprite])))
        w.append((None, si + 0xF, struct.pack('<H', flags)))
        if i >= 15:
            w.append((None, si + 0x12, struct.pack('<H', (i - 15) * RSIZE)))
        sq = ((y >> 7 << 8) & 0xFFFF) | ((x >> 7) * 2 & 0xFF) if byte() < 0xE0 else 0xFFFF
        w.append((None, 0x1951 + 2 * i, struct.pack('<H', sq)))
    w.append((None, 0xCB90, struct.pack('<H', ((py >> 7 << 8) + (px >> 7) * 2) & 0xFFFF)))
    for dy in range(-8, 8):
        for dx in range(-8, 8):
            sq = ((((py >> 7) + dy) << 8) + ((px >> 7) + dx) * 2) & 0xFFFF
            t = byte()
            if t < 0xB0:
                v = 0
            elif t < 0xC0:
                v = 0xB000 | word() & 0xFFF
            elif t < 0xC8:
                v = 0xA000 | word() & 0xFFF
            elif t < 0xD0:
                v = 0x9000 | word() & 0x1FF
            elif t < 0xD8:
                v = 0x8000 | byte()
            elif t < 0xDC:
                v = 0xD000 | byte()
            else:
                v = word() & 0x7FFF
            w.append((WSEG, sq, struct.pack('<H', v)))
    for i in range(32):
        e = 0xBB0C + 6 * i
        if byte() < 0xA0:
            w.append((None, e, struct.pack('<H', 0x8000)))
        else:
            w.append((None, e, struct.pack('<HHH', word() & 0x7FFF, word(), word())))
    w.append((None, 0xBC8C, struct.pack('<H', byte() % 0x22)))
    for i in range(0x80):
        if byte() < 0x30:
            w.append((None, 0xD43C + 6 * i, struct.pack('<HHH', 0, word(), word() | 1)))
    w.append((None, 0xCA97, bytes(byte() for _ in range(0x60))))
    w.append((None, 0xC398, struct.pack('<H', word() & 0x0004)))
    w.append((None, 0xC39A, struct.pack('<H', word() & 0x0200)))
    for c in range(8):
        sp = c * 0x55
        w.append((SPSEG, sp, bytes(byte() for _ in range(0x55))))
        w.append((SPSEG, sp + 0x24, bytes([byte() % 5])))
        w.append((SPSEG, sp + 0x26, bytes([byte() % 6])))
        w.append((None, 0x7F86 + c * 0x16, struct.pack('<HHHHHH', sp, SPSEG, (byte() % 8) * RSIZE, CSEG,
                                                     byte(), word() & 0x21)))
    for i in range(12):
        s = 0x7DBC + i * 0x1C
        w.append((None, s, bytes(byte() for _ in range(0x1C))))
        if byte() < 0x60 and i < 11:
            x = (px + byte() * 8 - 0x400) & 0xFFFF
            y = (py + byte() * 8 - 0x400) & 0xFFFF
            if byte() < 0x80:
                x, y = spots[byte() % 23]  # on an actor
            cast = 0xFFFE if byte() < 0x60 else 0x7F86 + (byte() % 8) * 0x16
            heading = 0xFFFF if byte() < 0x20 else (word() & 0x1FF) * 2
            w.append((None, s, struct.pack('<HHHH', x, word() & 0xFF, y, 0xBB0C + 6 * (byte() % 32))))
            w.append((None, s + 0xA, struct.pack('<H', heading)))
            w.append((None, s + 0xF, struct.pack('<H', word() & 0x1800 | 0x200)))
            w.append((None, s + 0x11, bytes([1 + byte() % 0x1E])))
            w.append((None, s + 0x12, struct.pack('<H', cast)))
            w.append((None, s + 0x1A, struct.pack('<H', byte() % 3)))
        else:
            w.append((None, s + 0x11, b'\0'))
    t = byte()
    occ = 0xFFFF if t < 0x40 else 0x1A4 if t < 0x80 else byte() % 23
    w.append((None, 0x7A44, struct.pack('<H', occ)))
    w.append((None, 0x7DB8, struct.pack('<H', byte() % 12)))
    launch = byte() < 0xE0
    actor = ACTORS + (15 + byte() % 8 if byte() < 0xC0 else byte() % 23) * SIZE
    cast = 0x7F86 + (byte() % 8) * 0x16
    dx = word()
    frames = 1 + byte() % 30
    return launch, actor, cast, dx, frames


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0x23BA, 0x002A, b'\xcb'), (0x1194, 0xA36C, b'\xcb'), (0, 0x9AE6, b'\xcb'),
                           (0, 0x9CA0, b'\xcb'), (0x2DA2, 0x000A, b'\xcb'), (0x1194, 0x59C8, b'\xcb'),
                           (0x29CF, 0x0270, b'\xcc')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    # The spell effects' DS leak stops a case, as in gen_spell.py.
    from unicorn import UC_HOOK_CODE, UcError
    from unicorn.x86_const import UC_X86_REG_DS

    def leak(mu, addr, size, _):
        if mu.reg_read(UC_X86_REG_DS) != a.ds:
            a.fault = 'ds'
            mu.emu_stop()
    for seg, off in [(0x2CDD, 0x03E5), (0x2CDD, 0x003F), (0x29CF, 0x09A3)]:
        lin = (LOAD_SEG + seg) * 16 + off
        a.mu.hook_add(UC_HOOK_CODE, leak, None, lin, lin)
    guard_dispatch(a)
    a.w16(0x1020, 0)
    a.w16(0x1022, TSEG)
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
        if struct.unpack_from('<H', mem, 0x67EC)[0] == LOAD_SEG:
            struct.pack_into('<H', mem, 0x67EC, 0)
        return bytes(mem)

    # A word access at DGROUP offset FFFF (runaway object lists): a 386
    # raises #GP for it in real mode; unicorn reads on past the segment.
    from unicorn import UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE
    wrap = []
    edge = a.lin(a.ds, 0xFFFF)
    a.mu.hook_add(UC_HOOK_MEM_READ | UC_HOOK_MEM_WRITE,
                  lambda mu, acc, addr, size, v, _: wrap.append(1) if size > 1 else None, None, edge, edge)

    rng = __import__('random').Random(0x1994_0441)
    for n in range(3000):
        seed = rng.getrandbits(32)
        k = rng.randrange(8)
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (CSEG, WSEG, FRAME, SPSEG, TSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        p = []

        def extra(byte, word, w):
            strike(byte, word, w, k * RSIZE)
            p.append(flight(byte, word, w))

        for seg, off, data in setup(seed, k, classes, extra):
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        launch, actor, cast, dx, frames = p[0]
        for o in RELOC:
            if a.r16(o) == DGROUP:
                a.w16(o, a.ds)
        before = unrelocated(a.seg_bytes(a.ds))
        wrap.clear()
        try:
            if launch:
                a._setup([0, STUB_SEG], {'si': cast, 'bx': actor, 'dx': dx})
                a._run(0x29CF, 0x02E2, STUB_SEG, 0)
            for _ in range(frames):
                a._setup([0, STUB_SEG], {})
                a._run(0x29CF, 0x0441, STUB_SEG, 0)
            if wrap:
                raise Fault(13)
            after = unrelocated(a.seg_bytes(a.ds))
            if a.r16(0x061C, CSEG) == a.ds:
                a.w16(0x061C, DGROUP, CSEG)
            changed = b''.join(struct.pack('<HB', i, after[i]) for i in range(0x10000) if after[i] != before[i])
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        except UcError:
            res = 'invalid'
        out.write(f'flight | {seed:x} {k} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
