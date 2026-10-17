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

Then 1194:59C8 whole (`Machine::message_shown`, SHOWN), one line a case:

  shown | NAME | FLAGS 5E6 | TEXT | <runs> | <digest> | <heap> | <ems>

what it changed in DGROUP (the digest without the mouse's words, MOUSE),
the heap's blocks and the 32 EMS banks' digest after.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv
from screens import ScreenArena
from gen_programstart import heap_line
from emu import LOAD_SEG

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


# 1194:4800 (near), its field (0000:9EE5) stubbed to return at once: (name,
# AX, BX, CX, text). The text at DGROUP 1044, the field's buffer at 1144
# (filled with FFh first, for its clearing).
PROMPTS = [
    ('rest', 0xFC, 0x44, 3, b'How many hours do you wish to rest?'),
    ('repair', 0xF8, 0x62, 4, b'How much will you\rpay for it?'),
    ('high-ah', 0x1FC, 0x44, 3, b'AH kept'),
    ('no-clear', 0x10, 0x20, 0, b'Nothing cleared'),
    ('colours', 0x30, 0x40, 6, b'\t\xd5Gold\t\xf0 asked'),
]

# 1194:5C30 (far), its field stubbed: (name, AX, CX, flags, text) with the
# text at 1044 (its copy formatted over it, as the creation's call has
# it) or `other` at 1300; flags `nodissolve` 5CA bit 2.
NAMED = [
    ('search', 0x55, 0x14, (), b'Enter the name of the city.'),
    ('creation', 0x3C, 0x12, (), b'What will be thy name,\rLord?'),
    ('centred', 0xFFFF, 8, (), b'Centred field'),
    ('nodissolve', 0x40, 4, ('nodissolve',), b'No dissolve'),
    ('other', 0x50, 6, ('other',), b'From elsewhere'),
    ('long', 0x10, 4, (), b'\r'.join([b'0123456789012345678901234567890'] * 9)),
]

# 2542:04B1 (near) to its end, dismissed by a click: (name, writes) over
# the started game's DGROUP.
STATUS = [
    ('plain', []),
    ('conditions', [(0x01A4 + 0x40D, b'\x26\x02'), (0x05CC, struct.pack('<I', 123457))]),
]

# 1194:020E (far): (name, mouse, title, entries, keys, action, flags) with
# the title at 1044, the entries at 1100 on (their table at 1180, a 0
# after), the keys at 1170. The action at the menu's wait: ('click', x,
# y), ('rclick', x, y) or ('keys', [scan]); none for a menu that gives up.
MENUS = [
    ('click-first', (160, 100), b'Pick one', [b'First', b'Second'], b'FS', ('click', 160, 0x60), ()),
    ('click-second', (160, 100), b'Pick one', [b'First', b'Second'], b'FS', ('click', 160, 0x72), ()),
    ('click-above', (160, 100), b'Pick one', [b'First', b'Second'], b'FS', ('click', 160, 0x10), ()),
    ('key-second', (160, 100), b'Pick one', [b'First', b'Second'], b'FS', ('keys', [0x1F]), ()),
    ('rclick', (160, 100), b'Pick one', [b'First', b'Second'], b'FS', ('rclick', 160, 0x60), ()),
    ('mouse-left', (40, 20), b'Rest', [b'Camp', b'Until healed', b'Cancel'], b'CUX', ('click', 160, 0x60), ()),
    ('many', (160, 100), b'A longer title\rof two lines',
     [b'One', b'Two', b'Three', b'Four', b'Five', b'Six'], b'OTHFIS', ('click', 160, 0x80), ()),
    ('codes', (160, 100), b'\t\xd5Coloured', [b'\x0c\x02Big one', b'\t\xf0Small'], b'BS', ('click', 160, 0x70), ()),
    ('nodissolve', (160, 100), b'Yes or no?', [b'Yes', b'No'], b'YN', ('click', 160, 0x60), ('nodissolve',)),
    ('too-wide', (160, 100), b'W' * 60, [b'Yes', b'No'], b'YN', None, ()),
]


def stack_bx(a, at):
    """A hook keeping BX at each 965A called from `at` (seg, off of the
    call): the formatter's leftover, an offset on the stack."""
    from unicorn import UC_HOOK_CODE
    from unicorn.x86_const import UC_X86_REG_BX, UC_X86_REG_SS, UC_X86_REG_SP
    from emu import LOAD_SEG
    seen = []

    def hook(mu, addr, size, _):
        ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        if (a.r16(sp + 2, seg=ss) - LOAD_SEG, (a.r16(sp, seg=ss) - 5) & 0xFFFF) == at:
            seen.append(mu.reg_read(UC_X86_REG_BX))
    lin = a.lin(LOAD_SEG, 0x965A)
    return seen, a.mu.hook_add(UC_HOOK_CODE, hook, None, lin, lin)


def state_line(a, d0, leave=None):
    """What changed in DGROUP since d0 (the box's segment words A846 and
    A84A taken 1000h lower where they are DGROUP's, as the port's), its
    digest without the mouse's words (or those at `leave`), the heap's
    blocks and the EMS banks' digest."""
    d1 = bytearray(a.seg_bytes(a.ds))
    for w in (0xA846, 0xA84A):
        if struct.unpack_from('<H', d1, w)[0] == a.ds:
            struct.pack_into('<H', d1, w, a.ds - 0x1000)
    offs, digest = runs(d0, d1, MOUSE if leave is None else leave)
    heap = heap_line(a, 'x').split(' | ')[2].strip()
    ems = fnv(b''.join(a.ems_bank(b) for b in range(0x20)))
    return f'{offs} | {digest} | {heap} | {ems:x}'


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
    a.waits = {(0, 0x8FC4)}
    field = a.lin(LOAD_SEG, 0x9EE5)

    def start(name):
        a.restore(snap)
        a.mu.mem_write(a.lin(a.r16(0xA900), 0), pattern)
        a.w16(0xB038, a.r16(0x1036))
        a.w16(0xB036, a.r16(0x102C))
        a.mouse, a.buttons, a.clicks, a.script = (160, 100), 0, [], []

    for name, ax, bx, cx, text in PROMPTS:
        start(name)
        a.code_write(field, b'\xcb')
        a.mu.mem_write(a.lin(a.ds, 0x1044), text + b'\0')
        a.mu.mem_write(a.lin(a.ds, 0x1144), b'\xff' * 8)
        d0 = a.seg_bytes(a.ds)
        regs = dict(ax=ax, bx=bx, cx=cx, si=0x1044, di=0x1144, es=a.ds)
        r = a.run_screen(0x1194, 0x4800, regs, near=True)
        assert r is None, f'{name}: the prompt waited {r}'
        out.write(f'prompt | {name} | {ax:x} {bx:x} {cx:x} | {text.hex()} | {state_line(a, d0)}\n')
        out.flush()
    for name, ax, cx, flags, text in NAMED:
        start(name)
        a.code_write(field, b'\xcb')
        si = 0x1300 if 'other' in flags else 0x1044
        a.mu.mem_write(a.lin(a.ds, si), text + b'\0')
        a.w16(0x05CA, (a.r16(0x05CA) & ~2) | (2 if 'nodissolve' in flags else 0))
        d0 = a.seg_bytes(a.ds)
        seen, h = stack_bx(a, (0x1194, 0x5C61))
        regs = dict(ax=ax, cx=cx, si=si, di=0x1144, es=a.ds)
        try:
            assert a.run_screen(0x1194, 0x5C30, regs) is None, f'{name}: the prompt waited'
        finally:
            a.mu.hook_del(h)
        out.write(f'named | {name} | {ax:x} {cx:x} {" ".join(flags) or "-"} | {text.hex()} | {seen[0]:x} | {state_line(a, d0)}\n')
        out.flush()
    for name, writes in STATUS:
        start(name)
        a.code_write(field, bytes(a.mu.mem_read(field, 1)))
        for off, data in writes:
            a.mu.mem_write(a.lin(a.ds, off), data)
        a.script = [('click', 160, 100)]
        d0 = a.seg_bytes(a.ds)
        seen, h = stack_bx(a, (0x2542, 0x0526))
        try:
            assert a.run_screen(0x2542, 0x04B1, {}, near=True) is None, f'{name}: the status waited'
        finally:
            a.mu.hook_del(h)
        assert not a.script, f'{name}: the click was left'
        w = ' '.join(f'{off:x}:{data.hex()}' for off, data in writes) or '-'
        out.write(f'status | {name} | {w} | {seen[0]:x} | {state_line(a, d0)}\n')
        out.flush()
    from unicorn.x86_const import UC_X86_REG_AX
    for name, mouse, title, entries, keys, action, flags in MENUS:
        start(name)
        a.mouse = mouse
        a.mu.mem_write(a.lin(a.ds, 0x1044), title + b'\0')
        table, at = [], 0x1100
        for e in entries:
            a.mu.mem_write(a.lin(a.ds, at), e + b'\0')
            table.append(at)
            at += len(e) + 1
        a.mu.mem_write(a.lin(a.ds, 0x1180), struct.pack(f'<{len(table) + 1}H', *table, 0))
        a.mu.mem_write(a.lin(a.ds, 0x1170), keys + b'\0')
        a.w16(0x05CA, (a.r16(0x05CA) & ~2) | (2 if 'nodissolve' in flags else 0))
        a.script = [action] if action else []
        d0 = a.seg_bytes(a.ds)
        assert a.run_screen(0x1194, 0x020E, dict(si=0x1044, bx=0x1170, di=0x1180)) is None, f'{name}: the menu waited'
        assert not a.script and not a.clicks, f'{name}: the action was left'
        assert [a.lin(a.r16(o + 2), a.r16(o)) for o in (0x8F80, 0x8F88)] == [a.lin(a.ds, 0xEDB0), a.lin(a.ds, 0xEBB0)], 'the cursor\'s buffers'
        pick = a.mu.reg_read(UC_X86_REG_AX)
        act = '-' if action is None else ' '.join(str(v) if isinstance(v, str) else
                                                    ','.join(f'{k:x}' for k in v) if isinstance(v, list) else f'{v:x}'
                                                    for v in action)
        out.write(f'menu | {name} | {mouse[0]:x} {mouse[1]:x} {" ".join(flags) or "-"} | {title.hex()} | '
                  f'{",".join(e.hex() for e in entries)} | {keys.hex()} | {act} | {pick:x} | {state_line(a, d0, MOUSE | CURSOR)}\n')
        out.flush()
    out.close()


# The mouse's words 8FC4 writes on 59C8's way (the cursor's x and y, and
# 8FA8): the front end's, left out of the `shown` lines' digests.
MOUSE = {0x8F92, 0x8F93, 0x8F94, 0x8F95, 0x8FA8, 0x8FA9}


# The cursor 3159:0F2A's wait draws (0000:5454) and takes away (541E): its
# place (8F7C, 8F7E), size (8F8E, 8F90) and 8F2A's return (8F8C), the
# screen under it kept at [8F80] (DGROUP EDB0h, by a segment of its own) and
# its picture at [8F88] (EBB0h): the front end's, left out of the `menu`
# lines' digests.
CURSOR = set(range(0x8F7C, 0x8F80)) | set(range(0x8F8C, 0x8F92)) | set(range(0xEBB0, 0xECB0)) | set(range(0xEDB0, 0xEEB0))


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
