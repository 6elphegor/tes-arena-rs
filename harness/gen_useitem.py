"""Emit test vectors for using items (src/use_item.rs): putting an item
on or taking it off (1194:7605, `equip_toggle`), the inventory screen's
click (1194:1AFB, `equip_click`), using an item (29CF:07C3, `item_cast`),
the once-a-day items (2294:1051 `open_charm`, 2294:0FFA `gold_charm`),
the use button (24FA:07CD, `use_item`) and dropping (1194:18F6,
`drop_item`, with 2433:09E1 `drop_to_floor` running).

Usage: gen_useitem.py ACD_unpacked.EXE OUT

Stubbed: the player's cast 29CF:00FA logs (spell, level byte 1AA) at
DGROUP FF00 + [FFE0] (FFE0 += 2); an item breaking 2AA1:021B logs SI at
FF40 + [FFE2] (FFE2 += 2); message boxes 1194:59C8 count at FFE8; the
click's refusals 1194:1BE0 (cant_use) count at FFE4 and 1194:1B78
(clash) at FFE6 then go on to 1BC9; the waits and redraws 1194:5E21,
0000:5454, 0000:9466, 3159:0006 return; the redraw 1194:23B1 only
remakes the armour class (1194:7719); the list 24FA:0B78 returns the
case's pick (written at DGROUP FFDC). The game's INT 3 at 2433:0298 is a no-op, as DOS's handler
makes it. EMS is off; the level's item list is in FRAME.

A case fills from a seed (`setup`, mirrored in tests/useitem.rs) the
player (attributes, class, level, health), the 40 item slots (types,
special ones among them, flags, charges, conditions, prices,
enchantments, spells), the count at 129D, 5CA, the minutes and gold, the
waiting spells, the fonts, the accessories' table at 7F81, the spells'
costs (3276), the inventory screen's state, the player's square and the
level's flags, the level's item count, the RNG's seed; and the case's
slot (and for charms that its day is today).

  useitem | op | seed | result

op: 0 toggle, 1 click, 2 cast, 3 charm, 4 use, 5 drop. Results:
"fault" / "hang", or the FNV-1a digest of the changed DGROUP and FRAME
bytes. The message's segment (61C) is compared image-relative.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from gen_frame import counter

FRAME = 0x6000
DATA = 0x3276
DGROUP = 0x3BA0
OPS = 6


def day_of(minutes):
    return (minutes % 0x7E900) // 0x5A0


def to100(v):
    """1194:5B07 for v >= 0."""
    q, r = divmod(v * 100, 256)
    return q + 1 if r >= 128 else q


def attr_for(p):
    """The smallest attribute byte that is `p` percent."""
    return next((v for v in range(256) if to100(v) == p), 0)


def setup(seed, op, gains):
    """(DGROUP writes, DATA writes, slot, extra). `gains` is DGROUP 4AFE..
    as loaded. Mirrored in tests/useitem.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    dword = lambda: word() | word() << 16
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w, d = [], []
    w.append((0x01CD, bytes(byte() for _ in range(8))))
    w.append((0x01A9, pb(byte())))
    w.append((0x01AA, pb(byte() % 30)))
    w.append((0x020A, pk(word() % 400)))
    w.append((0x020C, pk(word() % 400)))
    occupied = byte() % 41
    spells = []
    for i in range(40):
        b = bytearray(19)
        b[0] = (0x80 + byte() % 0x12) if byte() < 0x50 else byte() % 0x12
        cat = byte() % 4
        f = byte() & 0x33 | cat << 2
        if i < occupied or byte() < 0x10:
            f |= 0x40
        if byte() < 0x60:
            f |= 0x80
        b[0xE] = f
        b[1:3] = pk(word())
        b[3] = byte() % 4
        b[4] = byte()
        b[5] = byte()
        b[6:8] = pk(word() % 0x300)
        b[8:10] = pk(word())
        b[0xA:0xE] = struct.pack('<I', 0 if byte() < 0x40 else dword())
        b[0xF] = 0xFF if byte() < 0x30 else byte()
        b[0x10] = byte()
        v = byte()
        b[0x11] = (0xFF, 0xC1, v)[v % 3] if byte() < 0x40 else v
        b[0x12] = 0xFF if byte() < 0x40 else byte() % 8
        w.append((0x0212 + 0x13 * i, bytes(b)))
        spells.append(b[0x11])
    w.append((0x129D, pb(byte() % 41)))
    w.append((0x05CA, pk(word())))
    minutes = dword()
    w.append((0x05CC, struct.pack('<I', minutes)))
    w.append((0x05C2, struct.pack('<I', dword())))
    for at in (0x0F84, 0x0DC0, 0x0DC2, 0x0DC4, 0x0DC8, 0x1036, 0x102C):
        w.append((at, pk(word())))
    w.append((0x7F81, bytes(byte() % 0x18 for _ in range(0x12))))
    if op != 1:
        for sp in spells:
            base = 0x2A80 if sp & 0x80 else 0
            d.append((base + (sp & 0x7F) * 0x55 + 0x32, pk(word() >> (byte() % 12))))
    w.append((0x1296, pb(byte() % 40)))
    w.append((0x1297, pb(byte())))
    w.append((0x129A, pb(byte() & 1)))
    w.append((0x1299, pb(0 if byte() < 0xC0 else 1)))
    w.append((0x1270, pk(0 if byte() < 0xE0 else word())))
    w.append((0x126C, pk(word() % 12)))
    w.append((0x050A, pb(byte() % 41)))
    w.append((0xC3D0, pk(word())))
    w.append((0xC3D4, pk(word())))
    w.append((0xC39A, pk(word())))
    w.append((0xC398, pk(word())))
    w.append((0x05E6, pb(byte() % 6)))
    w.append((0x0D48, pb(byte() % 12)))
    w.append((0xBB0A, pb(byte())))
    w.append((0x0FF2, pb(byte() % 40)))
    w.append((0x9030, pk(FRAME)))
    w.append((0x8FA6, pb(0)))
    w.append((0x91F6, struct.pack('<I', dword())))
    slot = byte() % 40
    extra = byte()
    if op == 3 and extra & 1:
        w.append((0x0212 + 0x13 * slot + 1, pk(day_of(minutes))))
    # Aimed cases, written over the rest, at the edges the random ones
    # rarely reach.
    aim, a1, a2 = byte() % 4, byte(), byte()
    si = 0x0212 + 0x13 * slot
    other = 0x0212 + 0x13 * (1 if slot == 0 else 0)
    item = lambda at, ty, flags: [(at, pb(ty)), (at + 0xE, pb(flags))]
    if op == 0 and aim == 1:
        # Putting on an enchantment, the sum at 63h..65h percent.
        attr = a2 % 8
        w += item(si, a1 % 0x12, 0x41 | ((a1 >> 6) & 1) << 2)
        w += [(si + 0xF, pb(0)), (si + 0x11, pb(0)), (si + 0x12, pb(attr))]
        w.append((0x01CD + attr, pb(attr_for(0x59 + (a2 >> 3) % 3))))
    elif op == 0 and aim == 2:
        # A kind-1 accessory: its type's gain (type 4's is FFh, wrapping).
        t, attr = a1 % 5, a2 % 8
        gain = gains[t]
        p = a2 % 0x64 if gain == 0xFF else max(0, 0x63 + (a2 >> 3) % 3 - gain)
        w += item(si, t, 0x41 | 0x08 | 0x10)
        w += [(si + 0xF, pb(0)), (si + 0x11, pb(0)), (si + 0x12, pb(attr))]
        w.append((0x01CD + attr, pb(attr_for(p))))
    elif op == 0 and aim == 3:
        # A two-handed weapon put on with a special one worn.
        w += item(si, a1 % 0x12, 0x41 | 0x04) + [(si + 3, pb(2))]
        w += item(other, 0x8D + a2 % 3, 0xC0 | 0x04)
        w.append((0x129D, pb(40)))
    elif op == 1 and aim == 1:
        # An accessory clashing (or not) with one worn of its type.
        t = a1 % 4
        w.append((0x1270, pk(0)))
        w += item(si, t, 0x41 | 0x08 | (a2 % 4) << 4)
        w += item(other, t, 0xC1 | 0x08 | ((a2 >> 2) % 4) << 4)
    elif op == 2 and aim == 1:
        # An accessory of kind 1..3 whose divisor is 0.
        t = a1 % 4
        w += item(si, t, 0x41 | 0x08 | (1 + a2 % 3) << 4)
        w.append((0x7F81 + t, pb(0)))
    elif op == 2 and aim in (2, 3):
        # The condition at the cost's share, less 1, equal or more 1.
        cost = 15 * (a2 % 50) + a1 % 15
        ty = (0x80 + a1 % 0x0D) if aim == 3 else a1 % 0x12
        share = cost >> 1 if aim == 3 else cost // 15
        w += item(si, ty, 0x40 | ((a1 >> 7) & 1) << 2)
        w += [(si + 0x11, pb(a1)), (si + 6, pk(share + (a2 >> 6) % 3 - 1))]
        d.append(((0x2A80 if a1 & 0x80 else 0) + (a1 & 0x7F) * 0x55 + 0x32, pk(cost)))
    elif op == 3 and aim == 1:
        # The gold charm's condition about the roll (10..100).
        w.append((si + 6, pk(a1 % 0x70)))
    elif op == 5 and aim in (1, 2):
        # The first line about the list's limits.
        c, lines = a1 % 41, a2 % 12
        ax = c + lines - 7
        delta = (-1, 0, 1, 5, 6, 7)[(a1 >> 5) % 6]
        w += [(0x129A, pb(0)), (0x1299, pb(0)), (0x050A, pb(c)), (0x126C, pk(lines)), (0x1297, pb(ax + delta))]
    elif op == 5 and aim == 3:
        # Indoors in a building, by its kind.
        w += [(0xC39A, pk(0)), (0xC398, pk(0)), (0x05E6, pb(4)), (0x0D48, pb((8, 0x0B, 1, 3, 2, 5)[a1 % 6]))]
    return w, d, slot, extra


