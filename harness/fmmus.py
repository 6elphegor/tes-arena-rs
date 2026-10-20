"""A Sound Blaster Pro's FM chips for the game's own music driver to run
on, and the music driver loaded as the program's main loads it.

The game's main loads ARENA.BAT's music driver (-M; here SBP1FM.ADV, a
Miles AIL 2 XMIDI driver, as -Msbp1fm.adv -IOM220 would name it) right
after its sound driver (1194:0783: the AIL started once, 07DF, then
2F9F:01BF with DS:37EB). The AIL (segment 0FFD, the game's own code)
calls into the driver; here the driver's own code runs, on the card as
it sees it (gen_fmdrv.py's `Card`): the two OPL2s at 220h/221h and
222h/223h (228h/229h both, 388h/389h the AdLib's), whose status the
driver's detection reads (timer 1 started: C0h; reset: 0), and the
mixer at 224h/225h (a register store, which the driver writes and reads
back to find the card).

The AIL plays the music from the real-time clock's interrupt (IRQ 8,
int 70h; its period, cs:118h, 3906 us: 256 a second), which calls each
of its timer's clients when its period is due: the driver's serve
(120 a second), which reads the sequence and writes the chips. The
interrupt is the CMOS clock's (ports 70h/71h: its registers stored,
register C read as the interrupt's flags) through the second PIC (A0h,
A1h: its mask, the end of interrupt). Time is the harness's timer
ticks (each read of 0151 a tick, 65536 / 1193182 s, as sbdig.py's):
each tick owes the interrupts its time holds (14 or 15), and the CPU
takes them one after another as the hardware does with interrupts on:
anywhere outside the drivers and the AIL (the harness runs the game
with IF clear, as nothing else interrupts it), and inside them where
they have set IF; not while one is in service (until its end of
interrupt).

The calls the game makes into the AIL for its music (2F9F's wrappers:
01BF the driver's load, 001F a song started, 0438 one faded and
stopped, 000D the playing one asked after, 04A0 the music shut down;
the options screen's volume, 2542:07BD) are recorded with their
arguments and answers (`Music.calls`) for the port's replay; the
digital driver's load (2F9F:0364) shares four of the entries and is
left out.
"""
import struct

from emu import LOAD_SEG, STUB_SEG, fnv
from sbdig import SoundArena, AIL_SEG, TICK_NUM, TICK_DEN
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ
from unicorn import x86_const as x86
from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP, UC_X86_REG_SS, UC_X86_REG_SP, UC_X86_REG_EFLAGS, UC_X86_REG_AX, UC_X86_REG_DX

WRAPPERS = LOAD_SEG + 0x2F9F
OPTIONS = LOAD_SEG + 0x2542
# The digital driver's load (2F9F:0364..0437), whose calls into the AIL's
# shared entries aren't the music's.
DIGITAL_LOAD = range(0x0364, 0x0438)
# The AIL's entries the music calls, each with how its call is recorded:
# its name, the words it is passed (after the far return), and whether
# its answer is read (AX, or 'far' DX:AX).
ENTRIES = {
    0x0AF7: ('AF7', 2, 'ax'),    # the driver's image registered: its handle
    0x0C1D: ('C1D', 1, 'far'),   # described: its descriptor
    0x0C43: ('C43', 5, 'ax'),    # the device looked for: found
    0x0C49: ('C49', 5, None),    # the driver initialised
    0x0D88: ('D88', 1, 'ax'),    # a sequence's state table's size
    0x0D9A: ('D9A', 1, 'ax'),    # the timbre cache's size
    0x0DA0: ('DA0', 4, None),    # the timbre cache given
    0x0D8E: ('D8E', 8, 'ax'),    # a sequence registered: its handle
    0x0DA6: ('DA6', 2, 'ax'),    # a timbre it needs (bank << 8 | patch; FFFFh none)
    0x0DAC: ('DAC', 5, None),    # the timbre installed
    0x0E0C: ('E0C', 5, None),    # a channel's controller set
    0x0DC4: ('DC4', 2, None),    # the sequence started
    0x0DE8: ('DE8', 4, None),    # its relative volume, over a time
    0x0DCA: ('DCA', 2, None),    # stopped
    0x0D94: ('D94', 2, None),    # released
    0x0DD6: ('DD6', 2, 'ax'),    # its status: 0 stopped, 1 playing, 2 done
    0x0821: ('821', 2, None),    # the AIL shut down
}
# RTC interrupt vector (IRQ 8).
RTC_VECTOR = 0x70
CHIP = {0x220: 0, 0x221: 0, 0x222: 1, 0x223: 1, 0x228: 2, 0x229: 2, 0x388: 3, 0x389: 3}


