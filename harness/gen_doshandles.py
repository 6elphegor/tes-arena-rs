"""Emit the vectors for DOS's handles as the game's file calls meet them
(src/bsa.rs `Dos`): handle 2 used after a missing file, closed, and
handed to the next file opened.

Usage: gen_doshandles.py ACD_unpacked.EXE GAME_DIR SAVE_DIR OUT

The game started and set up as gen_programstart.py has it, then for each
case from that state a script of the game's own file calls, far-called:

  miss NAME / open NAME  0000:67C4 with SI the name (at DGROUP 1044):
                         the open that goes on after a missing file
  seek W                 0000:6D32, AX DX 0, CL W
  read N                 0000:6BEC, CX N, DS:SI the EMS frame
  close                  0000:6742
  copy FROM TO           25FD:056F, SI 1044 (FROM), DI 1051 (TO)
  log TEXT               27E3:000E (near), SI 1044 (TEXT): the logbook's
                         entry added to LOG.nn (nn the slot, 64 here),
                         its trim tried first where the log holds 2000
                         bytes or more (to the path DOS refuses)
  file NAME              no call: the file NAME as written so far

One line a case:

  dos | NAME | SCRIPT | RESULTS

SCRIPT the calls joined by `; `, RESULTS one per call joined by `; `: an
open's CS:6837; a seek's DX:AX; a read's AX and the frame's first AX
bytes' digest (when AX is no more than asked for); a close's AX and
carry; a copy's 126C, 126E, A836, A838, the four counts at 5F5C.., and
the destination as written (length and digest); a log's AX; a file's
length and digest as written, or `-`. A call that waits for
the keyboard (DOS reading CON) is `waits`, and ends the case.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, LOAD_SEG, fnv
from screens import ScreenArena

CASES = [
    ('con', 'miss NOSUCH.A; seek 2; seek 0; read 0; close; open LOG.01; seek 2; close'),
    ('free', 'miss NOSUCH.A; seek 2; read 0; close; miss NOSUCH.B; seek 2; seek 0; read 6; close'),
    ('reused', 'miss NOSUCH.A; close; open LOG.01; miss NOSUCH.B; seek 2; seek 0; read 10; close; close'),
    ('two-files', 'miss NOSUCH.A; close; open LOG.01; open CITYDATA.01; seek 2; close'),
    ('copy-missing', 'miss NOSUCH.A; close; copy NOSUCH.C NOSUCH.99'),
    ('copy-over', 'miss NOSUCH.A; close; copy NOSUCH.C STATES.01'),
    ('copy-there', 'miss NOSUCH.A; close; copy LOG.01 LOG.99'),
    ('copy-con', 'copy NOSUCH.C NOSUCH.99'),
    ('con-read', 'miss NOSUCH.A; read 10'),
    # The logbook: LOG.64 made and an entry added; then a log of 2000
    # bytes or more (the save's LOG.01, 801h bytes), whose trim's create
    # DOS refuses (error 3), so it only grows; the next entry too.
    ('log-new', 'log Found the first piece.; file LOG.64'),
    ('log-full', 'miss NOSUCH.A; close; copy LOG.01 LOG.64; log Found the second piece.; file LOG.64'),
    ('log-twice', 'miss NOSUCH.A; close; copy LOG.01 LOG.64; log One.; log Two.; file LOG.64'),
]


def step(a, op):
    """One call of the script; its result as the vectors write it."""
    w = op.split()
    cs = LOAD_SEG * 16
    if w[0] in ('miss', 'open'):
        a.mu.mem_write(a.lin(a.ds, 0x1044), w[1].encode() + b'\0')
        assert a.run_screen(0, 0x67C4, dict(si=0x1044)) is None
        return '%x' % struct.unpack('<H', a.mu.mem_read(cs + 0x6837, 2))[0]
    if w[0] == 'seek':
        assert a.run_screen(0, 0x6D32, dict(eax=0, edx=0, ecx=int(w[1]))) is None
        return '%x' % ((a._reg('dx') << 16) | a._reg('ax'))
    if w[0] == 'read':
        n = int(w[1], 16)
        frame = a.r16(0x9030)
        assert a.run_screen(0, 0x6BEC, dict(ecx=n, ds=frame, esi=0)) is None
        ax = a._reg('ax')
        got = a.seg_bytes(frame)[:ax] if ax <= n else b''
        return '%x %x' % (ax, fnv(bytes(got)))
    if w[0] == 'close':
        assert a.run_screen(0, 0x6742) is None
        return '%x %d' % (a._reg('ax'), a.mu.reg_read(a.UC_EFLAGS) & 1)
    if w[0] == 'copy':
        a.mu.mem_write(a.lin(a.ds, 0x1044), w[1].encode() + b'\0')
        a.mu.mem_write(a.lin(a.ds, 0x1051), w[2].encode() + b'\0')
        assert a.run_screen(0x25FD, 0x056F, dict(si=0x1044, di=0x1051)) is None
        words = [a.r16(o) for o in (0x126C, 0x126E, 0xA836, 0xA838)]
        counts = [a.r16(0x5F5C + 4 * k) for k in range(4)]
        dst = a.written.get(w[2])
        written = '-' if dst is None else '%x/%x' % (len(dst), fnv(bytes(dst)))
        return ' '.join('%x' % v for v in words + counts) + ' ' + written
    if w[0] == 'log':
        a.mu.mem_write(a.lin(a.ds, 0x1044), op[4:].encode() + b'\0')
        assert a.run_screen(0x27E3, 0x000E, dict(si=0x1044), near=True) is None
        return '%x' % a._reg('ax')
    if w[0] == 'file':
        f = a.written.get(w[1])
        return '-' if f is None else '%x/%x' % (len(f), fnv(bytes(f)))
    raise ValueError(op)


def main():
    EXE, GAME, SAVE, OUT = sys.argv[1:5]
    a = ScreenArena(EXE, GAME, SAVE)
    from unicorn.x86_const import UC_X86_REG_EFLAGS
    a.UC_EFLAGS = UC_X86_REG_EFLAGS
    a.init_memory()
    assert a.run_screen(0x21EE, 0) is None, 'the start waited'
    a.w16(0xCB8A, a.r16(0x9030))
    saved = a.stub()
    a.mu.mem_write(a.lin(a.ds, 0x1A80), b'\x40')
    a.mu.mem_write(a.lin(a.ds, 0x1A6C), b'\x40')
    assert a.run_screen(0x21EE, 0x016C) is None, 'the setup waited'
    a.unstub(saved)
    snap = a.snapshot()
    out = open(OUT, 'w')
    for name, script in CASES:
        a.restore(snap)
        results = []
        for op in script.split('; '):
            try:
                results.append(step(a, op))
            except Fault as e:
                assert e.args[0] == 0x21 and a._reg('ax') >> 8 == 0x3F, f'{name}: {op}: fault {e}'
                results.append('waits')
                break
        out.write(f'dos | {name} | {script} | {"; ".join(results)}\n')
        out.flush()
    out.close()


if __name__ == '__main__':
    main()
