"""Emit the original's world frames, run on from the user's saved game, for
view/src/worldtest.rs: what its 2D drawing leaves on the page in play
(the interface, the compass, the bars over the 3D view) and where the
player goes, frame by frame, under keys held.

Usage: gen_worldframes.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [NAME...]

The game is loaded from the title and taken into the dungeon made from
gen_dungeonscreens.py's seed as that does (`dungeon`), or into the main
quest's Crypt of Hearts (`crypt`). Its world loop is
then run on frame by frame (harness/screens.py `world_frames`: the timer
ticking at each of the loop's reads of it, nothing else advancing it),
the keys of each case of CASES held through the key interrupt
(`keys_held`) as a player holds them. The arrow cursor the mouse's
regions show isn't drawn (CURSOR_DRAW).

For each case, at the top of each frame run, the player's state:

  track | NAME | <frame> <level BB0A> <x BA62> <height BA64> <y BA66> <heading C3DA>

(hex; the place the physics moves, BA62..BA67, not the eye's C3D0..C3D5,
which the frame moves back along the heading after them). Then the page
as the frame after the case's last leaves it, without what the 3D
drawing put there: a pixel is the 3D view's when the last write to it in
that frame was made in the frame's drawing stages (BA4B 3..9: the floor
and ceiling, the sky, the walls, the flats). The rest (the interface, the
compass, the bars, the message line) the 2D drawing left:

  screen | NAME | <page digest> <palette digest> <mask digest>

as gen_screens.py writes a box's (the mask a byte a pixel, 1 the 2D
drawing's; the page's 3D pixels 0). With --dump DIR, NAME.page,
NAME.mask, NAME.dac and NAME.full (the page with its 3D) for
screentest's diff images (game pictures: never commit them).

and the 3D view itself, the page's pixels the drawing stages wrote last
(the mask's 0s; the others 0), for the engine's own drawing of it
(src/render.rs):

  view3d | NAME | <digest>

and the same from row 1 on (row 0 left 0): a ceiling's topmost row reads
one entry past its row table (0000:DB78's), the next heap block's header,
whose words are the original's heap's segments, which no other layout has:

  view3d1 | NAME | <digest>

First, DGROUP as the title's Load leaves it (the offsets
tests/vectors/loadexclude.txt names zeroed; the port's taken to the
harness's form, its image's segments 1000h higher: worldtest's
`load_canonical`):

  loadstate | <digest>

and the EMS banks 0..1Fh as it leaves them (64K each, end to end):

  loadems | <digest>

What the case's frames changed in DGROUP, from the case's start (its
set-up done) to its end (the offsets tests/vectors/stateexclude.txt
names left out: the harness's ways, the heaps' layouts): the offsets as
runs, and the digest of their bytes after:

  state | NAME | <start+length ...> | <digest>

and the same from the load (the place's entry counted too; the offsets
tests/vectors/stateloadexclude.txt names left out as well: the heaps'
segments made on the way in, scratch the load and the entry leave):

  stateload | NAME | <start+length ...> | <digest>

What the case's frames changed in the EMS banks 0..1Fh (64K each, end to
end), from the case's start: how many bytes, the digest of their offsets
(four bytes each, little-endian), the digest of their bytes after:

  stateems | NAME | <count> <offsets' digest> | <digest>

The colours at each wait for the retrace on the way, when they change
more than once, as gen_screens.py's:

  screenfade | NAME | <count> | <digest>

What each of the case's drawings (0000:A31D, its first stage) made its
row tables from: the inputs as the last call of 0000:DA8E found them
(`rows_watch`; the tables stay as that made them until the next, the
state since changed or not), each as eleven words (ROW_INPUTS, of the
flags only the bits DA8E reads), their digest:

  rows | NAME | <drawings> <digest>

A case that printed (the logbook's Print), what PRN took, its length
and digest:

  printed | NAME | <length> <digest>
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, STUB_SEG, fnv
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_FS, UC_X86_REG_SI
import unicorn.x86_const as x86
from screens import ScreenArena, PAGE_SEG_FIELD
import gen_dungeonscreens as gd
import gen_screens as gs

UP = 0x48
# The world loop's stages (BA4B) that draw the 3D view.
DRAWING = range(3, 10)
# Where the frame body draws the arrow cursor (0000:4C8B: 5A66 with the
# cursor 7A48.. the controls set, unless 7A48 is FFFFh): the original's
# mouse regions, which the view's own controls (the mouse captured) don't
# show. Here it is taken as not shown.
CURSOR_DRAW = 0x4C8B


def at_square(row, col, heading):
    """The player put at the middle of a square, facing `heading`: the
    place the physics moves (BA62..) and the eye (C3D0..) both."""
    def put(a):
        for base in (0xC3D0, 0xBA62):
            a.w16(base, (col << 7) + 0x40)
            a.w16(base + 4, (row << 7) + 0x40)
        a.w16(0xC3DA, heading)
    return put


def facing(heading):
    def put(a):
        a.w16(0xC3DA, heading)
    return put


def at_hour(hour, heading, minute=0):
    """Facing `heading` at `hour`:`minute` of the save's day: the minutes
    (05CC), the hour (05D6) and the minute (05D8) set to it, the sky's
    time (CCFE) as 0000:16F7 makes it from them."""
    def put(a):
        a.w16(0xC3DA, heading)
        m = struct.unpack('<I', a.mu.mem_read(a.lin(a.ds, 0x05CC), 4))[0]
        m = m - m % 1440 + hour * 60 + minute
        a.mu.mem_write(a.lin(a.ds, 0x05CC), struct.pack('<I', m))
        a.w16(0x05D6, hour)
        a.w16(0x05D8, minute)
        a.w16(0xCCFE, ((hour * 60 + minute) << 10) // 1440)
    return put


def night(heading):
    """Facing `heading` at ten at night of the save's day (`at_hour`)."""
    return at_hour(22, heading)


def storm(heading, angle):
    """`night(heading)` in a thunderstorm with a bolt forced: the weather
    rain (BB09 1), 5C8 bit 1 set, and a bolt begun as 2294:06B6..06FB
    begins one: the first of the six images (D8AE), its frame 0 (43C6),
    its place on the circle at `angle` (43C2, 43C4: 2710h times the sine
    table's words at B03C and B13C, high word doubled)."""
    def put(a):
        night(heading)(a)
        a.mu.mem_write(a.lin(a.ds, 0xBB09), b'\x01')
        a.w16(0x05C8, a.r16(0x05C8) | 1)
        a.w16(0x43C8, a.r16(0xD8AE))
        a.w16(0x43C6, 0)
        for at, table in ((0x43C2, 0xB03C), (0x43C4, 0xB13C)):
            v = struct.unpack('<h', struct.pack('<H', a.r16(table + 2 * angle)))[0]
            a.w16(at, (((0x2710 * v) >> 16) * 2) & 0xFFFF)
    return put


def volcanic(heading, dx, dy):
    """Facing `heading` with the location moved to province 3 at
    (84h + `dx`, 34h + `dy`) and the sky's objects made again
    (2294:08B9, the counts at 42B9 and 438C cleared first, as 0000:027A
    clears them): the volcano seen near (volcano2.dfa) or far
    (volcano3.img) by its distance."""
    def put(a):
        a.mu.mem_write(a.lin(a.ds, 0x0A46), b'\x03')
        a.w16(0x0A48, 0x84 + dx)
        a.w16(0x0A4A, 0x34 + dy)
        a.mu.mem_write(a.lin(a.ds, 0x42B9), b'\x00')
        a.mu.mem_write(a.lin(a.ds, 0x438C), b'\x00')
        a.call_far(0x2294, 0x08B9)
        a.w16(0xC3DA, heading)
    return put


# How far along +z (north) of the dungeon's start the ghost is put.
GHOST_AHEAD = 0x100


def ghost(a):
    """A ghost (type Ch, 1194:8CEE's remapped: CCC2's rows E00h and F00h
    9Eh) brought by a quest's encounter (1194:89A7: one at level 1,
    placed near the player), put GHOST_AHEAD north of the start's place
    (down the corridor there) and faced (heading 0): drawn through the
    light table (0000:953E, 78A8's rows by 9546)."""
    a.call_near(0x1194, 0x89A7, cx=1, ax=0xC, bx=1)
    a.w16(0x0C57 + 4, a.r16(0xC3D4) + GHOST_AHEAD)
    a.w16(0x0C57, a.r16(0xC3D0))
    a.w16(0xC3DA, 0)


def brought(ty, level=1, ahead=0x180, heading=0):
    """A creature of type `ty` at `level` brought by a quest's encounter
    (1194:89A7, as `ghost`), put `ahead` north of the player's place
    (down the start's corridor), the player facing `heading` (0: it)."""
    def put(a):
        a.call_near(0x1194, 0x89A7, cx=1, ax=ty, bx=level)
        a.w16(0x0C57 + 4, (a.r16(0xC3D4) + ahead) & 0xFFFF)
        a.w16(0x0C57, a.r16(0xC3D0))
        a.w16(0xC3DA, heading)
    return put


def exit_stairs(a):
    """South of the first level's up stairs facing them (the dungeon's
    start square), the ways back from the way out's travel map (0000:457C,
    45A4) counted from here for `exit_asked`."""
    at_square(11, 42, 0x100)(a)
    if not hasattr(a, 'exits_back'):
        a.exits_back = 0
        def back(*_):
            a.exits_back += 1
        for o in (0x457C, 0x45A4):
            a.mu.hook_add(UC_HOOK_CODE, back, None, a.lin(LOAD_SEG, o), a.lin(LOAD_SEG, o))
    a.exits_mark = a.exits_back


def exit_asked(a):
    """The way out's travel map shown and the world given back after it
    (the frame it was in run to its end: the next frame's top)."""
    return a.exits_back != a.exits_mark


def stairs_fight(a):
    """South of the first level's down stairs facing them (`playdown`'s
    place), a creature (type 1) brought two squares behind."""
    at_square(11, 10, 0x100)(a)
    brought(1, ahead=0x100, heading=0x100)(a)


def casting(ty, spell, level=1, ahead=0x200):
    """`brought(ty, level, ahead)`, then spell `spell` of the spellbook
    cast as its pick casts it (29CF:071F, AL the index, SI the player's
    record): an aimed spell waits for its target (0620)."""
    def put(a):
        brought(ty, level, ahead)(a)
        a._setup([0, STUB_SEG], {'eax': spell, 'esi': 0x1A4})
        a.mu.reg_write(UC_X86_REG_FS, a.ds)
        a._run(0x29CF, 0x071F, STUB_SEG, 0, count=10_000_000)
        assert a.r16(0x0620), 'the spell not waiting for its target'
    return put


# A seed whose next roll is 0: 99B5h (high word 0) stepped back once
# through the LCG (0000:6E10's 6D002Dh, inverted mod 2^32).
ENCOUNTER_SEED = 0x99B5 * pow(0x6D002D, -1, 1 << 32) % (1 << 32)


