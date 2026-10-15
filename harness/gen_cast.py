"""Emit test vectors for the player casting (src/cast.rs `cast_spell`,
29CF:071F, then `resolve_cast`, 29CF:0000) from the original code.

Usage: gen_cast.py ACD_unpacked.EXE GAME_DIR OUT

A case is gen_projectile.py's world (a strike case, then `flight`), the
cursor (the view's centre C3D6 at A0h), spell points, silence (5BE bit
200h), the area kind (5E6), and a random spell (gen_spell.py's ranges)
as the player's spell 80h + idx (3276:2A80); the player casts it from
the spellbook, and a waiting cast is resolved as a touch on actor slot
`slot` (AL 0) or on the player (AL 1); then the projectiles fly their
frames. The casting hands' graphics load
from GLOBAL.BSA, opened as the game does at startup. Stubs
and results as gen_projectile.py; the saved flags' high byte (7DBB)
holds the spells' segment, compared image-relative.

  cast | seed k idx touch slot mx sp flags town spell | result

"lowmem" marks a case that read low memory (0..3FFFh): a target list
run past its entries reads a record at 0:0, which the harness can't
compare (the image sits at 1000h here, at 0 in the port).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_FS
from emu import Fault, LOAD_SEG, STUB_SEG, DGROUP
from emu import changes
from dos import DosArena
from gen_attack import setup, fnv, CSEG, RSIZE
from gen_strike import strike, WSEG, FRAME, ACTORS, SIZE
from gen_spell import guard_dispatch
from gen_projectile import flight, SPSEG, TSEG, RELOC


def spell_bytes(rng):
    """A random spell record (55h bytes), as gen_spell.py makes them."""
    b = bytearray(rng.getrandbits(8) for _ in range(0x55))
    b[0x24] = (rng.randrange(6) - 1) & 0xFF
    b[0x26] = rng.randrange(6)
    for k in range(3):
        t = rng.randrange(0x100)
        b[0x29 + k] = 0xFF if t < 0x18 else 0x19 + t % 8 if t < 0x20 else t % 0x19
        b[0x2C + k] = rng.randrange(6) if b[0x29 + k] != 13 else rng.randrange(2)
        b[0x2F + k] = rng.randrange(16)
    return bytes(b)


class LowMemory(Exception):
    pass


def cast_unrelocated(a, mem, written=False):
    """The saved flags' high byte (7DBB, the spells' segment's, when the
    run wrote it: the base image holds the relocated byte too), the file
    loader's buffer (A90A) and the cast records' spell and caster
    segments, image-relative."""
    mem = bytearray(mem)
    if written and mem[0x7DBB] == (LOAD_SEG + 0x3276) >> 8:
        mem[0x7DBB] = 0x32
    if struct.unpack_from('<H', mem, 0xA90A)[0] == a.ds:
        struct.pack_into('<H', mem, 0xA90A, DGROUP)
    for c in range(8):
        at = 0x7F86 + 0x16 * c
        if struct.unpack_from('<H', mem, at + 2)[0] == LOAD_SEG + 0x3276:
            struct.pack_into('<H', mem, at + 2, 0x3276)
        if struct.unpack_from('<H', mem, at + 6)[0] == a.ds:
            struct.pack_into('<H', mem, at + 6, DGROUP)
    return bytes(mem)


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA, as the game does
    for seg, off, code in [(0x23BA, 0x002A, b'\xcb'), (0x1194, 0xA36C, b'\xcb'),
                           (0, 0x9CA0, b'\xcb'), (0x2DA2, 0x000A, b'\xcb'), (0x1194, 0x59C8, b'\xcb')]:
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
    a.w16(0xC3D6, 0xA0)
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

    # Reads of low memory (a target list run past its entries reads a
    # record at 0:0) can't be compared: the image sits at 1000h here but
    # at 0 in the port. Such cases are marked "lowmem" and skipped.
    saved = []
    a.mu.hook_add(UC_HOOK_MEM_WRITE, lambda mu, acc, addr, size, v, _: saved.append(addr), None,
                  a.lin(a.ds, 0x7DBA), a.lin(a.ds, 0x7DBB))
    low = []
    a.mu.hook_add(UC_HOOK_MEM_READ, lambda mu, acc, addr, size, v, _: low.append(addr), None, 0, 0x3FFF)
    rng = __import__('random').Random(0x1994_071F)
    # Not sharded (harness/par.py): a case starts from the state the one
    # before it left, so the cases must run in order.
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        k = rng.randrange(8)
        idx = rng.randrange(8)
        touch = rng.randrange(2)
        slot = ACTORS + rng.randrange(23) * SIZE
        mx = rng.randrange(0x140)
        spell = spell_bytes(rng)
        sp = rng.choice([0, 5, 0x40, 0x400, 0x7FFF])
        flags = rng.choice([0, 0, 0, 0x200])
        town = rng.choice([0, 0, 1, 4])
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
        a.w16(0x8F92, mx)
        a.w16(0x020A, sp)
        a.w16(0x05BE, a.r16(0x05BE) & ~0x200 | flags)
        a.mu.mem_write(a.lin(a.ds, 0x05E6), bytes([town]))
        a.mu.mem_write(a.lin(LOAD_SEG + 0x3276, 0x2A80 + 0x55 * idx), spell)
        before = cast_unrelocated(a, unrelocated(a.seg_bytes(a.ds)))
        wrap.clear()
        low.clear()
        saved.clear()
        try:
            a._setup([0, STUB_SEG], {'eax': 0x80 | idx, 'esi': 0x1A4})
            a.mu.reg_write(UC_X86_REG_FS, a.ds)
            a._run(0x29CF, 0x071F, STUB_SEG, 0)
            if a.r16(0x0620):
                if touch == 0:
                    a._setup([0, STUB_SEG], {'eax': 0, 'esi': slot, 'edx': 0})
                else:
                    a._setup([0, STUB_SEG], {'eax': 1, 'esi': 0x1A4, 'edx': 0})
                a.mu.reg_write(UC_X86_REG_FS, a.ds)
                a._run(0x29CF, 0x0000, STUB_SEG, 0)
            for _ in range(frames):
                a._setup([0, STUB_SEG], {})
                a._run(0x29CF, 0x0441, STUB_SEG, 0)
            if wrap:
                raise Fault(13)
            if low:
                raise LowMemory()
            after = cast_unrelocated(a, unrelocated(a.seg_bytes(a.ds)), bool(saved))
            if a.r16(0x061C, CSEG) == a.ds:
                a.w16(0x061C, DGROUP, CSEG)
            changed = changes(before, after)
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except LowMemory:
            res = 'lowmem'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        except UcError:
            res = 'invalid'
        out.write(f'cast | {seed:x} {k} {idx:x} {touch:x} {slot:x} {mx:x} {sp:x} {flags:x} {town:x} {spell.hex()} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
