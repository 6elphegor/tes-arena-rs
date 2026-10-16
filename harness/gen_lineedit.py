"""Emit test vectors for the typed field (src/lineedit.rs: `line_edit`,
0000:9EE9; `edit_cursor`, A0D9) and the key interrupt under it
(`key_event`, 0000:718A; `key_read`, 713C) from the original code.

Usage: gen_lineedit.py ACD_unpacked.EXE OUT   (SHARD=k/K: par.py)

op 0, the field: a font at 6800:0000 (B034), a text in DGROUP or at
6000h (DS:SI), the heap from A000h to F000h (0000:00CC), and the
registers drawn; 0000:9EE5 called far. Its pass's input, A172, is
stubbed to hand the case's keys in turn (a byte in AL, or CF for the
right button), Enter once they run out; the drawing and waits it calls
return at once (8F2A, 59A4, 9E0A, 946A, 6BE2). The heap's block runs
(9CD6, 6C36), as does the keys' flush (7112).

op 1, the cursor's pass: 0000:A0D9 called near on a field's state, the
rows' table at A6A0 as the game's (y * 140h), the screen at 7000h.

op 2, the keyboard: 1 to 8 bytes given to the key interrupt (0000:71BB,
AL and AH the byte, to 72B2: the port's in and the PIC's end left out),
then 0 to 4 reads (0000:713C).

  lineedit | op | seed | result

Results: "fault" / "hang", or FNV-1a digests: op 0 of the changed DGROUP
bytes and the text segment's, and of the heap's bytes; op 1 of the
changed DGROUP bytes and of the screen; op 2 of the changed DGROUP bytes
and of the bytes read.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from unicorn import UC_HOOK_CODE, UcError
from unicorn.x86_const import UC_X86_REG_AL, UC_X86_REG_EFLAGS
from emu import Arena, Fault, LOAD_SEG, DGROUP
from emu import changes, fnv, mine
from gen_attack import stream

TEXT_SEG = 0x6000
FONT_SEG = 0x6800
SCREEN = 0x7000
HEAP = (0xA000, 0xF000)
BUTTON = 0x100
KEYCODES = [0x01, 0x02, 0x0B, 0x0E, 0x10, 0x1C, 0x1D, 0x1E, 0x2A, 0x2C, 0x35, 0x36,
            0x38, 0x39, 0x47, 0x4B, 0x4D, 0x4F, 0x53, 0x58, 0x29, 0x0C, 0x34, 0x37]


def font(byte, op):
    """The font's bytes: its height, then 60h widths."""
    if op == 1:
        h = 2 + byte() % 12
        if byte() < 8:
            h = byte() % 2
        return bytes([h] + [1 + byte() % 10 for _ in range(0x60)])
    h = byte() % 14
    ws = []
    for _ in range(0x60):
        v = byte()
        ws.append(0 if v < 2 else 1 + v % 10)
    return bytes([h] + ws)


def text(byte, room):
    """(in DGROUP, offset, the text's bytes with its NUL and a tail)."""
    in_ds = byte() < 0x80
    off = 0x1044 + byte() % 0x40
    n = byte() % (room + 1)
    t = []
    for _ in range(n):
        c = byte()
        t.append(0x20 if c < 0x20 else 0x21 + c % 0x5E)
    zero = byte() < 0x80
    tail = [0 if zero else byte() for _ in range(0x40)]
    return in_ds, off, bytes(t + [0] + tail), n