def night_encounter(heading):
    """A minute before ten at night (`at_hour`), the clock's minute due on
    the first frame (884A 91 ticks back: due on the first frame here and
    in the view, whose frames tick before the frame, not at its waits)
    and the seed ENCOUNTER_SEED: the hour's encounter check (0000:1622 ->
    1194:887A, its roll the seed's next, nothing rolled before it in the
    frame) comes up and brings the wilderness's night encounter (89D4) as
    play does."""
    def put(a):
        at_hour(21, heading, minute=59)(a)
        a.w16(0x884A, (a.r16(0x0151) - 0x5B) & 0xFFFF)
        a.mu.mem_write(a.lin(a.ds, 0x91F6), struct.pack('<I', ENCOUNTER_SEED))
    return put


def resting(heading):
    """At ten at night (`at_hour`), the player's health down to 1 (+59h),
    a rest until healed begun as the camp's menu's second entry begins it
    (1194:424E, its menu 1194:020E giving 1)."""
    def put(a):
        at_hour(22, heading)(a)
        a.w16(0x01A4 + 0x59, 1)
        menu = a.lin(LOAD_SEG + 0x1194, 0x020E)
        saved = bytes(a.mu.mem_read(menu, 4))
        a.code_write(menu, b'\xb8\x01\x00\xcb')
        try:
            a.call_far(0x1194, 0x424E)
        finally:
            a.code_write(menu, saved)
        assert a.mu.mem_read(a.lin(a.ds, 0x1013), 1)[0], 'not resting'
    return put


# The world loop's read of the mouse (0000:8FC4): once a frame, round
# and round in a box's wait (1194:553B).
MOUSE_READ = (0, 0x8FC4)
# Space (a box's wait ends on any key).
KEY_SPACE = 0x39

# M (2542:0100's automap), F1 (the character sheet), Esc (both left).
KEY_M = 0x32
KEY_F1 = 0x3B
KEY_ESC = 0x01
# N (the sheet's equipment page, 3276:52B0's first record).
KEY_N = 0x31
KEY_PAD_PLUS = 0x4E
KEY_MINUS = 0x0C
KEY_D = 0x20
KEY_Y = 0x15
KEY_S = 0x1F
# L (2542:02A3: the logbook, 27E3:0088), U (034D: the use list), C
# (026D: the spellbook).
KEY_L = 0x26
KEY_J = 0x24
KEY_U = 0x16
KEY_C = 0x2E
# R (2542:01E6: camping, 1194:424E), the digit 2 and Enter (the hours
# typed, 1194:4800).
KEY_R = 0x13
KEY_2 = 0x03
KEY_ENTER = 0x1C
# The left Shift (C with it: the last spell again, 2542:057D).
KEY_LSHIFT = 0x2A


def last_spell(spell):
    """The spellbook's line `spell` picked last (5CAE), as its list's
    pick leaves it (2542:05A6)."""
    def put(a):
        a.mu.mem_write(a.lin(a.ds, 0x5CAE), bytes([spell]))
    return put


def crowded_square(a):
    """The player's square (CB90) given a byte in the plane at [D806],
    which 1194:5F13 takes for no room."""
    a.mu.mem_write(a.lin(a.r16(0xD806), a.r16(0xCB90)), bytes([1]))
# The automap's loop asks the mouse by the near entry (0000:2DB8: 8FC8).
AUTOMAP_READ = (0, 0x8FC8)
# The travel map's clicks (gen_areaplay.py's TRAVEL): High Rock's region,
# the place nearest (200, 100), its journey's box read, Travel.
TRAVEL = [('click', 70, 45), ('click', 200, 100), ('click', 200, 100), ('click', 62, 180)]

# Wizard's Fire, the save's spellbook's seventh (type 2: aimed).
WIZARDS_FIRE = 6

# The A key (2542:0100's ready, 5E0 bit 10h).
KEY_A = 0x1E
# The right button held with the mouse dragged down 60h mickeys before
# each frame: a swing (0000:3C9F) begun whenever the last has ended.
SWINGING = {'buttons': 2, 'drag': (0, 0x60)}
LET_GO = {'buttons': 0}


def wild_edge(a):
    """The wilderness's window at the grid's top row (origin y 0, 060A),
    the player just inside its second quarter (y 1008h: C3D4, BA66 and
    the last y, 0602), facing north (100h): walking on crosses into the
    first quarter, and 217B:0434 moves the origin to FFFFh, a row above
    the grid (217B:03C0 reads the words before it)."""
    a.w16(0x060A, 0)
    for o in (0xC3D4, 0xBA66, 0x0602):
        a.w16(o, 0x1008)
    a.w16(0xC3DA, 0x100)


def picking(level):
    """`locked_door`'s door, its lock of `level`, and picking armed (5CA
    bit 0, the lockpick's key: 2826:0AAB tries it once per door)."""
    def put(a):
        at_square(13, 4, 0)(a)
        n = a.mu.mem_read(a.lin(a.ds, 0xC6E4), 1)[0]
        a.mu.mem_write(a.lin(a.ds, 0xC624 + 3 * n), struct.pack('<HB', 0x0E04, level))
        a.mu.mem_write(a.lin(a.ds, 0xC6E4), bytes([n + 1]))
        a.w16(0x05CA, a.r16(0x05CA) | 1)
    return put


def levitating(put):
    """`put`, then the player levitating as the Levitate spell leaves
    them: spell flag 80h (05BE, the frame body's C39A bit 1, which
    0000:F9C4 keeps above water), its timer (the flag's word in the
    table at [1020], four bytes a flag) 30 minutes."""
    def both(a):
        put(a)
        a.w16(0x05BE, a.r16(0x05BE) | 0x80)
        off, seg = a.r16(0x1020), a.r16(0x1022)
        a.mu.mem_write(a.lin(seg, off + 4 * 7), struct.pack('<H', 30))
    return both


# A potion of healing (spell FFh at +11h) in a stack of one (flags 4Ch:
# a stack, the slot in use).
POTION = bytes([0x00, 0x01, 0, 0, 0, 0, 0, 0, 0, 0, 0x14, 0, 0, 0, 0x4C, 0xFF, 0xFF, 0xFF, 0])


def wounded_with_potion(a):
    """The player's health down to 64h (+59h) and a potion of healing
    (POTION) in the inventory's 39th slot (01A4 + 6Eh, 19 bytes a slot:
    the save's forty are all in use; the 39th, an accessory not worn,
    given up for it): the use list's fifth, before the coffer (8Dh)."""
    a.w16(0x01A4 + 0x59, 0x64)
    a.mu.mem_write(a.lin(a.ds, 0x01A4 + 0x6E + 0x13 * 38), POTION)


def dreaming(a):
    """The staff's line at stage 3 with its dream seen (F76 set, F77 3,
    F78 and F7D clear): "Where is" lists the stage's place tenth (1194:
    DFB9..E003: the nine kinds and 1C7E's third copied to 1044); the save
    itself is at stage 9, past it."""
    a.mu.mem_write(a.lin(a.ds, 0x0F76), b'\x01\x03\x00')
    a.mu.mem_write(a.lin(a.ds, 0x0F7D), b'\x00')


def used(index):
    """The use button's list's `index`-th item used as 24FA:07CD uses it
    once its list is picked from: the list made (0AFE), the item found
    (1194:7AE7, AX the index, SI the player), then used (29CF:07C3)."""
    def do(a):
        a.call_near(0x24FA, 0x0AFE, esi=0x01A4)
        a.call_far(0x1194, 0x7AE7, eax=index, esi=0x01A4)
        si = a.mu.reg_read(UC_X86_REG_SI)
        a.call_far(0x29CF, 0x07C3, esi=si)
    return do


def level_changed(a):
    return a.mu.mem_read(a.lin(a.ds, 0xBB0A), 1)[0] != 0


def text_shown(a):
    """A level's text up (0000:0628 set its time, 3C82)."""
    return a.mu.mem_read(a.lin(a.ds, 0x3C82), 1)[0] != 0


def locked_door(a):
    """South of the door at row 14, column 4 of the crypt's first level
    (B052h: a kind 1, sliding), facing it (heading 0), the door given a
    lock of level 5 (a record added to LOCKS, C624, 3 bytes: the square
    row << 8 | column, the level)."""
    at_square(13, 4, 0)(a)
    n = a.mu.mem_read(a.lin(a.ds, 0xC6E4), 1)[0]
    a.mu.mem_write(a.lin(a.ds, 0xC624 + 3 * n), struct.pack('<HB', 0x0E04, 5))
    a.mu.mem_write(a.lin(a.ds, 0xC6E4), bytes([n + 1]))


def message_up(a):
    """The message line's time set (061E)."""
    return a.r16(0x061E) != 0


def killed(a):
    """The player's health 0 and 2686:02B4 run, as a blow that kills runs
    it (2EBD:04D6 on the player): 5B1 bit 100h, the area left (C398 bit
    80h), the effects and spells on the player cleared."""
    a.w16(0x01A4 + 0x59, 0)
    a.call_far(0x2686, 0x02B4)


def died(a, vision=0):
    """From the world loop's end (0000:A45D, on the world's stack as
    `area_ended` goes on from it) through the outer loop (0CAD) to its
    death's way out (0CE1..0D43: the clip the screen, the ending's text
    picked, 1194:D459's vision unless F77 is 9, the save's), stopped at its
    RET (0D43), before the program's main takes the game back to its menu
    (0000:10B0, 108B). With `vision` the timer (0151) ticks at each read
    of it, as `titled` has it: the vision's movie's frames wait on it;
    and as the movie begins (2294:0700) that many clicks are queued at the
    screen's centre, its pages clicked through (the reads of the mouse
    before it, the world's way out's, take none)."""
    # The dying frame's tick: the harness ticks the timer (0151) at the
    # frame's waits for it (0000:A428, A43D), which a frame that ends the
    # world loop leaves before; the timer ticks all the same, before the
    # outer loop goes on (the view's frames tick before the frame; the
    # vision's movie makes it 0 again).
    a.w16(0x0151, (a.r16(0x0151) + 1) & 0xFFFF)
    ss, sp, words = a.world_stack
    a.mu.mem_write(a.lin(ss, sp), words)
    saved = a.stub(location=False)
    at = a.lin(LOAD_SEG, 0x0D43)
    t = a.lin(a.ds, 0x0151)

    def out(mu, addr, size, _):
        a.waiting = ('died',)
        mu.emu_stop()

    def tick(mu, access, addr, size, value, _):
        a.w16(0x0151, (a.r16(0x0151) + 1) & 0xFFFF)
    def clicks(mu, addr, size, _):
        a.clicks = [(160, 100)] * vision
    hooks = [a.mu.hook_add(UC_HOOK_CODE, out, None, at, at)]
    if vision:
        hooks.append(a.mu.hook_add(UC_HOOK_MEM_READ, tick, None, t, t + 1))
        movie = a.lin(LOAD_SEG + 0x2294, 0x0700)
        hooks.append(a.mu.hook_add(UC_HOOK_CODE, clicks, None, movie, movie))
    try:
        r = a.run_screen(0, 0xA45D, stack=(ss, sp), count=3_000_000_000)
    finally:
        for h in hooks:
            a.mu.hook_del(h)
        a.unstub(saved)
    assert r == ('died',), f'the death went on to {r}'


