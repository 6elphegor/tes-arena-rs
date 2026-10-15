"""Emit test vectors for the level-up screen (src/levelup.rs
`level_up_screen`, 1194:1034) and exhaustion (`exhaustion`, 2901:0014,
through the fatigue's minute 2901:0002, `Machine::fatigue_minute`).

Usage: gen_levelup.py ACD_unpacked.EXE OUT

Op 0, the screen, far-called with CX the levels gained. Stubbed: the
notice's sound 23BA:012B counts at FFE2, keeping the screen 1194:158F at FFE4,
the message box 1194:59CC (59C8 calls it) at FFE8; the drawing (0000:541E, A611, 9A98,
9426, A6B5, 1194:55B2, 2462) returns; the points' spending 1194:122C
spends nothing its first time (FFE0 counts the calls), then puts all of
POINTS on the attribute FFDC names. The item toggle 1194:7605 runs with
gen_useitem.py's stubs.

Op 1, a minute of fatigue with the exhaustion running (the player's
death 2686:02B4, the hour's rest 2901:016C and the hour 0000:142D
included), from gen_clock.py's set-up with the fatigue low, sometimes in
a fight (0FFB) or in water (C39A bits 10h, 4000h). Stubbed as there:
quests and encounters (no level is gained: 5CA 0); the message box
counts at FFE8 as in op 0, the view's fill 0000:9A72 returns.

A case fills from a seed (`setup`, mirrored in tests/levelup.rs).

  levelup | seed | op | result

Results: "fault" / "hang", or FNV-1a digests: op 0 the changed DGROUP
bytes; op 1 those, the creature segment (32 KB) and the timer table
(1 KB), as gen_clock.py's. Op 1 runs with 0000:142D's budget (50 M
instructions) and is "stuck" past DEPTH collapses nested inside each
other (the hour of each running out of fatigue again; tracked by SP at
2901:0014), where the port gives up: the original goes on until its
stack, in DGROUP below 974Eh (0000:85C6), overwrites its data.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG, STUB_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream
from gen_frame import counter
import gen_clock
from unicorn import UcError
from unicorn import UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP, UC_X86_REG_SP

# src/levelup.rs EXHAUSTION_DEPTH.
DEPTH = 200

RETF, RET = b'\xcb', b'\xc3'
CSEG, TSEG = gen_clock.CSEG, gen_clock.TSEG
WSEG = 0x6800

# inc word [FFE0]; test byte [FFE0], 1; jnz done; mov bx, [FFDC];
# mov al, [129C]; add [bx+1CD], al; mov byte [129C], 0; done: ret
SPEND = bytes([0xFF, 0x06, 0xE0, 0xFF, 0xF6, 0x06, 0xE0, 0xFF, 0x01, 0x75, 0x10,
               0x8B, 0x1E, 0xDC, 0xFF, 0xA0, 0x9C, 0x12, 0x00, 0x87, 0xCD, 0x01,
               0xC6, 0x06, 0x9C, 0x12, 0x00, 0xC3])


def setup_screen(s):
    """DGROUP writes and CX for op 0."""
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    pb = lambda v: bytes([v & 0xFF])
    w = []
    w.append((0x01CD, bytes(byte() for _ in range(8))))
    k = byte()
    if k < 0x10:
        base = b'\xff' * 8
    elif k < 0x20:
        i = byte() % 8
        base = bytes(0xFD if j == i else 0xFF for j in range(8))
    elif k < 0x28:
        base = b'\xff' * 4 + bytes(byte() for _ in range(4)) if byte() & 1 else bytes(byte() for _ in range(4)) + b'\xff' * 4
    elif k < 0x60:
        base = bytes(0xE0 + byte() % 0x20 for _ in range(8))
    else:
        base = bytes(byte() for _ in range(8))
    w.append((0x01D5, base))
    w.append((0x01A9, pb(byte())))
    w.append((0x01AA, pb(byte() % 30)))
    for at in (0x01FD, 0x01FF, 0x0201, 0x020A, 0x020C):
        w.append((at, pk(word())))
    w.append((0x0FDD, pb(0x8E if byte() < 0x40 else byte())))
    w.append((0x05E0, pk(word())))
    w.append((0x05C8, pk(word())))
    w.append((0x129C, pk(word())))
    w.append((0x9686, pk(word())))
    for at in (0x102C, 0x1036, 0x103A):
        w.append((at, pk(word())))
    for i in range(40):
        b = bytearray(19)
        b[0] = (0x80 + byte() % 0x12) if byte() < 0x50 else byte() % 0x12
        f = byte() & 0x33 | (byte() % 4) << 2
        if byte() < 0xA0:
            f |= 0x40
        if byte() < 0x80:
            f |= 0x80
        b[0xE] = f
        for j in (1, 2, 3, 4, 5, 6, 7, 8, 9, 0xA, 0xB, 0xC, 0xD, 0xF, 0x10, 0x11):
            b[j] = byte()
        b[0x12] = 0xFF if byte() < 0x40 else byte()
        w.append((0x0212 + i * 19, bytes(b)))
    w.append((0x91F6, bytes(byte() for _ in range(4))))
    w.append((0xFFDC, pk(byte() % 8)))
    k = byte()
    cx = 0 if k < 4 else (1 + byte() % 4) | (byte() << 8 if k < 0x10 else 0)
    return w, cx


def setup_tired(s):
    """DGROUP writes after gen_clock's set-up, for op 1."""
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    w.append((0x0201, pk(byte() % 11 if byte() < 0xE0 else word())))
    # No level gained (5CA bit 100h): the screen is op 0's.
    w.append((0x05CA, pk(0)))
    w.append((0x1013, bytes([0 if byte() < 0xE0 else byte()])))
    k = byte()
    w.append((0x0FFB, pk(byte() | 1 if k < 0x50 else 0)))
    w.append((0xC39A, pk(word() & 0x4210 if byte() < 0x80 else word() & 0x0200)))
    for at in (0x102C, 0x1036, 0x103A):
        w.append((at, pk(word())))
    # The walls plane (D806), which a corpse's square is read from: a
    # zeroed segment, as its unset segment (0) would read low memory.
    w.append((0xD806, pk(WSEG)))
    return w


