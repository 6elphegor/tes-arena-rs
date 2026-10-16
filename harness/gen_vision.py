"""Emit what the original shows at a game's end, the farewell's vision
(view/src/create.rs's `Creation::ending` is checked against it).

Usage: gen_vision.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [DUMP_DIR]

The game started and slot 1's save loaded as screens.py does, the songs
left out (2F9F:001F, 0438: AX 0) and the screen cleared; then the game over (5B1 bit 100h) as
the outer loop takes it, 0000:0CE9..0D3E: the whole screen the clip,
the text (57Ah with VISION.FLC, or 57Bh with JAGAR.FLC once the staff is
taken further, F77: each of SCRIPTS) and the vision 1194:D459 (the colours faded to black,
1194:55B2; the movie played with its text, 2294:0700).

What the player sees is sampled at each wait for the retrace (0000:6BB8,
9B77) and at each read of the game's timer count (0151, which ticks at
each read, as gen_createshot.py's run has it): the VGA's memory and the
DAC (6-bit) as they are there, each state kept when different from the
one before. The run stops after STATES states (the movie's frames are
its states; it loops) or where the original goes on from the movie
(1194:D4F7: the movie over).

  vision | script | F77 | STATES
  vision | F77 | n | screen | colours

the screen and colours their FNV-1a digests. With DUMP_DIR each state is
written there too (visionF77-N.raw: the 64000 bytes, then the 768 colours).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ, UcError
from unicorn.x86_const import *
from emu import LOAD_SEG, fnv
from screens import ScreenArena

STATES = 40


# The staff's stage (F77) each run has: 0 the vision (text 57Ah,
# VISION.FLC), 1 Jagar's (57Bh, JAGAR.FLC). (At 9, the main quest's end,
# the outer loop shows none.)
SCRIPTS = [0, 1]


def run(a, f77, dump):
    """The game over from the loaded game with F77 `f77`: the states seen."""
    mu = a.mu
    seen = []

    def sample(*_):
        shown = bytes(mu.mem_read(0xA0000, 64000))
        pal = bytes(a.dac)
        st = (fnv(shown), fnv(pal))
        if not seen or seen[-1][:2] != st:
            seen.append(st + ((shown + pal) if dump else None,))
            if len(seen) >= STATES:
                mu.emu_stop()

    def tick(mu, access, addr, size, value, ud):
        a.w16(0x0151, a.r16(0x0151) + 1)
        sample()

    def over(*_):
        mu.emu_stop()
    hooks = []
    t = a.lin(a.ds, 0x0151)
    hooks.append(mu.hook_add(UC_HOOK_MEM_READ, tick, None, t, t))
    for off in (0x6BB8, 0x9B77):
        x = a.lin(LOAD_SEG, off)
        hooks.append(mu.hook_add(UC_HOOK_CODE, sample, None, x, x))
    x = a.lin(LOAD_SEG + 0x1194, 0xD4F7)
    hooks.append(mu.hook_add(UC_HOOK_CODE, over, None, x, x))
    mu.ctl_flush_tb()
    # The screen the game left (the world's last frame) cleared: the view
    # fades the same.
    mu.mem_write(0xA0000, bytes(64000))
    a.w16(0x05B1, a.r16(0x05B1) | 0x100)
    mu.mem_write(a.lin(a.ds, 0x0F77), bytes([f77]))
    a._setup([0xFFF0], {})
    mu.reg_write(UC_X86_REG_CS, LOAD_SEG)
    mu.reg_write(UC_X86_REG_IP, 0x0CE9)
    a.fault = None
    err = None
    try:
        mu.emu_start(a.lin(LOAD_SEG, 0x0CE9), a.lin(LOAD_SEG, 0xFFF0), count=int(os.environ.get('COUNT', 3_000_000_000)))
    except UcError as e:
        err = str(e)
    for h in hooks:
        mu.hook_del(h)
    cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
    if err or a.fault:
        sys.exit('stopped at %04X:%04X: %s %s' % (cs - LOAD_SEG, ip, err, a.fault))
    print('F77 %d: %d states, stopped at %04X:%04X' % (f77, len(seen), cs - LOAD_SEG, ip), file=sys.stderr)
    return seen


def main():
    exe, game, save, out = sys.argv[1:5]
    dump = sys.argv[5] if len(sys.argv) > 5 else None
    a = ScreenArena(exe, game, save)
    a.start()
    for seg, off, code in [(0x2F9F, 0x001F, b'\x31\xc0\xcb'), (0x2F9F, 0x0438, b'\x31\xc0\xcb')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    base = a.snapshot()
    with open(out, 'w') as f:
        for f77 in SCRIPTS:
            a.restore(base)
            seen = run(a, f77, dump)
            f.write(f'vision | script | {f77} | {STATES}\n')
            for n, (s, p, raw) in enumerate(seen):
                f.write(f'vision | {f77} | {n} | {s:x} | {p:x}\n')
                if dump:
                    open(os.path.join(dump, f'vision{f77}-{n}.raw'), 'wb').write(raw)


if __name__ == '__main__':
    main()