def stub_log(index, base, reg_store):
    """push ax; push bx; mov bx,[index]; <store at bx+base>; add word
    [index],2; pop bx; pop ax; retf"""
    return (bytes([0x50, 0x53, 0x8B, 0x1E, index & 0xFF, index >> 8]) + reg_store(base)
            + bytes([0x83, 0x06, index & 0xFF, index >> 8, 0x02, 0x5B, 0x58, 0xCB]))


def store_cast(base):
    # mov [bx+base], al; mov ah, [1AA]; mov [bx+base+1], ah
    return bytes([0x88, 0x87, base & 0xFF, base >> 8, 0x8A, 0x26, 0xAA, 0x01,
                  0x88, 0xA7, (base + 1) & 0xFF, (base + 1) >> 8])


def store_si(base):
    # mov [bx+base], si
    return bytes([0x89, 0xB7, base & 0xFF, base >> 8])


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    a.GAME_INT3 = a.GAME_INT3 | {(0x2433, 0x0299)}
    rel = (0x7719 - (0x23B1 + 3)) & 0xFFFF
    clash = bytes([0xFF, 0x06, 0xE6, 0xFF, 0xEB, (0x1BC9 - (0x1B78 + 6)) & 0xFF])
    for seg, off, code in [
        (0x29CF, 0x00FA, stub_log(0xFFE0, 0xFF00, store_cast)),
        (0x2AA1, 0x021B, stub_log(0xFFE2, 0xFF40, store_si)),
        (0x1194, 0x59C8, counter(0xFFE8, True)),
        (0x1194, 0x1BE0, counter(0xFFE4, False)),
        (0x1194, 0x1B78, clash),
        (0x1194, 0x5E21, b'\xc3'),
        (0x1194, 0x23B1, bytes([0xE8, rel & 0xFF, rel >> 8, 0xC3])),
        (0, 0x5454, b'\xcb'), (0, 0x9466, b'\xcb'), (0x3159, 0x0006, b'\xcb'),
        # The list's pick, from DGROUP FFDC (mov ax, [FFDC]; retf): code
        # rewritten between runs isn't seen by unicorn's translated blocks.
        (0x24FA, 0x0B78, bytes([0xA1, 0xDC, 0xFF, 0xCB])),
    ]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    base_ds = a.seg_bytes(a.ds)
    base_data = a.seg_bytes(LOAD_SEG + DATA)
    gains = base_ds[0x4AFE:0x4B10]
    zero = bytes(0x10000)
    rng = random.Random(0x1994_07C3)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        op = n % OPS
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(a.lin(LOAD_SEG + DATA, 0), base_data)
        a.mu.mem_write(a.lin(FRAME, 0), zero)
        w, d, slot, extra = setup(seed, op, gains)
        for off, data in w:
            a.mu.mem_write(a.lin(a.ds, off), data)
        for off, data in d:
            a.mu.mem_write(a.lin(LOAD_SEG + DATA, off), data)
        si = 0x0212 + 0x13 * slot
        pick = 0xFFFF if extra >= 0xF0 else extra % 12
        a.w16(0xFFDC, pick)
        # call_far leaves the general registers as the last case left them.
        regs = dict(eax=0, ebx=0, ecx=0, edx=0, esi=si, edi=0, ebp=0, fs=a.ds, gs=a.ds)
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                a.call_near(0x1194, 0x7605, si=si)
            elif op == 1:
                a.call_far(0x1194, 0x1AFB, **dict(regs, edx=slot))
            elif op == 2:
                a.call_far(0x29CF, 0x07C3, **regs)
            elif op == 3:
                a.call_far(0x2294, 0x1051 if extra & 2 else 0x0FFA, **regs)
            elif op == 4:
                a.call_near(0x24FA, 0x07CD)
            else:
                a.call_far(0x1194, 0x18F6, **regs)
            after = bytearray(a.seg_bytes(a.ds))
            if struct.unpack_from('<H', after, 0x61C)[0] == LOAD_SEG + DGROUP and before[0x61C:0x61E] != after[0x61C:0x61E]:
                struct.pack_into('<H', after, 0x61C, DGROUP)
            res = f'{fnv(changes(before, bytes(after)) + changes(zero, a.seg_bytes(FRAME))):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'useitem | {op} | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