# The main menu's wait for its regions (0000:1114: 1194:5CF4).
MENU_WAIT = 0x1114


def titled(a, at=0x109B, stack=None, regs=None):
    """From the death's RET (`died`), the program's main on (0000:10AE's
    jump to 109B: 5B1 bit 100h, so 10B0 and 108B) through its menu's
    screens (10C9: the intro, 1246, then MENU.IMG) to the menu's wait
    for its regions (MENU_WAIT), with no input: the movies and pictures
    each run their course. Run from 109B on a stack of its own (the
    load's stack under the world's isn't main's). The timer (0151)
    ticks at each read of it (the movies' frames wait on it); the songs
    left out as everywhere (2F9F:001F). With `at`, `stack` and `regs`
    from elsewhere in main on its own frame (gen_titleplay.py's program
    start, 0000:1059)."""
    saved = a.stub(location=False)
    t = a.lin(a.ds, 0x0151)

    def tick(mu, access, addr, size, value, _):
        a.w16(0x0151, (a.r16(0x0151) + 1) & 0xFFFF)

    def menu(mu, addr, size, _):
        # The registers there kept for `menu_picked` (ES:SI the regions,
        # CX their count; main's frame and stack).
        a.menu_regs = {r: mu.reg_read(getattr(x86, 'UC_X86_REG_' + r.upper()))
                       for r in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp', 'es', 'ss', 'sp')}
        a.waiting = ('menu',)
        mu.emu_stop()
    hooks = [a.mu.hook_add(UC_HOOK_MEM_READ, tick, None, t, t + 1),
             a.mu.hook_add(UC_HOOK_CODE, menu, None, a.lin(LOAD_SEG, MENU_WAIT), a.lin(LOAD_SEG, MENU_WAIT))]
    try:
        r = a.run_screen(0, at, regs=regs, near=stack is None, stack=stack, count=3_000_000_000)
    finally:
        for h in hooks:
            a.mu.hook_del(h)
        a.unstub(saved)
    assert r == ('menu',), f'the way to the menu stopped by {r}'


def menu_picked(a, script):
    """From the menu's wait (`titled`, its registers kept), the program's
    main on with `script` answering its waits for input (`run_screen`'s:
    the menu's regions, 1194:5CF4; the load menu, 25FD:0242) until a
    world loop is about to run its first frame (0000:A2AC: the title's
    Load, 117E, into the loaded game), its stack kept for the frames
    after (`area_next`'s). The timer (0151) doesn't tick here: nothing on
    the way waits on it, and its one read (0000:4684, a title load's
    copy to 884A) then takes it as the menu found it, as the port's
    menu, which takes no time, leaves it."""
    saved = a.stub(location=False)

    def world(mu, addr, size, _):
        a.world_stack = a.stack_kept()
        a.waiting = ('world',)
        mu.emu_stop()
    at = a.lin(LOAD_SEG, 0xA2AC)
    hooks = [a.mu.hook_add(UC_HOOK_CODE, world, None, at, at)]
    regs = dict(a.menu_regs)
    stack = (regs.pop('ss'), regs.pop('sp'))
    a.waits = {MOUSE_READ}
    a.script = list(script)
    try:
        r = a.run_screen(0, MENU_WAIT, regs=regs, stack=stack, count=3_000_000_000)
    finally:
        for h in hooks:
            a.mu.hook_del(h)
        a.unstub(saved)
        a.waits = set()
        a.script = []
    assert r == ('world',), f'the menu went on to {r}'
    a.menu_loaded = True


def interface_digest(page):
    """The interface's rows of the page (146 on) as a load from the menu
    leaves them, the HUD's portrait, name and bars (columns 14..80) zeroed:
    what of the page is the engine's there (worldtest's
    `title_load_case`; the rest is the load menu's picture)."""
    return fnv(bytes(0 if 14 <= i % 320 <= 80 else page[i] for i in range(146 * 320, 64000)))


def serviced(seg, off, script):
    """A building's services run as its keeper's click runs them (the
    table at 869F: 1194:F48E the equipment store's, A73E the mages'
    guild's), between frames, its waits for input (the mouse's read,
    0000:8FC4) answered by `script` as gen_screens.py's are, until it
    returns."""
    def do(a):
        a.waits = {MOUSE_READ}
        a.script = list(script)
        try:
            assert a.run_screen(seg, off) is None, 'the services still waiting'
        finally:
            a.waits = set()
            a.script = []
        # The mouse back at the view's centre, where the view's controls
        # take it to be (the classic controls' centre region, 2826:00F0).
        a.mouse = (160, 100)
    return do


def talked(script):
    """The first townsperson clicked with the player on their square
    (gen_screens.py's `beside`), the seed SEED, and the click run as the
    frame runs it (1194:DB6C, from 0000:49D1) between frames, `script`
    answering its waits (`serviced`)."""
    run = serviced(0x1194, 0xDB6C, script)

    def do(a):
        gs.beside(a)
        gs.seeded(a)
        run(a)
    return do


def shopping(a):
    """A slot of the player's free (gen_screens.py's `room`), each item
    worn to half (`worn`) and the seed gen_screens.py's SEED (`seeded`):
    what the store's buying, selling and repairing need."""
    gs.room(a)
    gs.worn(a)
    gs.seeded(a)


def guild_shopping(a):
    gs.room(a)
    gs.seeded(a)


def temple_hurt(a):
    """The player at half health (gen_screens.py's `hurt`), seeded: what
    the temple's Heal needs."""
    gs.hurt(a)
    gs.seeded(a)


def diseased(a):
    """The player's afflictions (+40Dh, 05B1) a disease (bit 1) and only
    that, seeded: the temple's Cure lists one line."""
    a.mu.mem_write(a.lin(a.ds, 0x05B1), bytes([1]))
    gs.seeded(a)


def newgame(a):
    """The options screen's New (its handler, 2542:0627, as a click on it
    runs it): FCE 0, the area left (C398 bit 80h) with 5CA bit 8000h for
    the outer loop, the working slot's log and files deleted."""
    assert a.run_screen(0x2542, 0x0627, near=True) is None


def loaded(a):
    """Slot 1 loaded in play (25FD:028D with AX 1, 5C8 bit 4 clear: the
    load menu's pick) at the top of a frame, as gen_playload.py loads it,
    the sounds and the location record's build stubbed likewise."""
    saved = a.stub()
    try:
        a._setup([0, STUB_SEG], {'eax': 1})
        a._run(0x25FD, 0x028D, STUB_SEG, 0, count=500_000_000)
    finally:
        a.unstub(saved)
    for at in range(0xFFE0, 0x10000, 2):
        a.w16(at, 0)


def farewell(a):
    """The main quest at its third stage (F77 3), so a death plays the
    farewell vision (1194:D459, VISION.FLC with text 57Bh); 9A6E, where
    D45D reads the colours (6E38), filled with something they aren't, so
    the read is seen (the DAC, faded to the cleared 929E, is black)."""
    a.mu.mem_write(a.lin(a.ds, 0x0F77), bytes([3]))
    a.mu.mem_write(a.lin(a.ds, 0x9A6E), bytes((i * 7 + 1) & 0x3F for i in range(0x300)))


def levelled(a):
    """Experience gained (the player's +409h, 05AD, by LEVEL_XP: a
    level's worth for the save's level 22), 5CA bit 100h set as the gain sets it (2686:0182), so
    the next minute tick (0000:1644, `minute_due`) checks the level
    (2686:02D8) and brings up the level-up screen (1194:1034) inside its
    frame."""
    xp = a.r16(0x05AD) | a.r16(0x05AF) << 16
    xp = (xp + LEVEL_XP) & 0xFFFFFFFF
    a.w16(0x05AD, xp & 0xFFFF)
    a.w16(0x05AF, xp >> 16)
    a.w16(0x05CA, a.r16(0x05CA) | 0x100)


def minute_due(a):
    """The clock's last minute (884A) 90 ticks back: the next frame's
    minute tick runs."""
    a.w16(0x884A, (a.r16(0x0151) - 0x5A) & 0xFFFF)


def informant(a):
    """The main quest's informant in this tavern, as a rumour's offer
    leaves one (1194:CD9A): the step Rajhin's ring's (0FDD 0Fh), the
    informant the person talked to here (0FDE: the building's door
    square, 0FD2, its column halved, as 1194:CED7 compares it), the
    dungeons' province this one (0FE3: 0A46), the informant offered but
    not yet met (0FE6 2), met today (0FE9 the time, 5CC) with ten days
    (0FED) and a price of 100 gold (0FEE); the seed SEED."""
    w = a.r16(0x0FD2)
    a.mu.mem_write(a.lin(a.ds, 0x0FDD), bytes([0x0F]))
    a.w16(0x0FDE, w & 0xFF00 | (w & 0xFF) >> 1)
    a.mu.mem_write(a.lin(a.ds, 0x0FE3), bytes(a.mu.mem_read(a.lin(a.ds, 0x0A46), 1)))
    a.mu.mem_write(a.lin(a.ds, 0x0FE6), bytes([2]))
    a.mu.mem_write(a.lin(a.ds, 0x0FE9), bytes(a.mu.mem_read(a.lin(a.ds, 0x05CC), 4)))
    a.mu.mem_write(a.lin(a.ds, 0x0FED), bytes([10]))
    a.w16(0x0FEE, 100)
    gs.seeded(a)


# The haggle's Accept and the deal's Yes (NEGOTBUT.IMG's first button:
# 1194:6001's at 59h..88h, 7Fh..8Bh), and its strip's "...More".
ACCEPT = ('click', 110, 133)
MORE = ('click', 297, 172)
# The store's menu (1194:F48E): Buy, then Weapons and its first line
# twice (marked, then haggled: "...More", Accept, Yes); Sell, its first
# line (worn: "still want to sell it?" Yes, 0A0h, 69h), Accept, Yes;
# Repair, its first line, Yes and the words after; Exit.
STORE_SCRIPT = [('click', 0xA0, 0x43), ('click', 0xA0, 0x65), ('click', 0x70, 0x26), ('click', 0x70, 0x26), MORE, ACCEPT, ACCEPT,
                ('click', 0xA0, 0x5B), ('click', 0x80, 0x25), ('click', 0xA0, 0x69), ACCEPT, ACCEPT,
                ('click', 0xA0, 0x73), ('click', 0x70, 0x26), ACCEPT, ACCEPT,
                ('click', 0xA0, 0xA3)]
# The mages' guild's menu (1194:A73E): Buy, Potions, the first line, how
# many typed (1, Enter), Accept, Yes; Exit.
# The tavern's keeper (1194:B93C) with the informant here (`informant`):
# his words paged ("...More" twice), his price accepted, the rest of his
# words paged to the tavern's menu; its Exit.
INFORMANT_SCRIPT = [MORE, MORE, ACCEPT, MORE, MORE, MORE, MORE, ('click', 0xA0, 0xA3)]
TALK_WHERE_SCRIPT = [('click', 0xA0, 0x65), ('click', 100, 23), ('click', 100, 23), ('click', 0xA0, 0x60), ('click', 0xA0, 0x95)]
TALK_QUEST_SCRIPT = [('click', 0xA0, 0x65), ('click', 58, 105), ('click', 100, 95), ('click', 100, 95), ('click', 0xA0, 0x60), ('click', 0xA0, 0x95)]
TALK_PALACE_SCRIPT = [('click', 0xA0, 0x65), ('click', 100, 50), ('click', 100, 50), ('click', 0xA0, 0x60), ('click', 0xA0, 0x95)]
GUILD_SCRIPT = [('click', 0xA0, 0x46), ('click', 0xA0, 0x59), ('click', 0x70, 0x26), ('keys', [0x02]), ('keys', [0x1C]), ACCEPT, ACCEPT,
                ('click', 0xA0, 0xA3)]
# The temple's menu (1194:C258: Bless, Cure, Heal, Exit, boxes 18h high
# from 46h): Heal, its price accepted (Yes), "thou art healed" clicked
# away; Exit.
TEMPLE_HEAL_SCRIPT = [('click', 0xA0, 0x7F), ACCEPT, ('click', 0xA0, 0x60), ('click', 0xA0, 0x97)]
# Cure: the afflictions' list (1194:C36E), its one line clicked (marked
# as the list comes up, so taken: paid and cured with no question); Exit.
TEMPLE_CURE_SCRIPT = [('click', 0xA0, 0x67), ('click', 0x60, 0x32), ('click', 0xA0, 0x97)]
# Bless: the donation typed (1, 0, 0, Enter, a key a wait), "Receive our
# blessings" clicked away; Exit.
TEMPLE_BLESS_SCRIPT = [('click', 0xA0, 0x4F)] + [('keys', [k]) for k in (0x02, 0x0B, 0x0B, 0x1C)] + [('click', 0xA0, 0x60), ('click', 0xA0, 0x97)]
# The tavern's menu (1194:B93C): Drinks, the first line (bought and drunk:
# "You finish the ..." clicked away); Exit.
TAVERN_DRINK_SCRIPT = [('click', 0xA0, 0x43), ('click', 0x70, 0x26), ('click', 0xA0, 0x60), ('click', 0xA0, 0xA3)]
# Get a Room as gen_screens.py's `tavernroomtaken` takes it (the Single,
# a day typed, the price accepted, the deal confirmed), then the menu
# without its rooms left by its Exit (the third box).
TAVERN_ROOM_SCRIPT = [('click', 0xA0, 0x5B), ('click', 0x80, 0x31), ('keys', [0x02, 0x1C]), ('click', 0x70, 0x85),
                      ('click', 0x70, 0x85), ('click', 0xA0, 0x87)]


# The main quest's Crypt of Hearts: its .MIF (whose INFO names
# crypt1.inf).
CRYPT = b'41649375.MIF'

# name: (where, set-up, [(keys held, keys let go, frames, until[, clicks])...]):
# the clicks (x, y) are queued for the mouse's reads before the frames.
LEVEL_XP = 9_750_000
# The level's notice clicked away (1194:5E33), then on the attribute
# screen (1194:122C, the regions at 3276:5312) each line picked and its
# arrows' upper half clicked six times (a level's 3..6 points; once none
# are left a click does nothing, 1194:138E), then the button (region 0).
LEVEL_SCRIPT = [('click', 0xA0, 0x60)] + [act for r in range(8) for act in [('click', 10, 54 + 8 * r)] + [('click', 40, 0x32 + 8 * r)] * 6] + [('click', 10, 175)]
CASES = {
    'playlevelup': ('dungeon', levelled, [((), (), 2, None), ((), (), 10, None, [], {'do': minute_due, 'script': LEVEL_SCRIPT})]),
    # The dungeon's first frames, nothing held: the interface and the
    # compass as play draws them.
    'playdungeon': ('dungeon', None, [((), (), 3, None)]),
    # The first level's down stairs (row 10, column 10 of its map: bytes
    # 1Ch) walked into from the square south of them, facing north (100h):
    # the up arrow held until the level changes (0000:F76F), then let go
    # for two frames on the second level.
    'playdown': ('dungeon', at_square(11, 10, 0x100), [((UP,), (), 20, level_changed), ((), (UP,), 2, None)]),
    # The Crypt of Hearts entered (`crypt`), turned north and walked on
    # with the up arrow into the square in front of its start (row 4,
    # column 2), whose trigger shows the INF's first text (0000:0628);
    # let go, a frame more: the text over the view (0000:0605).
    'playcrypt': ('crypt', facing(0x100), [((UP,), (), 20, text_shown), ((), (UP,), 1, None)]),
    # A locked door (`locked_door`) clicked in the middle of the view two
    # frames on: not armed (5CA bit 0 clear), not picked, the lock's words
    # by the player's chance put on the message line (2826:09AC, 0B1A),
    # which the next frames draw.
    # The street outside the guild, its first frames, nothing held: the
    # city's 3D view (0000:D502's outdoor half, the sky).
    'playstreet': ('street', None, [((), (), 3, None)]),
    # The street turned north (heading 0): open sky over the houses, a
    # cloud, a lamp post.
    'playsky': ('street', facing(0), [((), (), 3, None)]),
    # The same at ten at night (`night`): the frame's light turns to the
    # night's (0000:0390) and the next frames are drawn by it.
    'playnight': ('street', night(0), [((), (), 3, None)]),
    # Out of the city's gate into the wilderness (`wild`), its first
    # frames, nothing held: the wild's 3D view (fields, the sky).
    'playwild': ('wild', None, [((), (), 3, None)]),
    # The street in rain, two squares short of a puddle (flat 60 at row
    # 91, column 41): the rainy INF's flats that mirror the sky (CDA1 bit
    # 2), each drawn then 0000:E37F over it.
    'playrain': ('wetstreet', at_square(89, 41, 0), [((), (), 3, None)]),
    # The street in snow: 0000:E26B's flakes over the view.
    'playsnow': ('snowstreet', None, [((), (), 3, None)]),
    # At night in a thunderstorm, a bolt forced at the circle's angle 0,
    # ahead: its second frame (2294:05A6, 0000:78A8) over the sky's flash.
    'playbolt': ('street', storm(0, 0), [((), (), 1, None)]),
    # The wilderness moved near Red Mountain (province 3, 20h south of
    # its 84h, 34h), facing it: the volcano's frames (volcano2.dfa,
    # 292B:0201's 5652 and 5A66); then far (A0h: volcano3.img, 5828).
    'playvolcano': ('wild', volcanic(0x100, 0, 0x20), [((), (), 1, None)]),
    'playvolcanofar': ('wild', volcanic(0x100, 0, 0xA0), [((), (), 1, None)]),
    # The main quest dungeon 39699021.MIF entered (`named`), its first
    # frames, nothing held: its start sees a square's cap.
    'playcap': ('capped', None, [((), (), 3, None)]),
    # The main quest dungeon 40516921.MIF entered (`named`), its first
    # frames, nothing held: its fog (2294:0B01, the view filled to the
    # horizon with the background colour, only the player's light).
    'playfog': ('fogged', None, [((), (), 3, None)]),
    # Its first frame: the fog's step making FOG.LGT's table (2294:0C36,
    # 0BEF's load) in the middle of the drawing, after the floor's half.
    'playfogfirst': ('fogged', None, []),
    # The dungeon with a ghost brought near and faced (`ghost`), one
    # frame: its flat drawn through the light table (0000:953E).
    'playghost': ('dungeon', ghost, [((), (), 1, None)]),
    'playlock': ('crypt', locked_door, [((), (), 2, None), ((), (), 10, message_up, [(160, 100)]), ((), (), 1, None)]),
    # The ghost (`ghost`) clicked in the middle of the view a frame on:
    # the drawing's hit test (0000:C4BB, before 78A8 scales it) finds it
    # under the click (CB5E, the hook CD3A), the frame's actor click
    # (1194:DB6C) names it on the message line.
    # The wilderness's top edge (`wild_edge`) walked over with the up
    # arrow: the window's origin a row above the grid, its blocks loaded
    # from what lies before it.
    'playwildedge': ('wild', wild_edge, [((UP,), (), 12, None), ((), (UP,), 1, None)]),
    # The street at noon (`at_hour`), facing north, walked on with the up
    # arrow for 60 frames and then stood in for 240: the townspeople
    # made on the way in walking about the player.
    'playcrowd': ('street', at_hour(12, 0), [((UP,), (), 60, None), ((), (UP,), 240, None)]),
    # A creature (type 1, level 1: `brought`) three squares down the
    # dungeon's corridor; the weapon readied (A held a frame), waited
    # for as the creature comes and strikes, then swung at it frame on
    # frame (`SWINGING`) till it falls, and let go: blows both ways, the
    # kill (2AA1:06A0), its corpse.
    'playfight': ('dungeon', brought(1), [((KEY_A,), (), 1, None), ((), (KEY_A,), 30, None),
                                          ((), (), 200, None, [], SWINGING), ((), (), 20, None, [], LET_GO)]),
    # The same creature four squares off, Wizard's Fire cast at it
    # (`casting`), the view clicked at its centre two frames on (the
    # frame's controls launch it from the player, 29CF:0000), and the
    # fire followed through its flight into the creature (29CF:037F's
    # missile, 0000:2566's moves).
    'playcast': ('dungeon', casting(1, WIZARDS_FIRE), [((), (), 2, None), ((), (), 10, None, [(160, 100)]), ((), (), 120, None)]),
    # The wilderness at ten at night, the hour's encounter roll come up
    # (`night_encounter`), stood in: what came comes for the player and
    # strikes.
    'playwildnight': ('wild', night_encounter(0x100), [((), (), 400, None)]),
    # A creature that casts (type Bh: one spell in its record's list at
    # +366h) six squares down the corridor, stood before: seeing the
    # player it casts (29CF:0145: its spell launched at the player) and
    # comes.
    'playcreaturecast': ('dungeon', brought(0xB, ahead=0x300), [((), (), 400, None)]),
    # `stairs_fight`: stood in while the creature comes and strikes, then
    # the up arrow held into the stairs mid-fight (0000:F76F), and two
    # dozen frames on the level below.
    'playstairsfight': ('dungeon', stairs_fight, [((), (), 40, None), ((UP,), (), 20, level_changed), ((), (UP,), 24, None)]),
    # The wilderness at night, a rest until healed begun (`resting`): each
    # frame 20 minutes (1194:4635), the clock's hours rolling for
    # encounters (1194:887A) until enemies come and end it ("Enemies are
    # nearby", the box read with Space), then what came comes on.
    'playrest': ('wild', resting(0x100), [((), (), 300, None, [], {'script': [('keys', [KEY_SPACE])]})]),
    # Camping (R, 2542:01E6) in the wilderness at noon: the camp's menu
    # (1194:020E) answered C (camp for a while), the hours typed (2,
    # Enter: 1194:46BF), then the rest's frames count them down (each 20
    # minutes, 1194:456F) to "done" (its box read with Space).
    'playcamp': ('wild', at_hour(12, 0x100), [((), (), 2, None), ((KEY_R,), (), 1, None, [], {'script': [('keys', [KEY_C]), ('keys', [KEY_2]), ('keys', [KEY_ENTER])]}),
                                             ((), (KEY_R,), 12, None, [], {'script': [('keys', [KEY_SPACE])]})]),
    # Shift+C (2542:056A with C and a Shift held, 057D..0594): the last
    # spell picked (`last_spell`: Wizard's Fire, the book's seventh) cast
    # again without the list (29CF:071F), its target clicked at the
    # view's centre; forty frames of its flight.
    'playquickcast': ('dungeon', last_spell(WIZARDS_FIRE), [((), (), 2, None), ((KEY_LSHIFT, KEY_C), (), 1, None),
                                                          ((), (KEY_C, KEY_LSHIFT), 10, None, [(160, 100)]), ((), (), 40, None)]),
    'playghostclick': ('dungeon', ghost, [((), (), 1, None), ((), (), 10, message_up, [(160, 100)]), ((), (), 1, None)]),
    # `locked_door`'s door with a lock of level 1, picking armed
    # (`picking`), clicked: the roll picks it (2826:0AAB, 1194:53F9), the
    # door opens; then the up arrow held through it and let go.
    'playpick': ('crypt', picking(1), [((), (), 2, None), ((), (), 20, None, [(160, 100)]), ((UP,), (), 40, None), ((), (UP,), 10, None)]),
    # The crypt two squares north of its diagonal wall at row 8, column 3
    # (kind 0Dh), facing it (heading 0): its faces never hide a light
    # behind them (0000:B7A9 returns at once for CB37 0Dh).
    'playdiagonal': ('crypt', at_square(6, 3, 0), [((), (), 3, None)]),
    # `playpick`'s door clicked open, three frames of it moving: each
    # frame the first face drawn after the walk steps it (C398 bit 1000h,
    # 0000:CD89) is drawn with CCF8 FFFFh, which hides no light (B7A9).
    'playdoormoving': ('crypt', picking(1), [((), (), 2, None), ((), (), 4, None, [(160, 100)])]),
    # The crypt's water (row 22, columns 6..10) walked into from the
    # square north of it (row 21, column 7), facing it (heading 0): the
    # player sinks (0000:F9C4's water), swims and bobs (C39A bit 10h,
    # CB68), the footsteps the swimming sound.
    'playswim': ('crypt', at_square(21, 7, 0), [((UP,), (), 30, None), ((), (UP,), 60, None)]),
    # The same levitating (`levitating`): kept up over the water, bobbing
    # (2826:0042).
    'playlevitate': ('crypt', levitating(at_square(21, 7, 0)), [((UP,), (), 30, None), ((), (UP,), 30, None)]),
    # Wounded with a potion (`wounded_with_potion`): two frames, the
    # A creature brought (`brought`) and fought off by standing: twenty
    # frames, then M held a frame (the automap, 2542:045A, left by a click
    # on its exit, 0000:2E30: F0h..120h, A4h..BBh), ten frames, F1 held a
    # frame (the character sheet, 1194:0CF8, left with Esc), thirty
    # frames: the screens opened and closed mid-fight.
    'playscreens': ('dungeon', brought(1), [((), (), 20, None), ((KEY_M,), (), 1, None, [], {'script': [('click', 0x100, 0xB0)], 'waits': {AUTOMAP_READ}}),
                                            ((), (KEY_M,), 10, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_ESC])]}),
                                            ((), (KEY_F1,), 30, None)]),
    # The two screens alone: in the dungeon, two frames, M held a frame
    # (the automap, left by its exit), ten frames; F1 held a frame (the
    # character sheet, left with Esc), ten frames.
    'playautomap': ('dungeon', None, [((), (), 2, None), ((KEY_M,), (), 1, None, [], {'script': [('click', 0x100, 0xB0)], 'waits': {AUTOMAP_READ}}),
                                       ((), (KEY_M,), 10, None)]),
    # The logbook: two frames, L held a frame (2542:02A3, 27E3:0088), a
    # click off its buttons twice (27E3:01FC: the next page, 03A6), Exit
    # (107h..127h, B4h..C3h) twice: the first only ends the pages' loop
    # (03A9 to 03CB), the loop after them (0137) asks again; ten frames.
    # The logbook by the bar's scroll button (2542:06CE's table, 02A3):
    # two frames, the button clicked (down at the next frame's read of
    # the mouse, up at the one after: the bar's click), a click off its
    # buttons, Exit twice; as playlog, but the SP kept at 12C6 a word
    # deeper.
    # The bar's crossed swords clicked with J held (2542:06CE's table,
    # 0486: the weapon readied): two frames, the button pressed (down at
    # the next frame's read), then the frame whose read finds it let go,
    # J held: the cursor on the bar, the controls end at 2826:00CB on
    # 02E2's carry, so no jump; J let go and the mouse back at the view's
    # centre (where the window's controls take it to be), five frames.
    'playbarjump': ('dungeon', None, [((), (), 2, None), ((), (), 1, None, [(102, 161)]), ((KEY_J,), (), 1, None),
                                       ((), (KEY_J,), 5, None, [], {'do': lambda a: setattr(a, 'mouse', (160, 100))})]),
    'playlogbar': ('dungeon', None, [((), (), 2, None), ((), (), 11, None, [(131, 186)], {'script': [('click', 0xA0, 0x60), ('click', 0x110, 0xBC), ('click', 0x110, 0xBC)]})]),
    'playlog': ('dungeon', None, [((), (), 2, None), ((KEY_L,), (), 1, None, [], {'script': [('click', 0xA0, 0x60), ('click', 0xA0, 0x60), ('click', 0x110, 0xBC), ('click', 0x110, 0xBC)]}),
                                   ((), (KEY_L,), 10, None)]),
    # The logbook printed: two frames, L held a frame, Print (25h..4Ch on
    # the bottom row) on the first page: every line of the log to PRN
    # with an LF CR and a form feed (27E3:0272), which ends the pages'
    # loop as Exit does; Exit leaves; ten frames. What PRN took is the
    # `printed` line.
    'playlogprint': ('dungeon', None, [((), (), 2, None), ((KEY_L,), (), 1, None, [], {'script': [('click', 0x30, 0xBC), ('click', 0x110, 0xBC)]}),
                                        ((), (KEY_L,), 10, None)]),
    # The screens a menu key opens inside the frame (2542:0100 from
    # 0000:4952, after the frame's actors, drawing and physics), each
    # case ending with that frame (`ends`): the screen closed, then the
    # rest of the frame's body (the clock, faces, interface, present,
    # counters) run after it. As playsheet, playlog, playautomap and
    # playbook, without the frames after.
    'playsheetend': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_ESC])], 'ends': True})]),
    'playlogend': ('dungeon', None, [((), (), 2, None), ((KEY_L,), (), 1, None, [], {'script': [('click', 0xA0, 0x60), ('click', 0x110, 0xBC), ('click', 0x110, 0xBC)], 'ends': True})]),
    'playautomapend': ('dungeon', None, [((), (), 2, None), ((KEY_M,), (), 1, None, [], {'script': [('click', 0x100, 0xB0)], 'waits': {AUTOMAP_READ}, 'ends': True})]),
    'playbookend': ('dungeon', None, [((), (), 2, None), ((KEY_C,), (), 1, None, [], {'script': [('rclick', 0xF0, 0xA0)], 'ends': True})]),
    # The same for the screens whose ends can leave the world loop: the
    # options screen (Esc, 2542:000A) left by its Resume (101h..13Fh on
    # the bottom row: no handler, 0084); the travel map (Shift+M, the
    # street's 2542:03BF) left by Esc (the world map's Exit's key); a
    # journey (gen_areaplay.py's travelfast: High Rock, the place nearest
    # (200, 100), its box read, Travel: 2542:0411..0434, the area left),
    # the rest of that frame ending the world loop, the outer loop on
    # into the next area, stopped before its first frame. And the options
    # screen's New (0..40h, 0627) in the frame, the rest of it ending the
    # world loop, the outer loop's way out and main back to its menu (as
    # playnew, which runs the handler between frames).
    'playoptionsend': ('dungeon', None, [((), (), 2, None), ((KEY_ESC,), (), 1, None, [], {'script': [('click', 288, 132)], 'ends': True})]),
    # The options screen's Load (2542:061E, the load menu 25FD:0242), Esc
    # let go first (held, the menu's click, 1194:564D, takes it as Esc and
    # leaves), slot 1 (the save's own game) taken: the load returns into
    # the frame Esc opened the screen in, which runs on to its end.
    'playoptionsload': ('dungeon', None, [((), (), 2, None), ((KEY_ESC,), (), 1, None, [], {'script': [('up', [KEY_ESC]), ('click', 96, 132), ('click', 160, 21)], 'ends': True})]),
    'playmapend': ('street', at_square(95, 52, 0x100), [((), (), 2, None), ((KEY_LSHIFT, KEY_M), (), 1, None, [], {'script': [('keys', [KEY_ESC])], 'ends': True})]),
    'playjourneyend': ('street', at_square(95, 52, 0x100), [((), (), 2, None), ((KEY_LSHIFT, KEY_M), (), 1, None, [], {'script': TRAVEL + [('keys', [KEY_SPACE])] * 4, 'leaves': True, 'ends': True})]),
    'playnewend': ('dungeon', None, [((), (), 2, None), ((KEY_ESC,), (), 1, None, [], {'script': [('click', 32, 132)], 'dies': True, 'title': True})]),
    # The first level's up stairs (row 10, column 42: byte 1Dh) walked
    # into from the start's square south of them, facing north: the way
    # out (CD4E, 0000:4537) keeps the world (A611), shows the travel map
    # (26F7:0000, 1A81 set), Esc on it stays, the world given back (A5D1,
    # A6B5) and the frame runs on to its end (`exit_asked`).
    'playexitmapend': ('dungeon', exit_stairs, [((UP,), (), 30, exit_asked, [], {'script': [('keys', [KEY_ESC])], 'ends': True})]),
    # The status box (S, 2542:037C -> 04B1) inside its frame, a click
    # closing it (0530..0569: the buttons let go, then a click or Esc
    # held), the rest of the frame after it.
    'playstatusend': ('dungeon', None, [((), (), 2, None), ((KEY_S,), (), 1, None, [], {'script': [('click', 160, 100)], 'ends': True})]),
    # The use list (U, 2542:034D): wounded with a potion in the fifth
    # line (`wounded_with_potion`), two frames, U held a frame: the list
    # in the bar's corner (066A, 06F8: 3159:06A0 over 5AF4), its fifth
    # line clicked twice (marked, then picked), the potion drunk; sixty
    # frames.
    'playuselist': ('dungeon', wounded_with_potion, [((), (), 2, None), ((KEY_U,), (), 1, None, [], {'script': [('click', 0xF0, 0xBD), ('click', 0xF0, 0xBD)]}),
                                                      ((), (KEY_U,), 60, None)]),
    # The spellbook (C, 2542:026D, 056A): two frames, C held a frame: the
    # book's names sorted (0000:0B9C) and listed in the bar's corner
    # (06F8), left by a right click (5AF4's flag 4000h); ten frames.
    'playbook': ('dungeon', None, [((), (), 2, None), ((KEY_C,), (), 1, None, [], {'script': [('rclick', 0xF0, 0xA0)]}),
                                    ((), (KEY_C,), 10, None)]),
    'playsheet': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_ESC])]}),
                                     ((), (KEY_F1,), 10, None)]),
    # The sheet's equipment page: two frames, F1 held a frame (the sheet),
    # N (1194:0D67: the equipment page, 16BD), Esc (its exit, back to
    # the sheet), Esc (the sheet left), ten frames.
    'playequip': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                     ((), (KEY_F1,), 10, None)]),
    # The equipment page's list scrolled: as playequip, with '+' twice
    # (the keypad's: 3276:57D8's second record, 1194:1A4A, the lines one
    # on) and '-' once (the third, 1A94, one back) between N and Esc.
    'playequipscroll': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('keys', [KEY_PAD_PLUS]), ('keys', [KEY_PAD_PLUS]),
                                                                                                   ('keys', [KEY_MINUS]), ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                           ((), (KEY_F1,), 10, None)]),
    # The equipment page's list clicked: as playequip, the second line
    # clicked (3276:57D8's first record, 1194:1AC6: marked, 1C5E) and
    # clicked again (its item, worn, taken off: 1AFB, the page drawn
    # again, 23B1) between N and Esc.
    'playequipwear': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('click', 0x40, 0x42), ('click', 0x40, 0x42),
                                                                                                 ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                         ((), (KEY_F1,), 10, None)]),
    # An item dropped from the equipment page: as playequip, the third
    # line clicked (marked, 1194:1C5E), D (3276:57D8's fourth record,
    # 17D8: "Do you wish to drop the ...?", 48E9's menu) answered Y (the
    # item dropped where the player stands, 18F6, the page drawn again)
    # between N and Esc.
    'playequipdrop': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('click', 0x40, 0x4D), ('keys', [KEY_D]), ('keys', [KEY_Y]),
                                                                                                 ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                         ((), (KEY_F1,), 10, None)]),
    # "Drop what?" on the equipment page: as playequipdrop, the third line
    # dropped (which leaves no line marked: 1194:1924), then D again
    # (17D8 with 1296 FFh: 18CA, "Drop what?" in the list's panel, 223A
    # with DI 2325: the page's loop over its first five records, 1299
    # set), the second line clicked (1AC6 leaves that loop; 2325 marks
    # the line, 1C5E), its question (17D8 again) answered Y, between N
    # and Esc.
    'playequipdropwhat': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('click', 0x40, 0x4D), ('keys', [KEY_D]), ('keys', [KEY_Y]),
                                                                                                     ('keys', [KEY_D]), ('click', 0x40, 0x42), ('keys', [KEY_Y]),
                                                                                                     ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                             ((), (KEY_F1,), 10, None)]),
    # "Drop what?"'s loop with a drop within: as playequipdropwhat to its
    # loop, then '+' (the list moved on in the loop: 1A4A), Y (no record's
    # key: only the ring takes it), D (17D8 within the loop: "Drop what?"
    # again, 2325 within), the second line clicked (the inner loop left,
    # the line marked, 1C5E; 17D8 over again), its question answered Y,
    # then Esc (the outer loop, its 1299 cleared by the inner, left: no
    # mark), between N and Esc.
    'playequipdropin': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('click', 0x40, 0x4D), ('keys', [KEY_D]), ('keys', [KEY_Y]),
                                                                                                   ('keys', [KEY_D]), ('keys', [KEY_PAD_PLUS]), ('keys', [KEY_Y]), ('keys', [KEY_D]),
                                                                                                   ('click', 0x40, 0x42), ('keys', [KEY_Y]), ('keys', [KEY_ESC]),
                                                                                                   ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                           ((), (KEY_F1,), 10, None)]),
    # The equipment page's Spells button: as playequip, S (3276:57D8's
    # sixth record, 1194:2365: the caster's list turned to the spells,
    # 1D5C), '+' (that list one line on), S (back to the items) between
    # N and Esc.
    'playequipspells': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('keys', [KEY_S]), ('keys', [KEY_PAD_PLUS]),
                                                                                                   ('keys', [KEY_S]), ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                           ((), (KEY_F1,), 10, None)]),
    # An item described on the equipment page: as playequip, the second
    # line right-clicked (1194:1AC6 with AFF9 2: marked, 1C5E, then
    # described in the list's panel, 1988, 223A), the click its wait
    # takes (5536), between N and Esc.
    'playequipdescribe': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('rclick', 0x40, 0x42), ('click', 0x40, 0x42),
                                                                                                     ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                             ((), (KEY_F1,), 10, None)]),
    # The staff's quest line on the equipment page: as playequip, the list
    # moved on to its end ('+' 34 times: the save's 40 items and one
    # quest's line), the last line (the staff's pieces, F77) clicked
    # (marked, 1194:1C5E, its count made again, 1F56), right-clicked
    # (1F56 again and described, 1988: "You currently have ...", 223A),
    # the click its wait takes, clicked again (1B6F: "This item is
    # needed...", 223A) and the click that wait takes, between N and Esc.
    'playequipstaff': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N])] + [('keys', [KEY_PAD_PLUS])] * 34 + [
        ('click', 0x40, 0x7A), ('rclick', 0x40, 0x7A), ('click', 0x40, 0x42), ('click', 0x40, 0x7A), ('click', 0x40, 0x42), ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                          ((), (KEY_F1,), 10, None)]),
    # An accessory refused on the equipment page: as playequip, the list
    # moved on 16 lines, the second line shown (the save's 18th item, a
    # ring of a kind worn already) clicked (marked, 1194:1C5E) and
    # clicked again (1AFB: 7804 finds the clash, 1B78: "You can only
    # equip one %s.", 223A) and the click its wait takes, between N and
    # Esc.
    'playequipclash': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N])] + [('keys', [KEY_PAD_PLUS])] * 16 + [
        ('click', 0x40, 0x42), ('click', 0x40, 0x42), ('click', 0x40, 0x42), ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                         ((), (KEY_F1,), 10, None)]),
    # An item the class can't use refused on the equipment page: as
    # playequipstaff's end of the list ('+' 34 times), the sixth line
    # shown (the save's last item, type 8Dh) clicked (marked) and clicked
    # again (1AFB: 7772 refuses it, 1B6F: 748E, 1BE0: "This item can't be
    # equipped.", 223A) and the click its wait takes, between N and Esc.
    'playequipcantuse': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N])] + [('keys', [KEY_PAD_PLUS])] * 34 + [
        ('click', 0x40, 0x6F), ('click', 0x40, 0x6F), ('click', 0x40, 0x42), ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                           ((), (KEY_F1,), 10, None)]),
    # A spell's page from the equipment page: as playequip, S (the
    # spells' list, 1194:2365), its second line clicked (marked, 1C5E)
    # and clicked again (1B3C: the spell's page, 51D1), the page's Exit
    # (its loop's 8FC4; the list drawn again, 21C2, 1DC8), between N and
    # Esc.
    'playequipspellpage': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'script': [('keys', [KEY_N]), ('keys', [KEY_S]), ('click', 0x40, 0x42), ('click', 0x40, 0x42),
                                                                                                      ('click', 0x120, 0xC0), ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                              ((), (KEY_F1,), 10, None)]),
    # No room to drop on the equipment page: the player's square (CB90)
    # given a byte in the plane at [D806] before F1, so the sheet's
    # 1194:5F13 finds no room (5E0 bit 20h); as playequip, D (17D8: "There is no room...",
    # 1962, in the list's panel, 223A) and the click its wait takes
    # (5536) between N and Esc.
    'playequipnoroom': ('dungeon', None, [((), (), 2, None), ((KEY_F1,), (), 1, None, [], {'do': crowded_square, 'script': [('keys', [KEY_N]), ('keys', [KEY_D]), ('click', 0x40, 0x42),
                                                                                                                         ('keys', [KEY_ESC]), ('keys', [KEY_ESC])]}),
                                                   ((), (KEY_F1,), 10, None)]),
    # Wounded with a potion of healing (`wounded_with_potion`): two frames, the
    # potion drunk (the use list's fifth: 29CF:0869 heals 25), five
    # frames, the Ring of Shielding used (the list's fourth: its spell
    # cast on the player, its charge spent), sixty frames on.
    'playuse': ('dungeon', wounded_with_potion, [((), (), 2, None), ((), (), 5, None, [], {'do': used(4)}),
                                                 ((), (), 60, None, [], {'do': used(3)})]),
    # In the town's first equipment store (`building`), set up to shop
    # (`shopping`): two frames, the store's services (`serviced`): a
    # weapon bought, an item sold, one repaired (STORE_SCRIPT), thirty
    # frames on.
    # A townsperson talked to: "Where is" (its box at 5Dh..74h), its
    # first line (the places of its first kind; the lines' box at 48h,
    # 13h, 9 rows high), 1194:E087's sorted list's first line (the way
    # told, its box clicked away), then the menu's Goodbye.
    'playtalkwhere': ('people', None, [((), (), 2, None), ((), (), 2, None, [], {'do': talked(TALK_WHERE_SCRIPT)})]),
    # "Where is" and its fourth line, the palace (marked by the first
    # click, taken by the second: 3159:0A1D; the way told at once,
    # 1194:E0DE): the list's DI, A872, as the talk's menu left it.
    'playtalkpalace': ('people', None, [((), (), 2, None), ((), (), 2, None, [], {'do': talked(TALK_PALACE_SCRIPT)})]),
    # As playtalkwhere, the staff's line at stage 3 (`dreaming`): the
    # list from 1044 with the stage's place tenth (1194:E003), its first
    # line taken (E04F: not the quest's, on as E074).
    'playtalkquest': ('people', dreaming, [((), (), 2, None), ((), (), 2, None, [], {'do': talked(TALK_WHERE_SCRIPT)})]),
    # The same list's tenth line: the down arrow (35h, 65h: 3159:09E8)
    # scrolls it into the last row, clicked twice (marked, then taken):
    # the quest's place (E015: A862's 9), its person's words (1194:D593:
    # one box, clicked away), back to the menu's Goodbye.
    'playtalkquestplace': ('people', dreaming, [((), (), 2, None), ((), (), 2, None, [], {'do': talked(TALK_QUEST_SCRIPT)})]),
    'playstore': ('store', shopping, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xF48E, STORE_SCRIPT)})]),
    # The dungeon, two frames, the player killed (`killed`) and the frame
    # that ends the world loop for it, the outer loop's death followed to
    # its way back to the program's menu (`died`; the save's F77 is 9: no
    # vision).
    'playdeath': ('dungeon', None, [((), (), 2, None), ((), (), 1, None, [], {'do': killed, 'dies': True})]),
    # The same death, then the program's main back to its menu (`titled`:
    # 0000:109B, 10B0, 108B, 10C9: the intro's movies and pictures, then
    # MENU.IMG), stopped at the menu's wait for its regions.
    'playtitle': ('dungeon', None, [((), (), 2, None), ((), (), 1, None, [], {'do': killed, 'dies': True, 'title': True})]),
    # playtitle, then the menu's Load (its region clicked: 0000:117E, the song stopped, the
    # game set up, 21EE:016C, the page cleared, PAL.COL's colours; the load
    # menu, 25FD:0242) and slot 1's row picked: the game loaded the
    # title's way (5C8 bit 4), stopped as its world loop is about to run
    # its first frame (`menu_picked`).
    'playtitleload': ('dungeon', None, [((), (), 2, None), ((), (), 1, None, [], {'do': killed, 'dies': True, 'title': True,
                                                                                    'menu': [('click', 160, 30), ('click', 160, 21)]})]),
    # The options screen's New between frames (`newgame`), then the frame
    # that ends the world loop for it, the outer loop's way out (0CDF, as
    # `died` stops at its RET, 0D43) and the program's main back to its
    # menu (`titled`).
    'playnew': ('dungeon', None, [((), (), 2, None), ((), (), 1, None, [], {'do': newgame, 'dies': True, 'title': True})]),
    # The street, two frames, slot 1 loaded in play between frames
    # (`loaded`), then play goes on in the game loaded: its frames drawn
    # on, over the page the street's left.
    'playloadplay': ('street', None, [((), (), 2, None), ((), (), 4, None, [], {'do': loaded})]),
    # The same death at the main quest's third stage (`farewell`): the
    # vision (1194:D459) on the way out, its pages clicked through
    # (`vision`: a click at the screen's centre for each).
    'playfarewell': ('dungeon', farewell, [((), (), 2, None), ((), (), 1, None, [], {'do': killed, 'dies': True, 'vision': 8})]),
    # In the mages' guild the load leaves (`guild`), a slot free and
    # seeded: two frames, a potion bought (GUILD_SCRIPT), thirty frames.
    # In the town's first tavern with the main quest's informant there
    # (`informant`): two frames, the keeper's services (the informant
    # first, 1194:CED7: his price haggled and accepted, the dungeon told),
    # thirty frames.
    'playinformant': ('tavern', informant, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xB93C, INFORMANT_SCRIPT)})]),
    'playguild': ('guild', guild_shopping, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xA73E, GUILD_SCRIPT)})]),
    # In the town's first temple: two frames, a service (the keeper's
    # click, 1194:C258), thirty frames. Heal with the player hurt
    # (`temple_hurt`), Cure with a disease (`diseased`), Bless with a
    # donation of 100.
    'playtempleheal': ('temple', temple_hurt, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xC258, TEMPLE_HEAL_SCRIPT)})]),
    'playtemplecure': ('temple', diseased, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xC258, TEMPLE_CURE_SCRIPT)})]),
    'playtemplebless': ('temple', gs.seeded, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xC258, TEMPLE_BLESS_SCRIPT)})]),
    # In the town's first tavern, seeded: two frames, a drink or a room
    # (1194:B93C), thirty frames.
    'playtaverndrink': ('tavern', gs.seeded, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xB93C, TAVERN_DRINK_SCRIPT)})]),
    'playtavernroom': ('tavern', gs.seeded, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xB93C, TAVERN_ROOM_SCRIPT)})]),
}


