"""A Sound Blaster Pro for the game's own digital sound driver to run on.

ARENA.BAT loads SBPDIG.ADV (-Ssbpdig.adv -IOS220 -IRQS7 -DMAS1), a Miles
AIL 2 digital driver: the game's main loads it (2F9F:0364, after the AIL's
start-up, 1194:07DF) and the AIL (segment 0FFD, the game's own code) calls
into it. Here the driver's own code runs, on the card as it sees it:

- the DSP at base+6 (reset: 1 then 0 written, AAh then to read), base+0Ah
  (its bytes to read), base+0Ch (commands and their bytes written; read,
  bit 7 clear: ready for one) and base+0Eh (read: bit 7 set while a byte
  waits; the read acknowledges the DSP's interrupt);
- its commands: E1h its version (3.02, a Pro's), D1h/D3h the speaker,
  40h a time constant, 14h a single-cycle 8-bit DMA transfer (its length
  less one, low byte first), 48h the high-speed block's length, 1Ch an
  auto-init transfer, 90h/91h high-speed auto-init/single-cycle ones,
  D0h/D4h a transfer halted/continued, DAh an auto-init's end, F2h an
  interrupt at once, E0h/E4h/E8h its test registers;
- the mixer at base+4/base+5 (a register store; register 0Eh bit 1
  stereo);
- the DMA controller's registers (kept, unused: a transfer's length is the
  DSP's) and the PICs' masks (0x21, 0xA1), end-of-interrupt (0x20, 0xA0)
  and in-service register (OCW3 0Bh, then a read: the driver checks it
  for a spurious IRQ 7, its 0000:057B).

A transfer of N bytes lasts N * (256 - time constant) microseconds (a
byte each 256 - TC us, stereo or not). Time is the timer's ticks as the
harness counts them (each read of 0151 in a movie's run a tick, 65536 /
1193182 s): a transfer shorter than a tick ends at once (the driver's
detection, 0CE4, waits for its few bytes' interrupt over 22h retraces); a
longer one ends at the first tick its length has passed. Its end raises
the card's IRQ, which the PIC passes on where unmasked and not in
service: the CPU takes it (its vector from the table at 0000:0000, 8 +
the IRQ) at the next instruction where it is allowed, as the hardware
takes it with interrupts on: anywhere outside the driver and the AIL
(the harness runs the game with IF clear, as nothing else interrupts
it), and inside them where they have set IF.

The AIL calls the game makes into the driver, and their answers, are
recorded for the port's tests (`Blaster.calls`): 7Bh the VOC file played
(its far pointer, the block, and the digest of the FFDCh bytes there),
7Dh started, 7Ch the status asked and answered, 7Eh stopped.
"""
import struct

from emu import LOAD_SEG, STUB_SEG, fnv
from screens import ScreenArena
from unicorn import UC_HOOK_CODE, UC_HOOK_MEM_READ
from unicorn import x86_const as x86
from unicorn.x86_const import UC_X86_REG_CS, UC_X86_REG_IP, UC_X86_REG_SS, UC_X86_REG_SP, UC_X86_REG_EFLAGS, UC_X86_REG_AX

# 1193182 / 65536 ticks a second: a tick in microseconds is 65536e6 /
# 1193182.
TICK_NUM, TICK_DEN = 65536 * 1_000_000, 1193182
AIL_SEG = 0x0FFD
# The AIL's entries the game calls the digital driver through: the
# function each jumps to the driver for.
AIL_CALLS = {0x0D46: '7B', 0x0D58: '7D', 0x0D52: '7C', 0x0D5E: '7E'}
# The digital driver's sound buffer, as 2F75:00DC hands it: FFDCh bytes.
BUFFER_BYTES = 0xFFDC


