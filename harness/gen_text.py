"""Emit test vectors for the game's text (src/text.rs `text`, 1194:A374
through its far entry A36C, with the substitutions 2DA2:000A).

Usage: gen_text.py ACD_unpacked.EXE GAME_DIR OUT

Runs under the DOS file layer (harness/dos.py) with the user's
POINTER1.DAT, TEMPLATE.DAT and NAMECHNK.DAT, EMS on with the frame at
FRAME. A case draws its mode (byte % 3), then the state listed in
tests/vectors/text_spec.txt, from its seed's byte stream; the output (mode
2) goes to OUT_SEG:0. The message box (1194:59CC) and the menu
(1194:020E) are stubbed, counting their calls at DGROUP FFE0 and FFE2.

  text | seed | result

Results: "stuck" (the original faults or doesn't return: which of the two,
after it has jumped into garbage, depends on the emulator's state from
earlier cases, not on memory or registers, so they are one result), or
FNV-1a digests of the changed DGROUP bytes,
the changed 3276 bytes, the output's first 200h bytes and the frame.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream
from gen_frame import counter
from unicorn import UcError

EXE, GAME, OUT = sys.argv[1:4]
FRAME, OUT_SEG, CSEG = 0x8800, 0x7000, 0x4C00
DATA = 0x3276
SPEC = os.path.join(os.path.dirname(__file__), '..', 'tests', 'vectors', 'text_spec.txt')
CHARS = b' abcdefghijklmnopqrstuvwxyz'

d = open(os.path.join(GAME, 'POINTER1.DAT'), 'rb').read()
KEYS = []
for i in range(0, 0x2710, 2):
    k = struct.unpack_from('<H', d, i)[0]
    if k == 0:
        break
    KEYS.append(k)


def spec():
    out = []
    for line in open(SPEC):
        line = line.split('#')[0].split()
        if line:
            out.append(line)
    return out


SPEC_LINES = spec()


def setup(seed):
    """(mode, [(seg, off, bytes)]) with seg 'ds', 'data' or 'c'. Mirrored
    in tests/text.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    mode = byte() % 3
    w = []
    for f in SPEC_LINES:
        seg, at, kind, args = f[0], int(f[1], 16), f[2], f[3:]
        if kind == 'b':
            v = bytes([byte() % int(args[0], 16)])
        elif kind == 'w':
            v = struct.pack('<H', word() % int(args[0], 16))
        elif kind == 'd':
            v = bytes(byte() for _ in range(4))
        elif kind == 'bc':
            c = [int(x, 16) for x in args[0].split(',')]
            v = bytes([c[byte() % len(c)]])
        elif kind == 'wc':
            c = [int(x, 16) for x in args[0].split(',')]
            v = struct.pack('<H', c[byte() % len(c)])
        elif kind == 'w2':
            lo = byte() % int(args[0], 16)
            hi = byte() % int(args[1], 16)
            v = bytes([lo, hi])
        elif kind == 'str':
            v = bytes(CHARS[byte() % 27] for _ in range(int(args[0], 16))) + b'\0'
        elif kind == 'bytes':
            v = bytes(byte() for _ in range(int(args[0], 16)))
        elif kind == 'seg':
            v = 'seg'
        elif kind == 'text':
            if byte() < 0xF0:
                k = KEYS[word() % len(KEYS)]
                t = k & 0xFF if k & 0x8000 else k
            else:
                t = word() % 0x700
            v = struct.pack('<H', t)
        elif kind == 'aim':
            if byte() < int(args[0], 16):
                for item in args[1].split(','):
                    addr, val = item.split(':')
                    n = int(val[1:], 16)
                    w.append((seg, int(addr, 16), bytes([n]) if val[0] == 'b' else struct.pack('<H', n)))
            continue
        else:
            raise ValueError(kind)
        w.append((seg, at, v))
    return mode, w


