"""Emit what the original shows of the main quest's dream after a rest
(1194:D3AF, its picture and text by 1194:D45D), for view/src/screentest.rs's
`the_dreams_as_the_original_shows_them`.

Usage: gen_dream.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT [DUMP_DIR]

The game started and slot 1's save loaded as screens.py does, the songs
left out (2F9F:001F, 0438: AX 0); then a dream pending (F76 1, F7A 0) and
D3AF called with the player's level byte (01AA) and the dreams' count
(F77) each of SCRIPTS: at level 1 the first dream (text 5DCh on
CHAOSVSN.FLC, colour 33h) comes before the count's; otherwise only the
count's (50Eh + F77, on VISION.FLC, colour 2Fh).

What the player sees is sampled as gen_vision.py samples it: at each wait
for the retrace (0000:6BB8, 9B77) and each read of the game's timer count
(0151, ticking at each read), the VGA's memory and the DAC (6-bit), each
state kept when different from the one before; the run stops after
STATES states, or where the first movie is over (1194:D4F7).

  dream | script | LEVEL | F77 | STATES
  dream | LEVEL | F77 | n | screen | colours

At level 1 the two movies play one after the other (D3F1, then D41C).
`through` plays the first to its end: a click (int 33h AX 3, read at
0000:9012) queued each GAP states, its page turned at 2294:084C or the
movie ended at 0845 (A902 bit 100h); the states sampled on through the
second's way in, THROUGH states from the first's end: D3F6's fade to
black from the first's last frame, the second's D45D (its fade, the
screen cleared), up to the second movie's first frame. The install's
speech is served (SPEECH\\nnnn_mmX.VOC): with it the movie's player draws
straight into the VGA (2F75:00B5 frees the page, A8FE A000:0000), its
first frame under the colours faded to black, a step the window's
movies don't take; the states stop before it.

  dreamthrough | script | LEVEL | F77 | GAP | THROUGH
  dreamthrough | LEVEL | F77 | n | screen | colours
  dreamthrough | LEVEL | F77 | key | n | k  (a click taken before state n,
                                            at the movie's k'th frame: its
                                            callback's calls, 0000:663F)
  dreamthrough | LEVEL | F77 | end | n      (the first movie over before n)
"""
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ, UcError
from unicorn.x86_const import *
from emu import LOAD_SEG, fnv
from screens import ScreenArena

STATES = 40

# (level byte, F77): the first dream at level 1; the count's alone.
SCRIPTS = [(1, 0), (2, 3)]
# The two movies one after the other (level 1), and the clicks' spacing.
THROUGH_SCRIPTS = [(1, 0), (1, 4)]
GAP = 24
THROUGH = 24


def run(a, level, f77, dump):
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
    # From a cleared screen, as gen_vision.py's run.
    mu.mem_write(0xA0000, bytes(64000))
    mu.mem_write(a.lin(a.ds, 0x01AA), bytes([level]))
    mu.mem_write(a.lin(a.ds, 0x0F76), bytes([1]))
    mu.mem_write(a.lin(a.ds, 0x0F77), bytes([f77]))
    mu.mem_write(a.lin(a.ds, 0x0F7A), bytes([0]))
    seg = LOAD_SEG + 0x1194
    a._setup([0xFFF0], {})
    mu.reg_write(UC_X86_REG_CS, seg)
    mu.reg_write(UC_X86_REG_IP, 0xD3AF)
    a.fault = None
    err = None
    try:
        mu.emu_start(a.lin(seg, 0xD3AF), a.lin(seg, 0xFFF0), count=int(os.environ.get('COUNT', 3_000_000_000)))
    except UcError as e:
        err = str(e)
    for h in hooks:
        mu.hook_del(h)
    cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
    if err or a.fault:
        sys.exit('stopped at %04X:%04X: %s %s' % (cs - LOAD_SEG, ip, err, a.fault))
    print('level %d F77 %d: %d states, stopped at %04X:%04X' % (level, f77, len(seen), cs - LOAD_SEG, ip), file=sys.stderr)
    return seen