class Blaster:
    """The card (see the module); `attach` it to a SoundArena."""

    def __init__(self, base=0x220, irq=7):
        self.base, self.irq = base, irq
        self.read = []
        self.reset = 0
        self.cmd, self.args, self.need = None, [], 0
        self.tc = None
        self.hs_block = 0
        self.mixer, self.mixer_sel = {}, 0
        self.dma = {}
        self.mask = {0x21: 0xB8, 0xA1: 0xFF}
        self.ocw3 = {0x20: 0x0A, 0xA0: 0x0A}
        self.in_service = 0
        self.ticks = 0
        # The transfer playing: (its end in ticks, or None for at once;
        # auto-init's block length or None).
        self.playing = None
        self.pending = False
        self.irq_hook = None
        self.calls = []
        self.a = None

    # --- the card's ports --------------------------------------------------

    def ports(self, port):
        b = self.base
        return b <= port < b + 0x10 or port in (0x20, 0x21, 0xA0, 0xA1) or port < 0x10 or 0x81 <= port <= 0x8F

    def port_in(self, port):
        b = self.base
        if port == b + 0x0A:
            return self.read.pop(0) if self.read else 0xFF
        if port == b + 0x0E:
            return 0x80 if self.read else 0x00
        if port == b + 0x0C:
            return 0x00
        if port == b + 0x06:
            return 0xFF
        if port == b + 0x05:
            return self.mixer.get(self.mixer_sel, 0)
        if port in (0x21, 0xA1):
            return self.mask[port]
        if port in (0x20, 0xA0):
            if self.ocw3[port] == 0x0B:
                return (1 << self.irq) & 0xFF if port == 0x20 and self.in_service else 0
            return 0
        return self.dma.get(port, 0)

    def port_out(self, port, value):
        b, value = self.base, value & 0xFF
        if port == b + 0x06:
            if value & 1:
                self.reset = 1
            elif self.reset:
                self.reset = 0
                self.read = [0xAA]
                self.cmd, self.args, self.need = None, [], 0
            return
        if port == b + 0x0C:
            self.dsp(value)
            return
        if port == b + 0x04:
            self.mixer_sel = value
            return
        if port == b + 0x05:
            self.mixer[self.mixer_sel] = value
            return
        if port in (0x21, 0xA1):
            self.mask[port] = value
            self.consider()
            return
        if port in (0x20, 0xA0):
            if value == 0x20:
                self.in_service = 0
                self.consider()
            elif value & 0x18 == 0x08:
                self.ocw3[port] = value
            return
        self.dma[port] = value

    def dsp(self, value):
        if self.cmd is not None:
            self.args.append(value)
            if len(self.args) < self.need:
                return
            cmd, args = self.cmd, self.args
            self.cmd, self.args, self.need = None, [], 0
            self.command(cmd, args)
            return
        need = {0x40: 1, 0x14: 2, 0x48: 2, 0x10: 1, 0xE0: 1, 0xE4: 1}.get(value, 0)
        if need:
            self.cmd, self.args, self.need = value, [], need
            return
        self.command(value, [])

    def command(self, cmd, args):
        if cmd == 0xE1:
            self.read += [3, 2]
        elif cmd == 0xE0:
            self.read.append(~args[0] & 0xFF)
        elif cmd == 0xE4:
            self.test = args[0]
        elif cmd == 0xE8:
            self.read.append(getattr(self, 'test', 0))
        elif cmd == 0x40:
            self.tc = args[0]
        elif cmd == 0x48:
            self.hs_block = (args[0] | args[1] << 8) + 1
        elif cmd == 0x14:
            self.start((args[0] | args[1] << 8) + 1, auto=False)
        elif cmd == 0x91:
            self.start(self.hs_block, auto=False)
        elif cmd in (0x1C, 0x90):
            self.start(self.hs_block, auto=True)
        elif cmd == 0xF2:
            self.playing = None
            self.raise_irq()
        elif cmd in (0xD0, 0xDA):
            self.playing = None
        elif cmd in (0xD1, 0xD3, 0xD4, 0x10):
            pass
        else:
            raise AssertionError(f'DSP command {cmd:02X} {args} not modelled')

    # --- time --------------------------------------------------------------

    def end_after(self, count):
        """The tick a transfer of `count` bytes started now ends at (None
        for at once: shorter than a tick)."""
        us = count * (256 - (self.tc if self.tc is not None else 0))
        if us * TICK_DEN < TICK_NUM:
            return None
        return self.ticks + -(-us * TICK_DEN // TICK_NUM)

    def start(self, count, auto):
        end = self.end_after(count)
        self.playing = (end, count if auto else None)
        if end is None:
            self.ended()

    def tick(self):
        self.ticks += 1
        if self.playing is not None and self.playing[0] is not None and self.ticks >= self.playing[0]:
            self.ended()

    def ended(self):
        _, block = self.playing
        self.playing = None if block is None else (self.end_after(block), block)
        self.raise_irq()

    # --- the interrupt -------------------------------------------------------

    def raise_irq(self):
        self.pending = True
        self.consider()

    def consider(self):
        """With the IRQ pending, unmasked and not in service, the CPU's
        taking of it watched for (every instruction until taken)."""
        a = self.a
        if a is None or not self.pending or self.in_service or self.mask[0x21] & (1 << self.irq):
            if self.irq_hook is not None and a is not None:
                a.mu.hook_del(self.irq_hook)
                self.irq_hook = None
            return
        if self.irq_hook is None:
            self.irq_hook = a.mu.hook_add(UC_HOOK_CODE, self.take)

    def take(self, mu, addr, size, _):
        cs, fl = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_EFLAGS)
        driver = self.a.r16(0x8F64)
        if not fl & 0x200 and cs in (driver, LOAD_SEG + AIL_SEG):
            return
        ip, ss, sp = mu.reg_read(UC_X86_REG_IP), mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        vec = struct.unpack('<HH', bytes(mu.mem_read(4 * (8 + self.irq), 4)))
        sp = (sp - 6) & 0xFFFF
        mu.mem_write(ss * 16 + sp, struct.pack('<HHH', ip, cs, fl & 0xFFFF))
        mu.reg_write(UC_X86_REG_SP, sp)
        mu.reg_write(UC_X86_REG_EFLAGS, fl & ~0x300)
        mu.reg_write(UC_X86_REG_CS, vec[1])
        mu.reg_write(UC_X86_REG_IP, vec[0])
        self.pending = False
        self.in_service = 1
        mu.hook_del(self.irq_hook)
        self.irq_hook = None

    # --- the game's calls, recorded ---------------------------------------

    def attach(self, a):
        self.a = a
        a.blaster = self
        mu = a.mu
        self.hooks = [mu.hook_add(UC_HOOK_MEM_READ, lambda *_: self.tick(), None, a.lin(a.ds, 0x0151), a.lin(a.ds, 0x0151))]
        for off, fn in AIL_CALLS.items():
            at = a.lin(LOAD_SEG + AIL_SEG, off)
            self.hooks.append(mu.hook_add(UC_HOOK_CODE, self.called, fn, at, at))
        mu.ctl_flush_tb()

    def detach(self):
        a = self.a
        for h in self.hooks:
            a.mu.hook_del(h)
        if self.irq_hook is not None:
            a.mu.hook_del(self.irq_hook)
            self.irq_hook = None
        a.blaster = None
        self.a = None

    def called(self, mu, addr, size, fn):
        a = self.a
        ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
        words = struct.unpack('<6H', bytes(mu.mem_read(ss * 16 + sp, 12)))
        ret = (words[1], words[0])
        if fn == '7B':
            off, seg, block = words[3], words[4], words[5]
            data = bytes(mu.mem_read(seg * 16 + off, BUFFER_BYTES))
            self.calls.append(f'7B:{block:x}:{fnv(data):x}')
        elif fn == '7C':
            # The answer read where the call returns.
            at = ret[0] * 16 + ret[1]
            def answered(mu, addr, size, _):
                self.calls.append(f'7C={mu.reg_read(UC_X86_REG_AX):x}')
                mu.hook_del(h[0])
            h = [mu.hook_add(UC_HOOK_CODE, answered, None, at, at)]
        else:
            self.calls.append(fn)

    def calls_line(self, name):
        """The calls, runs of the same one counted (7C=2*30)."""
        out = []
        for c in self.calls:
            if out and out[-1][0] == c:
                out[-1][1] += 1
            else:
                out.append([c, 1])
        return f'driver | {name} | ' + ' '.join(c if n == 1 else f'{c}*{n}' for c, n in out) + '\n'