def guild(a, base):
    """The mages' guild as the load leaves it (the save's place)."""
    a.restore(base)
    return a.snapshot()


def building(where):
    """The snapshot of the game in the town's first `where` (gen_screens.py's
    PLACES: its entry list and count), in at its door from the street as
    gen_screens.py's `place` goes in, its words on the way in read."""
    def enter(a, base):
        a.restore(base)
        assert a.area_ended() == ('world',), 'the guild not left'
        at, count = gs.PLACES[where]
        assert a.r16(count), f'no {where} in the town'
        sq = a.r16(a.r16(at), seg=a.r16(at + 2))
        door = (sq & 0xFF00) | ((sq & 0xFF) << 1)
        assert a.enter(door) == ('world',), f'the {where} not entered'
        # The building's words on the way in (191F, which its first frame
        # shows in a box waiting for a click, 0000:4903) taken as read, as
        # gen_screens.py's screens take them.
        a.mu.mem_write(a.lin(a.ds, 0x191F), b'\0')
        return a.snapshot()
    return enter


def street(a, base):
    """The snapshot of the game in the street, the guild left (the
    outer loop's next area the city, 5E6 0)."""
    a.restore(base)
    assert a.area_ended() == ('world',), 'the guild not left'
    return a.snapshot()


def people(a, base):
    """`street`, the guild left from the seed SEED (gen_screens.py's
    'people'): the townspeople the street's arrival places (0000:45B2,
    1793) the same on both sides."""
    a.restore(base)
    gs.seeded(a)
    assert a.area_ended() == ('world',), 'the guild not left'
    return a.snapshot()


