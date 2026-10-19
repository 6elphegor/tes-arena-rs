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

Lines:
  args | n | tail | carry | log | runs | digest | psp
  cd | n | drive cds first ax bx | env | carry | log | runs | digest

`tail` and `env` hex; `log` comma-separated: startup, sound:NAME,
music:NAME (hex), video, print:OFF; `runs` the DGROUP bytes the case
changed (start+length, hex) and `digest` theirs after (FNV-1a); `psp`
the digest of the PSP's 100h bytes after.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import fnv, LOAD_SEG
from dos import DosArena
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_EFLAGS, UC_X86_REG_SP, UC_X86_REG_SS

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
    out.close()


if __name__ == '__main__':
    main()
