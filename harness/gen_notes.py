"""Emit test vectors for the automap's notes (src/mapnotes.rs) from the
original code.

Usage: gen_notes.py ACD_unpacked.EXE OUT

A case draws from a seed (`setup`, mirrored in tests/notes.rs) one op and
the state it reads, with AUTOMAP.nn in memory (gen_automap's `MemDos`):

  op 0  0000:31FE, a click on the automap screen (`note_click`)
  op 1  0000:32FE, the notes drawn (`notes_draw`)
  op 2  23F1:00A3, a note added (`note_add`; AX, BX its place, SI F000h
        its text)

The record (POSITION 66C6, one of 16) has 40h notes: free (x 0), a map
square near the window (A880, A882), or a screen place (x bit 15), each
with letters and a NUL then bytes at random; some texts and places are
the typed one's (a duplicate). The file is sometimes cut short inside
the notes. The click (8F92, 8F94) is anywhere, inside the map, or on a
note's screen point; the font's widths (far pointer B034) are bytes at
random in a segment of their own, mostly 3..8; NOTE_FLAGS (A888) and
NOTE_POS (A892) at random or a note's position.

Stubbed, recording in DGROUP: 9EE9 (the typing: AX, BX, CX, DX to
F0F0..F0F7, then the text at F000 copied to 1044), 9E0A (a note drawn:
AX, BX to the log at F100, its count at F0FE), 2F10 and 59A4 (drawing).

  notes | seed | op | result

Results: "fault" / "hang", or the FNV-1a digests of the changed DGROUP
bytes (A90A unrelocated) and of AUTOMAP.nn.
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Fault, LOAD_SEG
from emu import changes, fnv, mine
from gen_attack import stream
from gen_automap import MemDos, digits

FONT_SEG = 0x7000

TYPE_STUB = bytes.fromhex(
    '8906F0F0' '891EF2F0' '890EF4F0' '8916F6F0'  # the call's AX, BX, CX, DX
    '5657' '51' '06' '1E07'                     # push si, di, cx, es; es = ds
    'BE00F0' 'BF4410' 'B93C00' 'F3A4'           # 3Ch bytes F000 -> 1044
    '07' '59' '5F' '5E' 'C3')
DRAW_STUB = bytes.fromhex(
    '57' '8B3EFEF0' '8985' '00F1' '899D' '02F1' '8306FEF0' '04' '5F' 'C3')


def letters(byte, n):
    """n characters: capitals mostly, a lower-case letter, a space or a
    byte from 80h now and then."""
    out = []
    for _ in range(n):
        k = byte() % 8
        if k == 0:
            out.append(0x20)
        elif k == 1:
            out.append(0x80 | byte())
        elif k == 2:
            out.append(0x61 + byte() % 26)
        else:
            out.append(0x41 + byte() % 26)
    return bytes(out)


def setup(seed):
    """(op, ax, bx, writes, font, name, file): writes are (offset, bytes)
    in DGROUP. Mirrored in tests/notes.rs."""
    s = stream(seed)
    byte = lambda: next(s)
    word = lambda: byte() | byte() << 8
    pk = lambda v: struct.pack('<H', v & 0xFFFF)
    op = byte() % 3
    w = []
    slot = 1 + byte() % 19
    w.append((0x6643, digits(slot)))
    rec = byte() % 16
    pos = rec * 0x2004
    w.append((0x66C6, struct.pack('<I', pos)))
    left, top = byte() % 0x40, byte() % 0x40
    w.append((0xA880, pk(left)))
    w.append((0xA882, pk(top)))
    typed = letters(byte, byte() % 12) + b'\0'
    typed = typed + bytes(byte() for _ in range(0x3C - len(typed)))
    if byte() < 0x30:
        typed = b'\0' + typed[1:]
    w.append((0xF000, typed))
    # The notes.
    f = bytearray(0x20040)
    for i in range(16):
        f[i * 0x2004:i * 0x2004 + 4] = struct.pack('<I', word() | word() << 16)
    notes = []
    for i in range(0x40):
        k = byte() % 4
        if k == 0:
            x, y = 0, word() if byte() < 0x40 else 0
        elif k == 1:
            x, y = (left + byte() % 0x48) & 0x7FFF, top + byte() % 0x30
        elif k == 2:
            x, y = 0x8000 | (0xCE + byte() % 0x4A), 0x23 + byte() % 0x90
        else:
            x, y = word(), word()
        t = letters(byte, byte() % 14) + b'\0'
        if byte() < 0x20:
            t = typed[:typed.index(b'\0') + 1]
            if byte() < 0x80:
                # The same text in the other case.
                t = bytes(c ^ 0x20 if 0x41 <= (c & 0xDF) <= 0x5A else c for c in t)
            if byte() < 0x80:
                x, y = None, None
        t = t + bytes(byte() for _ in range(0x3C - len(t)))
        notes.append((x, y, t[:0x3C]))
    # A duplicate's place is the one being added (op 2) or clicked.
    ax = word() % 0x140
    bx = word() % 0xC8
    k = byte() % 4
    if k == 1:
        ax, bx = 0x19 + byte() % 0xFF, 0x23 + byte() % 0x95
    elif k >= 2:
        j = byte() % 0x40
        x, y = notes[j][0], notes[j][1]
        if x is None or x == 0:
            ax, bx = 0x19 + byte() % 0xFF, 0x23 + byte() % 0x95
        elif x & 0x8000:
            ax, bx = (x & 0x7FFF) - byte() % 8, y
        else:
            ax = (0xCD - 3 * (x - left) - byte() % 6) & 0xFFFF
            bx = (0x28 + 3 * (y - top) + byte() % 9) & 0xFFFF
    add_x, add_y = (ax, bx) if op != 2 else ((word() & 0x7FFF) if byte() < 0x80 else 0x8000 | word(), word())
    for i, (x, y, t) in enumerate(notes):
        if x is None:
            x, y = add_x, add_y
        at = pos + 4 + i * 0x40
        f[at:at + 0x40] = struct.pack('<HH', x & 0xFFFF, y & 0xFFFF) + t
    if byte() < 0x30:
        f = f[:pos + 4 + byte() * 0x10 + byte() % 0x10]
    w.append((0x8F92, pk(ax)))
    w.append((0x8F94, pk(bx)))
    w.append((0xA888, bytes([byte()])))
    npos = pos + 4 + (byte() % 0x40) * 0x40 if byte() < 0x80 else word() | word() << 16
    w.append((0xA892, struct.pack('<I', npos)))
    fo = byte()
    # A fifth of the fonts all wide (50h..7Fh): long texts then reach the
    # edge of 343C's divide (a quotient past FFh faults).
    wide = byte() < 0x33
    font = []
    for _ in range(0x80):
        k = byte()
        if wide:
            font.append(0x50 + byte() % 0x30)
        else:
            font.append(3 + byte() % 6 if k < 0xC0 else 0x50 + byte() % 0x30 if k < 0xE8 else byte())
    font = bytes(font)
    w.append((0xB034, pk(fo) + pk(FONT_SEG)))
    w.append((0x9288, bytes([byte()])))
    w.append((0xF0FE, pk(0)))
    w.append((0x1044, bytes(byte() for _ in range(0x60))))
    return op, add_x, add_y, w, (fo, font), 'AUTOMAP.' + digits(slot).decode(), bytes(f)


def main():
    EXE, OUT = sys.argv[1:3]
    out = open(OUT, 'w', buffering=1)
    a = MemDos(EXE, os.path.dirname(EXE))
    for off, code in ((0x9EE9, TYPE_STUB), (0x9E0A, DRAW_STUB), (0x2F10, b'\xC3'), (0x59A4, b'\xC3')):
        a.mu.mem_write(a.lin(LOAD_SEG, off), code)
    base = a.snapshot()
    rng = random.Random(0x1994_31FE)
    total = int(os.environ.get('N', 3000))
    for n in range(total):
        seed = rng.getrandbits(32)
        if not mine(n, total):
            continue
        a.restore(base)
        a.mh = {}
        op, ax, bx, writes, (fo, font), name, data = setup(seed)
        for off, d in writes:
            a.mu.mem_write(a.lin(a.ds, off), d)
        a.mu.mem_write(a.lin(FONT_SEG, fo), font)
        a.files = {name: bytearray(data)}
        before = a.seg_bytes(a.ds)
        try:
            if op == 0:
                a.call_near(0, 0x31FE)
            elif op == 1:
                a.call_near(0, 0x32FE)
            else:
                a.call_far(0x23F1, 0x00A3, ax=ax, bx=bx, si=0xF000)
            if a.r16(0xA90A) == a.ds:
                a.w16(0xA90A, a.ds - LOAD_SEG)
            after = a.seg_bytes(a.ds)
            res = f'{fnv(changes(before, after)):x} {fnv(bytes(a.files[name])):x}'
        except Fault:
            res = 'fault'
        except AssertionError:
            res = 'hang'
        out.write(f'notes | {seed:x} | {op} | {res}\n')
    out.close()


if __name__ == '__main__':
    main()
