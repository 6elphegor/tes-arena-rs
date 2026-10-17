"""Emit where the original puts the player on leaving a city by its gate,
for view/src/screentest.rs's `a_gate_leads_out_as_the_original`.

Usage: gen_gates.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The save is loaded and the guild left for the street (Daggerfall). The
city's gates are its walls whose trigger (their low six bits) is the
trigger table's (C917) seventh or eighth, with an open square beside;
for the first GATES (16) of them, from each open square
beside it (the city's side and the wall's outer side alike), the
player is put on that square's centre facing the gate, the gate picked (CB92) and its trigger run (0000:43B1),
then the area left as the outer loop leaves it (`area_ended`): the
wilderness entered. The player's place there (C3D0, C3D4) and the wall
word of its square are the line.

  gate | CITY | COL ROW | FROM_COL FROM_ROW HEADING | X Y | WALL

The player's square (CB90) is set as a frame sets it (the square stood
on): the way out's nudge (217B:058F) measures it from the place kept
outside (0FD2), so the nudge goes the way it goes in play.

CITY is `save` for Daggerfall as the save has it, or PROVINCE:INDEX for
another place of CITIES, entered from that street as the outer loop
enters a city (5E6 0, its location 0A4C; the province's block loaded
first, 1194:2E18 with AL the province, as travel's arrival has it): the
location record built (1194:3AE8), the weather, the city built.
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from screens import ScreenArena

GATES = int(os.environ.get('GATES', '16'))
# A sample across the provinces: each one's first city-state, a town and
# a village (location index 0..7, 8..15, 16..31), and the Imperial City.
CITIES = [None] + [(p, i) for p in range(8) for i in (0, 9, 20)] + [(8, 0)]
# Helnim Wall (3:30), whose gate a playtest left by (seed 34).
CITIES.append((3, 30))
if os.environ.get('GATE_CITIES'):
    CITIES = [None if c == 'save' else tuple(map(int, c.split(':'))) for c in os.environ['GATE_CITIES'].split(',')]


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    a = ScreenArena(EXE, GAME, SAVE)
    a.start()
    a.restore(a.snapshot())
    assert a.area_ended() == ('world',), 'the guild not left'
    first = a.snapshot()
    with open(OUT, 'w') as out:
        for city in CITIES:
            a.restore(first)
            if city is not None:
                p, i = city
                a.call_far(0x1194, 0x2E14, eax=p)
                a.mu.mem_write(a.lin(a.ds, 0x0A4C), bytes([(p * 32 + i) & 0xFF]))
                a.mu.mem_write(a.lin(a.ds, 0x05E6), b'\x00')
                assert a.area_ended() == ('world',), f'the city {city} not entered'
            gates_of(a, out, 'save' if city is None else f'{city[0]}:{city[1]}')


def gates_of(a, out, name):
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
                # Every open square beside it: the city's side and the
                # wall's outer side alike (a playtest's jump can stand
                # the player on either).
                for dc, dr, h in [(0, -1, 0), (0, 1, 0x100), (-1, 0, 0x80), (1, 0, 0x180)]:
                    nc, nr = c + dc, r + dr
                    if 0 <= nc < w and 0 <= nr < d and word(nc, nr) == 0:
                        gates.append((c, r, nc, nr, h))
    for c, r, nc, nr, h in gates[:GATES]:
        a.restore(street)
        for o, v in ((0xC3D0, (nc << 7) + 0x40), (0xC3D4, (nr << 7) + 0x40), (0xC3DA, h),
                     (0xBA62, (nc << 7) + 0x40), (0xBA66, (nr << 7) + 0x40), (0xCB90, (nr << 8) | (nc << 1))):
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
        out.write(f'gate | {name} | {c:x} {r:x} | {nc:x} {nr:x} {h:x} | {x:x} {y:x} | {wall:x}\n')
        out.flush()


if __name__ == '__main__':
    main()