def timbre_bytes(mu, seg, off):
    """A timbre installed (0DAC's far pointer): its bytes, the word at its
    start its size (1194:4943 reads it there)."""
    n = struct.unpack('<H', bytes(mu.mem_read(seg * 16 + off, 2)))[0]
    return bytes(mu.mem_read(seg * 16 + off, n))


class Music:
    """The card and the clock (see the module); `attach` it to a
    MusicArena."""

    def __init__(self):
        self.status = 0
        self.mixer, self.mixer_sel = {}, 0
        self.sel = {}
        self.writes = 0
        self.cmos, self.cmos_sel = {}, 0
        self.mask = {0x21: 0xB8, 0xA1: 0xFF}
        self.in_service = False
        self.us = 0
        self.owed = 0
        self.ticks = 0
        self.irq_hook = None
        self.calls = []
        self.answer_hooks = {}
        self.answer_due = {}
        self.taken = 0
        self.a = None

    # --- the ports -----------------------------------------------------------

    def ports(self, port):
        return 0x220 <= port <= 0x225 or port in (0x228, 0x229, 0x388, 0x389, 0x70, 0x71, 0xA0, 0xA1)

    def port_in(self, port):
        if port in (0x220, 0x222, 0x228, 0x388):
            return self.status
        if port == 0x225:
            return self.mixer.get(self.mixer_sel, 0)
        if port == 0x71:
            reg = self.cmos_sel & 0x7F
            if reg == 0x0C:
                # Register C: the periodic interrupt's flag (and IRQF),
                # cleared by the read.
                return 0xC0 if self.in_service else 0
            return self.cmos.get(reg, 0)
        if port == 0xA1:
            return self.mask[0xA1]
        if port == 0xA0:
            return 0
        return 0xFF

    def port_out(self, port, value):
        value &= 0xFF
        if port == 0x224:
            self.mixer_sel = value
            return
        if port == 0x225:
            self.mixer[self.mixer_sel] = value
            return
        if port == 0x70:
            self.cmos_sel = value
            return
        if port == 0x71:
            self.cmos[self.cmos_sel & 0x7F] = value
            return
        if port == 0xA1:
            self.mask[0xA1] = value
            self.consider()
            return
        if port == 0xA0:
            if value == 0x20:
                self.in_service = False
                self.consider()
            return
        chip = CHIP.get(port)
        if chip is None:
            return
        if port & 1 == 0:
            self.sel[chip] = value
            return
        reg = self.sel.get(chip, 0)
        self.writes += 1
        if reg == 4:  # timer control, for the detection's status read
            if value & 0x80:
                self.status = 0
            elif value & 1:
                self.status = 0xC0

    def master(self, port, value):
        """The first PIC's mask (21h) and end of interrupt (20h), which the
        game's own port writes pass too."""
        value &= 0xFF
        if port == 0x21:
            self.mask[0x21] = value
            self.consider()

    # --- time ------------------------------------------------------------------

    def period(self):
        """The AIL's interrupt period in microseconds (cs:118h); 0 before
        its start."""
        return struct.unpack('<I', bytes(self.a.mu.mem_read((LOAD_SEG + AIL_SEG) * 16 + 0x118, 4)))[0]

    def tick(self):
        self.ticks += 1
        per = self.period()
        if per == 0 or per >= 0x8000_0000:
            return
        # The tick's microseconds, exactly (TICK_NUM / TICK_DEN).
        self.us += TICK_NUM
        while self.us >= per * TICK_DEN:
            self.us -= per * TICK_DEN
            self.owed += 1
        self.consider()

    # --- the interrupt -------------------------------------------------------

    def consider(self):
        a = self.a
        allowed = self.owed and not self.in_service and not self.mask[0xA1] & 1 and not self.mask[0x21] & 4
        if a is None or not allowed or self.vector() == (0, 0):
            if self.irq_hook is not None and a is not None:
                a.mu.hook_del(self.irq_hook)
                self.irq_hook = None
            return
        if self.irq_hook is None:
            self.irq_hook = a.mu.hook_add(UC_HOOK_CODE, self.take)

    def vector(self):
        return struct.unpack('<HH', bytes(self.a.mu.mem_read(4 * RTC_VECTOR, 4)))

    def take(self, mu, addr, size, _):
        cs, fl = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_EFLAGS)
        drivers = {self.a.r16(0x8F60), self.a.r16(0x8F64), LOAD_SEG + AIL_SEG} - {0}
        if not fl & 0x200 and cs in drivers:
            return
        if cs == STUB_SEG:
            return
        ip, ss, sp = mu.reg_read(UC_X86_REG_IP), mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        off, seg = self.vector()
        sp = (sp - 6) & 0xFFFF
        mu.mem_write(ss * 16 + sp, struct.pack('<HHH', ip, cs, fl & 0xFFFF))
        mu.reg_write(UC_X86_REG_SP, sp)
        mu.reg_write(UC_X86_REG_EFLAGS, fl & ~0x300)
        mu.reg_write(UC_X86_REG_CS, seg)
        mu.reg_write(UC_X86_REG_IP, off)
        self.owed -= 1
        self.taken += 1
        self.in_service = True
        mu.hook_del(self.irq_hook)
        self.irq_hook = None

    # --- the game's calls, recorded ---------------------------------------

    def attach(self, a):
        self.a = a
        a.music = self
        mu = a.mu
        self.answer_hooks = {}
        self.answer_due = {}
        self.hooks = [mu.hook_add(UC_HOOK_MEM_READ, lambda *_: self.tick(), None, a.lin(a.ds, 0x0151), a.lin(a.ds, 0x0151))]
        for off, entry in ENTRIES.items():
            at = a.lin(LOAD_SEG + AIL_SEG, off)
            self.hooks.append(mu.hook_add(UC_HOOK_CODE, self.called, entry, at, at))
        mu.ctl_flush_tb()

    KEPT = ('status', 'mixer', 'mixer_sel', 'sel', 'writes', 'cmos', 'cmos_sel', 'mask', 'in_service', 'us', 'owed',
            'ticks', 'calls', 'taken')

    def kept(self):
        import copy
        return {k: copy.deepcopy(getattr(self, k)) for k in self.KEPT}

    def kept_back(self, kept):
        import copy
        for k, v in kept.items():
            setattr(self, k, copy.deepcopy(v))
        self.answer_due = {}
        self.consider()

    def called(self, mu, addr, size, entry):
        name, n, answer = entry
        ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        words = struct.unpack(f'<{2 + n}H', bytes(mu.mem_read(ss * 16 + sp, 4 + 2 * n)))
        ret, args = (words[1], words[0]), words[2:]
        if ret[0] not in (WRAPPERS, OPTIONS) or (ret[0] == WRAPPERS and ret[1] in DIGITAL_LOAD):
            return
        if name == 'DAC':
            # (handle, bank, patch, far pointer): the timbre's bytes' digest.
            h, bank, patch, off, seg = args
            text = f'DAC:{bank:x}:{patch:x}:{fnv(timbre_bytes(mu, seg, off)):x}'
        elif name == 'D8E':
            # (handle, the XMIDI's far pointer, the sequence's number, the
            # state table's far pointer, 0, 0).
            text = f'D8E:{args[2]:x}:{args[1]:x}:{args[3]:x}:{args[5]:x}:{args[4]:x}'
        elif name in ('AF7', 'C1D', '821', 'D88', 'D9A'):
            text = name if name != 'AF7' else f'AF7:{args[1]:x}:{args[0]:x}'
        else:
            # The handle (the first word) left out: the words after it.
            text = ':'.join([name] + [f'{w:x}' for w in args[1:]])
        if answer is None:
            self.calls.append(text)
            return
        at = ret[0] * 16 + ret[1]
        self.answer_due[at] = (text, answer)
        if at not in self.answer_hooks:
            def answered(mu, addr, size, at):
                due = self.answer_due.pop(at, None)
                if due is not None:
                    text, kind = due
                    ax = mu.reg_read(UC_X86_REG_AX)
                    v = f'{mu.reg_read(UC_X86_REG_DX):x}:{ax:x}' if kind == 'far' else f'{ax:x}'
                    self.calls.append(f'{text}={v}')
            self.answer_hooks[at] = mu.hook_add(UC_HOOK_CODE, answered, at, at, at)
            self.hooks.append(self.answer_hooks[at])

    def calls_line(self, name):
        """The calls, runs of the same one counted (DD6:0=1*30)."""
        out = []
        for c in self.calls:
            if out and out[-1][0] == c:
                out[-1][1] += 1
            else:
                out.append([c, 1])
        return f'music | {name} | ' + ' '.join(c if n == 1 else f'{c}*{n}' for c, n in out) + '\n'


