"""The game with its music on: worldframes cases played with a music
driver loaded at the program's start.

The driver (SBP1FM.ADV, -Msbp1fm.adv -IOM220) is loaded as the program's
main loads it, before the title's Load (fmmus.py's `start(music=True)`),
on a Sound Blaster Pro's FM chips (fmmus.py's `Music`), and plays from
the AIL's clock interrupt. The songs are then started as the game starts
them (2F9F:001F: the .XFM file read into a heap block, SAMPLE.AD opened,
the sequence registered, each timbre it asks for read from the file,
1194:4943, and installed, the channels' volume set, the sequence started
at the music's volume), faded and stopped (0438) and asked after (000D)
as the game asks: the load's interior's song, a town's on its streets
(2294:04C0) and the frames' (2294:04F5: the town's song asked after
each frame until it ends, a dungeon's, a building's).

Each case's lines are gen_worldframes.py's (`track`, `state`,
`stateload`, `stateems`), its `heap` line (the heap's blocks' sizes and
flags, `heap_line`: what the songs and the shut down take and give back,
which DGROUP and the banks don't hold), its `music` line the calls into the AIL from
the load on, its place's way in and its frames, answers and all
(fmmus.py's `calls_line`), and the load's own: `loadstate` and `loadems`
(the state the load with music on leaves, as gen_worldframes.py's),
`music | load` (the driver's load's and the load's calls).

Usage: gen_musicplay.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [CASE...]
"""
import os
import sys

from emu import fnv
from fmmus import MusicArena, music_time
import gen_worldframes as gw

# The cases run with music on (gen_worldframes.py's): a town's street, at
# night too, the wilderness, a dungeon, a tavern; and this generator's own
# (OWN, SHUTDOWN).
CASES = ['playstreet', 'playnight', 'playwild', 'playdungeon', 'playtaverndrink', 'playjourneyon', 'playoptionsmusic',
         'playshutdown', 'playsongdone']
# This generator's own: `playjourneyend` (gen_worldframes.py's: the
# street's song started, then a journey to another town, whose arrival
# fades the song out and stops it, 2F9F:0438, and starts the town's,
# 2294:04C0) on into three of the town's frames, each asking after the
# town's song (2294:0509, 2F9F:000D).
JOURNEY = gw.CASES['playjourneyend']
OWN = {'playjourneyon': (JOURNEY[0], JOURNEY[1], [JOURNEY[2][0],
                                                  JOURNEY[2][1][:5] + ({**JOURNEY[2][1][5], 'ends': False},),
                                                  ((), (), 3, None)]),
       # The dungeon's song started, then the options screen (Esc) and its
       # music volume clicked up twice (2542:079C: D3A raised, its box
       # drawn, the sequence's volume set at once, 0FFD:0DE8 from 07BD)
       # and down once (07DE, 07FF), then Resume (playoptionsend's).
       'playoptionsmusic': ('dungeon', None, [((), (), 2, None),
                                              ((gw.KEY_ESC,), (), 1, None, [], {'script': [
                                                  ('click', 127, 83), ('click', 127, 83), ('click', 127, 107),
                                                  ('click', 288, 132)], 'ends': True})]),
       # The street's two frames, then the town's arrival song started as
       # arriving starts it (2294:04C0, `town_song`: the song stopped,
       # 430F set, the town's by its size started) and a frame, whose poll
       # (2294:0509) finds it playing; then the song played to its end
       # (2000 ticks of music time, about 110 s, `songs_done`) and a frame,
       # whose poll is answered 2, done: the town's song let go (430F
       # cleared) and the weather's asked for (2294:051B, 10B6).
       'playsongdone': ('street', None, [((), (), 2, None),
                                         ((), (), 1, None, [], {'do': lambda a: town_song(a)}),
                                         ((), (), 1, None, [], {'do': lambda a: songs_done(a)})])}


def town_song(a):
    """2294:04C0, the town's arrival song started, run alone between two
    frames."""
    assert a.run_screen(0x2294, 0x04C0) is None, 'the town song waited'


def songs_done(a):
    """The song playing taken to its end: 2000 ticks of the music's time
    passed between two frames (fmmus.py's `music_time`: the AIL's clock
    interrupts run with the CPU idle, the game's own timer left; a
    shortcut for the minutes of frames the song would take)."""
    music_time(a, 2000)
