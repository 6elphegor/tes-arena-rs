"""Emit the vectors for the message box (src/textbox.rs: 0000:965A,
`Machine::message_box`) at every kind of place its callers ask for.

Usage: gen_msgbox.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [DUMP_DIR]

With DUMP_DIR each background's record is written there as NAME.bg.

The game started and set up as gen_programstart.py has it (21EE:0000,
21EE:016C), then for each case from that state: the page (A900) filled
with a pattern (byte i: i x 7 + 3), the case's tiles (B038) and font
(B036) chosen, its text written, and 965A far-called with its registers.
One line a case:

  box | NAME | AX BX CX DX DI SEG | TILES FONT | TEXT | DX:AX | <runs> | <digest> | <heap> | <background>

TEXT the text's bytes in hex, at DGROUP 1044 (SEG `ds`) or at 8000h in
the EMS frame's segment ([9030], SEG `frame`); DI FFFFh / FFFEh the heap's,
any other the frame's ([902E]) plus DI. TILES and FONT the DGROUP words
B038 and B036 are given from. What the box changed in DGROUP (runs and
digest, the texts' segment word A846 taken 1000h lower as the port's),
the heap's blocks after, and the background's record (its 8-byte head
and w x h bytes after) as a digest, or `-` without flag 1. A box wider
than the screen (`wide`, x negative) has only its head recorded (`h:`
and the head's digest): the tiles' drawing (9727) writes its rows past
the page with 32-bit offsets, over the background's own buffer, and the
pixels are the front end's.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv
from screens import ScreenArena
from gen_programstart import heap_line

# (name, AX, BX, CX, DX, DI, text segment, tiles from, font from, text)
CASES = [
    ('loading', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b'Loading...'),
    ('two-lines-centred', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b'You find\rnothing here.'),
    ('lf-left', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b'Line one\nand line two\nthree'),
    ('colour-codes', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b'Gold: \t\xd5100\t\xf0 pieces\r'),
    ('font-codes', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b'\x0c\x02Big\r\x0c\x01small one'),
    ('heap-low-at', 0x20, 0x30, 1, 5, 0xFFFF, 'ds', 0x1036, 0x102C, b'At a place'),
    ('heap-high-centred', 0, 0, 7, 0x0A, 0xFFFE, 'ds', 0x1036, 0x102C, b'1234 gold'),
    ('fixed-size', 0x10, 0x14, 0x19, 5, 0xFFFF, 'ds', 0x1036, 0x102C, b'Fixed'),
    ('fixed-size-centred', 0, 0, 0x1F, 4, 0xFFFE, 'ds', 0x1036, 0x102C, b'Fixed\rcentred'),
    ('no-background', 0x30, 0x40, 6, 5, 0xFFFF, 'ds', 0x1036, 0x102C, b'Paused'),
    ('no-background-4006', 0, 0, 0x4006, 5, 0xFFFE, 'ds', 0x1036, 0x102C, b'Across only'),
    ('centred-across-only', 0, 0x50, 0x4003, 5, 0xFFFF, 'ds', 0x1036, 0x102C, b'Across'),
    ('centred-down-only', 0x18, 0, 0x4005, 5, 0xFFFE, 'ds', 0x1036, 0x102C, b'Down'),
    ('marble2', 0, 0, 0x4007, 5, 0, 'ds', 0x103A, 0x102C, b'Other tiles\rhere'),
    ('font-b', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x1028, b'In font B'),
    ('empty', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b''),
    ('cr-cr', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b'A\r\rB'),
    ('ends-cr', 0, 0, 0x4007, 5, 0, 'ds', 0x1036, 0x102C, b'Ends here\r'),
    ('far-text', 0, 0, 0x4007, 5, 0x100, 'frame', 0x1036, 0x102C, b'Far away\rtext'),
    ('far-text-heap', 0x40, 0x10, 1, 3, 0xFFFF, 'frame', 0x1036, 0x102C, b'Far'),
    ('wide', 0, 0, 0x4007, 5, 0xFFFF, 'ds', 0x1036, 0x102C,
     b'The quick brown fox jumps over the lazy dog, twice: the quick brown fox'),
    ('many-lines', 0, 0, 7, 2, 0xFFFE, 'ds', 0x1036, 0x102C, b'1\r22\r333\r4444\r55555\r666666\r7'),
    ('margin-0', 5, 6, 1, 0, 0xFFFF, 'ds', 0x1036, 0x102C, b'Tight'),
    ('odd-margin', 0, 0, 0x4007, 7, 0, 'ds', 0x1036, 0x102C, b'Odd margin'),
    ('right-edge', 0x120, 0xB0, 1, 5, 0xFFFF, 'ds', 0x1036, 0x102C, b'Off the edge'),
]

# 1194:59C8 whole, the text at DGROUP 1044: (name, flags, 5E6, text). The
# flags: `nowait` 5E0 bit 800h set (shown and left), else a click (down,
# then up) dismisses it; `nodissolve` 5CA bit 2. 5E6 4 is a dungeon's.
SHOWN = [
    ('nowait-street', ('nowait',), 0, b'Loading...'),
    ('wait-street', (), 0, b'You can\'t carry\rany more.'),
    ('nowait-dungeon', ('nowait',), 4, b'A dungeon\'s box'),
    ('wait-dungeon', (), 4, b'The door is locked.'),
    ('wait-nodissolve', ('nodissolve',), 0, b'No dissolve'),
    ('wait-dungeon-nodissolve', ('nodissolve',), 4, b'Line\rand line'),
]


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[5] if len(sys.argv) > 5 else None
    a = ScreenArena(EXE, GAME, SAVE)
    a.init_memory()
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    a.w16(0xCB8A, a.r16(0x9030))
    saved = a.stub()
    a.mu.mem_write(a.lin(a.ds, 0x1A80), b'\x40')
    a.mu.mem_write(a.lin(a.ds, 0x1A6C), b'\x40')
    assert a.run_screen(0x21EE, 0x016C) is None, 'the setup waited'
    a.unstub(saved)
    snap = a.snapshot()
    pattern = bytes((i * 7 + 3) & 0xFF for i in range(64000))
    out = open(OUT, 'w')
    for name, ax, bx, cx, dx, di, tseg, tiles, font, text in CASES:
        a.restore(snap)
        a.mu.mem_write(a.lin(a.r16(0xA900), 0), pattern)
        a.w16(0xB038, a.r16(tiles))
        a.w16(0xB036, a.r16(font))
        frame = a.r16(0x9030)
        if tseg == 'ds':
            seg, si = a.ds, 0x1044
        else:
            seg, si = frame, 0x8000
        a.mu.mem_write(a.lin(seg, si), text + b'\0')
        es = frame
        if di not in (0xFFFF, 0xFFFE):
            di = (a.r16(0x902E) + di) & 0xFFFF
        d0 = a.seg_bytes(a.ds)
        regs = dict(ax=ax, bx=bx, cx=cx, dx=dx, di=di, es=es, ds=seg, si=si, fs=0x58, gs=0x2A)
        assert a.run_screen(0, 0x965A, regs) is None, f'{name}: the box waited'
        from unicorn.x86_const import UC_X86_REG_AX, UC_X86_REG_DX
        rax, rdx = a.mu.reg_read(UC_X86_REG_AX), a.mu.reg_read(UC_X86_REG_DX)
        d1 = bytearray(a.seg_bytes(a.ds))
        if struct.unpack_from('<H', d1, 0xA846)[0] == a.ds:
            struct.pack_into('<H', d1, 0xA846, a.ds - 0x1000)
        offs, digest = runs(d0, d1)
        bg = '-'
        if cx & 1:
            bseg, boff = a.r16(0xA84A), a.r16(0xA848)
            w, h = a.r16(boff + 4, seg=bseg), a.r16(boff + 6, seg=bseg)
            rec = bytes(a.mu.mem_read(a.lin(bseg, boff), 8 + w * h))
            bg = f'{fnv(rec):x}' if w <= 320 else f'h:{fnv(rec[:8]):x}'
            if dump:
                open(os.path.join(dump, name + '.bg'), 'wb').write(rec)
        heap = heap_line(a, 'x').split(' | ')[2].strip()
        tname = 'ds' if tseg == 'ds' else 'frame'
        out.write(f'box | {name} | {ax:x} {bx:x} {cx:x} {dx:x} {di:x} {tname} | {tiles:x} {font:x} | {text.hex() or "-"} | '
                  f'{rdx:x}:{rax:x} | {offs} | {digest} | {heap} | {bg}\n')
        out.flush()
    for name, flags, place, text in SHOWN:
        a.restore(snap)
        a.mu.mem_write(a.lin(a.r16(0xA900), 0), pattern)
        a.w16(0xB038, a.r16(0x1036))
        a.w16(0xB036, a.r16(0x102C))
        a.w16(0x05E0, (a.r16(0x05E0) & ~0x800) | (0x800 if 'nowait' in flags else 0))
        a.w16(0x05CA, (a.r16(0x05CA) & ~2) | (2 if 'nodissolve' in flags else 0))
        a.mu.mem_write(a.lin(a.ds, 0x05E6), bytes([place]))
        a.mu.mem_write(a.lin(a.ds, 0x1044), text + b'\0')
        # The first click goes to the wait for the buttons' release before
        # the box (59E5), the second dismisses it.
        a.clicks = [] if 'nowait' in flags else [(160, 100), (160, 100)]
        d0 = a.seg_bytes(a.ds)
        assert a.run_screen(0x1194, 0x59C8, dict(si=0x1044)) is None, f'{name}: the box waited'
        assert not a.clicks, f'{name}: the click was left'
        d1 = bytearray(a.seg_bytes(a.ds))
        if struct.unpack_from('<H', d1, 0xA846)[0] == a.ds:
            struct.pack_into('<H', d1, 0xA846, a.ds - 0x1000)
        offs, digest = runs(d0, d1, MOUSE)
        heap = heap_line(a, 'x').split(' | ')[2].strip()
        ems = fnv(b''.join(a.ems_bank(b) for b in range(0x20)))
        out.write(f'shown | {name} | {" ".join(flags) or "-"} {place:x} | {text.hex()} | {offs} | {digest} | {heap} | {ems:x}\n')
        out.flush()
    out.close()


# The mouse's words 8FC4 writes on 59C8's way (the cursor's x and y, and
# 8FA8): the front end's, left out of the `shown` lines' digests.
MOUSE = {0x8F92, 0x8F93, 0x8F94, 0x8F95, 0x8FA8, 0x8FA9}


def runs(d0, d1, leave=()):
    """What changed from d0 to d1: its offsets as runs, and the digest
    of the bytes after (but those at `leave`)."""
    changed = [i for i in range(0x10000) if d0[i] != d1[i]]
    out = []
    for i in changed:
        if out and out[-1][0] + out[-1][1] == i:
            out[-1][1] += 1
        else:
            out.append([i, 1])
    return ' '.join(f'{s:x}+{n:x}' for s, n in out), f'{fnv(bytes(d1[i] for i in changed if i not in leave)):x}'


if __name__ == '__main__':
    main()
