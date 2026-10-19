"""Emit the vectors for the program's command line and its CD-ROM check
(src/cmdline.rs: `argv_split`, 0000:0118; `command_line`, 1194:066A;
`cd_check`, 21EE:0314).

Usage: gen_cmdline.py ACD_unpacked.EXE GAME_DIR OUT

The C start-up splits the PSP's command tail (PSP:80h, its length then
the text and a CR) into argv (0000:0118: the far pointers from the table
at [3F] (DGROUP 51h, argv[0] left), argc counted up from [3D]'s 1, each
argument's end overwritten with a NUL). main, given two words or more
(0000:1072), hands them to 1194:066A: each "-" option upper-cased in
place and read (the sound driver's name -S to 3827, the music driver's
-M to 37EB, the -??E digit to 37E9, -DMA/-IRQ decimal and -IO hex to the
port settings 3864..386F through the buffer 929E), anything else the
"Bad command line arguments" message and the carry. Then a driver named
starts the AIL (0FFD:07D6 once, 37EA) and is loaded (2F9F:0364 the
sound's, 2F9F:01BF the music's).

main's 21EE:0314 first: the current drive (int 21h AH 19h) to A836,
MSCDEX asked (int 2Fh AX 1500h: BX the CD drives, CL the first; with
none, "MSCDEX Driver not installed"; the current drive not the first,
int 2Fh AX 150Bh on it: BX ADADh and AX non-zero for a CD drive, else
the CD-ROM message); then the environment ([47]'s segment) searched for
"ARENADA" (7 characters, either case) and the first such string's text
past its 10th character copied to the data path 1AB9 (C:\\ARENA_CD).

The AIL start-up, the drivers' loads and the video's restore (0000:6F8C)
are logged and return at once; so is a message printed (int 21h AH 9h,
by its offset). A case's PSP is at segment 8F00h and its environment at
8E00h, outside the image and the heap in the harness and the port alike.

The program's own start (`c_start`, 0000:0000 to main; `main_head`,
0000:102E to its call of 21EE:0000) runs from the entry with DOS's and
the BIOS's answers modelled: the version, the interrupt table (each
vector a stand-in, `vector`), the ticks and midnight's flag, the
screen's attribute; the DTA's setting logged. It stops at main's call of
21EE:0000 (main), main's quitting after the CD-ROM check (quit) or the
start-up's for want of memory (memory).

Lines:
  args | n | tail | carry | log | runs | digest | psp
  cd | n | drive cds first ax bx | env | carry | log | runs | digest
  start | n | top version cx dx al attr | drive cds first ax bx | tail | env | stop | log | runs | digest | heap | psp | midnight

`tail` and `env` hex; `log` comma-separated: startup, sound:NAME,
music:NAME (hex), video, print:OFF; `runs` the DGROUP bytes the case
changed (start+length, hex) and `digest` theirs after (FNV-1a; main's
error handler's segment A906 taken 1000h lower, as the port's unrelocated
image has it; from the stack's segment on left out); `psp` the digest of the PSP's
100h bytes after, `heap` the first header's 11 bytes, `midnight` the
BIOS's byte 0040:0070.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv, LOAD_SEG
from dos import DosArena
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_EFLAGS, UC_X86_REG_IP, UC_X86_REG_SP, UC_X86_REG_SS

PSP, ENV = 0x8F00, 0x8E00
AIL_STARTUP = (0x0FFD, 0x07D6)
SOUND_LOAD = (0x2F9F, 0x0364)
MUSIC_LOAD = (0x2F9F, 0x01BF)
VIDEO = (0x0000, 0x6F8C)


class CmdArena(DosArena):
    def __init__(self, exe, game):
        super().__init__(exe, game)
        self.log = []
        self.cd = None
        self.start = None
        for at, what in ((AIL_STARTUP, 'startup'), (SOUND_LOAD, 'sound'), (MUSIC_LOAD, 'music'), (VIDEO, 'video')):
            lin = self.lin(LOAD_SEG + at[0], at[1])
            self.mu.mem_write(lin, b'\xcb')
            self.mu.hook_add(UC_HOOK_CODE, self._stub, begin=lin, end=lin, user_data=what)

    def _stub(self, mu, addr, size, what):
        if what in ('sound', 'music'):
            ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
            off, seg = struct.unpack('<HH', bytes(mu.mem_read(self.lin(ss, sp + 4), 4)))
            name = bytes(mu.mem_read(self.lin(seg, off), 80)).split(b'\0')[0]
            self.log.append(f'{what}:{name.hex()}')
        else:
            self.log.append(what)

    def _intr(self, mu, intno, u):
        ah = self._reg('ax') >> 8
        st = self.start
        if st is not None and self._start_int(intno, ah, st):
            return
        if intno == 0x21 and ah == 0x09:
            self.log.append(f'print:{self._reg("dx"):x}')
            return
        if intno == 0x21 and ah == 0x19 and self.cd is not None:
            self._wreg('ax', (self._reg('ax') & 0xFF00) | self.cd['drive'])
            return
        if intno == 0x2F and self.cd is not None:
            ax = self._reg('ax')
            if ax == 0x1500:
                if self.cd['cds']:
                    self._wreg('bx', self.cd['cds'])
                    self._wreg('cx', self.cd['first'])
                return
            if ax == 0x150B:
                self._wreg('ax', self.cd['ax'])
                self._wreg('bx', self.cd['bx'])
                return
        super()._intr(mu, intno, u)

    def _start_int(self, intno, ah, st):
        """DOS's and the BIOS's answers to the start-up: the version, the
        interrupt table (`vector`), the ticks, the screen's attribute."""
        if intno == 0x21 and ah == 0x30:
            self._wreg('ax', st['version'])
        elif intno == 0x21 and ah == 0x35:
            n = self._reg('ax') & 0xFF
            off, seg = struct.unpack('<HH', bytes(self.mu.mem_read(4 * n, 4)))
            self._wreg('bx', off)
            self._wreg('es', seg)
        elif intno == 0x21 and ah == 0x25:
            n = self._reg('ax') & 0xFF
            self.mu.mem_write(4 * n, struct.pack('<HH', self._reg('dx'), self._reg('ds')))
        elif intno == 0x21 and ah == 0x1A:
            self.log.append(f'dta:{(self._reg("ds") - LOAD_SEG) & 0xFFFF:x}:{self._reg("dx"):x}')
        elif intno == 0x1A and ah == 0:
            self._wreg('cx', st['cx'])
            self._wreg('dx', st['dx'])
            self._wreg('ax', (self._reg('ax') & 0xFF00) | st['al'])
        elif intno == 0x10 and ah == 8:
            self._wreg('ax', st['attr'] << 8 | 0x20)
        else:
            return False
        return True


