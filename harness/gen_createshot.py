"""Emit the screens of a new game's character creation as the original
draws them (view/src/create.rs is checked against them).

Usage: gen_createshot.py ACD_unpacked.EXE GAME_DIR OUT [DUMP_DIR [FRAMES]]

Runs creation, 1194:2498 (from 1194:24A6, after its song; the random
seed 91F6 set to SEED), on the game as screens.py starts it (21EE:0000:
the page, the fonts, the box tiles, EMS), the songs, the mouse's cursor
and the CyberMan's poll left out, files written kept nowhere.

Each script (SCRIPTS: a name and its inputs, as view/src/create.rs's
`parse_script` reads them) is fed to the program's input: a key through
0000:713C (the keyboard's next character), the keyboard buffer's count
(ACB0) or the held keys' table (ACB5 by scan code), each set for the
one read that takes it; a click through 0000:8FC8 and the driver (the
mouse's place and buttons, 8F92, 8F94, AFF9: pressed for one read, then
let go; never taken by a loop waiting for the buttons to be let go). The
game's timer count (0151) ticks at each read of it.

As a player answers a screen seen and settled, an input is taken at the
third poll in a row from one place with the screen (the VGA's memory and
the page, A900) unchanged and changed since the last input (not while
an animation plays, not before the answer is drawn), or at the 50th. The
screen then (the VGA's memory) and the colours (the DAC, 6-bit) are what
the player answered:

  createshot | script | script | INPUTS
  createshot | script | step | input@where | screen | colours

the screen and colours their FNV-1a digests; "end" the screen after the
last input, where creation asks for another. With DUMP_DIR each screen
is written there too (SCRIPT-STEP.raw: the 64000 bytes, the 768 colours).
What the player sees between two screens answered (a box dissolving in,
1194:4FF7; the colours faded) is sampled at each wait for the retrace
(0000:6BB8, 9B77: the VGA and the DAC as they are there), and written to
FRAMES (by default OUT's directory's createframes.txt):

  createframes | script | step | count | digest

the states seen after the input before `step` up to `step`'s screen: each
different from the one before, the first not the screen answered and the
last not `step`'s; the digest the FNV-1a of each state's two digests
(screen, colours: 8 bytes each, little-endian) in turn. With DUMP_DIR
each step's states are written there too (SCRIPT-STEP.frames: each 64000
bytes then 768).

A typed field's cursor (0000:A0D9) is kept from blinking on (A84C
held at 1 at each pass): its phase is time's. TRACE (the last polls), PROF (the hottest blocks), DEBUGAT=SEG:OFF,...
(registers there) and COUNT (the instruction budget) help debugging.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_BLOCK, UC_HOOK_CODE, UC_HOOK_INSN, UC_HOOK_MEM_READ, UcError
from unicorn.x86_const import *
from emu import LOAD_SEG, fnv
from screens import ScreenArena

SEED = 0x12345678

# The scripts: each from the first question (how the class is chosen).
SCRIPTS = [
    ('roll', 'G,enter,' + 'click:160:190,' * 12 + 'click:160:125,' * 10 + 'A'),
    ('generate', 'G,enter,A,B,C,A,B,C,A,B,C,A,N,enter'),
    ('rename', 'S,enter,type:Ria,enter,F,enter,click:240:80,Y,enter,enter,enter,enter,enter,click:20:10,back,type:Bo,enter,click:20:10,esc,type:Al,enter'),
    ('select', 'S,enter,type:Ria,enter,F,enter,click:240:80,Y,enter,enter,enter,enter,enter,'
               'click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:41:50,click:10:175,S,enter,click:240:20,click:240:20,click:10:175'),
]


def parse(script):
    out = []
    for t in script.split(','):
        if not t:
            continue
        if t == 'enter':
            out.append(('key', 0x0D))
        elif t == 'esc':
            out.append(('key', 0x1B))
        elif t == 'back':
            out.append(('key', 0x08))
        elif t.startswith('type:'):
            out.extend(('key', ord(c)) for c in t[5:])
        elif t.startswith('click:'):
            x, y = t[6:].split(':')
            out.append(('click', int(x), int(y)))
        elif len(t) == 1:
            out.append(('key', ord(t)))
        else:
            raise ValueError(t)
    return out


# Keys' scan codes (set 1): the held keys' table at ACB5 is by them.
SCANS = {0x1B: 0x01, 0x0D: 0x1C, 0x08: 0x0E, 0x20: 0x39}
SCANS.update({ord(c): sc for row, first in (('QWERTYUIOP', 0x10), ('ASDFGHJKL', 0x1E), ('ZXCVBNM', 0x2C), ('1234567890', 0x02))
              for sc, c in enumerate(row, first)})
HELD = 0xACB5
# The keyboard buffer's count of keys (0000:7138 reads it, and loops that
# wait for any key).
BUFFERED = 0xACB0
# With TRACE in the environment, the last polls and shows (printed at the
# end of a run).
TRACE = __import__('collections').deque(maxlen=40) if os.environ.get('TRACE') else None


class Run:
    def __init__(self, a, inputs):
        self.a = a
        self.inputs = list(inputs)
        self.step = 0
        self.shots = []
        self.pressed = None
        self.over = False
        self.release = None
        # A human answers a screen seen and settled: an input is taken at
        # the third poll in a row from one place with the screen (the
        # VGA's memory and the page drawn into, A900) the same, and
        # changed since the last input (not before the answer is drawn,
        # not during an animation); or at the 50th such poll.
        self.streak = {}
        self.answered = None
        self.answered_state = None
        self.hooks = []
        self.typed = None
        # The states seen at the retrace's waits since the last input:
        # (screen digest, colours digest, bytes or None).
        self.seen = []
        self.frames = []
        self.keep = False

    def arm(self):
        """The keyboard's memory watched where the script's keys are
        (a read hook over all of it slows the loops that scan it): the
        buffer's count (ACB0) and the keys' places in the held keys'
        table. (Hooks added while running aren't seen: all at the start.)"""
        a, mu = self.a, self.a.mu
        at = {a.lin(a.ds, BUFFERED)}
        for kind, *v in self.inputs:
            sc = SCANS.get(v[0], SCANS.get(v[0] & 0xDF)) if kind == 'key' else None
            if sc is not None:
                at.add(a.lin(a.ds, HELD + sc))
        self.hooks = [mu.hook_add(UC_HOOK_MEM_READ, self.held, None, x, x) for x in sorted(at)]

    def ready(self, site):
        """Whether an input may be taken at this poll (from `site`)."""
        if TRACE is not None:
            TRACE.append(('poll', self.step, site, self.seen))

        a = self.a
        now = (fnv(a.mu.mem_read(0xA0000, 64000)), fnv(a.mu.mem_read(a.lin(a.r16(0xA900), 0), 64000)))
        last, n = self.streak.get(site, (None, 0))
        n = n + 1 if last == now else 0
        self.streak[site] = (now, n)
        return (n >= 2 and now[0] != self.answered) or n >= 50

    def letting_go(self):
        """Whether the mouse is read by a loop waiting for its buttons to
        be let go (the read returns to `lcall 2F26:02A1` (optional), `test
        byte [AFF9], m`, `jne` back): no click is taken there."""
        mu = self.a.mu
        ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        near, ip, cs = struct.unpack('<HHH', mu.mem_read(ss * 16 + sp, 6))
        if near != 0x8FC7:
            return False
        code = bytes(mu.mem_read(cs * 16 + ip, 16))
        if code[0] == 0x9A and code[1:3] == b'\xa1\x02':
            code = code[5:]
        return code[:3] == b'\xf6\x06\xf9' and code[3] == 0xAF and code[5] == 0x75 and code[6] >= 0x80

    def caller(self):
        """Where the input routine was called from (its far caller)."""
        mu = self.a.mu
        ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        near, ip, cs = struct.unpack('<HHH', mu.mem_read(ss * 16 + sp, 6))
        if near in (0x713B, 0x8FC7):
            return '%04X:%04X' % (cs - LOAD_SEG, ip)
        return '0000:%04X' % near

    def retrace(self, mu, addr, size, ud):
        """A wait for the retrace: the state seen, if not the last seen."""
        shown = bytes(mu.mem_read(0xA0000, 64000))
        pal = bytes(self.a.dac)
        st = (fnv(shown), fnv(pal))
        if not self.seen or self.seen[-1][:2] != st:
            self.seen.append(st + ((shown + pal) if self.keep else None,))

    def shot(self, what):
        shown = bytes(self.a.mu.mem_read(0xA0000, 64000))
        # The states between: not the screen answered before, not this one.
        now = (fnv(shown), fnv(bytes(self.a.dac)))
        seen = [s for i, s in enumerate(self.seen) if i == 0 or s[:2] != self.seen[i - 1][:2]]
        if seen and self.answered_state is not None and seen[0][:2] == self.answered_state:
            seen = seen[1:]
        if seen and seen[-1][:2] == now:
            seen = seen[:-1]
        self.frames.append((self.step, seen))
        self.seen = []
        self.answered_state = now
        self.shots.append((self.step, what, shown, bytes(self.a.dac)))
        self.step += 1
        self.streak = {}
        self.answered = fnv(shown)

    def ret(self, mu, ax=None):
        """A near return from the routine's body (713C, 8FC8), DS left
        DGROUP as they leave it."""
        ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        ip = struct.unpack('<H', mu.mem_read(ss * 16 + sp, 2))[0]
        mu.reg_write(UC_X86_REG_SP, sp + 2)
        mu.reg_write(UC_X86_REG_DS, self.a.ds)
        if ax is not None:
            mu.reg_write(UC_X86_REG_AX, ax)
        mu.reg_write(UC_X86_REG_IP, ip)

    def end(self, mu):
        self.over = True
        mu.emu_stop()

    def key(self, mu, addr, size, ud):
        a = self.a
        k = 0
        # A typed field's cursor (0000:A0D9, XORed on every eighth pass of
        # its loop, A84C) kept from blinking on: its phase is the time's,
        # and the view draws it by its own frames.
        if self.caller() == '0000:A195':
            a.w16(0xA84C, 1)
        if self.typed is not None:
            k, self.typed = self.typed, None
            if self.release == a.lin(a.ds, BUFFERED):
                mu.mem_write(self.release, b'\0')
                self.release = None
        elif self.pressed is None and self.inputs and self.inputs[0][0] == 'key' and self.ready(self.caller()):
            k = self.inputs.pop(0)[1]
            self.shot('key:%02x@%s' % (k, self.caller()))
        elif self.pressed is None and not self.inputs and self.ready(self.caller()):
            self.shot('end@' + self.caller())
            return self.end(mu)
        self.ret(mu, (a.ds & 0xFF00) | k)

    def held(self, mu, access, addr, size, value, ud):
        """A read of the keyboard's memory: the next key, held (or in the
        buffer) for this read only (taken: its character isn't typed)."""
        a = self.a
        if self.release == addr:
            mu.mem_write(addr, b'\0')
            self.release = None
        elif self.pressed is None and self.inputs and self.inputs[0][0] == 'key' and \
                addr in (a.lin(a.ds, BUFFERED), a.lin(a.ds, HELD + SCANS.get(self.inputs[0][1], SCANS.get(self.inputs[0][1] & 0xDF, 0)))) and \
                self.ready('%04X:%04X' % (mu.reg_read(UC_X86_REG_CS) - LOAD_SEG, mu.reg_read(UC_X86_REG_IP))):
            k = self.inputs.pop(0)[1]
            cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
            how = 'buffered' if addr == a.lin(a.ds, BUFFERED) else 'held'
            self.shot('%s:%02x@%04X:%04X' % (how, k, cs - LOAD_SEG, ip))
            if how == 'buffered':
                # Its character there for the keyboard's read (7138).
                self.typed = k
            mu.mem_write(addr, b'\x01')
            self.release = addr

    def mouse(self, mu, addr, size, ud):
        a = self.a
        if self.pressed is not None:
            x, y = self.pressed
            a.w16(0xAFF9, a.r16(0xAFF9) & 0xFF00)
            self.pressed = None
        elif self.inputs and self.inputs[0][0] == 'click' and not self.letting_go() and self.ready(self.caller()):
            _, x, y = self.inputs.pop(0)
            self.shot('click:%d:%d@%s' % (x, y, self.caller()))
            self.pressed = (x, y)
            a.w16(0xAFF9, a.r16(0xAFF9) & 0xFF00 | 1)
        else:
            if not self.inputs and self.ready(self.caller()):
                self.shot('end@' + self.caller())
                return self.end(mu)
            x, y = a.r16(0x8F92), a.r16(0x8F94)
            a.w16(0xAFF9, a.r16(0xAFF9) & 0xFF00)
        a.w16(0x8F92, x)
        a.w16(0x8F94, y)
        # The driver itself (int 33h) says the same.
        a.mouse = (x, y)
        a.buttons = 1 if self.pressed is not None else 0
        mu.reg_write(UC_X86_REG_BX, y)
        self.ret(mu, x)


def machine(exe, game):
    """The game started as screens.py starts it (21EE:0000: the page, the
    fonts, the box tiles, the interface's pictures, EMS), with the songs,
    the mouse's cursor and the CyberMan's poll left out."""
    a = ScreenArena(exe, game)
    a.init_memory()
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    for seg, off, code in [(0x2F9F, 0x001F, b'\x31\xc0\xcb'), (0x2F9F, 0x0438, b'\x31\xc0\xcb'),
                           (0, 0x5422, b'\xc3'), (0, 0x5458, b'\xc3'), (0x2F26, 0x02A1, b'\xcb')]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    return a


def run(a, inputs, keep=False):
    mu = a.mu
    r = Run(a, inputs)
    r.keep = keep
    hooks = []
    r.arm()
    # The game's timer count (0151, the timer interrupt's): a tick each
    # time a wait reads it.
    t = a.lin(a.ds, 0x0151)

    def tick(mu, access, addr, size, value, ud):
        a.w16(0x0151, a.r16(0x0151) + 1)
    hooks.append(mu.hook_add(UC_HOOK_MEM_READ, tick, None, t, t))
    for off, f in ((0x713C, r.key), (0x8FC8, r.mouse), (0x6BB8, r.retrace), (0x9B77, r.retrace)):
        x = a.lin(LOAD_SEG, off)
        hooks.append(mu.hook_add(UC_HOOK_CODE, f, None, x, x))
    prof = {}
    if os.environ.get('PROF'):
        def block(mu, addr, size, ud):
            prof[addr] = prof.get(addr, 0) + 1
        hooks.append(mu.hook_add(UC_HOOK_BLOCK, block))
    a.w32(0x91F6, SEED)
    if os.environ.get('DEBUGAT'):
        for w in os.environ['DEBUGAT'].split(','):
            sg, of = (int(x, 16) for x in w.split(':'))

            def dbg(mu, addr, size, w):
                regs = {n: '%04X' % mu.reg_read(globals()['UC_X86_REG_' + n]) for n in ('AX', 'BX', 'CX', 'DX', 'SI', 'DI', 'DS', 'ES', 'FLAGS')}
                ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
                stack = struct.unpack('<6H', bytes(mu.mem_read(ss * 16 + sp, 12)))
                print('at', w, regs, 'stack', ' '.join('%04X' % v for v in stack), file=sys.stderr)
            x = a.lin(LOAD_SEG + sg, of)
            hooks.append(mu.hook_add(UC_HOOK_CODE, dbg, w, x, x))
    # Code translated before the hooks were added doesn't see them.
    mu.ctl_flush_tb()
    a._setup([0xFFF0, LOAD_SEG + 0x1194], {})
    mu.reg_write(UC_X86_REG_CS, LOAD_SEG + 0x1194)
    mu.reg_write(UC_X86_REG_IP, 0x24A6)
    err = None
    try:
        mu.emu_start(a.lin(LOAD_SEG + 0x1194, 0x24A6), a.lin(LOAD_SEG + 0x1194, 0xFFF0), count=int(os.environ.get('COUNT', 3_000_000_000)))
    except UcError as e:
        err = str(e)
    for h in hooks + r.hooks:
        mu.hook_del(h)
    for addr, n in sorted(prof.items(), key=lambda kv: -kv[1])[:12]:
        print('prof %05X %d' % (addr - LOAD_SEG * 16, n), file=sys.stderr)
    if TRACE is not None:
        for t in TRACE:
            print(*t, file=sys.stderr)
    if not r.over:
        cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
        if (cs, ip) == (LOAD_SEG + 0x1194, 0xFFF0) and a.fault is None and not r.inputs:
            r.over = True  # creation over (1194:25B7)
        else:
            err = err or 'stopped at %04X:%04X fault %s' % (cs - LOAD_SEG, ip, a.fault)
    return r, err


def main():
    exe, game, out = sys.argv[1:4]
    dump = sys.argv[4] if len(sys.argv) > 4 else None
    frames_out = sys.argv[5] if len(sys.argv) > 5 else os.path.join(os.path.dirname(os.path.abspath(out)), 'createframes.txt')
    f = open(out, 'w', buffering=1)
    ff = open(frames_out, 'w', buffering=1)
    a = machine(exe, game)
    base = a.snapshot()
    for name, script in SCRIPTS:
        a.restore(base)
        f.write(f'createshot | {name} | script | {script}\n')
        r, err = run(a, parse(script), keep=dump is not None)
        for step, seen in r.frames:
            digest = fnv(b''.join(struct.pack('<QQ', s[0], s[1]) for s in seen))
            ff.write(f'createframes | {name} | {step} | {len(seen)} | {digest:x}\n')
            if dump and seen:
                open(os.path.join(dump, f'{name}-{step}.frames'), 'wb').write(b''.join(s[2] for s in seen))
        for step, what, buf, pal in r.shots:
            f.write(f'createshot | {name} | {step} | {what} | {fnv(buf):x} | {fnv(pal):x}\n')
            if dump:
                open(os.path.join(dump, f'{name}-{step}.raw'), 'wb').write(buf + pal)
        if err:
            f.write(f'createshot | {name} | end | {err}\n')
            print(name, err, file=sys.stderr)
    f.close()
    ff.close()


if __name__ == '__main__':
    main()
