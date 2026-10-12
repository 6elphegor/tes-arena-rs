"""Emit test vectors for src/heap.rs from the original code.

Usage: gen_heap.py ACD_unpacked.EXE OUT

Each case sets up a heap with startup's 0000:00CC (first header at
START:0, TOP as top of memory) and runs a random sequence of operations:
  a:P     first fit, P paragraphs (0000:51F2)
  b:N     first fit, N bytes (51EC)
  h:N     last fit, N bytes (9CD6)
  f:S     free the block at segment S (6C36)
  w:L:V   write word V at linear address L (to corrupt a header: a
          size or flags word, or a next pointer aliased to the same
          address with a non-zero offset)
Segments are absolute (the heap is outside the image in both the
emulator and the port), so the same numbers work in both.

Lines: heap | START TOP | ops | results | digest
Results are DX:AX per allocation and "-" per free or write; the sequence
stops at the first "fault" (int3 or CPU fault) or "hang" (instruction
budget). The digest is FNV-1a of the heap's memory and the free count.
"""
import random
import struct
import sys

sys.path.insert(0, __import__('os').path.dirname(__file__))
from emu import Arena, Fault

EXE, OUT = sys.argv[1:3]
rng = random.Random(0x1994_51F2)
out = open(OUT, 'w', buffering=1)
START, TOP = 0x7000, 0x9000


def fnv(b):
    h = 0xcbf29ce484222325
    for c in b:
        h = ((h ^ c) * 0x100000001b3) & 0xffffffffffffffff
    return h


a = Arena(EXE)
BASE = bytes(a.mu.mem_read(0, 0x110000))


def run(op):
    k = op[0]
    if k == 'a':
        r = a.call_near(0, 0x51F2, eax=op[1])
    elif k == 'b':
        r = a.call_near(0, 0x51EC, eax=op[1])
    elif k == 'h':
        r = a.call_near(0, 0x9CD6, eax=op[1])
    elif k == 'f':
        a.call_near(0, 0x6C36, eax=op[1])
        return '-'
    else:
        a.mu.mem_write(op[1], struct.pack('<H', op[2]))
        return '-'
    return f"{r['dx']:x}:{r['ax']:x}"


def fmt(op):
    return ':'.join(op[0:1] + [f'{x:x}' for x in op[1:]])


for case in range(600):
    a.mu.mem_write(0, BASE)
    top = rng.choice([TOP] * 6 + [START + rng.randint(4, 0x400), START + 3, START + 2])
    a.w16(0x159, START)
    a.w16(0x15D, top)
    a.call_near(0, 0x00CC)
    live, ops, res = [], [], []
    corrupt = rng.random() < 0.25
    for _ in range(rng.randint(5, 40)):
        t = rng.random()
        if t < 0.25:
            op = ['a', rng.choice([0, 1, 2] + [rng.randint(1, 0x40)] * 4 + [rng.randint(1, 0x400)] * 2
                                  + [rng.randint(0, 0xFFFF)])]
        elif t < 0.4:
            op = ['b', rng.choice([0, 1, 16, 17] + [rng.randint(0, 0x2000)] * 4 + [rng.randint(0xFFF0, 0xFFFF)])]
        elif t < 0.55:
            op = ['h', rng.choice([0, 1, 16] + [rng.randint(0, 0x2000)] * 4 + [rng.randint(0, 0xFFFF)])]
        elif t < 0.9 or not corrupt:
            choices = live + [0] if live else [0]
            s = rng.choice(choices)
            if s in live and rng.random() < 0.8:
                live.remove(s)
            op = ['f', s]
        else:
            # A size or flags word of some header (live or freed).
            s = rng.choice(live + [START + 1]) - 1
            nxt = a.r16(2, seg=s)
            if nxt and rng.random() < 0.4:
                # Alias the next header as (seg - 1):10h, the same
                # address, so the walk stays in the heap (memory outside
                # it differs between the emulator and the port) but sees
                # a non-zero offset. Two writes: offset, then segment.
                for op in (['w', s * 16, 0x10], ['w', s * 16 + 2, nxt - 1]):
                    ops.append(op)
                    res.append(run(op))
                continue
            field, val = rng.choice([(8, rng.choice([0, 0xA000, rng.randint(0, 0xFFFF)])),
                                     (0xA, rng.choice([0, 1]))])
            op = ['w', s * 16 + field, val]
        ops.append(op)
        try:
            r = run(op)
        except Fault:
            res.append('fault')
            break
        except AssertionError:
            res.append('hang')
            break
        res.append(r)
        if op[0] in 'abh' and r != '0:0':
            live.append(int(r.split(':')[0], 16))
    mem = bytes(a.mu.mem_read(START * 16, (TOP - START) * 16)) + struct.pack('<H', a.r16(0x8F6E))
    out.write(f"heap | {START:x} {top:x} | {' '.join(fmt(o) for o in ops)} | {' '.join(res)} | {fnv(mem):x}\n")