def vector(n):
    """The interrupt table's stand-in vectors (segment high)."""
    return (0x0F00 + n) << 16 | (n * 0x111) & 0xFFFF


def runs_digest(d0, d1):
    changed = [i for i in range(0x10000) if d0[i] != d1[i]]
    runs = []
    for i in changed:
        if runs and runs[-1][0] + runs[-1][1] == i:
            runs[-1][1] += 1
        else:
            runs.append([i, 1])
    return ' '.join(f'{s:x}+{n:x}' for s, n in runs) or '-', f'{fnv(bytes(d1[i] for i in changed)):x}'


def carry(a):
    return a.mu.reg_read(UC_X86_REG_EFLAGS) & 1


def args_case(a, tail):
    """0000:0118 on the tail, then (argc 2 or more) 1194:066A as main
    calls it: DS:SI argv, CX argc, ES DGROUP."""
    a.mu.mem_write(a.lin(PSP, 0), bytes(0x100))
    a.mu.mem_write(a.lin(PSP, 0x80), bytes([len(tail)]) + tail + b'\r')
    a.w16(0x49, PSP)
    a.log = []
    d0 = a.seg_bytes(a.ds)
    a.call_near(0, 0x0118)
    cf = '-'
    argc = a.r16(0x3D)
    if argc >= 2:
        a.call_far(0x1194, 0x066A, ds=a.r16(0x41), si=a.r16(0x3F), cx=argc, es=a.ds)
        cf = str(carry(a))
    runs, digest = runs_digest(d0, a.seg_bytes(a.ds))
    psp = f'{fnv(bytes(a.mu.mem_read(a.lin(PSP, 0), 0x100))):x}'
    return f'{tail.hex()} | {cf} | {",".join(a.log) or "-"} | {runs} | {digest} | {psp}'


