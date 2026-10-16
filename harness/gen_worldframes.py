"""Emit the original's world frames, run on from the user's saved game, for
view/src/worldtest.rs: what its 2D drawing leaves on the page in play
(the interface, the compass, the bars over the 3D view) and where the
player goes, frame by frame, under keys held.

Usage: gen_worldframes.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [NAME...]

The game is loaded from the title and taken into the dungeon made from
gen_dungeonscreens.py's seed as that does (`dungeon`). Its world loop is
then run on frame by frame (harness/screens.py `world_frames`: the timer
ticking at each of the loop's reads of it, nothing else advancing it),
the keys of each case of CASES held through the key interrupt
(`keys_held`) as a player holds them.

For each case, at the top of each frame run, the player's state:

  track | NAME | <frame> <level BB0A> <x BA62> <height BA64> <y BA66> <heading C3DA>

(hex; the place the physics moves, BA62..BA67, not the eye's C3D0..C3D5,
which the frame moves back along the heading after them). Then the page
as the frame after the case's last leaves it, without what the 3D
drawing put there: a pixel is the 3D view's when the last write to it in
that frame was made in the frame's drawing stages (BA4B 3..9: the floor
and ceiling, the sky, the walls, the flats), or by the arrow cursor's
drawing (CURSOR_DRAW). The rest (the interface, the
compass, the bars, the message line) the 2D drawing left:

  screen | NAME | <page digest> <palette digest> <mask digest>

as gen_screens.py writes a box's (the mask a byte a pixel, 1 the 2D
drawing's; the page's 3D pixels 0). With --dump DIR, NAME.page,
NAME.mask, NAME.dac and NAME.full (the page with its 3D) for
screentest's diff images (game pictures: never commit them).

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
from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP
from screens import ScreenArena, PAGE_SEG_FIELD
import gen_dungeonscreens as gd

UP = 0x48
# The world loop's stages (BA4B) that draw the 3D view.
DRAWING = range(3, 10)
# The arrow cursor's drawing (0000:5A6A's loop, which the frame body calls
# at 4CB3 with the cursor 7A48.. the controls set): the original's mouse
# regions, which the view's own controls (the mouse captured) don't show.
CURSOR_DRAW = (0x5AE6, 0x5AFF)


def at_square(row, col, heading):
    """The player put at the middle of a square, facing `heading`: the
    place the physics moves (BA62..) and the eye (C3D0..) both."""
    def put(a):
        for base in (0xC3D0, 0xBA62):
            a.w16(base, (col << 7) + 0x40)
            a.w16(base + 4, (row << 7) + 0x40)
        a.w16(0xC3DA, heading)
    return put


def level_changed(a):
    return a.mu.mem_read(a.lin(a.ds, 0xBB0A), 1)[0] != 0


# name: (set-up, [(keys held, keys let go, frames, until)...])
CASES = {
    # The dungeon's first frames, nothing held: the interface and the
    # compass as play draws them.
    'playdungeon': (None, [((), (), 3, None)]),
    # The first level's down stairs (row 10, column 10 of its map: bytes
    # 1Ch) walked into from the square south of them, facing north (100h):
    # the up arrow held until the level changes (0000:F76F), then let go
    # for two frames on the second level.
    'playdown': (at_square(11, 10, 0x100), [((UP,), (), 20, level_changed), ((), (UP,), 2, None)]),
}


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
    frame = [0]
    def each(a):
        tracks.append(track_line(a, name, frame[0]))
        frame[0] += 1
    for held, let_go, n, until in steps:
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
        ip = mu.reg_read(UC_X86_REG_IP)
        cursor = mu.reg_read(UC_X86_REG_CS) == LOAD_SEG and CURSOR_DRAW[0] <= ip < CURSOR_DRAW[1]
        for k in range(size):
            o = addr - base + k
            if 0 <= o < 64000:
                last[o] = 1 if st in DRAWING or cursor else 2
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
    at = gd.dungeon(a, a.snapshot())
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    out = open(OUT, 'w', buffering=1)
    for name, (setup, steps) in CASES.items():
        if only and name not in only:
            continue
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
        if dump:
            os.makedirs(dump, exist_ok=True)
            for ext, data in (('page', page), ('mask', mask), ('dac', dac), ('full', full)):
                open(os.path.join(dump, f'{name}.{ext}'), 'wb').write(data)
    out.close()


if __name__ == '__main__':
    main()
