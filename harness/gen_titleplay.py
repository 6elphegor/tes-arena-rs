"""Emit the original's state where a game begins from the program's own
start, for view/src/worldtest.rs: the program started (21EE:0000, as
screens.py's `start` runs it), its main on from 0000:1059 (1194:3A51,
25FD:022A, 25FD:0219, 1194:3AE8, no arguments; 108B; 10C9: the intro,
MENU.IMG) to the menu's wait (gen_worldframes.py's `titled`), then the
menu's inputs (`menu_picked`) on until a world loop is about to run its
first frame (0000:A2AC).

Usage: gen_titleplay.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [--dump DIR] [NAME...]

The cases (CASES): `startload`, the menu's Load (L: 0000:117E) and slot
1's row (the user's save, SAVE_DIR's slot 1).

For each, DGROUP there (the offsets tests/vectors/loadexclude.txt names
zeroed; the port's in this form: worldtest's `load_canonical`), the EMS
banks 0..1Fh (64K each, end to end) and the interface's rows of the page
([A8FE]:[A900]; gen_worldframes.py's `interface_digest`):

  titleplay | NAME | <DGROUP digest> <EMS digest> <interface digest>

With --dump DIR, NAME.ds, NAME.ems and NAME.page (the page whole).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import LOAD_SEG, STUB_SEG, fnv
from screens import ScreenArena
import gen_worldframes as gw

KEY_L = 0x26

CASES = {
    'startload': [('click', 160, 30), ('click', 160, 21)],
}

# Where main's frame is made (its far call's return, its BP, argc 1).
FRAME = (0x9000, 0xFF00)


def program_start(a):
    """The program as far as its main's menu: 21EE:0000 on the memory as
    `init_memory` leaves it, then main from 0000:1059 on a frame of its
    own (BP at FRAME: the saved BP, the far return to STUB_SEG:0, argc 1)
    to the menu's wait (`titled`), the mouse's cursor left to the front end
as screens.py's `start` leaves it."""
    a.init_memory()
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    # The mouse's cursor neither drawn nor taken away (0000:5458, 5422),
    # as screens.py's `start` has it: the front end's.
    a.code_write(a.lin(LOAD_SEG, 0x5422), b'\xc3')
    a.code_write(a.lin(LOAD_SEG, 0x5458), b'\xc3')
    ss, sp = FRAME
    a.mu.mem_write(a.lin(ss, sp), struct.pack('<5H', 0, 0, STUB_SEG, 1, 0))
    gw.titled(a, at=0x1059, stack=(ss, sp), regs={'ebp': sp})


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    dump = sys.argv[sys.argv.index('--dump') + 1] if '--dump' in sys.argv else None
    only = [n for n in sys.argv[5:] if not n.startswith('--') and n != dump]
    skip = gw.excluded('loadexclude.txt')
    out = open(OUT, 'w', buffering=1)
    for name, script in CASES.items():
        if only and name not in only:
            continue
        print(name, file=sys.stderr)
        a = ScreenArena(EXE, GAME, SAVE)
        program_start(a)
        gw.menu_picked(a, script)
        # The harness's stubs' counters (FFE0..) zeroed, as `start` does.
        for at in range(0xFFE0, 0x10000, 2):
            a.w16(at, 0)
        ds = bytes(a.mu.mem_read(a.lin(a.ds, 0), 0x10000))
        ems = b''.join(a.ems_bank(b) for b in range(0x20))
        page = bytes(a.mu.mem_read(a.lin(a.r16(0xA900), a.r16(0xA8FE)), 64000))
        canon = bytes(0 if i in skip else b for i, b in enumerate(ds))
        out.write(f'titleplay | {name} | {fnv(canon):x} {fnv(ems):x} {gw.interface_digest(page):x}\n')
        if dump:
            os.makedirs(dump, exist_ok=True)
            for ext, data in (('ds', ds), ('ems', ems), ('page', page)):
                open(os.path.join(dump, f'{name}.{ext}'), 'wb').write(data)


if __name__ == '__main__':
    main()
