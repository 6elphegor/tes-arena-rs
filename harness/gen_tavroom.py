"""Emit test vectors for a tavern's room haggled over (src/tavern.rs
`tavern_room_haggle`, 1194:C14E: the innkeeper's names, the room's item,
then 1194:8020 in mode 3) from the original code.

Usage: gen_tavroom.py ACD_unpacked.EXE GAME_DIR OUT

The player is scripted as in gen_haggle.py (the input routine 1194:4E88
an interrupt served from queues by the kind it is asked for, 13D3; the
screen routines stubbed). A case fills from the case's own random draws:
the place key 0FD2 and a table of 1..6 records (4 bytes, the key word
first) at PSEG:0 pointed at by 1AB5 (count 1387), the name set A46 (0..23),
the room's kind 13C8 (0..4), quality and mood 13CA..13CB, the personality
1D3, the price 1391 and the RNG seed.

  tavroom | key count records(hex) set room quality mood per price seed |
            menus | offers | result

Result: "deal EAX"/"none", the inputs used, then FNV-1a of 7311.. and
1A4C.. (20h each), 1376..1377, 98DE..98E1 (the names' set, sex and
handle), the haggle's state as gen_haggle.py digests it but 6598 (DL,
left from the names' DX and not modelled), and the lines' buffers
(929E.., 987A.., 200h each), 137E and 13D3; or "fault"/"hang".
"""
import os
import random
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, fnv, mine
from unicorn.x86_const import UC_X86_REG_EFLAGS, UC_X86_REG_EAX

EXE, GAME, OUT = sys.argv[1:4]
out = open(OUT, 'w', buffering=1)
PSEG = 0x6000


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
a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA (DOS handle 5)
a.w16(0x9030, 0x7000)
BASE = a.snapshot()


def case_inputs(rng):
    """The case's inputs, all drawn from `rng`."""
    count = rng.randint(1, 6)
    records = [rng.getrandbits(32) for _ in range(count)]
    key = rng.getrandbits(16)
    if rng.random() < 0.8:
        # The key found among the records (as a town's is).
        i = rng.randrange(count)
        k = records[i] & 0xFFFF
        key = (k & 0xFF00) | ((k & 0x7F) << 1) | rng.getrandbits(1)
        records[i] = (records[i] & 0xFFFF0000) | (key & 0xFF00) | ((key & 0xFF) >> 1)
    name_set = rng.randint(0, 23)
    room = rng.randint(0, 4)
    quality = rng.getrandbits(8)
    mood = rng.getrandbits(8)
    per = rng.getrandbits(8)
    price = rng.choice([rng.randint(1, 60), rng.randint(1, 600), rng.randint(1, 70000), 1, 2])
    seed = rng.getrandbits(32) | 1
    menus = [rng.choice([1, 1, 1, 0, 2, 3]) for _ in range(12)]
    offers = []
    for _ in range(12):
        k = rng.random()
        if k < 0.05:
            offers.append(b'')
        elif k < 0.1:
            offers.append(b'1x')
        else:
            v = rng.choice([int(price * rng.uniform(0.3, 1.5)), int(price * rng.uniform(0.7, 1.05)), price])
            offers.append(str(max(0, v) % 100000).encode())
    return key, records, name_set, room, quality, mood, per, price, seed, menus, offers


rng = random.Random(0x1994_C14E)
total = int(os.environ.get('N', 3000))
for n in range(total):
    inputs = case_inputs(random.Random(rng.getrandbits(64)))
    if not mine(n, total):
        continue
    key, records, name_set, room, quality, mood, per, price, seed, menus, offers = inputs
    a.restore(BASE)
    a.menus, a.offers, a.used = list(menus), list(offers), [0, 0, 0]
    a.mu.mem_write(a.lin(PSEG, 0), b''.join(struct.pack('<I', r) for r in records) + bytes(8))
    a.w16(0x0FD2, key)
    a.w16(0x1387, len(records))
    a.w16(0x1AB5, 0)
    a.w16(0x1AB7, PSEG)
    a.mu.mem_write(a.lin(a.ds, 0x0A46), bytes([name_set]))
    a.mu.mem_write(a.lin(a.ds, 0x13C8), bytes([room, 0, quality, mood]))
    a.mu.mem_write(a.lin(a.ds, 0x01D3), bytes([per]))
    a.w32(0x1391, price)
    a.w32(0x91F6, seed)
    a.w16(0x91F0, 0)
    try:
        a._setup([0xFFF0], {'es': a.ds})
        a._run(0x1194, 0xC14E, 0x1000 + 0x1194, 0xFFF0)
        cf = a.mu.reg_read(UC_X86_REG_EFLAGS) & 1
        eax = a.mu.reg_read(UC_X86_REG_EAX)
        d = a.seg_bytes(a.ds)
        names = d[0x7311:0x7331] + d[0x1A4C:0x1A6C] + d[0x1376:0x1378] + d[0x98DE:0x98E2]
        state = (d[0x1044:0x1048] + d[0x1391:0x1395] + d[0x13CA:0x13CC] + d[0x6599:0x65A8] + d[0x65AA:0x65BC]
                 + d[0x91F6:0x91FA] + d[0x05C2:0x05C6] + d[0x0FE6:0x0FE7] + d[0x0FEE:0x0FF0] + d[0x9288:0x9289])
        shown = d[0x929E:0x949E] + d[0x987A:0x9A7A] + d[0x137E:0x137F] + d[0x13D3:0x13D4]
        result = 'none' if cf else f'deal {eax:x}'
        res = f'{result} {" ".join(map(str, a.used))} {fnv(names):x} {fnv(state):x} {fnv(shown):x}'
    except Fault:
        res = 'hang' if a.fault == 'script' else 'fault'
    except AssertionError:
        res = 'hang'
    recs = ','.join(f'{r:x}' for r in records)
    out.write(f'tavroom | {key:x} {len(records)} {recs} {name_set:x} {room:x} {quality:x} {mood:x} {per:x} {price:x} {seed:x} | '
              f'{" ".join(map(str, menus))} | {",".join(o.decode() for o in offers)} | {res}\n')
out.close()
