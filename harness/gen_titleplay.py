"""Emit the original's state where a game begins from the program's own
start, for view/src/worldtest.rs: the program started (21EE:0000, as
screens.py's `start` runs it), its main on from 0000:1059 (1194:3A51,
25FD:022A, 25FD:0219, 1194:3AE8, no arguments; 108B; 10C9: the intro,
MENU.IMG) to the menu's wait (gen_worldframes.py's `titled`), then the
menu's inputs (`menu_picked`) on until a world loop is about to run its
first frame (0000:A2AC).

Usage: gen_titleplay.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [NAME...]

The cases (CASES): `startload`, the menu's Load (L: 0000:117E) and slot
1's row (the user's save, SAVE_DIR's slot 1); `creation`, the menu's New
(0000:1130) and one character made through the original's own screens
(CREATION), then creation's vision (1194:D459, text 578h, its clicks:
`created`), the game set up (21EE:016C, 25FD:0219) and the start
dungeon's way in on to its first frame; `creationquiz`, the same with the
class the questions give (CREATION_QUIZ, 222C:0000); `creationsound` and
`creationquizsound`, those two with ARENA.BAT's sound driver loaded.

For each, DGROUP there (the offsets tests/vectors/loadexclude.txt names
zeroed; the port's in this form: worldtest's `load_canonical`), the EMS
banks 0..1Fh (64K each, end to end) and the interface's rows of the page
([A8FE]:[A900]; gen_worldframes.py's `interface_digest`):

  titleplay | NAME | <DGROUP digest> <EMS digest> <interface digest>

and for those with the driver their calls into it (sbdig.py's
`calls_line`): driver | NAME | ...

The PLAYS cases (`created...`) make a character of a class of their own
the same way, then play on in the start dungeon from its first frame
(`played_on`): the class's own ways in play (a caster's spell, a
Sorcerer's lack of spell points, an Acrobat's jump and lockpicking).
Beside their titleplay line: the track, the projectiles and actors, what
the frames changed in DGROUP and the EMS banks, and the last page.

With --dump DIR, NAME.ds, NAME.ems and NAME.page (the page whole).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ
from emu import LOAD_SEG, STUB_SEG, fnv
from screens import ScreenArena
from sbdig import Blaster, SoundArena, DRIVER_LOAD, driver_named
import gen_worldframes as gw

KEY_L = 0x26
R, I, A, ENTER, F, Y, S = 0x13, 0x17, 0x1E, 0x1C, 0x21, 0x15, 0x1F
G, B, C = 0x22, 0x30, 0x2E
KEY_P = 0x19

# A new game made through the original's screens, each step the answer to
# one of its waits for input: New (the menu's region at 160,120); the
# story's scrolls and pictures run their course; S, the class from the
# list (1194:2678), Acrobat its first line, taken with two clicks (the
# harness lets keys go before the list reads Enter); the name typed and
# Enter; F, female; the map's Enter, Morrowind clicked (240,80) and Y;
# the province's texts and the attribute screen's first box taken with
# Enter; the up arrow by the first attribute (41,50) clicked for every
# point; Done (10,175), S to keep them and Enter; the face moved on twice
# (240,20) and Done.
CREATION = ([('click', 160, 120), ('keys', [S]), ('click', 94, 51), ('click', 94, 51),
             ('keys', [R]), ('keys', [I]), ('keys', [A]), ('keys', [ENTER]), ('keys', [F]),
             ('keys', [ENTER]), ('click', 240, 80), ('keys', [Y])]
            + [('keys', [ENTER])] * 5
            + [('click', 41, 50)] * 16
            + [('click', 10, 175), ('keys', [S]), ('keys', [ENTER]),
               ('click', 240, 20), ('click', 240, 20), ('click', 10, 175)])

# A new game whose class the questions give (222C:0000): as CREATION, but
# G, generate, then the instructions' box taken with Enter (1194:260A),
# the ten questions answered by key (A, B, C in turn from A), and the
# class offered (1194:261F) taken with Y; then the name on as CREATION.
QUIZ_ANSWERS = [A, B, C, A, B, C, A, B, C, A]
CREATION_QUIZ = ([('click', 160, 120), ('keys', [G]), ('keys', [ENTER])]
                 + [('keys', [k]) for k in QUIZ_ANSWERS]
                 + [('keys', [Y])]
                 + CREATION[4:])

# The clicks the vision after creation takes (1194:D459, text 578h): a
# page turned for each of the first ten, the eleventh's press ending the
# movie (its release read after it, on the way into the game).
VISION_CLICKS = 11

# `creationsound`: `creation` with ARENA.BAT's sound driver loaded as
# main's command line loads it (`program_start`'s `sound`), the vision's
# speech played through it, its pages turned as its parts end.
CASES = {
    'startload': [('click', 160, 30), ('click', 160, 21)],
    'creation': CREATION,
    'creationsound': CREATION,
    'creationquiz': CREATION_QUIZ,
    'creationquizsound': CREATION_QUIZ,
}

# A new game whose questions are all answered A: the class offered a
# Sorcerer (1A9 23h), whose spell points start at none (020A 0, of 174:
# 1194:2D28 gives the class none to begin with).
CREATION_SORCERER = ([('click', 160, 120), ('keys', [G]), ('keys', [ENTER])]
                     + [('keys', [A])] * 10
                     + [('keys', [Y])]
                     + CREATION[4:])

# Characters made through the original's screens, then played on in the
# start dungeon from its first frame, as gen_worldframes.py's cases play
# (its `play`: the steps, a track line at the top of each frame): the
# class's own ways in play. NAME: (creation's script, setup between
# creation and the first frame, steps).
PLAYS = {
    # `creationquiz`'s caster (1A9 E5h, 87 spell points, the book its
    # class gives: Fire Dart and Light Heal, spells 61 and 62) opening its
    # book (C, 2542:056A: a new game has no spell picked last, 5CAE FFh),
    # its first line, Fire Dart (marked already), clicked: cast
    # (29CF:071F), its target clicked at the view's centre a frame on, the
    # dart followed through its flight; its spell points spent.
    'createdcast': (CREATION_QUIZ, None, [((), (), 2, None), ((gw.KEY_C,), (), 1, None, [], {'script': [('click', 0xF0, 0x9A)]}),
                                          ((), (gw.KEY_C,), 10, None, [(160, 100)]), ((), (), 30, None)]),
    # The Sorcerer (CREATION_SORCERER) the same, with no spell points: "Not
    # enough spell points..." in a box, which a click takes.
    'createdsorcerer': (CREATION_SORCERER, None, [((), (), 2, None), ((gw.KEY_C,), (), 1, None, [], {'script': [('click', 0xF0, 0x9A), ('click', 160, 100)]}),
                                                  ((), (gw.KEY_C,), 20, None)]),
    # `creation`'s Acrobat (1A9 C9h) jumping (J): its jump's own rise
    # (2826:0279: C9h, every other frame by CCBC).
    'createdjump': (CREATION, None, [((), (), 2, None), ((gw.KEY_J,), (), 1, None), ((), (gw.KEY_J,), 30, None)]),
    # The Acrobat put west of the start dungeon's door at row 23, column 4
    # (its own lock, LOCKS' 1704h, level 10), facing it (heading 80h):
    # picking armed with P (2542:0279), the door clicked (2826:0AAB: the
    # roll, 1194:53F9, by the class's factor, 2A3E), then the up arrow held
    # into it and let go.
    'createdpick': (CREATION, gw.at_square(23, 3, 0x80), [((), (), 2, None), ((KEY_P,), (), 1, None), ((), (KEY_P,), 20, None, [(160, 100)]),
                                                          ((gw.UP,), (), 40, None), ((), (gw.UP,), 10, None)]),
}

# Run only when named.
NAMED_ONLY = {}

# Where main's frame is made (its far call's return, its BP, argc 1).
FRAME = (0x9000, 0xFF00)


def program_start(a, sound=False):
    """The program as far as its main's menu: 21EE:0000 on the memory as
    `init_memory` leaves it, then main from 0000:1059 on a frame of its
    own (BP at FRAME: the saved BP, the far return to STUB_SEG:0, argc 1)
    to the menu's wait (`titled`), the mouse's cursor left to the front end
    as screens.py's `start` leaves it. With `sound` (a SoundArena) ARENA.BAT's
    sound driver is loaded where main's command line loads it (1194:066A's
    076D, before 0000:108B), on a Blaster (sbdig.py's `more`)."""
    a.init_memory()
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    # The mouse's cursor neither drawn nor taken away (0000:5458, 5422),
    # as screens.py's `start` has it: the front end's.
    a.code_write(a.lin(LOAD_SEG, 0x5422), b'\xc3')
    a.code_write(a.lin(LOAD_SEG, 0x5458), b'\xc3')
    ss, sp = FRAME
    a.mu.mem_write(a.lin(ss, sp), struct.pack('<5H', 0, 0, STUB_SEG, 1, 0))
    hooks = []
    if sound:
        def load(mu, addr, size, _):
            # (Once: the run goes on from 108B itself, `press`'s way.)
            mu.hook_del(hooks.pop())
            driver_named(a)
            Blaster(more=True).attach(a)
            a.pressed = DRIVER_LOAD
            mu.emu_stop()
        at = a.lin(LOAD_SEG, 0x108B)
        hooks.append(a.mu.hook_add(UC_HOOK_CODE, load, None, at, at))
    try:
        gw.titled(a, at=0x1059, stack=(ss, sp), regs={'ebp': sp})
    finally:
        for h in hooks:
            a.mu.hook_del(h)


def created(a, script, clicks=VISION_CLICKS):
    """`menu_picked` for a new game: the timer (0151) ticking at each read
    of it, as `titled` has it (the story's pictures and the movies wait on
    it), up to the vision's end (0000:1162): from there into the game it
    stands, as `menu_picked` has it for the title's Load; the install's
    speech served (SPEECH\\nnnn_mmX.VOC, as the farewell's case serves
    it); and as the vision's movie begins (2294:0700) its clicks queued at
    the screen's centre (`clicks`, VISION_CLICKS; none with the sound
    driver, whose speech turns the pages as its parts end), none left by
    the first frame."""
    speech = os.path.join(a.game_dir, 'SPEECH')
    if os.path.isdir(speech):
        a.names.update({n.upper(): os.path.join(speech, n) for n in os.listdir(speech)})
    t = a.lin(a.ds, 0x0151)
    ticking = [True]

    def tick(mu, access, addr, size, value, _):
        if ticking[0]:
            a.w16(0x0151, (a.r16(0x0151) + 1) & 0xFFFF)

    def movie(mu, addr, size, _):
        a.clicks = [(160, 100)] * clicks

    def seen(mu, addr, size, _):
        ticking[0] = False
    mv = a.lin(LOAD_SEG + 0x2294, 0x0700)
    over = a.lin(LOAD_SEG, 0x1162)
    hooks = [a.mu.hook_add(UC_HOOK_MEM_READ, tick, None, t, t + 1),
             a.mu.hook_add(UC_HOOK_CODE, movie, None, mv, mv),
             a.mu.hook_add(UC_HOOK_CODE, seen, None, over, over)]
    try:
        gw.menu_picked(a, script)
    finally:
        for h in hooks:
            a.mu.hook_del(h)
    assert not a.clicks and not a.buttons, f'clicks left over: {a.clicks}'


def played_on(a, name, setup, steps, out, skip):
    """From the first frame a new game reaches (`created`) on, `setup`
    then `steps` played as gen_worldframes.py's `run_case` plays a case
    (its `play`; the mouse's cursor kept from drawing, CURSOR_DRAW):
    the track, the projectiles and actors beside it, then what the frames
    changed in DGROUP (`state_line`, loadexclude.txt's and
    stateexclude.txt's offsets aside) and in the EMS banks
    (`state_runs`), and the page and colours the last frame left:

      track | NAME | ...
      objects | NAME | ...
      titlestate | NAME | <runs> | <digest>
      titleems | NAME | <count> <offsets digest> | <digest>
      titlescreen | NAME | <page digest> <colours digest>"""
    a.rows_drawn = []
    a.waits = set()
    a.script = []
    a.clicks = []
    a.mouse = (160, 100)
    a.buttons = 0
    a.menu_loaded = False
    if setup:
        setup(a)
    pre = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    pre_ems = b''.join(a.ems_bank(b) for b in range(0x20))

    def no_cursor(*_):
        a.w16(0x7A48, 0xFFFF)
    at = a.lin(LOAD_SEG, gw.CURSOR_DRAW)
    cursor = a.mu.hook_add(UC_HOOK_CODE, no_cursor, None, at, at)
    tracks = []
    try:
        full, _, _, dac = gw.play(a, name, steps, tracks)
    finally:
        a.mu.hook_del(cursor)
    out.writelines(tracks)
    out.write(gw.objects_line(a, name))
    after = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
    out.write(gw.state_line(name, pre, after, skip | gw.excluded(), kind='titlestate'))
    out.write(gw.state_runs('titleems', name, pre_ems, b''.join(a.ems_bank(b) for b in range(0x20))))
    out.write(f'titlescreen | {name} | {fnv(full):x} {fnv(bytes(dac)):x}\n')


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    skip = gw.excluded('loadexclude.txt')
    out = open(OUT, 'w', buffering=1)
    plays = {n: p[0] for n, p in PLAYS.items()}
    for name, script in (CASES | plays | {n: c for n, c in NAMED_ONLY.items() if n in only}).items():
        if only and name not in only:
            continue
        print(name, file=sys.stderr)
        sound = name.endswith('sound')
        a = SoundArena(EXE, GAME, SAVE) if sound else ScreenArena(EXE, GAME, SAVE)
        program_start(a, sound)
        if name.startswith('creat'):
            created(a, script, 0 if sound else VISION_CLICKS)
        else:
            gw.menu_picked(a, script)
        # The harness's stubs' counters (FFE0..) zeroed, as `start` does.
        for at in range(0xFFE0, 0x10000, 2):
            a.w16(at, 0)
        ds = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
        ems = b''.join(a.ems_bank(b) for b in range(0x20))
        page = bytes(a.mu.mem_read(a.lin(a.r16(0xA900), a.r16(0xA8FE)), 64000))
        canon = bytes(0 if i in skip else b for i, b in enumerate(ds))
        out.write(f'titleplay | {name} | {fnv(canon):x} {fnv(ems):x} {gw.interface_digest(page):x}\n')
        if sound:
            # The calls into the driver from its load on (sbdig.py's
            # `calls_line`): the vision's speech's among them.
            out.write(a.blaster.calls_line(name))
        if dump:
            os.makedirs(dump, exist_ok=True)
            for ext, data in (('ds', ds), ('ems', ems), ('page', page)):
                open(os.path.join(dump, f'{name}.{ext}'), 'wb').write(data)
        if name in PLAYS:
            played_on(a, name, PLAYS[name][1], PLAYS[name][2], out, skip)
            if dump:
                open(os.path.join(dump, f'{name}.end.ds'), 'wb').write(bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000)))


if __name__ == '__main__':
    main()
