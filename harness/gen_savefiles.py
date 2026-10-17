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

The files are taken to the port's form first (`canonical`): SAVEGAME's
level block (DGROUP BB08.., FD00h on in the file) holds segments of the
image (the level's hooks, C3A4..C3BB), which the harness's load puts
1000h paragraphs higher than the port's; and where the level has no
ceiling (its plane's segment, D748, 0) the save writes the 32 KB at
0000:0000 as the ceiling's plane, the interrupt table on (the BIOS's and
DOS's memory, not the game's: the port's is its image), left out as 0s
(view/src/savetest.rs does the same to the view's).

With --dump DIR, each file as written (in that form): DIR/NAME.play/<file>
and DIR/NAME.save/<file> (game data: never commit them).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
import struct
from emu import DGROUP, LOAD_SEG, fnv
from screens import ScreenArena
import gen_dungeonscreens as gd
import gen_screens as gs
import gen_worldframes as gw

UP = gw.UP

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
    out.close()


if __name__ == '__main__':
    main()
