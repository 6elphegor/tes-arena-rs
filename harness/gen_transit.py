"""Emit test vectors for moving between areas (src/transit.rs) from the
original code.

Usage: gen_transit.py ACD_unpacked.EXE OUT

Four kinds of case, by the seed's first byte:
- next: the outer loop after an area's run, from 0000:0ED5 (the stack
  holding the call's return and 0000:FFF0) to the next area's run, whose
  entries (0A83, 090A, 09FA, 0936, 0993) are replaced by stops recording
  the entry's number (FFD0) and SI (FFD2); FFF0 a stop for leaving
  (number 6). `area_next`.
- run: an area's run (0000:0936 with SI the name, 0993, 09FA, 0A83,
  090A) with the 3D world (0000:0AA9) stubbed. `area_begin`, `area_end`.
- world: 0000:0AA9 with SI the name, the world loop (A26F) stubbed:
  `world_begin` (with 2294:025B, `mode_3d`, and 0000:1776), `world_end`.
- hook: the way out 0000:4537 or 44BD as CD4E holds (`area_exit`; other
  values none), the place 0000:45B2 (`area_place`) or the entry
  0000:4632 (`area_entry`, its sky 0000:464A..467C replaced by a count).

Stubbed callees count their calls in DGROUP (see STUBS); some keep
registers: 1194:D459 AX, SI, DI at FFB0..FFB4, 1194:3AE8 AX at FFB6,
0000:28BC BX, AX at FFB8, FFBA, 0000:0AA9 SI at FFBC (and CD32, CD4E at
FFC4, FFC6), 0000:1793 AX at
FF92. 26F7:0000 and 217B:0058 return the carry from FFDE bits 0 and 1;
25CA:01E6 returns FFC0.

  transit | seed | result

Results: "fault" / "hang" / "none", or the FNV-1a digest of the changed
DGROUP bytes, far pointers' segments image-relative.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG
from emu import changes
from emu import mine
from gen_attack import fnv, stream
from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP, UC_X86_REG_SP, UC_X86_REG_SS, UC_X86_REG_DS, UC_X86_REG_ES, UC_X86_REG_FS, UC_X86_REG_GS, UC_X86_REG_SI


def counter(at, far, keep=()):
    """mov [k], reg for (k, reg) in keep; inc word [at]; ret/retf"""
    code = b''
    regs = {'ax': 0x06, 'si': 0x36, 'di': 0x3E, 'bx': 0x1E}
    for k, r in keep:
        code += bytes([0x89, regs[r], k & 0xFF, k >> 8])
    return code + bytes([0xFF, 0x06, at & 0xFF, at >> 8, 0xCB if far else 0xC3])


def carry(at, bit):
    """inc word [at]; test byte [FFDE], bit; jz +2; stc; retf; clc; retf"""
    return bytes([0xFF, 0x06, at & 0xFF, at >> 8, 0xF6, 0x06, 0xDE, 0xFF, bit, 0x74, 0x02, 0xF9, 0xCB, 0xF8, 0xCB])


def stop(n, si=True):
    """mov word [FFD0], n; (mov [FFD2], si;) jmp far STUB_SEG:0"""
    code = bytes([0xC7, 0x06, 0xD0, 0xFF, n, 0])
    if si:
        code += bytes([0x89, 0x36, 0xD2, 0xFF])
    return code + bytes([0xEA, 0, 0, STUB_SEG & 0xFF, STUB_SEG >> 8])


STUBS = [
    (0x2F9F, 0x0438, counter(0xFFE0, True)),
    (0x1194, 0xD459, counter(0xFFE2, True, [(0xFFB0, 'ax'), (0xFFB2, 'si'), (0xFFB4, 'di')])),
    (0, 0x742C, counter(0xFFE4, False)),
    (0x1194, 0x3AE8, counter(0xFFE6, True, [(0xFFB6, 'ax')])),
    (0, 0x0F05, counter(0xFFE8, True)),
    (0x1194, 0x0292, counter(0xFFEA, True)),
    (0x217B, 0x0084, counter(0xFFEC, True)),
    (0x217B, 0x04DC, counter(0xFFEE, True)),
    (0x2294, 0x0ED4, counter(0xFFF0, True)),
    (0, 0x28BC, counter(0xFFF2, False, [(0xFFB8, 'bx'), (0xFFBA, 'ax')])),
    (0, 0x17F9, counter(0xFFF6, True)),
    (0x2294, 0x11F2, counter(0xFFF8, True)),
    (0x292B, 0x095E, counter(0xFFFA, True)),
    (0, 0xA611, counter(0xFFFC, True)),
    (0, 0xA606, counter(0xFFFE, False)),
    (0x26F7, 0x0000, carry(0xFFA0, 1)),
    (0, 0xA5D1, counter(0xFFA2, False)),
    (0, 0xA6B5, counter(0xFFA4, True)),
    (0, 0x9426, counter(0xFFA6, True)),
    # (0000:9466 is a far call of 946A.)
    (0x2542, 0x0446, counter(0xFFAA, True)),
    (0x2433, 0x0199, counter(0xFFAC, True)),
    (0x1194, 0xD55F, counter(0xFFAE, True)),
    (0, 0x1793, counter(0xFF90, False, [(0xFF92, 'ax')])),
    (0, 0x2974, counter(0xFF94, False)),
    (0x217B, 0x0184, counter(0xFF96, True)),
    (0x217B, 0x0058, carry(0xFF98, 2)),
    # 0000:464A: inc word [FF9A]; jmp 467D.
    (0, 0x464A, bytes([0xFF, 0x06, 0x9A, 0xFF, 0xEB, 0x467D - 0x4650])),
    (0, 0x8FC4, counter(0xFF80, True)),
    (0x25FD, 0x034B, counter(0xFF82, True)),
    (0, 0x16F7, counter(0xFF84, True)),
    # 25CA:01E6: mov ax, [FFC0]; inc word [FF86]; retf.
    (0x25CA, 0x01E6, bytes([0xA1, 0xC0, 0xFF]) + counter(0xFF86, True)),
    (0, 0x068A, counter(0xFF88, True)),
    (0, 0x2631, counter(0xFF8A, True)),
    (0, 0x5D70, counter(0xFF8C, True)),
    (0, 0x3503, counter(0xFF8E, True)),
    (0, 0xA26F, counter(0xFF70, True)),
    (0x23F1, 0x013C, counter(0xFF72, True)),
    (0, 0x5E3E, counter(0xFF74, True)),
    (0, 0x4EFD, counter(0xFF76, True)),
    (0, 0x946A, counter(0xFF78, False)),
]
# 0000:0AA9: the level hooks the run set (cleared after it) kept: mov
# ax, [CD32]; mov [FFC4], ax; mov ax, [CD4E]; mov [FFC6], ax; then SI
# and the count.
WORLD = (0, 0x0AA9, bytes([0xA1, 0x32, 0xCD, 0xA3, 0xC4, 0xFF, 0xA1, 0x4E, 0xCD, 0xA3, 0xC6, 0xFF])
         + counter(0xFFF4, False, [(0xFFBC, 'si')]))
STOPS = [(0, 0x0A83, stop(0)), (0, 0x090A, stop(1)), (0, 0x09FA, stop(3)), (0, 0x0936, stop(4)),
         (0, 0x0993, stop(2)), (0, 0xFFF0, stop(6, False))]
RUNS = [0x0936, 0x0993, 0x09FA, 0x0A83, 0x090A]

# Far pointers' segment words the code may write (relocated in the
# emulator).
FAR = [0xC3A6, 0xC3AA, 0xC3AE, 0xC3B2, 0xC3B6, 0xC3BA, 0xCD1C, 0xCD24, 0xCD28, 0xCD2C, 0xCD30, 0xCD34,
       0xCD3C, 0xCD44, 0xCD48, 0xCD4C, 0xCD50, 0xCD54, 0xCD58, 0xCD5C, 0xCD64, 0xCD6C, 0xCD70]
NAMES = [0x1351, 0x1385, 0x1319, 0x0A36]


def setup(seed):
    """(offset, bytes) DGROUP writes and the case's kind. Mirrored in
    tests/transit.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    kind = byte() % 4
    sub = byte() % 5
    area = lambda: (0, 1, 2, 3, 4, 4, 0, 1)[byte() % 8] if byte() < 0xE0 else byte()
    w.append((0x4BD0, pb(area())))
    w.append((0x05E6, pb(area())))
    w.append((0x05C8, pk(word())))
    v = word() & 0x7FFF
    if byte() < 0x10:
        v |= 0x8000
    w.append((0x05CA, pk(v)))
    v = word() & 0xFEFF
    if byte() < 0x10:
        v |= 0x100
    w.append((0x05B1, pk(v)))
    w.append((0x05E4, pb(byte())))
    w.append((0xC398, pk(word())))
    w.append((0xC39A, pk(word())))
    w.append((0x0FCE, pk(0 if byte() < 0x40 else word())))
    w.append((0x0FD0, pk(word())))
    w.append((0x0D58, pk(0 if byte() < 0x50 else NAMES[byte() % 4])))
    k = byte()
    w.append((0x0F77, pb(0 if k < 0x50 else 9 if k < 0x80 else byte())))
    w.append((0x0A46, pb(8 if byte() < 0x80 else byte() % 10)))
    w.append((0x0A4C, pb(byte())))
    w.append((0x0A56, bytes(byte() for _ in range(4))))
    w.append((0x0608, pk(word())))
    w.append((0x060A, pk(word())))
    w.append((0x01A9, pb(0xC9 if byte() < 0x30 else byte())))
    w.append((0x01A8, pb(6 if byte() < 0x30 else byte() % 8)))
    k = byte()
    t = (0xFF, 0x100, 0x300, 0x301)[byte() % 4] if k < 0x40 else word() % 0x400
    w.append((0xCCFE, pk(t)))
    w.append((0x0D37, pk(word())))
    w.append((0xC3D0, pk(word())))
    w.append((0xC3D4, pk(word())))
    w.append((0x0D39, pb(byte())))
    w.append((0xBB0A, pb(byte())))
    w.append((0xBB09, pb((0, 1, 2, 0x81, 0x82)[byte() % 5] if byte() < 0xC0 else byte())))
    w.append((0x0DA8, pk((0, 4, 8)[byte() % 3])))
    k = byte()
    w.append((0xCD4E, pk((0x4537, 0x44BD, word())[k % 3]) + pk(0)))
    w.append((0xFFDE, pb(byte())))
    w.append((0xFFC0, pk(8 if byte() < 0x80 else word())))
    w.append((0x1A81, pb(byte())))
    w.append((0xC994, pb(byte())))
    w.append((0x1351, b'temple3.mif\0'))
    return w, kind, sub


