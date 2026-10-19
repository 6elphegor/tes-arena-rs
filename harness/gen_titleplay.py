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
dungeon's way in on to its first frame.

For each, DGROUP there (the offsets tests/vectors/loadexclude.txt names
zeroed; the port's in this form: worldtest's `load_canonical`), the EMS
banks 0..1Fh (64K each, end to end) and the interface's rows of the page
([A8FE]:[A900]; gen_worldframes.py's `interface_digest`):

  titleplay | NAME | <DGROUP digest> <EMS digest> <interface digest>

and for `creationsound` its calls into the sound driver (sbdig.py's
`calls_line`): driver | NAME | ...

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
}

# Run only when named.
NAMED_ONLY = {'creationquiz': CREATION_QUIZ, 'creationquizsound': CREATION_QUIZ}

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


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    skip = gw.excluded('loadexclude.txt')
    out = open(OUT, 'w', buffering=1)
    for name, script in (CASES | {n: c for n, c in NAMED_ONLY.items() if n in only}).items():
        if only and name not in only:
            continue
        print(name, file=sys.stderr)
        sound = name.endswith('sound')
        a = SoundArena(EXE, GAME, SAVE) if sound else ScreenArena(EXE, GAME, SAVE)
        program_start(a, sound)
        if name.startswith('creation'):
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


if __name__ == '__main__':
    main()
