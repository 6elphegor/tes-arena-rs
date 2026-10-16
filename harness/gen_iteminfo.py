"""Emit test vectors for an item's description (src/use_item.rs
`item_info`, 1194:7CBA, which a right click on the equipment list shows
through 1194:1988) from the original code.

Usage: gen_iteminfo.py ACD_unpacked.EXE OUT [N]

Each case is an item of 13h bytes drawn from a stream (seed = the case's
number): its type (below 18h, within the name lists; now and then a
special one, bit 80h), the flags at
+0Eh (taken; its kind, armour kind, identified bits and worn at random),
a weight at +1 (one of the quarters 2413 lists, or 100h and up with its
bytes swapped), the bytes +3..+5, the condition and its maximum (+6,
+8), the enchantment +0Fh and the attribute +12h (FFh at times). It is
put in the player's first slot (DGROUP 212h) with 12B0 3 and 12B2 0 as
1988 leaves them, and 7CBA near-called with SI on it, ES DGROUP.

  iteminfo | <item hex> | result

Results: "special" (7E44 reached: a special item's box), "stack" (7CFE:
an identified stack's text), "hang", "fault", or the FNV-1a digests of
the changed DGROUP and DATA (3276) bytes.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from emu import Arena, Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream

DATA = 0x3276
SLOT = 0x0212
STOPS = {0x7E44: 'special', 0x7CFE: 'stack'}


def item(seed):
    """The item's 13h bytes. Their layout: type, weight word, +3, +4, +5,
    condition word, maximum word, +0Ah..+0Dh, flags, +0Fh, +10h, +11h,
    +12h."""
    s = stream(seed)
    byte = lambda: next(s)
    b = bytearray(byte() for _ in range(0x13))
    if byte() < 0xD0:
        # Within the name lists (a type far past them names whatever
        # memory holds there).
        b[0] %= 0x18
    else:
        b[0] = 0x80 | (b[0] & 0x1F)
    quarters = (0x40, 0x80, 0xC0)
    w = quarters[byte() % 3] if byte() < 0x80 else (byte() % 40 + 1) << 8 | byte() & 0xC0
    struct.pack_into('<H', b, 1, w)
    mx = byte() << 8 | byte()
    if byte() < 0x40:
        mx &= 0xFF
    struct.pack_into('<H', b, 8, mx)
    struct.pack_into('<H', b, 6, (byte() * mx >> 8) & 0xFFFF)
    b[0x0E] = 0x40 | (b[0x0E] & 0xBF)
    if byte() < 0x60:
        b[0x0F] = 0xFF
    if byte() < 0x80:
        b[0x12] = 0xFF
    else:
        b[0x12] = byte() % 8
    return bytes(b)


def main():
    exe, out = sys.argv[1], sys.argv[2]
    n = int(sys.argv[3]) if len(sys.argv) > 3 else 3000
    a = Arena(exe)
    hit = []

    def stop(mu, addr, size, what):
        hit.append(what)
        mu.emu_stop()
    for off, what in STOPS.items():
        lin = a.lin(LOAD_SEG + 0x1194, off)
        a.mu.hook_add(UC_HOOK_CODE, stop, what, lin, lin)
    base = a.mu.mem_read(0, 0x110000)
    with open(out, 'w') as f:
        for k in range(n):
            it = item(k)
            if not mine(k, n):
                continue
            a.mu.mem_write(0, bytes(base))
            a.mu.mem_write(a.lin(a.ds, SLOT), it)
            a.w16(0x12B0, 3)
            a.w16(0x12B2, 0)
            ds0, d0 = a.seg_bytes(a.ds), a.seg_bytes(LOAD_SEG + DATA)
            hit.clear()
            try:
                a._setup([0xFFF0], {'esi': SLOT})
                a.mu.reg_write(__import__('unicorn').x86_const.UC_X86_REG_CS, LOAD_SEG + 0x1194)
                a.mu.emu_start(a.lin(LOAD_SEG + 0x1194, 0x7CBA), a.lin(LOAD_SEG + 0x1194, 0xFFF0), count=2_000_000)
                if a.fault is not None:
                    raise Fault(a.fault)
                if hit:
                    r = hit[0]
                elif a.mu.reg_read(__import__('unicorn').x86_const.UC_X86_REG_IP) != 0xFFF0:
                    r = 'hang'
                else:
                    r = '%x %x' % (fnv(changes(ds0, a.seg_bytes(a.ds))), fnv(changes(d0, a.seg_bytes(LOAD_SEG + DATA))))
            except (Fault, UcError):
                r = 'fault'
            f.write(f'iteminfo | {it.hex()} | {r}\n')


if __name__ == '__main__':
    main()
