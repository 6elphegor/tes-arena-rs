"""Emit the original's area transitions played from the user's saved
game, for view/src/areaplaytest.rs: the stairs down and back up, the
wilderness's blocks crossed each way, a city's gate out and in again,
buildings entered and left, fast travel to another place, a dungeon in
the wilderness entered, each followed through the frames after.

Usage: gen_areaplay.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [NAME...]

The cases are run as gen_worldframes.py runs its (the same places, the
keys held through the key interrupt, the clicks queued for the mouse's
reads, the timer ticking at the frame's waits); a step that leaves the
area (`leaves`: a door or a gate clicked) goes on through the outer loop
into the next area (`ScreenArena.area_next`), its boxes on the way in
(a building's welcome) read with Space, and the case plays on there.

The lines are gen_worldframes.py's for each case: the player's track
(the level, the physics' place, the heading at the top of each frame),
what the frames changed in DGROUP from the case's start (`state`) and
from the load (`stateload`), what they changed in the EMS banks from the
case's start (`stateems`), and the page as the last frame's 2D drawing
left it (`screen`):

  track | NAME | <frame> <level> <x> <height> <y> <heading>
  state | NAME | <start+length ...> | <digest>
  stateload | NAME | <start+length ...> | <digest>
  stateems | NAME | <count> <offsets' digest> | <digest>
  screen | NAME | <page digest> <palette digest> <mask digest>

and, where a box opens in a frame over that frame's drawing (0000:8F2A
keeping what it covers: the palace's audience), the 3D view's rows of
the page (the first 92h) as the first such box finds them, the frame's
number the track's:

  boxview | NAME | <frame> <digest>

and, where a province's map opens (26F7:0176), the page as its picture
and its places' icons leave it (26F7:01CD, before its copy to bank 4),
the first such:

  provincepage | NAME | <digest>

and, for a journey's days (1194:30F1), what each day shows once its
frame of the travel animation is presented (312C, after 0000:9466): the
screen (the VGA's memory) and the colours (the DAC, 6-bit), with how
many waits for the retrace (0000:6BA8) and reads of the game's timer
(0151) came since the day before (or the days' start, 30F1): none, the
days run as fast as the machine does.

  journey | NAME | <day> <screen digest> <colours digest> <retraces> <ticks>

and the journey as shown, from Travel's first fade (26F7:081E, 1194:55B2)
to its handler's end (08F6, after the last 55B2): each wait for the
retrace (r: 0000:6BA8, and 9B74 in the colours' setting) and each day
presented (d) with the screen and colours then,
runs of the same counted:

  journeyshow | NAME | <r|d> <screen digest> <colours digest> <count>

With --dump DIR, each case's NAME.ds/.ems (as it ends) and NAME.ds0/
.ems0 (as it starts) for worldtest's state reports (game data: never
commit them).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, fnv
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ
from screens import ScreenArena, PAGE_SEG_FIELD
import gen_dungeonscreens as gd
import gen_worldframes as w

UP = 0x48
LEFT = 0x4B
SPACE = 0x39
SHIFT = 0x2A
M = 0x32


def level_is(n):
    return lambda a: a.mu.mem_read(a.lin(a.ds, 0xBB0A), 1)[0] == n


def heading_near(h):
    """The heading within 10h of `h` (the turn keys step it by more than
    one a frame)."""
    return lambda a: ((a.r16(0xC3DA) - h + 0x10) & 0x1FF) <= 0x20


# The script a building's welcome box (its wait for input, 0000:8FC4
# round and round) is read by.
BOXES = {'script': [('keys', [SPACE])] * 4}
# A click in the middle of the view (the mouse there: its column the
# drawing's pick, CB92), the area left in its frame.
CLICK = [(160, 100)]
LEAVES = dict(BOXES, leaves=True)
# The travel map's clicks (gen_screens.py's `worldunwell`'s, well): High
# Rock's region, the place nearest (200, 100), its journey's box read,
# Travel.
TRAVEL = [('click', 70, 45), ('click', 200, 100), ('click', 200, 100), ('click', 62, 180)]


def poisoned(put):
    """`put`, then the player poisoned (05B1 bit 0): unwell for the
    journey's question (26F7:07CD's 5B1 & 7), the poison's damage on
    each of its days (0000:142D)."""
    def setup(a):
        put(a)
        a.mu.mem_write(a.lin(a.ds, 0x05B1), bytes([a.mu.mem_read(a.lin(a.ds, 0x05B1), 1)[0] | 1]))
    return setup


def wild_at(x=None, y=None, heading=0):
    """The wilderness's player put at `x` and/or `y` (the place the
    physics moves, BA62/BA66, and the eye's, C3D0/C3D4) with the last
    place the window's quarters are measured from (0600, 0602) the same,
    facing `heading`: walking on crosses into the window's first or last
    quarter and 217B:0434 moves the origin a block."""
    def put(a):
        if x is not None:
            for o in (0xC3D0, 0xBA62, 0x0600):
                a.w16(o, x)
        if y is not None:
            for o in (0xC3D4, 0xBA66, 0x0602):
                a.w16(o, y)
        a.w16(0xC3DA, heading)
        a.origin0 = (a.r16(0x0608), a.r16(0x060A))
    return put


def origin_moved(a):
    """The wilderness's window moved a block (its origin, 0608 and 060A,
    no longer the case's start's)."""
    return (a.r16(0x0608), a.r16(0x060A)) != a.origin0


# The player just inside the window's third quarter, facing south (y
# grows): walking on crosses into the last and the origin moves a block
# south.
wild_south = wild_at(y=0x2FF8, heading=0)
# A crossing south: the up arrow held till the window moves, let go for
# a frame, the player put back before the next quarter (`wild_south`).
WILD_SOUTH = [((UP,), (), 12, origin_moved), ((), (UP,), 1, None, [], dict(do=wild_south))]


def before_entrance(a):
    """The player on the square south of WILD065's entrance (row 49,
    column 12 of the window's first row of blocks), 20h into it, facing
    north (100h), the last place the same."""
    x, y = (12 << 7) + 0x40, (50 << 7) + 0x20
    for o in (0xC3D0, 0xBA62, 0x0600):
        a.w16(o, x)
    for o in (0xC3D4, 0xBA66, 0x0602):
        a.w16(o, y)
    a.w16(0xC3DA, 0x100)


def building(row, col, heading, back=None):
    """Before a building's door in the street (`at_square`), two frames,
    the door clicked (the area left for its interior), three frames
    inside (its welcome read), the way out in front clicked (an
    interior's every trigger leaves it, 0000:44BD), three frames in the
    street. With `back`, the way out is behind: the left arrow held till
    the player faces `back` first."""
    turn = [((LEFT,), (), 40, heading_near(back)), ((), (LEFT,), 10, None, CLICK, LEAVES)] if back is not None \
        else [((), (), 10, None, CLICK, LEAVES)]
    return ('street', w.at_square(row, col, heading),
            [((), (), 2, None), ((), (), 10, None, CLICK, LEAVES), ((), (), 3, None)] + turn + [((), (), 3, None)])


CASES = {
    # The first level's down stairs walked into (`playdown`'s), then the
    # left arrow held until the player faces back up the corridor, the
    # up arrow held into the up stairs: the first level again, three
    # frames on.
    'transitstairs': ('dungeon', w.at_square(11, 10, 0x100),
                      [((UP,), (), 20, level_is(1)), ((LEFT,), (UP,), 40, heading_near(0x100)),
                       ((UP,), (LEFT,), 30, level_is(0)), ((), (UP,), 3, None)]),
    # The wilderness's window's quarters crossed each way from the
    # place a gate leaves the player (`wild`): north (y just inside the
    # second quarter, 100h), south (just inside the third, 0), west (x,
    # 180h) and east (80h); the up arrow held till the window moves, then
    # three frames.
    'transitwildnorth': ('wild', wild_at(y=0x1008, heading=0x100), [((UP,), (), 12, origin_moved), ((), (UP,), 3, None)]),
    'transitwildsouth': ('wild', wild_at(y=0x2FF8, heading=0), [((UP,), (), 12, origin_moved), ((), (UP,), 3, None)]),
    'transitwildwest': ('wild', wild_at(x=0x1008, heading=0x180), [((UP,), (), 12, origin_moved), ((), (UP,), 3, None)]),
    'transitwildeast': ('wild', wild_at(x=0x2FF8, heading=0x80), [((UP,), (), 12, origin_moved), ((), (UP,), 3, None)]),
    # Daggerfall's west gate (column 0, row 15: trigger 7) clicked from
    # the square inside it, facing it: the wilderness, three frames; the
    # left arrow held till the player faces the gate again (80h), the
    # gate clicked from outside (the wilderness's trigger 6): the city,
    # three frames.
    'transitgate': ('street', w.at_square(15, 1, 0x180),
                    [((), (), 2, None), ((), (), 10, None, CLICK, LEAVES), ((), (), 3, None),
                     ((LEFT,), (), 40, heading_near(0x80)), ((), (LEFT,), 10, None, CLICK, LEAVES), ((), (), 3, None)]),
    # The town's first tavern (row 8, column 6Ch), its temple (8, 33h),
    # its equipment store (4, 6Ch), the palace (8, 7Dh) and the mages'
    # guild (5Eh, 34h), each from the street square before its door (the
    # tavern's and the temple's the square of their sign, bit 8000h: walked
    # on). The temple's, the palace's and the guild's ways out are two
    # squares south of their entries (heading 0).
    'transittavern': building(7, 108, 0),
    'transittemple': building(8, 50, 0x80, back=0),
    'transitstore': building(3, 108, 0),
    'transitpalace': building(8, 124, 0x80, back=0),
    'transitguild': building(95, 52, 0x100, back=0),
    # Fast travel from the street before the guild: two frames, Shift+M
    # held (2542:03BF in the frame's input: 26F7's travel map), High
    # Rock's region clicked (the province's map), the place nearest
    # (200, 100) clicked (Wind Keep: the journey's box), the box read
    # with a click, Travel (62, 180): the days pass, the screen ends with
    # the carry and the area is left; the outer loop on into the village,
    # its welcome (the arrival's text) read with Space; three frames.
    'travelfast': ('street', w.at_square(95, 52, 0x100),
                   [((), (), 2, None),
                    ((SHIFT, M), (), 1, None, [], dict(script=TRAVEL + BOXES['script'], leaves=True)),
                    ((), (SHIFT, M), 3, None)]),
    # `travelfast` poisoned: Travel asks "Considering your condition..."
    # (1194:48DB's menu, 6B1D) and its Yes (the first entry, (160, 118))
    # travels anyway, the poison working through the journey's days.
    'travelunwell': ('street', poisoned(w.at_square(95, 52, 0x100)),
                     [((), (), 2, None),
                      ((SHIFT, M), (), 1, None, [], dict(script=TRAVEL + [('click', 160, 118)] + BOXES['script'], leaves=True)),
                      ((), (SHIFT, M), 3, None)]),
    # A dungeon in the wilderness: the window crossed south three times
    # (`wild_south`), which brings the grid's block (1Fh, 23h), WILD065
    # (its entrance, a 3333h wall: trigger 8), to the window's first
    # row; the player put before it (`before_entrance`), two frames, the
    # entrance clicked (217B:04DC, 0000:08EC: the area left), the outer
    # loop on into the dungeon; three frames.
    'wilddungeon': ('wild', wild_south,
                    WILD_SOUTH * 2 + [((UP,), (), 12, origin_moved), ((), (UP,), 1, None, [], dict(do=before_entrance)),
                                      ((), (), 2, None), ((), (), 10, None, CLICK, LEAVES), ((), (), 3, None)]),
}


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    a = ScreenArena(EXE, GAME, SAVE)
    w.rows_watch(a)
    a.start()
    base = a.snapshot()
    loaded = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    if dump:
        # The state as the load leaves it (worldtest's state report from
        # the load, ARENA_FROM=load).
        os.makedirs(dump, exist_ok=True)
        open(os.path.join(dump, 'base.ds'), 'wb').write(loaded)
        open(os.path.join(dump, 'base.ems'), 'wb').write(b''.join(a.ems_bank(b) for b in range(0x20)))
    skip = w.excluded()
    skip_load = skip | w.excluded('stateloadexclude.txt') | w.excluded('loadexclude.txt')
    places = {}
    out = open(OUT, 'w', buffering=1)
    for name, (where, setup, steps) in CASES.items():
        if only and name not in only:
            continue
        if where not in places:
            places[where] = {'dungeon': gd.dungeon, 'street': w.street, 'wild': w.wild}[where](a, base)
        print(name, file=sys.stderr)
        tracks = []
        boxed = []

        def box(*_):
            if not boxed:
                seg = a.r16(PAGE_SEG_FIELD)
                boxed.append((len(tracks) - 1, fnv(bytes(a.mu.mem_read(a.lin(seg, 0), 0x92 * 320)))))
        hook = a.mu.hook_add(UC_HOOK_CODE, box, None, a.lin(LOAD_SEG, 0x8F2A), a.lin(LOAD_SEG, 0x8F2A))
        province = []

        def drawn(*_):
            if not province:
                seg = a.r16(PAGE_SEG_FIELD)
                province.append(fnv(bytes(a.mu.mem_read(a.lin(seg, 0), 64000))))
        hook2 = a.mu.hook_add(UC_HOOK_CODE, drawn, None, a.lin(LOAD_SEG + 0x26F7, 0x01CD), a.lin(LOAD_SEG + 0x26F7, 0x01CD))
        days = []
        waited = [0, 0]

        def retrace(*_):
            waited[0] += 1
            show('r')

        def tick(*_):
            waited[1] += 1

        def days_start(*_):
            waited[0] = waited[1] = 0

        shown = []
        showing = [False]

        def show(kind):
            if showing[0]:
                at = (kind, fnv(a.vga()), fnv(bytes(a.dac)))
                if shown and shown[-1][0] == at:
                    shown[-1][1] += 1
                else:
                    shown.append([at, 1])

        def show_on(*_):
            showing[0] = True

        def show_off(*_):
            showing[0] = False

        def day(*_):
            show('d')
            days.append(f'journey | {name} | {len(days)} {fnv(a.vga()):x} {fnv(bytes(a.dac)):x} {waited[0]} {waited[1]}\n')
            waited[0] = waited[1] = 0
        hooks = [a.mu.hook_add(UC_HOOK_CODE, retrace, None, a.lin(LOAD_SEG, 0x6BA8), a.lin(LOAD_SEG, 0x6BA8)),
                 a.mu.hook_add(UC_HOOK_MEM_READ, tick, None, a.lin(a.ds, 0x0151), a.lin(a.ds, 0x0152)),
                 a.mu.hook_add(UC_HOOK_CODE, days_start, None, a.lin(LOAD_SEG + 0x1194, 0x30F1), a.lin(LOAD_SEG + 0x1194, 0x30F1)),
                 a.mu.hook_add(UC_HOOK_CODE, day, None, a.lin(LOAD_SEG + 0x1194, 0x312C), a.lin(LOAD_SEG + 0x1194, 0x312C)),
                 a.mu.hook_add(UC_HOOK_CODE, lambda *_: show('r'), None, a.lin(LOAD_SEG, 0x9B74), a.lin(LOAD_SEG, 0x9B74)),
                 a.mu.hook_add(UC_HOOK_CODE, show_on, None, a.lin(LOAD_SEG + 0x26F7, 0x081E), a.lin(LOAD_SEG + 0x26F7, 0x081E)),
                 a.mu.hook_add(UC_HOOK_CODE, show_off, None, a.lin(LOAD_SEG + 0x26F7, 0x08F6), a.lin(LOAD_SEG + 0x26F7, 0x08F6))]
        try:
            full, page, mask, dac = w.run_case(a, places[where], name, setup, steps, tracks)
        finally:
            a.mu.hook_del(hook)
            a.mu.hook_del(hook2)
            for h in hooks:
                a.mu.hook_del(h)
        for t in tracks:
            out.write(t)
        after = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
        out.write(w.state_line(name, a.pre_state[0], after, skip))
        out.write(w.state_line(name, loaded, after, skip_load, 'stateload'))
        ems = b''.join(a.ems_bank(b) for b in range(0x20))
        out.write(w.state_runs('stateems', name, a.pre_state[1], ems))
        out.write(f'screen | {name} | {fnv(page):x} {fnv(dac):x} {fnv(mask):x}\n')
        for frame, digest in boxed:
            out.write(f'boxview | {name} | {frame} {digest:x}\n')
        for digest in province:
            out.write(f'provincepage | {name} | {digest:x}\n')
        for line in days:
            out.write(line)
        for (kind, vga, dac), n in shown:
            out.write(f'journeyshow | {name} | {kind} {vga:x} {dac:x} {n}\n')
        out.write(w.rows_line(a, name))
        if dump:
            os.makedirs(dump, exist_ok=True)
            for ext, data in (('ds', after), ('ems', ems), ('ds0', a.pre_state[0]), ('ems0', a.pre_state[1]),
                              ('page', page), ('mask', mask), ('dac', dac), ('full', full)):
                open(os.path.join(dump, f'{name}.{ext}'), 'wb').write(data)
    out.close()


if __name__ == '__main__':
    main()