def cd_case(a, cd, env):
    a.mu.mem_write(a.lin(ENV, 0), bytes(0x200))
    a.mu.mem_write(a.lin(ENV, 0), env)
    a.w16(0x47, ENV)
    a.cd = cd
    a.log = []
    d0 = a.seg_bytes(a.ds)
    a.call_far(0x21EE, 0x0314)
    runs, digest = runs_digest(d0, a.seg_bytes(a.ds))
    a.cd = None
    head = f'{cd["drive"]:x} {cd["cds"]:x} {cd["first"]:x} {cd["ax"]:x} {cd["bx"]:x}'
    return f'{head} | {env.hex()} | {carry(a)} | {",".join(a.log) or "-"} | {runs} | {digest}'


STOPS = {0x1051: 'main', 0x10C2: 'quit', 0x0248: 'memory'}
STACK = ((0x4A8F - 0x3BA0) * 16, 0x10000)


def start_case(a, st, cd, tail, env):
    """The program from its entry 0000:0000, loaded at LOAD_SEG with its
    PSP at PSP (its top of memory `top`, its environment ENV) and its
    stack where the header puts it (SS 4A8Fh past the load, SP 80h), up
    to main's call of 21EE:0000 (0000:1051), main's quitting after the
    CD-ROM check (10C2) or the start-up's for want of memory (0248)."""
    for n in range(0x100):
        a.mu.mem_write(4 * n, struct.pack('<I', vector(n)))
    a.mu.mem_write(a.lin(PSP, 0), bytes(0x100))
    a.w16(2, st['top'], seg=PSP)
    a.w16(0x2C, ENV, seg=PSP)
    a.mu.mem_write(a.lin(PSP, 0x80), bytes([len(tail)]) + tail + b'\r')
    a.mu.mem_write(a.lin(ENV, 0), bytes(0x200))
    a.mu.mem_write(a.lin(ENV, 0), env)
    a.start, a.cd, a.log = st, cd, []
    d0 = a.seg_bytes(a.ds)
    stopped = []

    def stop(mu, addr, size, what):
        stopped.append(what)
        mu.emu_stop()
    hooks = [a.mu.hook_add(UC_HOOK_CODE, stop, begin=a.lin(LOAD_SEG, off), end=a.lin(LOAD_SEG, off), user_data=what)
             for off, what in STOPS.items()]
    a._setup([], dict(ds=PSP, es=PSP, ss=LOAD_SEG + 0x4A8F, sp=0x80))
    a.mu.reg_write(UC_X86_REG_CS, LOAD_SEG)
    a.mu.reg_write(UC_X86_REG_IP, 0)
    a.fault = None
    a.mu.emu_start(a.lin(LOAD_SEG, 0), 0xFFFFF, count=20_000_000)
    for h in hooks:
        a.mu.hook_del(h)
    assert a.fault is None and stopped, f'the start ran off: fault {a.fault}'
    a.start = a.cd = None
    d1 = bytearray(a.seg_bytes(a.ds))
    # main's error handler's segment (0000:11DA's), 1000h lower as the port's.
    w = struct.unpack_from('<H', d1, 0xA906)[0]
    struct.pack_into('<H', d1, 0xA906, (w - LOAD_SEG) & 0xFFFF if d0[0xA906:0xA908] != d1[0xA906:0xA908] else w)
    # The stack (SS 4A8Fh, SP from 80h to C10h: DGROUP EEF0h..FAFFh) holds
    # the start-up's and main's frames, not the game's state; past it the
    # heap's first header (the `heap` field), in the harness's DGROUP
    # window only (its image is 1000h higher).
    d1[STACK[0]:STACK[1]] = d0[STACK[0]:STACK[1]]
    runs, digest = runs_digest(d0, d1)
    first = a.r16(0x159)
    heap = bytes(a.mu.mem_read(a.lin(first, 0), 11)).hex() if stopped[0] != 'memory' else '-'
    psp = f'{fnv(bytes(a.mu.mem_read(a.lin(PSP, 0), 0x100))):x}'
    bios = a.mu.mem_read(a.lin(0x40, 0x70), 1)[0]
    head = ' '.join(f'{st[k]:x}' for k in ('top', 'version', 'cx', 'dx', 'al', 'attr'))
    cdh = ' '.join(f'{cd[k]:x}' for k in ('drive', 'cds', 'first', 'ax', 'bx'))
    return (f'{head} | {cdh} | {tail.hex()} | {env.hex()} | {stopped[0]} | {",".join(a.log) or "-"} | '
            f'{runs} | {digest} | {heap} | {psp} | {bios:x}')