class Mpu(Music):
    """A Roland MPU-401 at 330h (data) and 331h (status and command), for
    MT32MPU.ADV, with the clock and PICs as `Music`'s. As the interface
    reports it: the status's bit 7 (DSR) clear while a byte waits to be
    read at 330h, bit 6 (DRR) clear as it is always ready to take one
    (the low six bits read 1s, as an unconnected bus does); command FFh
    resets it (an acknowledgement, FEh, to read; in UART mode only back
    out of it, with none), 3Fh puts it in UART mode (acknowledged), any
    other command acknowledged in its intelligent mode and ignored in
    UART mode; in UART mode each byte written to 330h goes to the MIDI
    out, kept (`midi`) for the comparison as the FM chips' writes are
    counted. A byte waiting raises the interface's interrupt (IRQ 2) on
    the real card; MT32MPU.ADV polls the status instead and hooks no
    vector for it (0Ah and 71h stay 0:0 after its load; only the AIL's
    clock, 70h, is set), so the interrupt isn't modelled."""

    def __init__(self):
        super().__init__()
        self.uart = False
        self.queue = []
        self.midi = bytearray()

    KEPT = Music.KEPT + ('uart', 'queue', 'midi')

    def ports(self, port):
        return port in (0x330, 0x331, 0x70, 0x71, 0xA0, 0xA1)

    def port_in(self, port):
        if port == 0x331:
            return 0x3F | (0 if self.queue else 0x80)
        if port == 0x330:
            return self.queue.pop(0) if self.queue else 0xFE
        return super().port_in(port)

    def port_out(self, port, value):
        value &= 0xFF
        if port == 0x331:
            if value == 0xFF:
                if self.uart:
                    self.uart = False
                    return
                self.queue = [0xFE]
            elif not self.uart:
                if value == 0x3F:
                    self.uart = True
                self.queue.append(0xFE)
            return
        if port == 0x330:
            if self.uart:
                self.midi.append(value)
                self.writes += 1
            return
        super().port_out(port, value)

    def midi_line(self, name):
        """The MIDI out since the last line: its length and digest."""
        line = f'mpu | {name} | {len(self.midi)} {fnv(bytes(self.midi)):x}\n'
        self.midi = bytearray()
        return line


