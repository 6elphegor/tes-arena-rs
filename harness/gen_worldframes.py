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
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, STUB_SEG, fnv
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_WRITE
from unicorn.x86_const import UC_X86_REG_FS
from screens import ScreenArena, PAGE_SEG_FIELD
import gen_dungeonscreens as gd

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


def night_encounter(heading):
    """At ten at night (`at_hour`) the wilderness's encounter as a roll
    that comes up brings it (1194:887A past its roll, 88AA..88C8): the
    night (1238, 1194:561C's), 5C8 bit 1000h cleared, the rank 1 (1294),
    and the row of the terrain (A50) plus 7 at night brought (89D4, BX
    the row)."""
    def put(a):
        at_hour(22, heading)(a)
        a.w16(0x1238, 1)
        a.w16(0x05C8, a.r16(0x05C8) & ~0x1000)
        a.w16(0x1294, 1)
        row = a.mu.mem_read(a.lin(a.ds, 0x0A50), 1)[0] + 7
        assert row <= 0xC, 'no encounter for the terrain'
        a.call_near(0x1194, 0x89D4, ebx=row)
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
    # The wilderness at night, its encounter brought (`night_encounter`),
    # stood in: what came comes for the player and strikes.
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
}


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


def run_case(a, at, name, setup, steps, tracks):
    a.restore(at)
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
        # loop round the mouse's read, 0000:8FC4, which a frame reads once.)
        if 'script' in opts:
            a.waits = {MOUSE_READ}
            a.script = list(opts['script'])
        drag[0] = opts.get('drag')
        a.motion = (0, 0)
        if held:
            a.keys_held(held)
        if let_go:
            a.keys_held(let_go, down=False)
        why, _ = a.world_frames(n, until=until, each=each)
        assert why == ('frames',), f'{name}: stopped by {why}'
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
    a.start()
    base = a.snapshot()
    loaded = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
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
    skip = excluded()
    skip_load = skip | excluded('stateloadexclude.txt') | excluded('loadexclude.txt')
    for name, (where, setup, steps) in CASES.items():
        if only and name not in only:
            continue
        if where not in places:
            places[where] = {'dungeon': gd.dungeon, 'crypt': crypt, 'street': street, 'wetstreet': wet_street, 'snowstreet': weathered(3), 'wild': wild, 'capped': named(CAPPED), 'fogged': named(FOGGED)}[where](a, base)
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