def starts(r):
    ok = dict(drive=3, cds=1, first=3, ax=0, bx=0)
    plain = b'COMSPEC=C:\\COMMAND.COM\0PATH=C:\\DOS\0\0'
    bat = b'-Ssbpdig.adv -IOS220 -IRQS7 -DMAS1 -Msbp1fm.adv -IOM220'
    base = dict(top=0xA000, version=0x0005, cx=0x0012, dx=0x3456, al=0, attr=7)
    yield base, ok, b'', plain
    yield dict(base, al=1, version=0x1406), ok, bat, b'ARENADATA=D:\\ARENA\0\0'
    yield base, dict(ok, cds=0), b'', plain
    yield base, dict(ok, drive=2, ax=0, bx=0xADAD), bat, plain
    yield dict(base, top=0x5B00), ok, b'', plain
    yield dict(base, top=0x5B50), ok, b'', plain
    yield dict(base, top=0x5B4F), ok, b'', plain
    for _ in range(12):
        st = dict(top=r.choice([0xA000, 0x9FC0, 0x8000, 0xFFFF, 0x5C00]), version=r.randrange(0x10000),
                  cx=r.randrange(0x18), dx=r.randrange(0x10000), al=r.choice([0, 0, 1, 0x80]), attr=r.randrange(0x100))
        cd = dict(drive=r.randrange(5), cds=r.choice([0, 1, 1, 2]), first=r.randrange(5),
                  ax=r.choice([0, 1]), bx=r.choice([0xADAD, 0]))
        yield st, cd, r.choice([b'', bat, b'  -X ', b'a b c']), r.choice([plain, b'ARENADATA=E:\\X\0\0', b'\0\0'])


# The options ARENA.BAT's set-ups pass, and the others the parser tells.
DRIVERS = [b'SBPDIG.ADV', b'SBDIG.ADV', b'PASDIG.ADV', b'SBP1FM.ADV', b'SBFM.ADV', b'ADLIBG.ADV',
           b'GENMIDI.ADV', b'MT32MPU.ADV', b'sbawe32.adv', b'gf1midi.adv']


def token(r):
    k = r.randrange(16)
    num = lambda: str(r.choice([0, 1, 2, 3, 5, 7, 10, 15, 65535, 70000, -3])).encode()
    hexv = lambda: r.choice([b'220', b'240', b'330', b'388', b'2a0', b'FFFF', b'1g0', b'', b'12345'])
    case = (lambda s: s.lower()) if r.random() < 0.3 else (lambda s: s)
    if k == 0:
        return case(b'-S' + r.choice(DRIVERS))
    if k == 1:
        return case(b'-M' + r.choice(DRIVERS))
    if k == 2:
        return case(b'-IOS' + hexv())
    if k == 3:
        return case(b'-IOM' + hexv())
    if k == 4:
        return case(b'-IRQS' + num())
    if k == 5:
        return case(b'-IRQM' + num())
    if k == 6:
        return case(b'-DMAS' + num())
    if k == 7:
        return case(b'-DMAM' + num())
    if k == 8:
        return case(b'-' + bytes(r.choice(b'ABCX') for _ in range(2)) + b'E' + bytes(r.choice(b'0123456789AZ ') for _ in range(r.randrange(7))))
    if k == 9:
        return r.choice([b'-', b'-S', b'-M', b'-D', b'-IO', b'-IR', b'-X', b'/S', b'sbpdig.adv', b'-?'])
    if k == 10:
        return b'-S' + bytes(r.choice(b'ABCDEFGH.\x80\xff\x21') for _ in range(r.randrange(1, 70)))
    if k == 11:
        return b'-M' + bytes(r.randrange(0x21, 0x100) for _ in range(r.randrange(1, 70)))
    if k == 12:
        return b'-DMA' + bytes(r.choice(b'SM') for _ in range(1)) + bytes(r.choice(b'-+0123456789x') for _ in range(r.randrange(8)))
    if k == 13:
        return b'-IRQ' + bytes(r.choice(b'SMX') for _ in range(1)) + bytes(r.choice(b'+-09') for _ in range(r.randrange(6)))
    if k == 14:
        return b'-IO' + bytes(r.choice(b'SMX') for _ in range(1)) + bytes(r.choice(b'0123456789ABCDEFabcdefG') for _ in range(r.randrange(7)))
    return b'-' + bytes(r.choice(b'SMDIROEQZ') for _ in range(r.randrange(1, 6)))


