"""Emit test vectors for src/random.rs from the original code."""
import random, sys, os
sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena

a = Arena(sys.argv[1])
rng = random.Random(1994)
out = open(sys.argv[2], 'w')
TABLE_SEG = 0x7000

# Plain LCG path (table disabled), range() via far entry 0000:6EEA.
for _ in range(2000):
    seed = rng.getrandbits(32)
    lo = rng.getrandbits(16)
    hi = (lo + rng.randint(1, 0xffff)) & 0xffff
    a.w16(0x91F0, 0); a.w32(0x91F6, seed)
    ax = a.call_far(0, 0x6EEA, ax=lo, bx=hi)
    out.write(f'range {seed} {lo} {hi} {ax} {a.r32(0x91F6)}\n')

# Table path via far entry 0000:6EA0: run sequences across several refills.
for _ in range(50):
    seed = rng.getrandbits(32)
    refill = rng.choice([2, 4, 6, 16, 64, 100])
    nwords = 64
    words = [rng.getrandbits(16) for _ in range(nwords)]
    start = rng.randint(1, 10)
    for i, w in enumerate(words):
        a.w16(i * 2, w, seg=TABLE_SEG)
    a.w32(0x91F6, seed); a.w16(0x91EE, 0); a.w16(0x91F0, TABLE_SEG)
    a.w16(0x91F2, start); a.w16(0x91F4, refill)
    vals = [a.call_far(0, 0x6EA0) for _ in range(120)]
    out.write(f'table {seed} {refill} {start} {" ".join(map(str, words))} | {" ".join(map(str, vals))} | {a.r32(0x91F6)}\n')