# Cases followed by the music's shut down (2F9F:04A0, as main's end,
# 0000:10B8, and the options screen's Quit, 21EE:020D, run it): the song
# faded and stopped (0438), the timbre cache (when there is one), the
# state table, the driver's image and the sound driver's (8F62, when
# loaded) given back (0000:6C26), the AIL shut down (0821). Run alone
# after `playstreet`'s frames, its calls on the case's line.
SHUTDOWN = {'playshutdown': 'playstreet'}
# The file-open hook's segment word (A906: 0000:11DA's code segment, the
# harness's 1000h higher than the port's): left out of the comparisons.
HOOK_SEGMENT = {0xA906, 0xA907}


def heap_line(a, name):
    """The heap's blocks from the first header (DGROUP 0157: a far
    pointer; each header's next at +0 and +2, its size at +8, its flags at
    +0Ah), as `heap | NAME | COUNT DIGEST` over their sizes and flags (the
    segments, the harness's 1000h higher, left out)."""
    off, seg = a.r16(0x157), a.r16(0x159)
    out = []
    while (seg | off) and len(out) < 500:
        hdr = bytes(a.mu.mem_read(seg * 16 + off, 0xB))
        out.append(f'{hdr[8] | hdr[9] << 8:x}:{hdr[10]:x}')
        off, seg = hdr[0] | hdr[1] << 8, hdr[2] | hdr[3] << 8
    return f'heap | {name} | {len(out)} {fnv(",".join(out).encode()):x}\n'


def main(kind='fm'):
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    a = MusicArena(EXE, GAME, SAVE)
    gw.rows_watch(a)
    a.start(music=True, kind=kind)
    out = open(OUT, 'w', buffering=1)
    loaded = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    loaded_ems = b''.join(a.ems_bank(b) for b in range(0x20))
    if dump:
        os.makedirs(dump, exist_ok=True)
        open(os.path.join(dump, 'base.ds'), 'wb').write(loaded)
        open(os.path.join(dump, 'base.ems'), 'wb').write(loaded_ems)
    skip_base = gw.excluded('loadexclude.txt') | HOOK_SEGMENT
    if not only:
        out.write(f'loadstate | {fnv(bytes(0 if i in skip_base else b for i, b in enumerate(loaded))):x}\n')
        out.write(f'loadems | {fnv(loaded_ems):x}\n')
        out.write(a.music.calls_line('load'))
        if kind == 'mt32':
            out.write(a.music.midi_line('load'))
    a.music.calls = []
    if kind == 'mt32':
        a.music.midi = bytearray()
    base = a.snapshot()
    skip = gw.excluded() | HOOK_SEGMENT
    skip_load = skip | gw.excluded('stateloadexclude.txt') | gw.excluded('loadexclude.txt')
    places = {}
    for name in CASES:
        if only and name not in only:
            continue
        where, setup, steps = OWN[name] if name in OWN else gw.CASES[SHUTDOWN.get(name, name)]
        if where not in places:
            places[where] = {'dungeon': gw.gd.dungeon, 'crypt': gw.crypt, 'street': gw.street, 'wild': gw.wild,
                             'tavern': gw.building('tavern')}[where](a, base)
        print(name, file=sys.stderr)
        tracks = []
        gw.run_case(a, places[where], name, setup, steps, tracks)
        if name in SHUTDOWN:
            assert a.run_screen(0x2F9F, 0x04A0) is None, 'the shut down waited'
        for t in tracks:
            out.write(t)
        out.write(a.music.calls_line(name))
        if kind == 'mt32':
            out.write(a.music.midi_line(name))
        out.write(heap_line(a, name))
        print(f'  {a.music.ticks} ticks, {a.music.taken} clock interrupts, {a.music.writes} chip writes', file=sys.stderr)
        after = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
        out.write(gw.state_line(name, a.pre_state[0], after, skip))
        out.write(gw.state_line(name, loaded, after, skip_load, 'stateload'))
        ems = b''.join(a.ems_bank(b) for b in range(0x20))
        out.write(gw.state_runs('stateems', name, a.pre_state[1], ems))
        if dump:
            open(os.path.join(dump, f'{name}.ds'), 'wb').write(after)
            open(os.path.join(dump, f'{name}.ems'), 'wb').write(ems)
            open(os.path.join(dump, f'{name}.ds0'), 'wb').write(a.pre_state[0])
            open(os.path.join(dump, f'{name}.ems0'), 'wb').write(a.pre_state[1])
    out.close()


if __name__ == '__main__':
    main()
