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

The colours at each wait for the retrace on the way, when they change
more than once, as gen_screens.py's:

  screenfade | NAME | <count> | <digest>
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, fnv
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_WRITE
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


def night(heading):
    """Facing `heading` at ten at night of the save's day: the minutes
    (05CC) and the hour (05D6, minute 05D8 0) set to 22:00, the sky's time
    (CCFE) as 0000:16F7 makes it from them."""
    def put(a):
        a.w16(0xC3DA, heading)
        m = struct.unpack('<I', a.mu.mem_read(a.lin(a.ds, 0x05CC), 4))[0]
        m = m - m % 1440 + 22 * 60
        a.mu.mem_write(a.lin(a.ds, 0x05CC), struct.pack('<I', m))
        a.w16(0x05D6, 22)
        a.w16(0x05D8, 0)
        a.w16(0xCCFE, ((22 * 60) << 10) // 1440)
    return put


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
    'playlock': ('crypt', locked_door, [((), (), 2, None), ((), (), 10, message_up, [(160, 100)]), ((), (), 1, None)]),
}


def street(a, base):
    """The snapshot of the game in the street, the guild left (the
    outer loop's next area the city, 5E6 0)."""
    a.restore(base)
    assert a.area_ended() == ('world',), 'the guild not left'
    return a.snapshot()


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
    def no_cursor(*_):
        a.w16(0x7A48, 0xFFFF)
    cursor = a.mu.hook_add(UC_HOOK_CODE, no_cursor, None, a.lin(LOAD_SEG, CURSOR_DRAW), a.lin(LOAD_SEG, CURSOR_DRAW))
    try:
        return play(a, name, steps, tracks)
    finally:
        a.mu.hook_del(cursor)


def play(a, name, steps, tracks):
    frame = [0]
    def each(a):
        tracks.append(track_line(a, name, frame[0]))
        frame[0] += 1
    for held, let_go, n, until, *clicks in steps:
        a.clicks.extend(clicks[0] if clicks else [])
        if held:
            a.keys_held(held)
        if let_go:
            a.keys_held(let_go, down=False)
        why, _ = a.world_frames(n, until=until, each=each)
        assert why == ('frames',), f'{name}: stopped by {why}'
        # The top of the frame is seen again as the next run's first.
        tracks.pop()
        frame[0] -= 1
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


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    base = a.snapshot()
    places = {}
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    out = open(OUT, 'w', buffering=1)
    for name, (where, setup, steps) in CASES.items():
        if only and name not in only:
            continue
        if where not in places:
            places[where] = {'dungeon': gd.dungeon, 'crypt': crypt, 'street': street, 'wild': wild, 'capped': named(CAPPED), 'fogged': named(FOGGED)}[where](a, base)
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
    out.close()


if __name__ == '__main__':
    main()