def setup(seed, op, auras):
    """(writes as (segment or None, offset, bytes), CX). Mirrored in
    tests/levelup.rs."""
    s = stream(seed)
    if op == 0:
        w, cx = setup_screen(s)
        return [(None, o, d) for o, d in w], cx
    w, _ = gen_clock.setup(seed ^ 0x5A5A5A5A, auras)
    w += [(None, o, d) for o, d in setup_tired(s)]
    return w, 0


def install_stubs(a):
    """The stubs (see the module's notes)."""
    rel = (0x7719 - (0x23B1 + 3)) & 0xFFFF
    clash = bytes([0xFF, 0x06, 0xE6, 0xFF, 0xEB, (0x1BC9 - (0x1B78 + 6)) & 0xFF])
    for seg, off, code in [
        # The screen's own.
        (0x23BA, 0x012B, counter(0xFFE2, True)), (0x1194, 0x158F, counter(0xFFE4, False)),
        (0x1194, 0x59CC, counter(0xFFE8, False)), (0x1194, 0x122C, SPEND),
        (0, 0x541E, RETF), (0, 0xA611, RETF), (0, 0x9A98, RETF), (0, 0x9426, RETF), (0, 0xA6B5, RETF),
        (0x1194, 0x55B2, RETF), (0x1194, 0x2462, RET),
        # gen_useitem.py's, for 1194:7605.
        (0x1194, 0x1BE0, counter(0xFFEC, False)), (0x1194, 0x1B78, clash), (0x1194, 0x5E21, RET),
        (0x1194, 0x23B1, bytes([0xE8, rel & 0xFF, rel >> 8, 0xC3])),
        (0, 0x5454, RETF), (0, 0x9466, RETF), (0x3159, 0x0006, RETF),
        # The exhaustion's and gen_clock.py's.
        (0, 0x9A72, RET),
        (0x1194, 0x99F8, RETF), (0x1194, 0x98E8, RETF), (0x1194, 0x887A, RETF), (0x1194, 0x86F2, RETF),
    ]:
        a.mu.mem_write(a.lin(LOAD_SEG + seg, off), code)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    install_stubs(a)
    a.w16(0x1042, CSEG)
    a.w16(0x91F0, 0)
    auras = [t + 1 for t in range(0x30) if a.mu.mem_read(a.lin(a.ds, 0x2DE6 + t), 1)[0] == 0xFE]
    base_ds = a.seg_bytes(a.ds)
    zero = bytes(0x10000)

    def unrelocated(mem):
        mem = bytearray(mem)
        for o in gen_clock.RELOC:
            if struct.unpack_from('<H', mem, o)[0] == a.ds:
                struct.pack_into('<H', mem, o, DGROUP)
        return bytes(mem)

    # The exhaustion's nesting (each collapse's hour may run out of fatigue
    # and collapse again inside it): SP at each entry still open. Past
    # DEPTH nested the case stops, as the port gives up there.
    nest, deep = [], [False]
    entry = a.lin(LOAD_SEG + 0x2901, 0x0014)

    def on_entry(mu, addr, size, _):
        sp = mu.reg_read(UC_X86_REG_SP)
        while nest and nest[-1] <= sp:
            nest.pop()
        nest.append(sp)
        if len(nest) > DEPTH:
            deep[0] = True
            mu.emu_stop()
    a.mu.hook_add(UC_HOOK_CODE, on_entry, begin=entry, end=entry)

    rng = random.Random(0x1994_1034)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        op = n % 2
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        for seg in (CSEG, TSEG, WSEG):
            a.mu.mem_write(a.lin(seg, 0), zero)
        w, cx = setup(seed, op, auras)
        for seg, off, data in w:
            a.mu.mem_write(a.lin(a.ds if seg is None else seg, off), data)
        for o in gen_clock.RELOC:
            if a.r16(o) == DGROUP:
                a.w16(o, a.ds)
        before = unrelocated(a.seg_bytes(a.ds))
        regs = dict(eax=0, ebx=0, ecx=cx, edx=0, esi=0, edi=0, ebp=0, fs=a.ds, gs=a.ds, eflags=0x2)
        nest.clear()
        deep[0] = False
        try:
            if op == 0:
                a.call_far(0x1194, 0x1034, **regs)
                if a.fault is not None:
                    raise Fault(a.fault)
            else:
                a._setup([0, STUB_SEG], regs)
                a._run(0x2901, 0x0002, STUB_SEG, 0)
            if deep[0]:
                raise Fault('deep')
            after = unrelocated(a.seg_bytes(a.ds))
            if os.environ.get('DUMP') == f'{seed:x}':
                for i in range(0x10000):
                    if before[i] != after[i]:
                        print(f'{i:04x} {before[i]:02x} {after[i]:02x}', file=sys.stderr)
            res = f'{fnv(changes(before, after)):x}'
            if op == 1:
                res += f' {fnv(a.seg_bytes(CSEG)[:0x8000]):x} {fnv(a.seg_bytes(TSEG)[:0x400]):x}'
        except (Fault, AssertionError, UcError) as e:
            if os.environ.get('WHY'):
                cs, ip = a.mu.reg_read(UC_X86_REG_CS) - LOAD_SEG, a.mu.reg_read(UC_X86_REG_IP)
                print(f'{seed:x} op {op}: {e!r} at {cs & 0xFFFF:04x}:{ip:04x}', file=sys.stderr)
            res = 'stuck'
        out.write(f'levelup | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