def field(seed):
    """(DGROUP writes, (in DGROUP, offset, text), registers, keys).
    Mirrored in tests/lineedit.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    f = font(byte, 0)
    mx = byte() % 0x30
    if byte() < 0x20:
        mx = word()
    in_ds, off, t, n = text(byte, min(mx, 0x28))
    x = word() % 320
    y = byte() % 200
    cx = 0 if byte() < 0x80 else word() % 0x180
    if byte() < 0x10:
        cx = word()
        # A space measured first takes DGROUP's segment's high byte as
        # part of its width (A1B9): the port takes it as more than any
        # click below 2000h; the emulator's segment isn't the game's.
        if 0x2000 <= cx < 0x8000:
            cx &= 0x1FFF
    if in_ds:
        # A click past the text's end moves the whole segment (the cursor
        # counted past the end, A1B9), DGROUP's relocated words with it:
        # not comparable. In DGROUP the click is at the start.
        cx = 0
    w = 1 + byte() % 0x80
    h = 1 + byte() % 0x10
    if byte() < 0x10:
        w = word()
    writes = [(0xA902, pk(word())), (0xA84C, pk(word())), (0xACD1, b'\0')]
    if not in_ds:
        # The click is measured on DGROUP at the text's offset (A1B9).
        ct = []
        for _ in range(byte() % 0x20):
            c = byte()
            ct.append(0x20 if c < 0x40 else 0x21 + c % 0x5E)
        writes.append((off, bytes(ct + [0])))
    keys = []
    for _ in range(1 + byte() % 40):
        r = byte()
        if r < 0x60:
            k = 0x20 + byte() % 0x5F
        elif r < 0x78:
            k = 0x08
        elif r < 0x88:
            k = 0x7F
        elif r < 0x94:
            k = 0x0E
        elif r < 0xA0:
            k = 0x0F
        elif r < 0xB8:
            k = 0x01
        elif r < 0xD0:
            k = 0xFF
        elif r < 0xD8:
            k = 0x1B
        elif r < 0xE0:
            k = 0x0D
        elif r < 0xE4:
            k = BUTTON
        else:
            k = byte()
        keys.append(k)
    keys.append(0x0D)
    regs = dict(ax=x, bx=y, cx=cx, dx=mx, si=off, di=w, bp=h)
    return f, writes, (in_ds, off, t), regs, keys


def cursor(seed):
    """(font, DGROUP writes, (in DGROUP, offset, text)). Mirrored in
    tests/lineedit.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    f = font(byte, 1)
    in_ds, off, t, n = text(byte, 0x28)
    cur = byte() % (n + 3)
    blink = word() & ~7 if byte() < 0xA0 else word()
    x = word() % 320
    # Past the rows' table (200 rows) the row is DGROUP's next words, some
    # of them relocated segments: not comparable.
    y = byte() % 200
    colour = byte()
    writes = [(0xA83E, pk(cur)), (0xA84C, pk(blink)), (0xA836, pk(x)), (0xA838, pk(y)),
              (0x9288, bytes([colour])), (0xA83A, pk(off))]
    return f, writes, (in_ds, off, t)


