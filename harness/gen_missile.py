"""Emit test vectors for an arrow shot (src/projectile.rs `fire_missile`,
29CF:037F) from the original code.

Usage: gen_missile.py ACD_unpacked.EXE OUT

A case is gen_projectile.py's setup (a strike case, then `flight`), then
the cursor (8F92, the view's centre C3D6 at A0h) and a shot of flat `flat` from `src` (0, the player
aimed by the cursor, or an actor slot) in place of the launch, then the
frames of 29CF:0441. Stubs and results as gen_projectile.py.

  missile | seed k src mx flat | result
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG, DGROUP
from emu import changes
from gen_attack import setup, fnv, CSEG, RSIZE
from gen_strike import strike, WSEG, FRAME, ACTORS, SIZE
from gen_spell import guard_dispatch
from gen_projectile import flight, SPSEG, TSEG, RELOC


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    for seg, off, code in [(0x23BA, 0x002A, b'\xcb'), (0x1194, 0xA36C, b'\xcb'), (0, 0x9AE6, b'\xcb'),
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

    rng = __import__('random').Random(0x1994_037F)
    # Not sharded (harness/par.py): a case starts from the state the one
    # before it left, so the cases must run in order.
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        k = rng.randrange(8)
        src = 0 if rng.random() < 0.6 else ACTORS + rng.randrange(23) * SIZE
        mx = rng.randrange(0x140)
        flat = 0x1B if rng.random() < 0.8 else rng.randrange(0x60)
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
        before = unrelocated(a.seg_bytes(a.ds))
        wrap.clear()
        try:
            a.w16(0x8F92, mx)
            a._setup([0, STUB_SEG], {'eax': flat, 'ebx': src})
            a._run(0x29CF, 0x037F, STUB_SEG, 0)
            for _ in range(frames):
                a._setup([0, STUB_SEG], {})
                a._run(0x29CF, 0x0441, STUB_SEG, 0)
            if wrap:
                raise Fault(13)
            after = unrelocated(a.seg_bytes(a.ds))
            if a.r16(0x061C, CSEG) == a.ds:
                a.w16(0x061C, DGROUP, CSEG)
            changed = changes(before, after)
            res = f'{fnv(changed):x} {fnv(a.seg_bytes(CSEG)[:0x8000]):x} {fnv(a.seg_bytes(WSEG)[:0x8000]):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        except UcError:
            res = 'invalid'
        out.write(f'missile | {seed:x} {k} {src:x} {mx:x} {flat:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