def weathered(w):
    """`street`, but its zone's weather (A5A + the zone at A51) `w` as the
    guild is left, so 0000:0F05 makes the street's weather from it (2
    rain, 3 snow) and its INF the weather's."""
    def enter(a, base):
        a.restore(base)
        zone = a.mu.mem_read(a.lin(a.ds, 0x0A51), 1)[0]
        a.mu.mem_write(a.lin(a.ds, 0x0A5A + zone), bytes([w]))
        assert a.area_ended() == ('world',), 'the guild not left'
        return a.snapshot()
    return enter


wet_street = weathered(2)


def wild(a, base):
    """The snapshot of the game just outside the city, the way a gate
    leaves it (0000:43A0, the city's trigger 7 or 8): the guild left for
    the street, the city written as the wilderness's blocks 1..4
    (217B:027A, its files kept in memory), the next area 1 (5E6), then
    the street left."""
    a.restore(base)
    assert a.area_ended() == ('world',), 'the guild not left'
    a.call_far(0x217B, 0x027A)
    a.mu.mem_write(a.lin(a.ds, 0x05E6), b'\x01')
    assert a.area_ended() == ('world',), 'the wilderness not entered'
    return a.snapshot()


def named(name):
    """The snapshot of the game just inside the named level `name`, the
    way the outer loop enters one (0000:0993, `Run::Start`): the guild
    left for the street, then the street left with the next area 2
    (5E6), no interior (D58) and the level's name at 0A36."""
    def enter(a, base):
        a.restore(base)
        assert a.area_ended() == ('world',), 'the guild not left'
        a.mu.mem_write(a.lin(a.ds, 0x05E6), b'\x02')
        a.w16(0x0D58, 0)
        a.mu.mem_write(a.lin(a.ds, 0x0A36), name + b'\0')
        assert a.area_ended() == ('world',), f'{name} not entered'
        return a.snapshot()
    return enter


