"""Emit the files the original writes, run on from the user's saved game,
for view/src/savetest.rs: what play writes on the way into a place, and
what a save made there writes, byte for byte (as digests).

Usage: gen_savefiles.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [NAME...]

The game is loaded from the title (harness/screens.py `start`) and taken
into a place as gen_worldframes.py takes it (the guild the load leaves,
the street, the wilderness out of the gate, the dungeon, the Crypt of
Hearts), its world loop run on for the frames of the case (keys held as
there). Files the game writes are kept in memory (ScreenArena's DOS
layer: never on disk), so what play wrote from the load to the case's
end is:

  play | NAME | <file> <length> <digest>

one line per file made or changed (its name as the game opened it, upper
case), and the files it deleted:

  playdel | NAME | <file> ...

Then the game is saved there as a player saves it: Esc's options screen
(2542:000A), its Save (160, 132), the sixth slot's row (100, 81: slot
5), "a" typed and Enter (gen_screens.py's optionssaved). What that wrote
and deleted:

  save | NAME | <file> <length> <digest>
  savedel | NAME | <file> ...

and that save loaded back from the title by a game started afresh on the
save's folder with the files the save wrote (`load_line`): the random
seed it reaches (the harness's clock's, which the view takes) and
DGROUP's digest as the load leaves it (at the world loop's first frame),
tests/vectors/loadexclude.txt's offsets 0 (as gen_worldframes.py's
loadstate):

  loadplace | NAME | <seed> | <digest>

The files are taken to the port's form first (`canonical`): SAVEGAME's
level block (DGROUP BB08.., FD00h on in the file) holds segments of the
image (the level's hooks, C3A4..C3BB), which the harness's load puts
1000h paragraphs higher than the port's; and where the level has no
ceiling (its plane's segment, D748, 0) the save writes the 32 KB at
0000:0000 as the ceiling's plane, the interrupt table on (the BIOS's and
DOS's memory, not the game's: the port's is its image), left out as 0s
(view/src/savetest.rs does the same to the view's).

Then gen_screens.py's screens, each run once as that runs it (from the
load, or in its building), and the files made, changed or deleted from
the load to the screen (the way into its building counted), where there
are any:

  screen | NAME | <file> <length> <digest>
  screendel | NAME | <file> ...

for view/src/screentest.rs's `screen`, which checks each screen's files
against these (none where a screen has no lines).

With --dump DIR, each file as written (in that form): DIR/NAME.play/<file>
and DIR/NAME.save/<file>, DIR/NAME.screen/<file> (game data: never commit
them).
"""
import os
import shutil
import sys
import tempfile

sys.path.insert(0, os.path.dirname(__file__))
import struct
from emu import DGROUP, LOAD_SEG, fnv
from unicorn import UC_HOOK_CODE
from screens import ScreenArena
import gen_dungeonscreens as gd
import gen_screens as gs
import gen_worldframes as gw

UP = gw.UP
# The random seed (DGROUP).
SEED = 0x91F6

# The save as optionssaved makes it.
SAVE_SCRIPT = [('click', 160, 132), ('click', 100, 81)] + [('keys', [k]) for k in (0x1E, 0x1C)]

# name: (where, set-up, steps as gen_worldframes.py's CASES).
CASES = {
    # Straight from the load, in the guild.
    'saveguild': ('guild', None, []),
    # The street outside the guild, three frames on.
    'savestreet': ('street', None, [((), (), 3, None)]),
    # Out of the city's gate, three frames on.
    'savewild': ('wild', None, [((), (), 3, None)]),
    # The dungeon's down stairs taken (gen_worldframes.py's playdown):
    # its second level.
    'savedungeon': ('dungeon', gw.at_square(11, 10, 0x100), [((UP,), (), 20, gw.level_changed), ((), (UP,), 2, None)]),
    # The Crypt of Hearts, its first text shown (playcrypt): a main
    # quest dungeon's locks kept (292B:095E).
    'savecrypt': ('crypt', gw.facing(0x100), [((UP,), (), 20, gw.text_shown), ((), (UP,), 1, None)]),
}


def image_segments(exe):
    """The DGROUP offsets of words holding a segment of the image: the
    image's relocations into DGROUP, and tests/vectors/loadsegs.txt's
    (stored as the game runs)."""
    d = open(exe, 'rb').read()
    count, table, header = struct.unpack_from('<H', d, 6)[0], struct.unpack_from('<H', d, 0x18)[0], struct.unpack_from('<H', d, 8)[0]
    out = set()
    for i in range(count):
        off, seg = struct.unpack_from('<HH', d, table + 4 * i)
        lin = seg * 16 + off - DGROUP * 16
        if 0 <= lin < 0xFFFF:
            out.add(lin)
    path = os.path.join(os.path.dirname(__file__), '..', 'tests', 'vectors', 'loadsegs.txt')
    for line in open(path):
        f = line.split('#')[0].split()
        if f:
            out.add(int(f[0], 16))
    return out