def through(a, level, f77, dump):
    """The first movie played to its end by clicks, then the second's way
    in (see the module's doc): (states, keys, end)."""
    mu = a.mu
    seen, keys = [], []
    st8 = {'end': None, 'pressed': 0, 'frames': 0}

    def sample(*_):
        shown = bytes(mu.mem_read(0xA0000, 64000))
        pal = bytes(a.dac)
        st = (fnv(shown), fnv(pal))
        if not seen or seen[-1][:2] != st:
            seen.append(st + ((shown + pal) if dump else None,))
            if st8['end'] is None and not a.clicks and len(seen) - st8['pressed'] >= GAP:
                a.clicks.append((160, 100))
                st8['pressed'] = len(seen)
            if st8['end'] is not None and len(seen) >= st8['end'] + THROUGH:
                mu.emu_stop()


    def tick(mu, access, addr, size, value, ud):
        a.w16(0x0151, a.r16(0x0151) + 1)
        sample()

    def frame(*_):
        st8['frames'] += 1

    def key(*_):
        keys.append((len(seen), st8['frames']))

    def over(*_):
        if st8['end'] is None:
            st8['end'] = len(seen)
        else:
            mu.emu_stop()
    hooks = []
    t = a.lin(a.ds, 0x0151)
    hooks.append(mu.hook_add(UC_HOOK_MEM_READ, tick, None, t, t))
    for off in (0x6BB8, 0x9B77):
        x = a.lin(LOAD_SEG, off)
        hooks.append(mu.hook_add(UC_HOOK_CODE, sample, None, x, x))
    for off in (0x084C, 0x0845):
        x = a.lin(LOAD_SEG + 0x2294, off)
        hooks.append(mu.hook_add(UC_HOOK_CODE, key, None, x, x))
    # Each frame's call of the movie's callback (0000:663F).
    x = a.lin(LOAD_SEG, 0x663F)
    hooks.append(mu.hook_add(UC_HOOK_CODE, frame, None, x, x))
    x = a.lin(LOAD_SEG + 0x1194, 0xD4F7)
    hooks.append(mu.hook_add(UC_HOOK_CODE, over, None, x, x))
    mu.ctl_flush_tb()
    mu.mem_write(0xA0000, bytes(64000))
    mu.mem_write(a.lin(a.ds, 0x01AA), bytes([level]))
    mu.mem_write(a.lin(a.ds, 0x0F76), bytes([1]))
    mu.mem_write(a.lin(a.ds, 0x0F77), bytes([f77]))
    mu.mem_write(a.lin(a.ds, 0x0F7A), bytes([0]))
    seg = LOAD_SEG + 0x1194
    a._setup([0xFFF0], {})
    mu.reg_write(UC_X86_REG_CS, seg)
    mu.reg_write(UC_X86_REG_IP, 0xD3AF)
    a.fault = None
    err = None
    try:
        mu.emu_start(a.lin(seg, 0xD3AF), a.lin(seg, 0xFFF0), count=int(os.environ.get('COUNT', 3_000_000_000)))
    except UcError as e:
        err = str(e)
    for h in hooks:
        mu.hook_del(h)
    cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
    if err or a.fault:
        sys.exit('stopped at %04X:%04X: %s %s' % (cs - LOAD_SEG, ip, err, a.fault))
    print('through level %d F77 %d: %d states, keys %s, end %s, stopped at %04X:%04X' % (level, f77, len(seen), keys, st8['end'], cs - LOAD_SEG, ip), file=sys.stderr)
    assert st8['end'] is not None, 'the first movie never ended'
    return seen, keys, st8['end']


def main():
    exe, game, save, out = sys.argv[1:5]
    dump = sys.argv[5] if len(sys.argv) > 5 else None
    a = ScreenArena(exe, game, save)
    a.start()
    for seg, off, code in [(0x2F9F, 0x001F, b'\x31\xc0\xcb'), (0x2F9F, 0x0438, b'\x31\xc0\xcb')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    base = a.snapshot()
    with open(out, 'w') as f:
        for level, f77 in SCRIPTS:
            a.restore(base)
            seen = run(a, level, f77, dump)
            f.write(f'dream | script | {level} | {f77} | {STATES}\n')
            for n, (s, p, raw) in enumerate(seen):
                f.write(f'dream | {level} | {f77} | {n} | {s:x} | {p:x}\n')
                if dump:
                    open(os.path.join(dump, f'dream{level}-{f77}-{n}.raw'), 'wb').write(raw)
        # The install's speech (SPEECH\\nnnn_mmX.VOC, 2F75:000A's open):
        # without it the movie's player copies FFDCh bytes into its 4E20h
        # buffer (2F75:00DC, the bank's first word not 0) and the heap
        # after it is overwritten; the first movie's end then fails the
        # heap's walk (0000:5213, error 7).
        speech = os.path.join(sys.argv[2], 'SPEECH')
        if os.path.isdir(speech):
            a.names.update({n.upper(): os.path.join(speech, n) for n in os.listdir(speech)})
        for level, f77 in THROUGH_SCRIPTS:
            a.restore(base)
            a.clicks = []
            seen, keys, end = through(a, level, f77, dump)
            f.write(f'dreamthrough | script | {level} | {f77} | {GAP} | {THROUGH}\n')
            for n, frames in keys:
                f.write(f'dreamthrough | {level} | {f77} | key | {n} | {frames}\n')
            f.write(f'dreamthrough | {level} | {f77} | end | {end}\n')
            for n, (s, p, raw) in enumerate(seen):
                f.write(f'dreamthrough | {level} | {f77} | {n} | {s:x} | {p:x}\n')
                if dump:
                    open(os.path.join(dump, f'through{level}-{f77}-{n}.raw'), 'wb').write(raw)


if __name__ == '__main__':
    main()
