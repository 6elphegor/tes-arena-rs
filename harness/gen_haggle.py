"""Emit test vectors for src/haggle.rs (1194:8020, buying, selling and
the main quest's informant) from the original code.

Usage: gen_haggle.py ACD_unpacked.EXE GAME_DIR OUT

The player is scripted: the input routine (1194:4E88) is replaced by an
interrupt served from three queues by the kind it is asked for (DGROUP
13D3: 0 a keypress, 1 a menu choice, 2 a typed offer). The text and
screen routines it calls (1194:4E54, 4B8E, 49FF, 4D31, 5FC0, and
0000:9AF8, 9AE6) are stubbed; the dialogue's line choice (1194:83FD)
runs, as it draws a random number. The informant's lines (modes 5, 6)
have their codes substituted by 2DA2:000A: stubbed, counting its calls
in DGROUP FFE0 (src/haggle.rs `HaggleIo::expand`; the text port is
verified on its own).

Lines: haggle | price per quality mood mode dl seed gold fe6 fdd | menus |
       offers | result: "deal EAX" or "none", the inputs used (menus offers
       keys), FNV-1a of DGROUP 1044..1047, 1391..1394, 13CA..13CB,
       6598..65BB (but 65A8, the saved stack pointer), the RNG seed, the
       gold (05C2), 0FE6, 0FEE..0FEF, 9288 and FFE0..FFE1; or
       "fault"/"hang" (the script ran out).
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault
from unicorn.x86_const import UC_X86_REG_EFLAGS

EXE, GAME, OUT = sys.argv[1:4]
rng = random.Random(0x1994_8020)
out = open(OUT, 'w', buffering=1)


from emu import fnv  # noqa: E402 (C FNV-1a, emu.py)


class Script(Exception):
    pass


class HaggleArena(DosArena):
    def _intr(self, mu, intno, x):
        if intno == 0xF5:
            kind = mu.mem_read(self.lin(self.ds, 0x13D3), 1)[0]
            if kind == 2:
                if not self.offers:
                    self.fault = 'script'
                    mu.emu_stop()
                    return
                text = self.offers.pop(0)
                self.used[1] += 1
                mu.mem_write(self.lin(self._reg('es'), self._reg('di')), text + b'\0')
                self._wreg('ax', len(text))
            elif kind == 1:
                if not self.menus:
                    self.fault = 'script'
                    mu.emu_stop()
                    return
                self.used[0] += 1
                self._wreg('ax', self.menus.pop(0))
            else:
                self.used[2] += 1
                self._wreg('ax', 0)
            return
        super()._intr(mu, intno, x)


a = HaggleArena(EXE, GAME)
a.patch(0x1194, 0x4E88, b'\xcd\xf5\xc3')
for off in (0x4E54, 0x4B8E, 0x49FF, 0x4D31, 0x5FC0):
    a.patch(0x1194, off, b'\xc3')
for off in (0x9AF8, 0x9AE6):
    a.patch(0, off, b'\xcb')
# 2DA2:000A (far): INC WORD [FFE0]; RETF.
a.patch(0x2DA2, 0x000A, b'\xff\x06\xe0\xff\xcb')
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA
a.w16(0x9030, 0x7000)
BASE = a.snapshot()

for case in range(int(os.environ.get('N', 2500))):
    a.restore(BASE)
    mode = rng.choice([1, 2, 4, 5, 6])
    price = rng.choice([rng.randint(1, 500), rng.randint(1, 5000), rng.randint(1, 70000), 1, 2, 64001, 65000, 65001,
                        rng.getrandbits(32)])
    per = rng.randint(0, 100)
    quality = rng.getrandbits(8)
    mood = rng.getrandbits(8)
    seed = rng.getrandbits(32) | 1
    dl = rng.getrandbits(8)
    # A fifth of the gold exactly the asking price (halved over 65000),
    # where the informant's payment is at its edge.
    asking = price >> 1 if price > 65000 else price
    gold = rng.choice([0, rng.randint(0, 700), rng.randint(0, 80000), rng.getrandbits(32), asking])
    fe6 = rng.getrandbits(8)
    fdd = rng.randint(0, 15)
    menus = [rng.choice([1, 1, 1, 0, 2, 3]) for _ in range(12)]
    base = price >> 1 if price > 64000 else price
    offers = []
    for _ in range(12):
        k = rng.random()
        if k < 0.05:
            offers.append(b'')
        elif k < 0.1:
            offers.append(b'1x')
        else:
            v = rng.choice([int(base * rng.uniform(0.3, 1.5)), int(base * rng.uniform(0.7, 1.05)),
                            int(base * rng.uniform(0.95, 1.3)), rng.randint(0, 99999), base])
            offers.append(str(max(0, v) % 100000).encode())
    a.menus, a.offers, a.used = list(menus), list(offers), [0, 0, 0]
    a.w32(0x91F6, seed)
    a.w16(0x91F0, 0)
    a.w32(0x05C2, gold)
    a.mu.mem_write(a.lin(a.ds, 0x0FE6), bytes([fe6]))
    a.mu.mem_write(a.lin(a.ds, 0x0FDD), bytes([fdd]))
    try:
        a._setup([0xFFF0], {'eax': price, 'esi': per, 'ebx': quality, 'ecx': mood,
                            'edx': mode << 8 | dl, 'edi': 0x1A4C})
        a._run(0x1194, 0x8020, 0x1000 + 0x1194, 0xFFF0)
        cf = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
        eax = a.mu.reg_read(__import__('unicorn').x86_const.UC_X86_REG_EAX)
        d = a.seg_bytes(a.ds)
        # 65A8 holds the routine's saved stack pointer: left out.
        state = (d[0x1044:0x1048] + d[0x1391:0x1395] + d[0x13CA:0x13CC] + d[0x6598:0x65A8] + d[0x65AA:0x65BC]
                 + d[0x91F6:0x91FA] + d[0x05C2:0x05C6] + d[0x0FE6:0x0FE7] + d[0x0FEE:0x0FF0] + d[0x9288:0x9289]
                 + d[0xFFE0:0xFFE2])
        result = 'none' if cf else f'deal {eax:x}'
        res = f'{result} {" ".join(map(str, a.used))} {fnv(state):x}'
    except Fault as e:
        res = 'hang' if a.fault == 'script' else 'fault'
    except AssertionError:
        res = 'hang'
    out.write(f'haggle | {price:x} {per:x} {quality:x} {mood:x} {mode:x} {dl:x} {seed:x} {gold:x} {fe6:x} {fdd:x} | '
              f'{" ".join(map(str, menus))} | {",".join(o.decode() for o in offers)} | {res}\n')
out.close()