crypt = named(CRYPT)

# A main quest dungeon whose start sees a square's cap (0000:CBFF).
CAPPED = b'39699021.MIF'
# A main quest dungeon with fog (its INF's, CB5C).
FOGGED = b'40516921.MIF'


def track_line(a, name, frame):
    lvl = a.mu.mem_read(a.lin(a.ds, 0xBB0A), 1)[0]
    vals = [frame, lvl, a.r16(0xBA62), a.r16(0xBA64), a.r16(0xBA66), a.r16(0xC3DA)]
    return f'track | {name} | {" ".join(f"{v:x}" for v in vals)}\n'


ROW_INPUTS = (0xC398, 0xC39A, 0xC3D2, 0xC3A2, 0xC3D8, 0xCCF6, 0xCCF4, 0xC3D6, 0xA914, 0xC3DA, 0xD80C)


def row_inputs(a):
    """What 0000:DA8E makes the row tables from now: the flags (of C398
    only bits 4000h and 40h, of C39A bit 8, all it reads), the eye's
    height, the horizon's rows, the view's scales, its left edge, the
    heading and the ceiling's height."""
    v = [a.r16(o) for o in ROW_INPUTS]
    v[0] &= 0x4040
    v[1] &= 8
    return v


def rows_watch(a):
    """Each call of 0000:DA8E noted (`a.rows_made`, its inputs) and each
    drawing (0000:A31D) given the last's (`a.rows_drawn`); a snapshot
    keeps the last call's, a restore brings it back."""
    a.rows_made = None
    a.rows_drawn = []
    kept = []

    def made(*_):
        a.rows_made = row_inputs(a)

    def drawn(*_):
        a.rows_drawn.append(a.rows_made if a.rows_made is not None else row_inputs(a))
    a.mu.hook_add(UC_HOOK_CODE, made, None, a.lin(LOAD_SEG, 0xDA8E), a.lin(LOAD_SEG, 0xDA8E))
    a.mu.hook_add(UC_HOOK_CODE, drawn, None, a.lin(LOAD_SEG, 0xA31D), a.lin(LOAD_SEG, 0xA31D))
    snapshot, restore = a.snapshot, a.restore

    def snapshot_rows():
        snap = snapshot()
        kept.append((snap, a.rows_made))
        return snap

    def restore_rows(snap):
        restore(snap)
        a.rows_made = next((made for s, made in kept if s is snap), None)
    a.snapshot, a.restore = snapshot_rows, restore_rows