# SAVEGAME's level block: DGROUP BB08h, DE7h bytes, after the screen and
# the palette.
LEVEL_STATE, LEVEL_BYTES, LEVEL_AT = 0xBB08, 0x0DE7, 0xFA00 + 0x300


def canonical(segs, name, data, no_ceiling):
    """`data`, the file `name` as the original wrote it, with the image's
    segments in SAVEGAME's level block taken 1000h paragraphs down (the
    port's image sits at 0, the harness's at LOAD_SEG), and with
    `no_ceiling` the ceiling's plane (low memory) 0s."""
    if not name.startswith('SAVEGAME.') or len(data) < LEVEL_AT + LEVEL_BYTES:
        return data
    data = bytearray(data)
    if no_ceiling:
        at = LEVEL_AT + LEVEL_BYTES + 0x10000
        data[at:at + 0x8000] = bytes(len(data[at:at + 0x8000]))
    for off in segs:
        if LEVEL_STATE <= off < LEVEL_STATE + LEVEL_BYTES - 1:
            at = LEVEL_AT + off - LEVEL_STATE
            w = struct.unpack_from('<H', data, at)[0]
            if w >= LOAD_SEG:
                struct.pack_into('<H', data, at, w - LOAD_SEG)
    return bytes(data)


# Inputs past gen_screens.py's script for the files (see `main`): the
# failed theft's words clicked away, the store left (1194:F526 writes its
# stock and deletes its empty repairs).
ON = {'storesteal': [('click', 0xA0, 0x64)]}


# The cases loaded back outdoors, and the world frames run after their
# load (`load_line`): the street's and the wilderness's entry (the sky's
# colours, the dither) is made in the loop's first frame, past where the
# load stops.
FRAMES = {'savestreet': 3, 'savewild': 3}


def load_line(exe, game, save, name, saved, skip, dump=None):
    """The save just made loaded back from the title, by a game started
    afresh on a folder of the save's files and what the save wrote (as
    the view's save folder holds them): the random seed it reaches (the
    harness's clock's) and DGROUP's digest, `skip`'s offsets 0
    (tests/vectors/loadexclude.txt)."""
    tmp = tempfile.mkdtemp(prefix='arena-savefiles-')
    try:
        for n in os.listdir(save):
            shutil.copy(os.path.join(save, n), os.path.join(tmp, n))
        for n, d in saved.items():
            # Not the working slot's (*.64: the automap kept, 23F1:0145),
            # which the view keeps in memory, not in the save's folder.
            if not n.endswith('.64'):
                open(os.path.join(tmp, n), 'wb').write(d)
        b = ScreenArena(exe, game, tmp)
        b.start(5)
        seed = struct.unpack('<I', b.mu.mem_read(b.lin(b.ds, SEED), 4))[0]
        ds = bytes(b.mu.mem_read(b.lin(b.ds, 0), 0x10000))
        frames = []
        if name in FRAMES:
            frames = load_frames(b, FRAMES[name])
    finally:
        shutil.rmtree(tmp)
    if dump:
        open(os.path.join(dump, f'{name}.loadds'), 'wb').write(bytes(0 if i in skip else v for i, v in enumerate(ds)))
        for k, (fds, fems) in enumerate(frames, 1):
            open(os.path.join(dump, f'{name}.loadds{k}'), 'wb').write(fds)
            open(os.path.join(dump, f'{name}.loadems{k}'), 'wb').write(fems)
    out = f'loadplace | {name} | {seed:x} | {fnv(bytes(0 if i in skip else v for i, v in enumerate(ds))):x}\n'
    skip_frames = skip | gw.excluded()
    for k, (fds, fems) in enumerate(frames, 1):
        out += f'loadframe | {name} | {k} | {fnv(bytes(0 if i in skip_frames else v for i, v in enumerate(fds))):x} {fnv(fems):x}\n'
    return out