# `SoundArena.press`'s keys for the sound driver's load (`driver_loaded`)
# from where a run stopped for it (`run_screen` goes on after).
DRIVER_LOAD = ('driver',)


class SoundArena(ScreenArena):
    """A ScreenArena whose ports answer as the attached Blaster's where it
    has one (`blaster`), else as ScreenArena's: without one, the same."""

    blaster = None

    def press(self, keys, up=False):
        if keys is DRIVER_LOAD:
            driver_loaded(self)
            return
        super().press(keys, up)

    def _in(self, mu, port, size, ud):
        if self.blaster is not None and self.blaster.ports(port):
            return self.blaster.port_in(port)
        return super()._in(mu, port, size, ud)

    def _out(self, mu, port, size, value, ud):
        if self.blaster is not None and self.blaster.ports(port):
            self.blaster.port_out(port, value)
            return
        return super()._out(mu, port, size, value, ud)


def driver_loaded(a):
    """The sound driver named at 3827 loaded as main loads it (1194:076D:
    the AIL started once, 07DF, then 2F9F:0364 with DS:3827), on the stack
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
        assert at == end, f'the driver load stopped at {at[0]:04X}:{at[1]:04X}'
    run(0x1194, 0x07DF, [0xFFF0], near=True)
    run(0x2F9F, 0x0364, [0, STUB_SEG, 0x3827, a.ds])
    for r, v in kept.items():
        mu.reg_write(getattr(x86, 'UC_X86_REG_' + r.upper()), v)