def relocated(a, mem, slots):
    """The far pointers' segments at `slots` image-relative."""
    mem = bytearray(mem)
    for o in slots:
        v = struct.unpack_from('<H', mem, o)[0]
        if v >= LOAD_SEG:
            struct.pack_into('<H', mem, o, v - LOAD_SEG)
    return bytes(mem)


def make(exe, patches):
    a = Arena(exe)
    for seg, off, code in patches:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)
    a.w16(0xAFF9, 0)
    a.base = a.seg_bytes(a.ds)
    return a


def run_next(a):
    """0000:0ED5 with its call's return and FFF0 on the stack, to a stop."""
    a._setup([0x1234, 0xFFF0], {})
    a._run(0, 0x0ED5, STUB_SEG, 0, count=5_000_000)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    n_ = make(EXE, STUBS + [WORLD] + STOPS)
    r_ = make(EXE, STUBS + [WORLD])
    w_ = make(EXE, STUBS)
    rng = random.Random(0x1994_0CAD)
    total = int(os.environ.get('N', 4000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        w, kind, sub = setup(seed)
        a = (n_, r_, w_, r_)[kind]
        a.mu.mem_write(a.lin(a.ds, 0), a.base)
        for off, data in w:
            a.mu.mem_write(a.lin(a.ds, off), data)
        before = relocated(a, a.seg_bytes(a.ds), FAR)
        res = None
        try:
            if kind == 0:
                run_next(a)
            elif kind == 1:
                a.call_near(0, RUNS[sub], si=struct.unpack_from('<H', a.seg_bytes(a.ds), 0x0D58)[0] or 0x1351)
            elif kind == 2:
                a.call_near(0, 0x0AA9, si=NAMES[sub % 4])
            elif sub < 2:
                hook = struct.unpack_from('<I', before, 0xCD4E)[0]
                if hook in (0x4537, 0x44BD):
                    a.call_far(0, hook)
                else:
                    res = 'none'
            elif sub < 4:
                a.call_far(0, 0x45B2)
            else:
                a.call_far(0, 0x4632)
            if res is None:
                after = relocated(a, a.seg_bytes(a.ds), FAR)
                ch = changes(before, after)
                res = f'{fnv(ch):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'transit | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