class MusicArena(SoundArena):
    """A SoundArena with, at `start(music=True)`, the music driver loaded
    as the program's main loads it (`music_loaded`) on the card (`Music`),
    whose ports then answer as the card's; without it, the same."""

    music = None
    music_at_start = False
    # The driver loaded: 'fm' (SBP1FM.ADV on the FM chips, `Music`) or
    # 'mt32' (MT32MPU.ADV on an MPU-401, `Mpu`).
    music_kind = 'fm'

    def start(self, slot=1, sound=False, music=False, kind='fm'):
        self.music_at_start = music
        self.music_kind = kind
        super().start(slot, sound)

    def stub(self, location=True):
        # With a music driver loaded the songs are started (2F9F:001F) as
        # the game starts them, not counted.
        saved = super().stub(location)
        if self.music is not None or self.music_at_start:
            lin = self.lin(LOAD_SEG + 0x2F9F, 0x001F)
            for at, b in saved:
                if at == lin:
                    self.code_write(at, b)
            saved = [s for s in saved if s[0] != lin]
        return saved

    def snapshot(self):
        m = self.music
        return (super().snapshot(), m.kept() if m is not None else None)

    def restore(self, snap):
        base, kept = snap
        super().restore(base)
        if kept is not None and self.music is not None:
            self.music.kept_back(kept)

    def run_screen(self, seg, off, *args, **kw):
        r = super().run_screen(seg, off, *args, **kw)
        if self.music_at_start and (seg, off) == (0x25FD, 0x0219):
            self.music_at_start = False
            # main's file-open hook, 0000:11DA at A904 (0000:1047, before
            # 21EE:0000, which ScreenArena's start runs alone): with an FM
            # driver a song's ".xmi" opened as its ".xfm".
            self.w16(0xA904, 0x11DA)
            self.w16(0xA906, LOAD_SEG)
            music_named(self, self.music_kind)
            (Mpu() if self.music_kind == 'mt32' else Music()).attach(self)
            music_loaded(self)
        return r

    def _in(self, mu, port, size, ud):
        if self.music is not None and self.music.ports(port):
            return self.music.port_in(port)
        return super()._in(mu, port, size, ud)

    def _out(self, mu, port, size, value, ud):
        if self.music is not None:
            if port in (0x20, 0x21):
                self.music.master(port, value)
            elif self.music.ports(port):
                self.music.port_out(port, value)
                return
        return super()._out(mu, port, size, value, ud)


