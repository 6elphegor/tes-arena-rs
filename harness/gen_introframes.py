"""Emit what the original shows through its intro and up to its main menu,
frame by frame (view/src/create.rs is checked against it).

Usage: gen_introframes.py ACD_unpacked.EXE GAME_DIR OUT

Runs the program's main from 0000:10C9 (as it starts, and as it comes
back after a game's end or the options screen's New): the intro (1246:
INTRO.FLC, TITLE.IMG faded in and held, QUOTE.IMG faded in and held,
SCROLL.FLC, SCROLL01..03.IMG dissolved in and held, each change through
1194:55B2's fade to black and the screen cleared), then MENU.IMG shown
and its colours set (10FA), up to the menu's wait for its regions
(1114); and the opening story the menu's Start plays (0000:1323:
SCROLL.FLC, INTRO01..09.IMG dissolved in and held, 55B2) to its return.
The game is as gen_createshot.py starts it (21EE:0000), the
songs, the mouse's cursor and the CyberMan's poll left out.

What the player sees is sampled at each wait for the retrace (0000:6BA8,
and 9B74 in the colours' setting, 9B56) and at each read of the game's
timer (0151: the movies' frames wait on it, a tick a read): the screen
(the VGA's memory) and the colours (the DAC, 6-bit). Each state, in
turn, with how many of each it was seen at:

  introframes | case | script | INPUTS
  introframes | case | n | screen | colours | retraces | ticks
  introframes | case | end | SEED

the screen and colours their FNV-1a digests; SEED the dissolve's
generator (4B8E) at the end.

A case's inputs are keys (`KEY@POLL[+HELD]`, the key's character in hex):
pressed at the POLL-th read of the mouse (0000:8FC8, which each pass of
the waits and each movie frame makes), or with `sN` for POLL as the run's
N-th state is first seen (in a fade, which reads no mouse), its character in the keyboard's
buffer (ACB0) until a flush (7112) takes it and the key held (the held
keys' table at ACB5, by scan code) for HELD waits for the retrace (by
default 1), or until a loop waits for every key to be let go (0000:66A2,
a movie's end by a key). With POLLS in the environment, the read each
state starts at is printed.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ
from unicorn.x86_const import *
from emu import LOAD_SEG, fnv
from gen_createshot import machine, SCANS

HELD = 0xACB5
BUFFERED = 0xACB0
MENU_WAIT = 0x1114

# Each case: a name, where it starts (0000:10C9 near, the program's main
# on to its menu; 1323 far, the opening story after the menu's Start), and
# its keys (character, poll, held). The mouse's reads: INTRO.FLC's frames
# 1..76, the title's wait 78..357, the quote's 359..781, SCROLL.FLC's
# frames 782..830, the scrolls' waits 831..2020, 2022..3211, 3213..3632.
CASES = [
    ('none', 0x10C9, []),
    # A key in INTRO.FLC (65CD: ACB0; 6697: the keys waited up, flushed).
    ('intromovie', 0x10C9, [(0x0D, 20, 1)]),
    # Enter in the title's wait (13BF: ACB0), let go at once.
    ('titlekey', 0x10C9, [(0x0D, 120, 1)]),
    # Esc in the title's wait, held through the quote (its ACB6 tests).
    ('titleesc', 0x10C9, [(0x1B, 120, 2000)]),
    # Esc tapped there: let go before the quote's wait tests it (12CD).
    ('titletap', 0x10C9, [(0x1B, 120, 1)]),
    # Enter, and Esc held, in the quote's wait.
    ('quotekey', 0x10C9, [(0x0D, 500, 1)]),
    ('quoteesc', 0x10C9, [(0x1B, 500, 2000)]),
    # Esc held in SCROLL.FLC: the movie's end waits for it to be let go.
    ('scrollmovie', 0x10C9, [(0x1B, 800, 2000)]),
    # A key in the first scroll's wait, then Esc held in the second's.
    ('scrollkeys', 0x10C9, [(0x0D, 1000, 1), (0x1B, 1500, 2000)]),
    # The opening story: SCROLL.FLC (frames 1..50), INTRO01..09.IMG
    # (their waits 351 reads apart from 51).
    ('story', 0x1323, []),
    # Keys in a fade (no poll there; pressed as the state is seen): Enter
    # in TITLE.IMG's fade in (states 94..128), Esc held and Esc tapped
    # (let go at the next wait for the retrace) in QUOTE.IMG's (154..188).
    ('titlefade', 0x10C9, [(0x0D, 's110', 1)]),
    ('quotefadeesc', 0x10C9, [(0x1B, 's170', 2000)]),
    ('quotefadetap', 0x10C9, [(0x1B, 's170', 1)]),
    # A key in the second picture's wait, Esc held in the fourth's.
    ('storykeys', 0x1323, [(0x0D, 500, 1), (0x1B, 1300, 2000)]),
]


class Run:
    def __init__(self, a, keys):
        self.a = a
        self.keys = list(keys)
        self.polls = 0
        self.held = None
        self.states = []
        self.last = None

    def sample(self, kind):
        a = self.a
        shown = bytes(a.mu.mem_read(0xA0000, 64000))
        pal = bytes(a.dac)
        if self.last is None or self.last != (shown, pal):
            self.last = (shown, pal)
            self.states.append([fnv(shown), fnv(pal), 0, 0])
            if os.environ.get('POLLS'):
                print(len(self.states) - 1, 'at poll', self.polls, file=sys.stderr)
            if self.keys and self.keys[0][1] == 's%d' % (len(self.states) - 1):
                self.press()
        self.states[-1][kind] += 1

    def press(self):
        """The next key pressed: in the buffer, held."""
        a = self.a
        k, _, n = self.keys.pop(0)
        self.let_go()
        sc = SCANS[k]
        a.w16(BUFFERED, a.r16(BUFFERED) + 1)
        a.mu.mem_write(a.lin(a.ds, HELD + sc), b'\x01')
        self.held = (sc, n)

    def retrace(self, mu, addr, size, ud):
        self.sample(2)
        if self.held is not None:
            sc, n = self.held
            if n <= 1:
                self.let_go()
            else:
                self.held = (sc, n - 1)

    def tick(self, mu, access, addr, size, value, ud):
        a = self.a
        a.w16(0x0151, (a.r16(0x0151) + 1) & 0xFFFF)
        self.sample(3)

    def let_go(self):
        if self.held is not None:
            self.a.mu.mem_write(self.a.lin(self.a.ds, HELD + self.held[0]), b'\0')
            self.held = None

    def all_up(self, mu, addr, size, ud):
        self.let_go()

    def mouse(self, mu, addr, size, ud):
        a = self.a
        self.polls += 1
        if self.keys and self.keys[0][1] == self.polls:
            self.press()
        x, y = a.r16(0x8F92), a.r16(0x8F94)
        a.w16(0xAFF9, a.r16(0xAFF9) & 0xFF00)
        ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        ip = struct.unpack('<H', mu.mem_read(ss * 16 + sp, 2))[0]
        mu.reg_write(UC_X86_REG_SP, sp + 2)
        mu.reg_write(UC_X86_REG_DS, a.ds)
        mu.reg_write(UC_X86_REG_AX, x)
        mu.reg_write(UC_X86_REG_BX, y)
        mu.reg_write(UC_X86_REG_IP, ip)


def run(a, start, keys):
    mu = a.mu
    r = Run(a, keys)
    t = a.lin(a.ds, 0x0151)
    hooks = [mu.hook_add(UC_HOOK_MEM_READ, r.tick, None, t, t + 1)]
    for seg, off, f in ((0, 0x6BA8, r.retrace), (0, 0x9B74, r.retrace), (0, 0x8FC8, r.mouse), (0, 0x66A2, r.all_up)):
        x = a.lin(LOAD_SEG + seg, off)
        hooks.append(mu.hook_add(UC_HOOK_CODE, f, None, x, x))

    def menu(mu, addr, size, _):
        # Its registers kept for `pick` (ES:SI the regions, main's stack).
        a.menu_regs = {r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()])
                       for r in ('eax', 'ebx', 'ecx', 'edx', 'esi', 'edi', 'ebp', 'es', 'ss', 'sp')}
        a.waiting = ('menu',)
        mu.emu_stop()
    m = a.lin(LOAD_SEG, MENU_WAIT)
    hooks.append(mu.hook_add(UC_HOOK_CODE, menu, None, m, m))
    mu.ctl_flush_tb()
    try:
        if start == 0x10C9:
            res = a.run_screen(0, start, near=True, count=3_000_000_000)
        else:
            res = a.run_screen(0, start, count=3_000_000_000)
    finally:
        for h in hooks:
            mu.hook_del(h)
    want = ('menu',) if start == 0x10C9 else None
    assert res == want, f'stopped by {res} (fault {a.fault})'
    return r


# The menu's picks (`pick`): Load (region 0, on to 0000:117E) and Exit
# (2, on to 11B1), each after Esc held from the title's wait (`titleesc`'s
# way to the menu).
PICKS = [('menuload', 0, 0x117E), ('menuexit', 2, 0x11B1)]


def pick(a, region, stop):
    """From the menu's wait (`run`'s, its registers kept), the region
    `region` picked as 1194:5CF4 returns it (B7C6 the region, B7C4 bit 1),
    main on from 1119 (55B2) to `stop`, sampled as `run` samples."""
    mu = a.mu
    r = Run(a, [])
    t = a.lin(a.ds, 0x0151)
    hooks = [mu.hook_add(UC_HOOK_MEM_READ, r.tick, None, t, t + 1)]
    for off, f in ((0x6BA8, r.retrace), (0x9B74, r.retrace)):
        x = a.lin(LOAD_SEG, off)
        hooks.append(mu.hook_add(UC_HOOK_CODE, f, None, x, x))

    def stopped(mu, addr, size, _):
        a.waiting = ('picked',)
        mu.emu_stop()
    x = a.lin(LOAD_SEG, stop)
    hooks.append(mu.hook_add(UC_HOOK_CODE, stopped, None, x, x))
    a.w16(0xB7C6, region)
    a.w16(0xB7C4, a.r16(0xB7C4) | 2)
    regs = dict(a.menu_regs)
    stack = (regs.pop('ss'), regs.pop('sp'))
    mu.ctl_flush_tb()
    try:
        res = a.run_screen(0, 0x1119, regs=regs, stack=stack, count=3_000_000_000)
    finally:
        for h in hooks:
            mu.hook_del(h)
    assert res == ('picked',), f'stopped by {res} (fault {a.fault})'
    return r


def main():
    exe, game, out = sys.argv[1:4]
    only = sys.argv[4:]
    a = machine(exe, game)
    base = a.snapshot()
    with open(out, 'w') as f:
        for name, start, keys in CASES:
            if only and name not in only:
                continue
            a.restore(base)
            f.write('introframes | %s | script | %s\n' % (name, ','.join('%02x@%s+%d' % k for k in keys) or '-'))
            r = run(a, start, keys)
            for n, (s, c, rt, tk) in enumerate(r.states):
                f.write(f'introframes | {name} | {n} | {s:x} | {c:x} | {rt} | {tk}\n')
            f.write('introframes | %s | end | %04x\n' % (name, a.r16(0x4B8E)))
            print(name, len(r.states), 'states', file=sys.stderr)
        for name, region, stop in PICKS:
            if only and name not in only:
                continue
            a.restore(base)
            run(a, 0x10C9, [(0x1B, 120, 2000)])
            r = pick(a, region, stop)
            for n, (s, c, rt, tk) in enumerate(r.states):
                f.write(f'menupick | {name} | {n} | {s:x} | {c:x} | {rt} | {tk}\n')
            print(name, len(r.states), 'states', file=sys.stderr)


if __name__ == '__main__':
    main()
