"""Emit what the town's welcome box keeps of the screen it covers (0000:
4ACD..4B18: the box's background saved into the load bank [1030] at
offset 0 before the box is drawn, 0000:965E's flag 1), from the user's
saved game, as the original keeps it.

Usage: gen_welcome.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The save is loaded and the guild left for the street as gen_worldframes.py
does (`street`); four of the world loop's frames run, then a town's
arrival is set (5C8 bit 100h, ENTERCTY.$$$ written with WELCOME) and one
more frame runs to the welcome's wait for a key. The kept background (its
header: x | 8000h, y, width, height; then its rows) is the page as the
last frame left it: its digest, and the box's place, are the line, for
view/src/worldtest.rs's `the_welcome_keeps_the_screen_as_the_original`.

Lines:  welcome | street | x y w h | fnv(header and rows)
"""

import os
import struct
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from emu import fnv
from screens import ScreenArena
import gen_worldframes as W

# The arrival's words (ENTERCTY.$$$ as `Machine::travel_arrival` writes
# it; any text: the box's size follows it).
WELCOME = b'You have arrived in a test town.\r\0'
# The controls' read of the mouse (0000:8FC4): the box's wait polls it.
WAIT = (0, 0x8FC4)


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    base = a.snapshot()
    a.restore(W.street(a, base))
    a.waits = set()
    a.script = []
    a.clicks = []
    a.mouse = (160, 100)
    a.buttons = 0
    why, _ = a.world_frames(4)
    assert why == ('frames',), why
    name = bytes(a.mu.mem_read(a.lin(a.ds, 0x130C), 13)).split(b'\0')[0].decode('latin1').upper()
    a.written[name] = bytearray(WELCOME)
    a.w16(0x05C8, a.r16(0x05C8) | 0x100)
    a.waits = {WAIT}
    why, _ = a.world_frames(1)
    assert why[0] == 'call', f'the welcome waits: {why}'
    kept = a.ems_bank(a.r16(0x1030))
    x, y, w, h = struct.unpack('<4H', kept[:8])
    with open(OUT, 'w') as out:
        out.write(f'welcome | street | {x & 0x7FFF:x} {y:x} {w:x} {h:x} | {fnv(kept[:8 + w * h]):x}\n')


if __name__ == '__main__':
    main()
