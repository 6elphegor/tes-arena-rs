"""Emit test vectors for a riddle's answer (292B:05A2, src/riddle.rs
`Machine::riddle_answered`) from the original code.

Usage: gen_riddle.py ACD_unpacked.EXE OUT   (SHARD=k/K: par.py)

A case fills from a seed (`setup`, mirrored in tests/riddle.rs): the
riddle's answers in the EMS frame's segment ([9030] FRAME, EMS off: 0000:
5C86 maps nothing) from [7C62], each after a `:`, to a CR (the last to a
CR and a backquote, or a NUL); the answer typed (one of them in another
case, or another text, or nothing), which the field (0000:9EE5, a stub)
leaves at 1044; the texts and triggers' numbers for a right and a wrong
answer (7C64..7C6A); the level's triggers (C6E4 of them at C624: squares,
numbers), the walls under them (segment [D806] WALLS), the walls' sounds
(C3C0); the square's entry (3D44). The sound (23BA:002A) is a stub that
logs AX and CX.

  riddle | seed | result

Results: "stuck" (a fault or a hang), or the FNV-1a digests of the
changed DGROUP bytes (the stub's log at FF00.. left out), of the changed
walls' segment, of the sounds' numbers (words), then the last sound's CX
(hex; 0 with none).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UcError, UC_HOOK_CODE
from emu import Arena, Fault, LOAD_SEG, changes, fnv, mine
from gen_attack import stream

FRAME = 0x6C00
WALLS = 0x7C00
LETTERS = b'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ'
# The sound's stub: push bx; mov bx, [FFFE]; mov [bx+FF00], ax;
# mov [bx+FF02], cx; add word [FFFE], 4; pop bx; retf.
SOUND_STUB = bytes.fromhex('53 8b1efeff 898700ff 898f02ff 8306feff04 5b cb')


def setup(seed):
    """(DGROUP writes, FRAME writes, WALLS writes, the answer typed).
    Mirrored in tests/riddle.rs, draw for draw."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    ds, frame, walls = [], [], []
    ds.append((0x8FA6, b'\0'))
    ds.append((0x9030, pk(FRAME)))
    ds.append((0xD806, pk(WALLS)))
    # The answers: 1..4, each 1..8 letters.
    answers = []
    for _ in range(1 + byte() % 4):
        answers.append(bytes(LETTERS[byte() % len(LETTERS)] for _ in range(1 + byte() % 8)))
    text = b''.join(b':' + a + b'\r' for a in answers)
    text += b'`' if byte() < 0xC0 else b''
    if text.endswith(b'`') and byte() < 0x80:
        # Past the end, what reads as another answer: the first's.
        text += b':' + answers[0]
    if byte() < 0x40:
        # The last answer ended by the NUL instead of its CR.
        text = text[:text.rindex(b'\r')]
    at = 0x100 + byte()
    frame.append((at - 1, b'?'))
    frame.append((at, text + b'\0'))
    ds.append((0x7C62, pk(at)))
    pick = byte()
    if pick < 0x80:
        a = answers[byte() % len(answers)]
        typed = bytes(c ^ 0x20 if byte() < 0x40 else c for c in a)
    elif pick < 0xA0:
        typed = b''
    elif pick < 0xC0:
        a = answers[byte() % len(answers)]
        typed = a[:max(len(a) - 1, 0)]
    else:
        typed = bytes(LETTERS[byte() % len(LETTERS)] for _ in range(1 + byte() % 8))
    for off in (0x7C64, 0x7C66):
        ds.append((off, pk(word())))
    for off in (0x7C68, 0x7C6A):
        ds.append((off, bytes([byte() % 4])))
    n = byte() % 6
    ds.append((0xC6E4, bytes([n])))
    for i in range(n):
        sq = (byte() % 64) << 8 | byte()
        ds.append((0xC624 + 3 * i, pk(sq) + bytes([byte() % 4])))
        off = (sq & 0xFF00) | ((sq & 0xFF) * 2 & 0xFF)
        walls.append((off, pk(word())))
    ds.append((0xC3C0, bytes(2 + byte() % 0x3E for _ in range(4))))
    ds.append((0x3D44, pk(0xF000 + byte() * 2)))
    return ds, frame, walls, typed


def main():
    EXE, OUT = sys.argv[1:3]
    a = Arena(EXE)
    mu = a.mu
    # The field (0000:9EE5): a RETF, the answer put at 1044 as it's reached.
    mu.mem_write(a.lin(LOAD_SEG, 0x9EE5), b'\xcb')
    typed = [b'']

    def field(mu, addr, size, _):
        mu.mem_write(a.lin(a.ds, 0x1044), typed[0] + b'\0')
    lin = a.lin(LOAD_SEG, 0x9EE5)
    mu.hook_add(UC_HOOK_CODE, field, None, lin, lin)
    mu.mem_write(a.lin(LOAD_SEG + 0x23BA, 0x002A), SOUND_STUB)
    ds0 = a.seg_bytes(a.ds)
    out = open(OUT, 'w', buffering=1)
    rng = random.Random(0x1994_05A2)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        mu.mem_write(a.lin(a.ds, 0), ds0)
        mu.mem_write(a.lin(FRAME, 0), bytes(0x10000))
        mu.mem_write(a.lin(WALLS, 0), bytes(0x10000))
        dsw, fw, ww, typed[0] = setup(seed)
        for off, data in dsw:
            mu.mem_write(a.lin(a.ds, off), data)
        for off, data in fw:
            mu.mem_write(a.lin(FRAME, off), data)
        for off, data in ww:
            mu.mem_write(a.lin(WALLS, off), data)
        a.w16(0xFFFE, 0)
        before = a.seg_bytes(a.ds)
        wbefore = a.seg_bytes(WALLS)
        try:
            a.call_far(0x292B, 0x05A2)
            after = a.seg_bytes(a.ds)
            k = a.r16(0xFFFE)
            log = after[0xFF00:0xFF00 + k]
            numbers = b''.join(log[i:i + 2] for i in range(0, k, 4))
            last = struct.unpack_from('<H', log, k - 2)[0] if k else 0
            c = changes(before[:0xFF00], after[:0xFF00])
            res = f'{fnv(c):x} {fnv(changes(wbefore, a.seg_bytes(WALLS))):x} {fnv(numbers):x} {last:x}'
            if os.environ.get('DUMP') == f'{seed:x}':
                for i in range(0, len(c), 3):
                    o, v = struct.unpack_from('<HB', c, i)
                    print(f'{o:04x} {before[o]:02x} {v:02x}', file=sys.stderr)
                print('typed', typed[0], 'log', log.hex(), file=sys.stderr)
        except (Fault, AssertionError, UcError):
            res = 'stuck'
        out.write(f'riddle | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