def load_frames(b, n):
    """The world loop run on `n` frames from the load (nothing held, the
    arrow cursor not drawn: gen_worldframes.py's `run_case`): DGROUP and
    the EMS banks 0..1Fh at the top of each frame after the first."""
    b.waits = set()
    b.script = []
    b.clicks = []
    b.mouse = (160, 100)
    b.buttons = 0
    def no_cursor(*_):
        b.w16(0x7A48, 0xFFFF)
    cursor = b.mu.hook_add(UC_HOOK_CODE, no_cursor, None, b.lin(LOAD_SEG, gw.CURSOR_DRAW), b.lin(LOAD_SEG, gw.CURSOR_DRAW))
    out = []
    try:
        for _ in range(n):
            why, _ = b.world_frames(1)
            assert why == ('frames',), f'the frame after the load stopped by {why}'
            out.append((bytes(b.mu.mem_read(b.lin(b.ds, 0), 0x10000)), b''.join(b.ems_bank(k) for k in range(0x20))))
    finally:
        b.mu.hook_del(cursor)
    return out


def files_changed(before, after):
    """The files of `after` (name: bytes) new or changed since `before`."""
    return {n: bytes(d) for n, d in after.items() if n not in before or bytes(before[n]) != bytes(d)}


def lines(kind, name, written, deleted):
    out = [f'{kind} | {name} | {n} {len(d):x} {fnv(d):x}\n' for n, d in sorted(written.items())]
    out.append(f'{kind}del | {name} | {" ".join(sorted(deleted))}\n')
    return out


def dump_files(dump, name, kind, written):
    d = os.path.join(dump, f'{name}.{kind}')
    os.makedirs(d, exist_ok=True)
    for n, data in written.items():
        open(os.path.join(d, n), 'wb').write(data)


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    segs = image_segments(EXE)
    skip_load = gw.excluded('loadexclude.txt')
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    base = a.snapshot()
    loaded = ({k: bytes(v) for k, v in a.written.items()}, set(a.deleted))
    places = {'guild': lambda a, b: b, 'street': gw.street, 'wild': gw.wild, 'dungeon': gd.dungeon, 'crypt': gw.crypt}
    at = {}
    out = open(OUT, 'w', buffering=1)
    for name, (where, setup, steps) in CASES.items():
        if only and name not in only:
            continue
        print(name, file=sys.stderr)
        if where not in at:
            at[where] = places[where](a, base)
        if steps or setup:
            gw.run_case(a, at[where], name, setup, steps, [])
        else:
            a.restore(at[where])
        played = {n: canonical(segs, n, d, a.r16(0xD748) == 0) for n, d in files_changed(loaded[0], a.written).items()}
        for l in lines('play', name, played, a.deleted - loaded[1]):
            out.write(l)
        if dump:
            dump_files(dump, name, 'play', played)
        before = ({k: bytes(v) for k, v in a.written.items()}, set(a.deleted))
        a.waits = {gs.WAIT}
        a.script = list(SAVE_SCRIPT)
        a.clicks = []
        a.mouse = (160, 100)
        a.buttons = 0
        a.run_screen(gs.MENU, 0x000A, near=True)
        assert not a.script, f'{name}: the save menu stopped early'
        saved = {n: canonical(segs, n, d, a.r16(0xD748) == 0) for n, d in files_changed(before[0], a.written).items()}
        for l in lines('save', name, saved, a.deleted - before[1]):
            out.write(l)
        if dump:
            dump_files(dump, name, 'save', saved)
        out.write(load_line(EXE, GAME, SAVE, name, files_changed(before[0], a.written), skip_load, dump))
    # gen_screens.py's screens, each run once as it runs them (in its
    # building), and what the run wrote, where it wrote anything. The
    # tavern looked at is noted on the automap at the square under the
    # cursor (CB92, 2826:0D0x), which gen_screens.py's call (AX the
    # square only) leaves 0: here the one screentest's right click in
    # the view's centre picks (row 7, column 0Eh).
    gs.BEFORE['looktavern'] = lambda a: a.w16(0xCB92, 0x071C)
    for name, (handler, script, *waits) in gs.SCREENS.items():
        if only and name not in only:
            continue
        # Screens the view's menus run past the original's stop before
        # they show it (the shop's turn is replayed to its next question):
        # their files after the original goes on as far.
        script = script + ON.get(name, [])
        print(name, file=sys.stderr)
        at = gs.place(a, base, gs.WHERE[name]) if name in gs.WHERE else base
        # From the load: a building's way in counted (the view's screens
        # go in as part of theirs).
        before = loaded
        gs.draw(a, at, name, handler, script, 0x00, *waits)
        made = {n: canonical(segs, n, d, a.r16(0xD748) == 0) for n, d in files_changed(before[0], a.written).items()}
        gone = a.deleted - before[1]
        if made or gone:
            for l in lines('screen', name, made, gone):
                out.write(l)
            if dump:
                dump_files(dump, name, 'screen', made)
    out.close()


if __name__ == '__main__':
    main()
