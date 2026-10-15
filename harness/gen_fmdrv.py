"""Emit test vectors for view/src/fmdrv.rs: the OPL register writes the
game's own Sound Blaster FM driver makes while it plays a song.

Usage: gen_fmdrv.py ACD_unpacked.EXE GAME_DIR [DUMP_DIR] OUT

Runs under the DOS file layer (harness/dos.py) with the user's game
directory. As the game does, the wrapper 2F9F:01BF loads SBP1FM.ADV (a
Miles AIL 2 XMIDI driver) through the AIL API in segment 0FFD, and
2F9F:001F starts a song: it registers the XMIDI sequence, installs the
timbres the driver asks for from SAMPLE.AD, sets controller 7 of
channels 1..16 to 127, starts the sequence and sets its relative volume
to the music volume (D3A, a case setting here). The timer is then
stepped as the hardware would: the RTC interrupt service 0FFD:058C runs
once per 1/256 s, and calls the driver's serve (120 Hz) when its
period is due.

A case may change songs as the game does (2F9F:0438, then 001F): at
service FADE the volume ramps to 0 over 800 ms (AIL 0FFD:0DE8), at STOP
the sequence is stopped (0DCA) and released (0D94) and the next song
started; the driver (its timbre cache, voices and channels) carries on.
Without FADE ("-") the next song is started at STOP with the first
still playing (001F alone, as on loading into an interior): the driver
then plays both.

Every write to the Sound Blaster Pro's FM ports is recorded with the
number of driver services so far (0 for the writes made while the
song starts, then 1, 2, ...; a song started at STOP's are STOP's):
220h/221h is the left OPL2 (chip 0), 222h/223h the right (chip 1).
The driver writes neither 228h/229h (both) nor 388h/389h after its
detection; such writes would be chips 2 and 3, which the port never
makes, so the counts would differ.
A record is service (u32 LE), chip, register, value; the vectors hold
only digests (FNV-1a) of the stream, one per window of WINDOW services,
so the test can name the first window that differs.

Lines: song volume services window count then | digest digest ...
with `then` "-" or NEXT@FADE@STOP (FADE "-" for none).
DUMP_DIR, if given, receives each case's whole stream as text
(service chip reg value, hex) for debugging; never commit those.
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import LOAD_SEG, STUB_SEG, fnv, mine
from unicorn import UC_HOOK_CODE, UC_HOOK_INSN
from unicorn.x86_const import UC_X86_INS_IN, UC_X86_INS_OUT

EXE, GAME, OUT = sys.argv[1], sys.argv[2], sys.argv[-1]
DUMP = sys.argv[3] if len(sys.argv) > 4 else None
DRIVER = 'SBP1FM.ADV'
WINDOW = 100
SONGS = [
    'ARABCITY', 'ARABTOWN', 'ARAB_VLG', 'CITY', 'COMBAT', 'CREDITS', 'DUNGEON1', 'DUNGEON2', 'DUNGEON3', 'DUNGEON4', 'DUNGEON5', 'EQUIPMNT',
    'EVIL', 'EVLINTRO', 'MAGIC_2', 'NIGHT', 'OVERCAST', 'OVERSNOW', 'PALACE', 'PERCNTRO', 'RAINING', 'SHEET', 'SNEAKING', 'SNOWING', 'SQUARE',
    'SUNNYDAY', 'SWIMMING', 'TAVERN', 'TEMPLE', 'TOWN', 'VILLAGE', 'VISION', 'WINGAME',
]
# (song, volume, services, then): every song's first 10 s; whole songs,
# their ends and loops; quieter volumes; song changes.
CASES = [(f'{s}.XFM', 100, 1200, None) for s in SONGS] + [
    ('CITY.XFM', 37, 1200, None), ('TEMPLE.XFM', 64, 1200, None), ('TAVERN.XFM', 0, 1200, None),
    ('CITY.XFM', 100, 3000, None), ('TOWN.XFM', 100, 3000, None), ('SUNNYDAY.XFM', 100, 12000, None),
    ('DUNGEON2.XFM', 90, 12000, None), ('CREDITS.XFM', 100, 12000, None), ('EVIL.XFM', 100, 12000, None),
    ('CITY.XFM', 100, 2400, ('NIGHT.XFM', 300, 400)), ('COMBAT.XFM', 80, 2400, ('DUNGEON3.XFM', 900, 1000)),
    ('TAVERN.XFM', 100, 3000, ('EQUIPMNT.XFM', 1100, 1140)), ('SNEAKING.XFM', 127, 2400, ('SNEAKING.XFM', 500, 600)),
    ('PALACE.XFM', 100, 2400, ('TEMPLE.XFM', None, 300)), ('CITY.XFM', 100, 1800, ('CITY.XFM', None, 200)),
]
CHIP = {0x220: 0, 0x221: 0, 0x222: 1, 0x223: 1, 0x228: 2, 0x229: 2, 0x388: 3, 0x389: 3}
NAME = 0x8000  # a free paragraph for the far file names
SERVE = (LOAD_SEG + 0x0FFD) * 16 + 0x616  # the ISR's call of a due client


class Card:
    """The Sound Blaster Pro as the driver sees it: two OPL2s whose timer
    status the detection reads, and the mixer, a register store (the
    driver writes and reads back register 0Ah to find the card)."""

    def __init__(self):
        self.status = 0
        self.mixer = {}
        self.sel = {}
        self.writes = []
        self.service = 0

    def port_in(self, uc, port, size, ud):
        if port in (0x220, 0x222, 0x228, 0x388):
            return self.status
        if port == 0x225:
            return self.mixer.get(self.mixer.get('sel'), 0)
        return 0xFF

    def port_out(self, uc, port, size, value, ud):
        value &= 0xFF
        if port == 0x224:
            self.mixer['sel'] = value
        elif port == 0x225:
            self.mixer[self.mixer.get('sel')] = value
        chip = CHIP.get(port)
        if chip is None:
            return
        if port & 1 == 0:
            self.sel[chip] = value
            return
        reg = self.sel.get(chip, 0)
        self.writes.append((self.service, chip, reg, value))
        if reg == 4:  # timer control, for the detection's status read
            if value & 0x80:
                self.status = 0
            elif value & 1:
                self.status = 0xC0


def start(a, song):
    a.mu.mem_write(NAME * 16 + 0x100, song.encode() + b'\0')
    a._setup([0, STUB_SEG, 0x100, NAME, 0], {})
    a._run(0x2F9F, 0x001F, STUB_SEG, 0, count=50_000_000)
    assert a.r16(0xEB00) != 0xFFFF, 'song not registered'


def ail(a, off, *args):
    """An AIL call (segment 0FFD) with the driver's and sequence's handles."""
    a._setup([0, STUB_SEG, a.r16(0xEB93), a.r16(0xEB00), *args], {})
    a._run(0x0FFD, off, STUB_SEG, 0, count=5_000_000)


def play(song, volume, services, then):
    a = DosArena(EXE, GAME)
    a.init_heap()
    card = Card()
    a.mu.hook_add(UC_HOOK_INSN, card.port_in, None, 1, 0, UC_X86_INS_IN)
    a.mu.hook_add(UC_HOOK_INSN, card.port_out, None, 1, 0, UC_X86_INS_OUT)
    a.run_range(0, 0x6F3A, 0x6F87)  # open GLOBAL.BSA, as startup does
    a.mu.mem_write(NAME * 16, DRIVER.encode() + b'\0')
    a._setup([0, STUB_SEG, 0, NAME], {})
    a._run(0x2F9F, 0x01BF, STUB_SEG, 0, count=20_000_000)
    assert a.r16(0x8F5E) or a.r16(0x8F60), 'driver not loaded'
    card.writes.clear()  # detection and reset: not the song's
    a.mu.mem_write((LOAD_SEG + 0x3BA0) * 16 + 0x0D3A, bytes([volume]))
    start(a, song)

    def served(uc, addr, size, ud):
        card.service += 1
    a.mu.hook_add(UC_HOOK_CODE, served, None, SERVE, SERVE)
    faded = then and then[1] is None
    while card.service < services:
        if then and not faded and card.service == then[1]:
            ail(a, 0x0DE8, 0, 800)  # the volume to 0 over 800 ms
            faded = True
        if then and card.service == then[2]:
            if then[1] is not None:
                ail(a, 0x0DCA)  # stopped
                ail(a, 0x0D94)  # released
            start(a, then[0])
            then = None
        a._setup([0, STUB_SEG, 0x202], {})  # IRET frame: ip, cs, flags
        a._run(0x0FFD, 0x058C, STUB_SEG, 0, count=5_000_000)
    return card.writes


def digests(writes, services):
    out = []
    for w0 in range(0, services + 1, WINDOW):
        b = b''.join(struct.pack('<IBBB', *w) for w in writes if w0 <= w[0] < w0 + WINDOW)
        out.append('%08x' % fnv(b))
    return out


def main():
    out = open(OUT, 'w', buffering=1)
    total = len(CASES)
    for n, (song, volume, services, then) in enumerate(CASES):
        if not mine(n, total):
            continue
        writes = play(song, volume, services, then)
        after = '%s@%s@%d' % (then[0], '-' if then[1] is None else then[1], then[2]) if then else '-'
        if DUMP:
            with open(os.path.join(DUMP, f'{song}.{volume}.{services}.{after}.txt'), 'w') as d:
                for w in writes:
                    d.write('%d %d %02x %02x\n' % w)
        out.write(f'{song} {volume} {services} {WINDOW} {len(writes)} {after} | {" ".join(digests(writes, services))}\n')


if __name__ == '__main__':
    main()
