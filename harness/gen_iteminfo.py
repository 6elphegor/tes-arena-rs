"""Emit test vectors for an item's description (src/use_item.rs
`item_info`, 1194:7CBA, which a right click on the equipment list shows
through 1194:1988) from the original code.

Usage: gen_iteminfo.py ACD_unpacked.EXE GAME_DIR OUT [N]

Runs under the DOS file layer (harness/dos.py) with the user's
POINTER1.DAT and TEMPLATE.DAT, the heap set up and EMS on with the frame
at FRAME (as gen_text.py), so that an identified stack's text (7CF8,
1194:A36C mode 2) and a special item's box (7E44: its text appended in a
block 9CD2 allocates, shown by 1194:59CC, freed) run as they are. The
box is stubbed: it records the colour 9288 and the text at DS:SI.

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

Results: "hang" (the text's key not in the index, 1194:A3EE, searched for
for ever), "fault", or the FNV-1a digests of the changed DGROUP
and DATA (3276) bytes, the carry (1: the box shown, 1988 draws nothing)
and the box's colour and text ("-" without one).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_DS, UC_X86_REG_EFLAGS, UC_X86_REG_IP, UC_X86_REG_SI
from dos import DosArena
from emu import Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream
from gen_frame import RET

DATA = 0x3276
SLOT = 0x0212
FRAME = 0x8800


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
    exe, game, out = sys.argv[1], sys.argv[2], sys.argv[3]
    n = int(sys.argv[4]) if len(sys.argv) > 4 else 3000
    a = DosArena(exe, game)
    a.patch(0x1194, 0x59CC, RET)
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.enable_ems(FRAME)
    shown = []

    def box(mu, addr, size, _):
        ds, si = mu.reg_read(UC_X86_REG_DS), mu.reg_read(UC_X86_REG_SI)
        t = bytearray()
        while len(t) < 0x7530:
            c = mu.mem_read(a.lin(ds, (si + len(t)) & 0xFFFF), 1)[0]
            if c == 0:
                break
            t.append(c)
        shown.append(bytes([a.r16(0x9288) & 0xFF]) + bytes(t))
    lin = a.lin(LOAD_SEG + 0x1194, 0x59CC)
    a.mu.hook_add(UC_HOOK_CODE, box, None, lin, lin)
    # 1194:A3EE: the text's key searched for in the index, for ever when
    # it isn't there (a stack type past the potions' texts).
    spins = [0]

    def spin(mu, addr, size, _):
        spins[0] += 1
        if spins[0] > 0x40000:
            mu.emu_stop()
    lin = a.lin(LOAD_SEG + 0x1194, 0xA3EE)
    a.mu.hook_add(UC_HOOK_CODE, spin, None, lin, lin)
    base = a.snapshot()
    with open(out, 'w') as f:
        for k in range(n):
            it = item(k)
            if not mine(k, n):
                continue
            a.restore(base)
            a.mu.mem_write(a.lin(a.ds, SLOT), it)
            a.w16(0x12B0, 3)
            a.w16(0x12B2, 0)
            a.w16(0x91F0, 0)
            ds0, d0 = a.seg_bytes(a.ds), a.seg_bytes(LOAD_SEG + DATA)
            shown.clear()
            spins[0] = 0
            try:
                a._setup([0xFFF0], {'esi': SLOT})
                a.mu.reg_write(UC_X86_REG_CS, LOAD_SEG + 0x1194)
                a.fault = None
                a.mu.emu_start(a.lin(LOAD_SEG + 0x1194, 0x7CBA), a.lin(LOAD_SEG + 0x1194, 0xFFF0), count=100_000_000)
                if a.fault is not None:
                    raise Fault(a.fault)
                if a.mu.reg_read(UC_X86_REG_IP) != 0xFFF0:
                    r = 'hang'
                else:
                    # A text's substitutions keep DGROUP's segment at
                    # A90A: the image's, as the port's (gen_text.py).
                    if a.r16(0xA90A) == a.ds:
                        a.w16(0xA90A, a.ds - LOAD_SEG)
                    carry = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
                    b = '%x' % fnv(shown[0]) if shown else '-'
                    r = '%x %x %d %s' % (fnv(changes(ds0, a.seg_bytes(a.ds))), fnv(changes(d0, a.seg_bytes(LOAD_SEG + DATA))), carry, b)
            except (Fault, UcError):
                r = 'fault'
            f.write(f'iteminfo | {it.hex()} | {r}\n')


if __name__ == '__main__':
    main()