IDLE = 0x0500  # a free paragraph (DOS's communication area): `jmp $` for `music_time`


def music_time(a, ticks):
    """`ticks` of the music's time passed between two frames, the game's
    own timer (0151, the BIOS count) left as it is: the CPU idles at a
    `jmp $` outside the game while the AIL's clock interrupts owed for
    that time run (each calling the driver's serve, which reads the
    sequence on, so a song that reaches its end is done, status 2, as it
    would be after that long of play). A harness shortcut for a song's
    end, which takes minutes of frames to reach; nothing the game keeps
    is touched, only the AIL's, the driver's and the card's."""
    mu, m = a.mu, a.music
    kept = {r: mu.reg_read(getattr(x86, 'UC_X86_REG_' + r.upper())) for r in a.KEPT}
    mu.mem_write(IDLE, b'\xEB\xFE')
    assert not m.in_service, 'music_time: an interrupt already in service'
    assert m.vector() != (0, 0) and m.period(), 'the AIL clock not going'
    mu.reg_write(UC_X86_REG_CS, IDLE >> 4)
    mu.reg_write(UC_X86_REG_IP, 0)
    pc = lambda: mu.reg_read(UC_X86_REG_CS) * 16 + mu.reg_read(UC_X86_REG_IP)
    for _ in range(ticks):
        m.tick()
        spins = 0
        # On from wherever the last slice stopped (in a handler, its
        # end and IRET still to come), until nothing is owed or in
        # service and the CPU is back at the idle loop.
        while m.owed or m.in_service or pc() != IDLE:
            spins += 1
            assert spins < 10_000, f'music_time: the clock interrupt not taken (owed {m.owed}, in service {m.in_service}, masks {m.mask})'
            mu.emu_start(pc(), IDLE if not (m.owed or m.in_service) else 0xFFFFF, count=20_000)
    for r, v in kept.items():
        mu.reg_write(getattr(x86, 'UC_X86_REG_' + r.upper()), v)


def music_named(a, kind='fm'):
    """A music driver as the command line leaves it (37EB, 3864; its IRQ
    and DMA channel, 3866 and 3868, the driver's own, FFFFh, unless given):
    -Msbp1fm.adv -IOM220, or ARENA.BAT's -Mmt32mpu.adv -IOM330 -IRQM2
    -DMAM1 (cmdline.rs's parser leaves the uppercased name and the
    numbers)."""
    if kind == 'mt32':
        a.mu.mem_write(a.lin(a.ds, 0x37EB), b'MT32MPU.ADV\0')
        a.w16(0x3864, 0x330)
        a.w16(0x3866, 2)
        a.w16(0x3868, 1)
        return
    a.mu.mem_write(a.lin(a.ds, 0x37EB), b'SBP1FM.ADV\0')
    a.w16(0x3864, 0x220)


def music_loaded(a):
    """The music driver named at 37EB loaded as main loads it (1194:0783:
    the AIL started once, 07DF, then 2F9F:01BF with DS:37EB), on the stack
    under the game's, the game's registers kept round it."""
    mu = a.mu
    kept = {r: mu.reg_read(getattr(x86, 'UC_X86_REG_' + r.upper())) for r in a.KEPT}
    sp = (kept['sp'] - 0x400) & 0xFFFF
    def run(seg, off, words, near=False):
        words = list(words)
        mu.mem_write(a.lin(kept['ss'], sp - 2 * len(words)), struct.pack(f'<{len(words)}H', *words))
        for r, v in dict(ss=kept['ss'], sp=sp - 2 * len(words), ds=a.ds, es=a.ds).items():
            mu.reg_write(getattr(x86, 'UC_X86_REG_' + r.upper()), v)
        mu.reg_write(UC_X86_REG_CS, LOAD_SEG + seg)
        mu.reg_write(UC_X86_REG_IP, off)
        end = (LOAD_SEG + seg, 0xFFF0) if near else (STUB_SEG, 0)
        mu.emu_start(a.lin(LOAD_SEG + seg, off), a.lin(*end), count=200_000_000)
        at = (mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP))
        assert at == end, f'the music driver load stopped at {at[0]:04X}:{at[1]:04X}'
    run(0x1194, 0x07DF, [0xFFF0], near=True)
    run(0x2F9F, 0x01BF, [0, STUB_SEG, 0x37EB, a.ds])
    assert a.r16(0x8F5E) or a.r16(0x8F60), 'the music driver not loaded'
    for r, v in kept.items():
        mu.reg_write(getattr(x86, 'UC_X86_REG_' + r.upper()), v)