def call_far(a, seg, off, budget=100_000_000, **regs):
    """emu.call_far with a larger budget (a text with several names runs
    past 10M instructions) and faults reported."""
    from emu import STUB_SEG
    from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP, UC_X86_REG_SS, UC_X86_REG_SP, UC_X86_REG_DS, UC_X86_REG_ES, UC_X86_REG_EAX, UC_X86_REG_EDI
    from unicorn.x86_const import UC_X86_REG_EFLAGS, UC_X86_REG_FS, UC_X86_REG_GS
    mu = a.mu
    ss, sp = 0x9000, 0xfff0 - 4
    mu.mem_write(a.lin(ss, sp), struct.pack('<HH', 0, STUB_SEG))
    for r, v in ((UC_X86_REG_DS, a.ds), (UC_X86_REG_ES, regs['es']), (UC_X86_REG_SS, ss), (UC_X86_REG_SP, sp),
                 (UC_X86_REG_EAX, regs['eax']), (UC_X86_REG_EDI, regs['edi']), (UC_X86_REG_CS, LOAD_SEG + seg), (UC_X86_REG_IP, off)):
        mu.reg_write(r, v)
    # Every case starts from the same registers: one that faulted may leave
    # the direction flag set, and garbage paths read leftover ones.
    from unicorn.x86_const import UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EBP
    for r in (UC_X86_REG_EBX, UC_X86_REG_ECX, UC_X86_REG_EDX, UC_X86_REG_ESI, UC_X86_REG_EBP):
        mu.reg_write(r, 0)
    for r, v in ((UC_X86_REG_EFLAGS, 0x0002), (UC_X86_REG_FS, a.ds), (UC_X86_REG_GS, a.ds)):
        mu.reg_write(r, v)
    a.fault = None
    mu.emu_start(a.lin(LOAD_SEG + seg, off), a.lin(STUB_SEG, 0), count=budget)
    if a.fault is not None:
        raise Fault(a.fault)
    assert mu.reg_read(UC_X86_REG_CS) == STUB_SEG, 'did not return'


def main():
    out = open(OUT, 'w', buffering=1)
    a = DosArena(EXE, GAME)
    a.patch(0x1194, 0x59CC, counter(0xFFE0, False))
    a.patch(0x1194, 0x020E, counter(0xFFE2, True))
    a.init_heap()
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
    a.enable_ems(FRAME)
    BASE = a.snapshot()
    rng = random.Random(0x1994_A374)
    rds = LOAD_SEG + DATA
    dump = os.environ.get('DUMP')
    total = int(os.environ.get('N', 4500))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total) or dump and seed != int(dump, 16):
            continue
        a.restore(BASE)
        mode, writes = setup(seed)
        for seg, at, v in writes:
            base = {'ds': a.ds, 'data': rds, 'c': LOAD_SEG + CSEG}[seg]
            if v == 'seg':
                v = struct.pack('<H', LOAD_SEG + CSEG)
            a.mu.mem_write(a.lin(base, at), v)
        a.w16(0x91F0, 0)
        if dump:
            print('5c8', hex(a.r16(0x5C8)), '137e', hex(a.r16(0x137E) & 0xFF), '6e54', a.r16(0x6E54) & 0xFF)
            from unicorn import UC_HOOK_CODE
            from unicorn.x86_const import UC_X86_REG_BX, UC_X86_REG_DI, UC_X86_REG_ES
            def tr(mu, addr, size, u):
                from unicorn.x86_const import UC_X86_REG_DS
                for at in (0x0344, 0x0350, 0x03BD):
                    if addr == a.lin(LOAD_SEG + 0x2DA2, at):
                        print('at', hex(at), 'ds', hex(mu.reg_read(UC_X86_REG_DS) - LOAD_SEG))
                if addr == a.lin(LOAD_SEG + 0x2DA2, 0x0074):
                    bx = mu.reg_read(UC_X86_REG_BX)
                    h = struct.unpack('<H', mu.mem_read(a.lin(LOAD_SEG + DATA, bx), 2))[0]
                    at = a.r16(0x730D, seg=LOAD_SEG + DATA)
                    print('handler', hex(h), 'bx', hex(bx), 'text at', hex(at))
            a.mu.hook_add(UC_HOOK_CODE, tr)
        before = a.seg_bytes(a.ds)
        before_data = a.seg_bytes(rds)
        try:
            call_far(a, 0x1194, 0xA36C, eax=mode, es=OUT_SEG, edi=0)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            after_data = a.seg_bytes(rds)
            changed = changes(before, after)
            dchanged = changes(before_data, after_data)
            text = bytes(a.mu.mem_read(a.lin(OUT_SEG, 0), 0x200))
            frame = a.seg_bytes(FRAME)
            # %en writes 8A52, 8A54 in the places list's segment (CSEG).
            places = bytes(a.mu.mem_read(a.lin(LOAD_SEG + CSEG, 0x8A52), 4))
            res = f'{fnv(changed):x} {fnv(dchanged):x} {fnv(text):x} {fnv(frame):x} {fnv(places):x}'
            if dump:
                open(OUT + '.ds', 'w').write(''.join(f'{i:04x} {before[i]:02x} {after[i]:02x}\n' for i in range(0x10000) if after[i] != before[i]))
                open(OUT + '.frame', 'wb').write(frame)
                open(OUT + '.data', 'w').write(''.join(f'{i:04x} {before_data[i]:02x} {after_data[i]:02x}\n' for i in range(0x10000) if after_data[i] != before_data[i]))
        except (Fault, UcError) as e:
            res = 'stuck'
            if dump:
                from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP
                print('fault', e, hex(a.mu.reg_read(UC_X86_REG_CS) - LOAD_SEG), hex(a.mu.reg_read(UC_X86_REG_IP)))
        except AssertionError:
            res = 'stuck'
        out.write(f'text | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