def tails(r):
    yield b''
    yield b' '
    yield b'   '
    yield b'-Ssbpdig.adv -IOS220 -IRQS7 -DMAS1 -Msbp1fm.adv -IOM220'
    yield b'-ssbdig.adv -ios220 -irqs5 -dmas1 -msbfm.adv -iom388'
    yield b'-Mgenmidi.adv -IOM330'
    yield b'-Ssbpdig.adv'
    yield b'  -IOS220   -IRQS7  '
    yield b'-BADOPTION'
    yield b'-IOS220 junk -IRQS7'
    yield b'\t-IOS220'
    for _ in range(300):
        seps = [b' ', b' ', b' ', b'  ', b'\t']
        t = b''
        for i in range(r.randrange(1, 9)):
            t += (r.choice(seps) if i or r.random() < 0.2 else b'') + token(r)
        if r.random() < 0.2:
            t += b' ' * r.randrange(1, 4)
        yield t[:127]


def envs(r):
    yield b'COMSPEC=C:\\COMMAND.COM\0PATH=C:\\DOS\0\0'
    yield b'ARENADATA=D:\\ARENA\0\0'
    yield b'PATH=C:\\\0arenadata=E:\\GAMES\\ARENA\0\0'
    yield b'ArEnAdAtA=X\0ARENADATA=SECOND\0\0'
    yield b'ARENADAT\0\0'
    yield b'ARENADA\0\0'
    yield b'ARENADAX=YYYYYYYYYY\0\0'
    yield b'\0\0'
    for _ in range(40):
        e = b''
        for _ in range(r.randrange(0, 5)):
            k = r.randrange(5)
            if k == 0:
                v = b'ARENADATA=' + bytes(r.choice(b'CDEFG:\\ARENA_') for _ in range(r.randrange(0, 30)))
            elif k == 1:
                v = bytes(r.choice(b'aArReEnNdDtT') for _ in range(r.randrange(1, 12))) + b'=' + b'Z' * r.randrange(4)
            elif k == 2:
                v = b'ARENADA' + bytes(r.choice(b'=TAX') for _ in range(r.randrange(0, 5))) + b'Q' * r.randrange(8)
            else:
                v = bytes(r.choice(b'ABCPATH=\\;:') for _ in range(r.randrange(1, 20)))
            e += v.replace(b'\0', b'') + b'\0'
        yield e + b'\0'


def cds(r):
    yield dict(drive=3, cds=1, first=3, ax=0, bx=0)
    yield dict(drive=2, cds=0, first=0, ax=0, bx=0)
    yield dict(drive=2, cds=1, first=3, ax=0, bx=0xADAD)
    yield dict(drive=2, cds=2, first=3, ax=0x5A, bx=0xADAD)
    yield dict(drive=2, cds=1, first=3, ax=1, bx=0)
    for _ in range(20):
        yield dict(drive=r.randrange(6), cds=r.choice([0, 1, 2]), first=r.randrange(6),
                   ax=r.choice([0, 1, 0xFFFF]), bx=r.choice([0xADAD, 0, 0x1234]))


def main():
    EXE, GAME, OUT = sys.argv[1:4]
    a = CmdArena(EXE, GAME)
    base = a.snapshot()
    r = random.Random(1994)
    out = open(OUT, 'w')
    for n, tail in enumerate(tails(r)):
        a.restore(base)
        out.write(f'args | {n} | {args_case(a, tail)}\n')
    cases = list(cds(r))
    n = 0
    for env in envs(r):
        for cd in (cases if n < 8 else [r.choice(cases)]):
            a.restore(base)
            out.write(f'cd | {n} | {cd_case(a, cd, env)}\n')
            n += 1
    for n, (st, cd, tail, env) in enumerate(starts(r)):
        a.restore(base)
        out.write(f'start | {n} | {start_case(a, st, cd, tail, env)}\n')
    out.close()


if __name__ == '__main__':
    main()