def rows_line(a, name):
    drawn = a.rows_drawn
    return f'rows | {name} | {len(drawn)} {fnv(b"".join(struct.pack("<11H", *v) for v in drawn)):x}\n'


def run_case(a, at, name, setup, steps, tracks):
    a.restore(at)
    a.rows_drawn = []
    a.waits = set()
    a.script = []
    a.clicks = []
    a.mouse = (160, 100)
    a.buttons = 0
    a.menu_loaded = False
    if setup:
        setup(a)
    # The state the frames start from (for the state comparison's
    # changes: worldtest's `state_measured`).
    a.pre_state = (bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000)), b''.join(a.ems_bank(b) for b in range(0x20)))
    def no_cursor(*_):
        a.w16(0x7A48, 0xFFFF)
    cursor = a.mu.hook_add(UC_HOOK_CODE, no_cursor, None, a.lin(LOAD_SEG, CURSOR_DRAW), a.lin(LOAD_SEG, CURSOR_DRAW))
    try:
        return play(a, name, steps, tracks)
    finally:
        a.mu.hook_del(cursor)


def play(a, name, steps, tracks):
    frame = [0]
    drag = [None]
    def each(a):
        tracks.append(track_line(a, name, frame[0]))
        frame[0] += 1
        # The step's drag: the mouse moved by it before each frame.
        if drag[0] is not None:
            a.motion = drag[0]
    for held, let_go, n, until, *more in steps:
        a.clicks.extend(more[0] if more else [])
        # A step's options: the buttons held through it (`buttons`, 2 the
        # right), the mouse's motion before each of its frames (`drag`).
        opts = more[1] if len(more) > 1 else {}
        if 'buttons' in opts:
            a.buttons = opts['buttons']
        # (`script`: actions for the waits for input on the way, a box's
        # loop round the mouse's read, 0000:8FC4, which a frame reads once;
        # `waits`, other reads a screen's loop asks by.)
        if 'script' in opts:
            a.waits = {MOUSE_READ} | opts.get('waits', set())
            a.script = list(opts['script'])
        drag[0] = opts.get('drag')
        a.motion = (0, 0)
        # (`do`: something the player does between frames, before the
        # step's: an item used, a key's screen.)
        if 'do' in opts:
            opts['do'](a)
        if held:
            a.keys_held(held)
        if let_go:
            a.keys_held(let_go, down=False)
        why, _ = a.world_frames(n, until=until, each=each)
        if opts.get('dies'):
            # (`dies`: the frames end the world loop, the player dead: the
            # outer loop's death followed, `died`; the case ends there.)
            assert why == ('ended',), f'{name}: stopped by {why}'
            # (`vision`: the farewell's pages clicked through, `died`, and
            # the install's speech served, SPEECH\\nnnn_mmX.VOC, as
            # gen_dream.py serves it: without it the movie's player copies
            # FFDCh bytes from the bank into a stale buffer (2F75:00DC) and
            # the heap fails.)
            names = dict(a.names)
            speech = os.path.join(a.game_dir, 'SPEECH')
            if opts.get('vision') and os.path.isdir(speech):
                a.names.update({n.upper(): os.path.join(speech, n) for n in os.listdir(speech)})
            try:
                died(a, opts.get('vision', 0))
            finally:
                a.names = names
            if opts.get('title'):
                titled(a)
            if 'menu' in opts:
                # (`menu`: the menu's inputs after `title`, on to the
                # loaded game's first frame, `menu_picked`.)
                menu_picked(a, opts['menu'])
            page = a.page()
            return page, page, bytes([1]) * 64000, bytes(a.dac)
        clicked = any(act[0] in ('click', 'rclick') for act in opts.get('script', []))
        if why == ('ended',) and opts.get('leaves'):
            # The area left in the step's frame (a door, a gate, a
            # journey): the outer loop on into the next (`area_next`, its
            # boxes on the way in read by the script), stopped before its
            # first frame, which the next step's first top sees; a
            # screen's clicks done, the mouse back at the view's centre
            # first, as below.
            why = a.area_next()
            assert why == ('world',), f'{name}: the next area stopped by {why}'
            if opts.get('ends'):
                # (`ends` with `leaves`: the case ends there, before the
                # next area's first frame, whose top isn't reached.)
                page = a.page()
                return page, page, bytes([1]) * 64000, bytes(a.dac)
            if clicked:
                a.mouse = (160, 100)
                a.clicks = []
                a.buttons = 0
            continue
        assert why == ('frames',), f'{name}: stopped by {why}'
        if opts.get('ends'):
            # (`ends`: the case ends with the step's last frame, a screen
            # its menu key opened inside it (2542:0100 from 0000:4952)
            # closed and the rest of that frame's body run after it: the
            # state as that frame leaves it, no frame after.)
            tracks.pop()
            page = a.page()
            return page, page, bytes([1]) * 64000, bytes(a.dac)
        if clicked:
            # A screen's clicks done, the mouse back at the view's centre,
            # where the view's controls take it to be (2826:00F0); the
            # last click's release, which the screen left for the next
            # read, the screen's (the window gives the screen whole clicks).
            a.mouse = (160, 100)
            a.clicks = []
            a.buttons = 0
        # The top of the frame is seen again as the next run's first.
        tracks.pop()
        frame[0] -= 1
    drag[0] = None
    a.motion = (0, 0)
    # The last frame again, its page's writes watched.
    base = a.lin(a.r16(PAGE_SEG_FIELD), 0)
    last = bytearray(64000)
    def wrote(mu, access, addr, size, value, _):
        st = a.mu.mem_read(a.lin(a.ds, 0xBA4B), 1)[0]
        for k in range(size):
            o = addr - base + k
            if 0 <= o < 64000:
                last[o] = 1 if st in DRAWING else 2
    h = a.mu.hook_add(UC_HOOK_MEM_WRITE, wrote, None, base, base + 64000 - 1)
    try:
        why, _ = a.world_frames(1, each=each)
    finally:
        a.mu.hook_del(h)
    assert why == ('frames',), f'{name}: the last frame stopped by {why}'
    tracks.pop()
    full = a.page()
    mask = bytes(int(k != 1) for k in last)
    page = bytes(x if k else 0 for x, k in zip(full, mask))
    return full, page, mask, bytes(a.dac)


