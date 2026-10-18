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
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, STUB_SEG, fnv
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_FS, UC_X86_REG_SI
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


def crowded_square(a):
    """The player's square (CB90) given a byte in the plane at [D806],
    which 1194:5F13 takes for no room."""
    a.mu.mem_write(a.lin(a.r16(0xD806), a.r16(0xCB90)), bytes([1]))
# The automap's loop asks the mouse by the near entry (0000:2DB8: 8FC8).
AUTOMAP_READ = (0, 0x8FC8)

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


def died(a):
    """From the world loop's end (0000:A45D, on the world's stack as
    `area_ended` goes on from it) through the outer loop (0CAD) to its
    death's way out (0CE1..0D43: the clip the screen, the ending's text
    picked, 1194:D459's vision unless F77 is 9, the save's), stopped at its
    RET (0D43), before the program's main takes the game back to its menu
    (0000:10B0, 108B)."""
    ss, sp, words = a.world_stack
    a.mu.mem_write(a.lin(ss, sp), words)
    saved = a.stub(location=False)
    at = a.lin(LOAD_SEG, 0x0D43)

    def out(mu, addr, size, _):
        a.waiting = ('died',)
        mu.emu_stop()
    h = a.mu.hook_add(UC_HOOK_CODE, out, None, at, at)
    try:
        r = a.run_screen(0, 0xA45D, stack=(ss, sp))
    finally:
        a.mu.hook_del(h)
        a.unstub(saved)
    assert r == ('died',), f'the death went on to {r}'
    # The dying frame's tick: the harness ticks the timer (0151) at the
    # frame's waits for it (0000:A428, A43D), which a frame that ends the
    # world loop leaves before; the timer ticks all the same (the view's
    # frames tick before the frame).
    a.w16(0x0151, (a.r16(0x0151) + 1) & 0xFFFF)


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
GUILD_SCRIPT = [('click', 0xA0, 0x46), ('click', 0xA0, 0x59), ('click', 0x70, 0x26), ('keys', [0x02]), ('keys', [0x1C]), ACCEPT, ACCEPT,
                ('click', 0xA0, 0xA3)]


# The main quest's Crypt of Hearts: its .MIF (whose INFO names
# crypt1.inf).
CRYPT = b'41649375.MIF'

# name: (where, set-up, [(keys held, keys let go, frames, until[, clicks])...]):
# the clicks (x, y) are queued for the mouse's reads before the frames.
CASES = {
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
    'playghostclick': ('dungeon', ghost, [((), (), 1, None), ((), (), 10, message_up, [(160, 100)]), ((), (), 1, None)]),
    # `locked_door`'s door with a lock of level 1, picking armed
    # (`picking`), clicked: the roll picks it (2826:0AAB, 1194:53F9), the
    # door opens; then the up arrow held through it and let go.
    'playpick': ('crypt', picking(1), [((), (), 2, None), ((), (), 20, None, [(160, 100)]), ((UP,), (), 40, None), ((), (UP,), 10, None)]),
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
    'playstore': ('store', shopping, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xF48E, STORE_SCRIPT)})]),
    # The dungeon, two frames, the player killed (`killed`) and the frame
    # that ends the world loop for it, the outer loop's death followed to
    # its way back to the program's menu (`died`; the save's F77 is 9: no
    # vision).
    'playdeath': ('dungeon', None, [((), (), 2, None), ((), (), 1, None, [], {'do': killed, 'dies': True})]),
    # In the mages' guild the load leaves (`guild`), a slot free and
    # seeded: two frames, a potion bought (GUILD_SCRIPT), thirty frames.
    # In the town's first tavern with the main quest's informant there
    # (`informant`): two frames, the keeper's services (the informant
    # first, 1194:CED7: his price haggled and accepted, the dungeon told),
    # thirty frames.
    'playinformant': ('tavern', informant, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xB93C, INFORMANT_SCRIPT)})]),
    'playguild': ('guild', guild_shopping, [((), (), 2, None), ((), (), 30, None, [], {'do': serviced(0x1194, 0xA73E, GUILD_SCRIPT)})]),
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
            died(a)
            page = a.page()
            return page, page, bytes([1]) * 64000, bytes(a.dac)
        if why == ('ended',) and opts.get('leaves'):
            # The area left in the step's frame (a door, a gate): the
            # outer loop on into the next (`area_next`, its boxes on the
            # way in read by the script), stopped before its first
            # frame, which the next step's first top sees.
            why = a.area_next()
            assert why == ('world',), f'{name}: the next area stopped by {why}'
            continue
        assert why == ('frames',), f'{name}: stopped by {why}'
        if any(act[0] in ('click', 'rclick') for act in opts.get('script', [])):
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
                             'store': building('store'), 'tavern': building('tavern'), 'guild': guild}[where](a, base)
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
        after = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
        out.write(state_line(name, a.pre_state[0], after, skip))
        out.write(state_line(name, loaded, after, skip_load, 'stateload'))
        ems = b''.join(a.ems_bank(b) for b in range(0x20))
        out.write(state_runs('stateems', name, a.pre_state[1], ems))
        out.write(f'screen | {name} | {fnv(page):x} {fnv(dac):x} {fnv(mask):x}\n')
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