def keyboard(seed):
    """(DGROUP writes, the bytes in, the reads). Mirrored in
    tests/lineedit.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    w = []
    for scan in [0x1D, 0x2A, 0x36, 0x38, 0x53, 0x4B, 0x4D, byte() & 0x7F, byte() & 0x7F]:
        w.append((0xACB5 + scan, b'\xff' if byte() < 0x40 else b'\0'))
    w.append((0xAC44, bytes([byte() % 4])))
    w.append((0xA902, pk(word())))
    w.append((0xACB2, pk(0xFFFF if byte() < 0x20 else 0)))
    w.append((0xAC46, pk(0xFFFF if byte() < 0x20 else 0)))
    w.append((0xACB4, bytes([byte()])))
    w.append((0xAC48, pk(0xAC4C + byte() % 0x65)))
    w.append((0xAC4A, pk(0xAC4C + byte() % 0x64)))
    w.append((0xACB0, pk(byte() % 0x65)))
    w.append((0xAC4C, bytes(byte() for _ in range(0x64))))
    codes = []
    for _ in range(1 + byte() % 8):
        r = byte()
        if r < 0x80:
            c = KEYCODES[byte() % len(KEYCODES)]
        elif r < 0xC0:
            c = KEYCODES[byte() % len(KEYCODES)] | 0x80
        elif r < 0xC8:
            c = 0xF0
        else:
            c = byte()
        codes.append(c)
    return w, codes, byte() % 5


def rows():
    return b''.join(struct.pack('<H', (y * 0x140) & 0xFFFF) for y in range(200))


def install(a):
    for off in (0x8F2A, 0x59A4, 0x9E0A, 0x946A, 0x6BE2, 0xA172):
        a.mu.mem_write(a.lin(LOAD_SEG, off), b'\xc3')
    state = {'keys': []}

    def key(mu, address, size, _):
        k = state['keys'].pop(0) if state['keys'] else 0x0D
        f = mu.reg_read(UC_X86_REG_EFLAGS)
        if k == BUTTON:
            mu.reg_write(UC_X86_REG_EFLAGS, f | 1)
        else:
            mu.reg_write(UC_X86_REG_EFLAGS, f & ~1)
            mu.reg_write(UC_X86_REG_AL, k)

    at = a.lin(LOAD_SEG, 0xA172)
    a.mu.hook_add(UC_HOOK_CODE, key, None, at, at)
    return state


def unreloc(a, after):
    """DGROUP's segment as the image has it where the code wrote it."""
    if struct.unpack_from('<H', after, 0xA83C)[0] == a.ds:
        struct.pack_into('<H', after, 0xA83C, DGROUP)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = Arena(EXE)
    state = install(a)
    a.w16(0x159, HEAP[0])
    a.w16(0x15D, HEAP[1])
    a.call_near(0, 0x00CC)
    base_ds = a.seg_bytes(a.ds)
    base_heap = bytes(a.mu.mem_read(HEAP[0] * 16, (HEAP[1] - HEAP[0]) * 16))
    zero = bytes(0x10000)
    rng = random.Random(0x1994_9EE9)
    total = int(os.environ.get('N', 6000))
    for n in range(total):
        op = n % 3
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.mu.mem_write(a.lin(a.ds, 0), base_ds)
        a.mu.mem_write(HEAP[0] * 16, base_heap)
        for seg in (TEXT_SEG, FONT_SEG, SCREEN):
            a.mu.mem_write(a.lin(seg, 0), zero)
        try:
            if op == 0:
                f, w, (in_ds, off, t), regs, keys = field(seed)
                a.mu.mem_write(a.lin(FONT_SEG, 0), f)
                a.w16(0xB034, 0)
                a.w16(0xB036, FONT_SEG)
                for o, d in w:
                    a.mu.mem_write(a.lin(a.ds, o), d)
                seg = a.ds if in_ds else TEXT_SEG
                a.mu.mem_write(a.lin(seg, off), t)
                state['keys'] = list(keys)
                before = a.seg_bytes(a.ds)
                before_t = a.seg_bytes(TEXT_SEG)
                a.call_far(0, 0x9EE5, ds=seg, **regs)
                after = bytearray(a.seg_bytes(a.ds))
                unreloc(a, after)
                heap = bytes(a.mu.mem_read(HEAP[0] * 16, (HEAP[1] - HEAP[0]) * 16))
                res = f'{fnv(changes(before, bytes(after)) + changes(before_t, a.seg_bytes(TEXT_SEG))):x} {fnv(heap):x}'
            elif op == 1:
                f, w, (in_ds, off, t) = cursor(seed)
                a.mu.mem_write(a.lin(FONT_SEG, 0), f)
                a.w16(0xB034, 0)
                a.w16(0xB036, FONT_SEG)
                a.mu.mem_write(a.lin(a.ds, 0xA6A0), rows())
                a.w16(0xA900, SCREEN)
                for o, d in w:
                    a.mu.mem_write(a.lin(a.ds, o), d)
                a.w16(0xA83C, a.ds if in_ds else TEXT_SEG)
                a.mu.mem_write(a.lin(a.ds if in_ds else TEXT_SEG, off), t)
                before = a.seg_bytes(a.ds)
                a.call_near(0, 0xA0D9)
                after = bytearray(a.seg_bytes(a.ds))
                res = f'{fnv(changes(before, bytes(after))):x} {fnv(a.seg_bytes(SCREEN)):x}'
            else:
                w, codes, reads = keyboard(seed)
                for o, d in w:
                    a.mu.mem_write(a.lin(a.ds, o), d)
                before = a.seg_bytes(a.ds)
                for c in codes:
                    a._setup([c | c << 8], {})
                    a._run(0, 0x71BB, LOAD_SEG, 0x72B2)
                got = bytes(a.call_near(0, 0x713C)['ax'] & 0xFF for _ in range(reads))
                res = f'{fnv(changes(before, a.seg_bytes(a.ds))):x} {fnv(got):x}'
        except (Fault, UcError):
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'lineedit | {op} | {seed:x} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