def excluded(name='stateexclude.txt'):
    """The DGROUP offsets tests/vectors/stateexclude.txt (or `name`)
    leaves out."""
    out = set()
    path = os.path.join(os.path.dirname(__file__), '..', 'tests', 'vectors', name)
    for line in open(path):
        f = line.split('#')[0].split()
        if len(f) >= 2:
            out.update(range(int(f[0], 16), int(f[1], 16)))
    return out


def state_runs(kind, name, before, after):
    """What changed between `before` and `after` (the EMS banks 0..1Fh,
    64K each, end to end): how many bytes, the digest of their offsets
    (each four bytes, little-endian) and of their bytes after."""
    changed = [i for i in range(len(after)) if before[i] != after[i]]
    offs = fnv(b''.join(struct.pack('<I', i) for i in changed))
    return f'{kind} | {name} | {len(changed):x} {offs:x} | {fnv(bytes(after[i] for i in changed)):x}\n'


def state_line(name, before, after, skip, kind='state'):
    """What the case's frames changed in DGROUP (the offsets left out
    aside): the offsets, as runs `start+length`, and the digest of their
    bytes after."""
    changed = [i for i in range(0x10000) if before[i] != after[i] and i not in skip]
    runs = []
    for i in changed:
        if runs and runs[-1][0] + runs[-1][1] == i:
            runs[-1][1] += 1
        else:
            runs.append([i, 1])
    offs = ' '.join(f'{s:x}+{n:x}' for s, n in runs)
    return f'{kind} | {name} | {offs} | {fnv(bytes(after[i] for i in changed)):x}\n'


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    a = ScreenArena(EXE, GAME, SAVE)
    rows_watch(a)
    a.start()
    base = a.snapshot()
    loaded = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    loaded_ems = b''.join(a.ems_bank(b) for b in range(0x20))
    if dump:
        # The state as the load leaves it (the state comparison's start,
        # worldtest's `state_measured`, before the places' entries).
        os.makedirs(dump, exist_ok=True)
        open(os.path.join(dump, 'base.ds'), 'wb').write(bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000)))
        open(os.path.join(dump, 'base.ems'), 'wb').write(b''.join(a.ems_bank(b) for b in range(0x20)))
    places = {}
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    out = open(OUT, 'w', buffering=1)
    # The state as the title's Load leaves it: DGROUP, the offsets
    # tests/vectors/loadexclude.txt names zeroed (the port's in this form:
    # worldtest's `load_canonical`).
    skip_base = excluded('loadexclude.txt')
    if not only:
        out.write(f'loadstate | {fnv(bytes(0 if i in skip_base else b for i, b in enumerate(loaded))):x}\n')
        out.write(f'loadems | {fnv(loaded_ems):x}\n')
    skip = excluded()
    skip_load = skip | excluded('stateloadexclude.txt') | excluded('loadexclude.txt')
    for name, (where, setup, steps) in CASES.items():
        if only and name not in only:
            continue
        if where not in places:
            places[where] = {'dungeon': gd.dungeon, 'crypt': crypt, 'street': street, 'wetstreet': wet_street, 'snowstreet': weathered(3), 'wild': wild, 'capped': named(CAPPED), 'fogged': named(FOGGED),
                             'people': people, 'store': building('store'), 'tavern': building('tavern'), 'temple': building('temple'), 'guild': guild}[where](a, base)
        at = places[where]
        print(name, file=sys.stderr)
        seen = []

        def sample(*_):
            if not seen or seen[-1] != bytes(a.dac):
                seen.append(bytes(a.dac))
        hooks = [a.mu.hook_add(UC_HOOK_CODE, sample, None, a.lin(LOAD_SEG, o), a.lin(LOAD_SEG, o)) for o in (0x6BB8, 0x9B77)]
        tracks = []
        try:
            full, page, mask, dac = run_case(a, at, name, setup, steps, tracks)
        finally:
            for h in hooks:
                a.mu.hook_del(h)
        for t in tracks:
            out.write(t)
        if len(seen) > 1:
            out.write(f'screenfade | {name} | {len(seen)} | {fnv(b"".join(struct.pack("<Q", fnv(d)) for d in seen)):x}\n')
        out.write(rows_line(a, name))
        if a.printed:
            out.write(f'printed | {name} | {len(a.printed):x} {fnv(bytes(a.printed)):x}\n')
        after = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
        out.write(state_line(name, a.pre_state[0], after, skip))
        out.write(state_line(name, loaded, after, skip_load, 'stateload'))
        ems = b''.join(a.ems_bank(b) for b in range(0x20))
        out.write(state_runs('stateems', name, a.pre_state[1], ems))
        out.write(f'screen | {name} | {fnv(page):x} {fnv(dac):x} {fnv(mask):x}\n')
        if a.menu_loaded:
            out.write(f'interface | {name} | {interface_digest(full):x}\n')
        out.write(f'view3d | {name} | {fnv(bytes(x if not k else 0 for x, k in zip(full, mask))):x}\n')
        # The 3D pixels from row 1 on: the topmost row of a ceiling reads
        # past its row table (DB78's last entry, the next heap block's
        # header), whose words are the original's heap's segments.
        out.write(f'view3d1 | {name} | {fnv(bytes(x if not k and i >= 320 else 0 for i, (x, k) in enumerate(zip(full, mask)))):x}\n')
        if dump:
            os.makedirs(dump, exist_ok=True)
            for ext, data in (('page', page), ('mask', mask), ('dac', dac), ('full', full)):
                open(os.path.join(dump, f'{name}.{ext}'), 'wb').write(data)
            # The game's state as the case leaves it, for the view's state
            # comparison (worldtest's `state_measured`): DGROUP, and the
            # EMS banks 0..1Fh.
            open(os.path.join(dump, f'{name}.ds'), 'wb').write(bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000)))
            open(os.path.join(dump, f'{name}.ems'), 'wb').write(b''.join(a.ems_bank(b) for b in range(0x20)))
            open(os.path.join(dump, f'{name}.ds0'), 'wb').write(a.pre_state[0])
            open(os.path.join(dump, f'{name}.ems0'), 'wb').write(a.pre_state[1])
    out.close()


if __name__ == '__main__':
    main()
