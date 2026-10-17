"""Emit where the original puts the player on leaving a city by its gate,
for view/src/screentest.rs's `a_gate_leads_out_as_the_original`.

Usage: gen_gates.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The save is loaded and the guild left for the street (Daggerfall). The
city's gates are its walls whose trigger (their low six bits) is the
trigger table's (C917) seventh or eighth, with an open square beside;
for the first GATES of them the player is put on that square's centre
facing the gate, the gate picked (CB92) and its trigger run (0000:43B1),
then the area left as the outer loop leaves it (`area_ended`): the
wilderness entered. The player's place there (C3D0, C3D4) and the wall
word of its square are the line.

  gate | COL ROW | FROM_COL FROM_ROW HEADING | X Y | WALL
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from screens import ScreenArena

GATES = 4


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    a.restore(a.snapshot())
    assert a.area_ended() == ('world',), 'the guild not left'
    street = a.snapshot()
    seg = a.r16(0xD806)
    w, d = min(a.r16(0xC357), 128), min(a.r16(0xC359), 128)
    trig = bytes(a.mu.mem_read(a.lin(a.ds, 0xC917), 0x10))

    def word(c, r, seg=seg):
        return a.r16((r << 8) | (c << 1), seg=seg)
    gates = []
    for r in range(d):
        for c in range(w):
            v = word(c, r)
            if v and ((v & 0x8000) or (v >> 8) == (v & 0xFF)) and (v & 0x3F) in (trig[7], trig[8]):
                for dc, dr, h in [(0, -1, 0), (0, 1, 0x100), (-1, 0, 0x80), (1, 0, 0x180)]:
                    nc, nr = c + dc, r + dr
                    if 0 <= nc < w and 0 <= nr < d and word(nc, nr) == 0:
                        gates.append((c, r, nc, nr, h))
                        break
    with open(OUT, 'w') as out:
        for c, r, nc, nr, h in gates[:GATES]:
            a.restore(street)
            for o, v in ((0xC3D0, (nc << 7) + 0x40), (0xC3D4, (nr << 7) + 0x40), (0xC3DA, h),
                         (0xBA62, (nc << 7) + 0x40), (0xBA66, (nr << 7) + 0x40)):
                a.w16(o, v)
            a.w16(0xCB92, (r << 8) | (c << 1))
            saved = a.stub(location=False)
            try:
                assert a.run_screen(0, 0x43B1) is None, 'the gate waited'
            finally:
                a.unstub(saved)
            assert a.r16(0xC398) & 0x80, f'the gate at {c}, {r} led nowhere'
            assert a.area_ended() == ('world',), 'the wilderness not entered'
            x, y = a.r16(0xC3D0), a.r16(0xC3D4)
            wall = word(x >> 7, y >> 7, seg=a.r16(0xD806))
            out.write(f'gate | {c:x} {r:x} | {nc:x} {nr:x} {h:x} | {x:x} {y:x} | {wall:x}\n')


if __name__ == '__main__':
    main()
