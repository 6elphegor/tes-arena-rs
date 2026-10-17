"""Emit the house the staff's end shows last, its text over it
(view/src/screentest.rs checks the view's `Movie::still` of HOUSE.IMG
against it).

Usage: gen_house.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [DUMP_DIR]

The game started and slot 1's save loaded as screens.py does, the songs
left out (2F9F:001F, 0438: AX 0); then 1194:D986's house run from
1194:DA25 (the movies and speeches before it are the view's movies,
checked elsewhere): the screen cleared and shown (0000:9426, 9466),
HOUSE.IMG loaded into the EMS frame and drawn in its own colours (7428,
5828, 9B4E), text 579h made (1194:A36C) and its lines drawn one under
another from row 32h, centred on column A0h, in [102C]'s font and
colour 5Ch (0000:5364), then shown (9466) at 1194:DAC9, where the
screen and the colours are taken:

  house | screen | colours

their FNV-1a digests. With DUMP_DIR, house.raw there (the 64000 bytes,
then the 768 colours).
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError
from unicorn.x86_const import *
from emu import LOAD_SEG, fnv
from screens import ScreenArena


def main():
    exe, game, save, out = sys.argv[1:5]
    dump = sys.argv[5] if len(sys.argv) > 5 else None
    a = ScreenArena(exe, game, save)
    a.start()
    for seg, off, code in [(0x2F9F, 0x001F, b'\x31\xc0\xcb'), (0x2F9F, 0x0438, b'\x31\xc0\xcb')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    mu = a.mu
    a._setup([0xFFF0], {})
    cs = LOAD_SEG + 0x1194
    mu.reg_write(UC_X86_REG_CS, cs)
    mu.reg_write(UC_X86_REG_IP, 0xDA25)
    a.fault = None
    err = None
    try:
        mu.emu_start(a.lin(cs, 0xDA25), a.lin(cs, 0xDAC9), count=500_000_000)
    except UcError as e:
        err = str(e)
    at = (mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP))
    if err or a.fault or at != (cs, 0xDAC9):
        sys.exit('stopped at %04X:%04X: %s %s' % (at[0] - LOAD_SEG, at[1], err, a.fault))
    shown = bytes(mu.mem_read(0xA0000, 64000))
    pal = bytes(a.dac)
    with open(out, 'w') as f:
        f.write(f'house | {fnv(shown):x} | {fnv(pal):x}\n')
    if dump:
        os.makedirs(dump, exist_ok=True)
        open(os.path.join(dump, 'house.raw'), 'wb').write(shown + pal)


if __name__ == '__main__':
    main()
