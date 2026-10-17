"""The original's screens drawn under unicorn, for gen_screens.py: a DOS
machine enough for the game's start (0000:6F02, 21EE:0000), a saved game
loaded from the title (25FD:0309), and a screen's routine run until it
first waits for input, when the page it drew into is read. From the
loaded game the original's own outer loop can take the player out of the
building (`area_ended`) and in at another's door (`enter`).

The machine: the BIOS video calls are taken and do nothing (mode 13h is
the screen; AH 0Fh says mode 3 before it); the mouse driver is present
(int 33h AX 0) with its cursor at `mouse` and no button down; the
keyboard's buffer is empty (int 16h AH 1 sets ZF). The VGA's DAC writes
(ports 3C8h/3C9h) are kept as the palette; port 3DAh's retrace bits
toggle at each read. Interrupt vectors asked for (int 21h AH 35h) are
0000:0000, and setting one (AH 25h) is taken.

The first wait for input stops the run (`Waiting`): a mouse button's
state asked for (int 33h AX 3, 5, 6), a key's (int 16h), or a routine
named in `waits`. Keys are pressed as the hardware would: each scan code
and its release given to the game's key interrupt (`press`).
"""
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from dos import DosArena
from emu import Fault, LOAD_SEG, STUB_SEG
from unicorn import UC_HOOK_INSN, UC_HOOK_CODE
from unicorn.x86_const import *

PAGE_SEG_FIELD = 0xA900
# Memory below the image: the EMS page maps, the far pointer 1020's 80h
# bytes, the stack.
MAPS = 0x0780
EXTRA = (0x0900, 0x0010)
STACK = (0x0A00, 0x5FF0)
HEAP_LOW, VGA, HEAP_HIGH, HEAP_TOP = 0x5C00, 0xA000, 0xB000, 0xF000
EMS_FRAME = 0xF000


# The mouse asked for (8FC4) by a loop waiting for the buttons to be let go
# (1194:5E21: its return address), not for input.
RELEASE_WAITS = {(0x1194, 0x5E26)}


class Waiting(Exception):
    """The screen's routine asked for input: what it asked by."""


class ScreenArena(DosArena):
    def __init__(self, exe, game, *more):
        super().__init__(exe, game, *more)
        self.dac = bytearray(0x300)
        self.dac_at = 0
        self.dac_read_at = 0
        self.retrace = 0
        self.mouse = (160, 100)
        self.buttons = 0
        # The mouse's motion since the last read of it (INT 33h function
        # 0Bh: mickeys, taken and cleared by the read).
        self.motion = (0, 0)
        self.waits = set()
        self.std_devices = True
        self.mu.hook_add(UC_HOOK_INSN, self._in, None, 1, 0, UC_X86_INS_IN)
        self.mu.hook_add(UC_HOOK_INSN, self._out, None, 1, 0, UC_X86_INS_OUT)
        self.timer = 0
        self.written = {}
        self.clicks = []
        self.dta = (0x0040, 0x0100)
        # Files the game deleted: the game directory's copies no longer
        # found (never touched on disk). The save folder (the first of
        # `more`) is the game's save path, as the view's --save-dir is:
        # without a NAMES.DAT of its own, the game directory's (the
        # install's, its slots "EMPTY") isn't the save path's.
        self.deleted = set()
        if more and not any(n.upper() == 'NAMES.DAT' for n in os.listdir(more[0])):
            self.deleted.add('NAMES.DAT')
        self.found = []
        # What to do at each arrival at a wait, in turn: ('click', x, y)
        # queues a click and goes on; with none left the run stops.
        self.script = []

    # --- memory -------------------------------------------------------------

    def _setup(self, ret_words, regs):
        """As emu's, on a stack below the image (STACK), the heap having
        the conventional memory the usual stack (9000h) sits in."""
        mu = self.mu
        ss, sp = STACK
        sp -= 2 * len(ret_words)
        mu.mem_write(self.lin(ss, sp), struct.pack(f'<{len(ret_words)}H', *ret_words))
        for r, v in dict(ds=self.ds, es=self.ds, fs=self.ds, gs=self.ds, ss=ss, sp=sp,
                         eax=0, ebx=0, ecx=0, edx=0, esi=0, edi=0, ebp=0, eflags=2).items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
        for r, v in regs.items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def init_memory(self):
        """The heap over conventional memory from the image's end (5C00h)
        to F000h, with the VGA's 64 KB (A000h) a block in use: free
        5C00h..9FFEh, VGA, free B000h..EFFDh. EMS on with its frame at
        F000h."""
        self.w16(0x0063, 0x03D4, seg=0x0040)  # the BIOS's CRTC port (its status at +6)
        self.init_heap(HEAP_LOW, HEAP_TOP)
        def header(seg, nxt, prev, size, used):
            self.mu.mem_write(self.lin(seg, 0), struct.pack('<HHHHHB', 0, nxt, 0, prev, size, used))
        low = VGA - 1 - HEAP_LOW - 1
        high = HEAP_TOP - HEAP_HIGH - 3
        header(HEAP_LOW, VGA - 1, 0, low, 0)
        header(VGA - 1, HEAP_HIGH, HEAP_LOW, 0x1000, 1)
        header(HEAP_HIGH, 0, VGA - 1, high, 0)
        self.w16(0x8F6E, low + high)
        self.enable_ems(EMS_FRAME, maps_seg=MAPS)
        # Every bank free, as the start's EMS set-up leaves them (0000:5DA0:
        # 8FAA's words FFFFh).
        for b in range(self.r16(0x9032)):
            self.w16(0x8FAA + 2 * b, 0xFFFF)

    # --- ports --------------------------------------------------------------

    def _in(self, mu, port, size, _):
        if port == 0x3DA:
            self.retrace ^= 9
            return self.retrace
        if port == 0x3C9:
            v = self.dac[self.dac_read_at % 0x300]
            self.dac_read_at += 1
            return v
        if port == 0x60:
            return 0
        return 0

    def _out(self, mu, port, size, value, _):
        if port == 0x3C8:
            self.dac_at = (value & 0xFF) * 3
        elif port == 0x3C7:
            self.dac_read_at = (value & 0xFF) * 3
        elif port == 0x3C9:
            if size == 1:
                self.dac[self.dac_at % 0x300] = value & 0x3F
                self.dac_at += 1
            else:
                for k in range(size):
                    self.dac[self.dac_at % 0x300] = (value >> (8 * k)) & 0x3F
                    self.dac_at += 1

    def snapshot(self):
        # The world loop's stack kept (`area_ended`, `world_frames`) is the
        # snapshot's too: restored without it, leaving an area resumed
        # whichever world loop ran last (a named level's run, whose end
        # clears the level hooks at 0000:09D8, for the street left from
        # the load's guild, whose loop returns through 0ED5 and doesn't).
        return (super().snapshot(), bytes(self.dac), {k: bytes(v) for k, v in self.written.items()},
                set(self.deleted), getattr(self, 'world_stack', None))

    def restore(self, snap):
        base, dac, written, deleted, world_stack = snap
        super().restore(base)
        self.dac = bytearray(dac)
        self.written = {k: bytearray(v) for k, v in written.items()}
        self.deleted = set(deleted)
        if world_stack is not None:
            self.world_stack = world_stack

    # --- files written ------------------------------------------------------

    def _path_name(self):
        lin = self.lin(self._reg('ds'), self._reg('dx'))
        raw = bytes(self.mu.mem_read(lin, 128)).split(b'\0')[0].decode('latin1')
        return raw.replace(':', '\\').split('\\')[-1].upper()

    def _dos_write(self):
        """Files the game writes (int 21h AH 3Ch create, 40h write, 41h
        delete) kept in memory, never on disk; an open (3Dh) finds them
        first."""
        ah = self._reg('ax') >> 8
        if ah in (0x3C, 0x3D, 0x41) and os.environ.get('SCREENS_DEBUG'):
            print('dos %02X' % ah, self._path_name(), file=sys.stderr)
        if ah == 0x3D and self._path_name() in self.deleted:
            self._wreg('ax', 2)
            self._set_cf(True)
            return True
        if ah == 0x3C or (ah == 0x3D and self._path_name() in self.written):
            name = self._path_name()
            if ah == 0x3C:
                self.written[name] = bytearray()
                self.deleted.discard(name)
            h = 5
            while h in self.handles:
                h += 1
            self.handles[h] = [self.written[name], 0]
            self._wreg('ax', h)
            self._set_cf(False)
            return True
        if ah == 0x3F and isinstance(self.handles.get(self._reg('bx'), [b''])[0], bytearray):
            f = self.handles[self._reg('bx')]
            n = self._reg('cx')
            data = bytes(f[0][f[1]:f[1] + n])
            f[1] += len(data)
            self.mu.mem_write(self.lin(self._reg('ds'), self._reg('dx')), data)
            self._wreg('ax', len(data))
            self._set_cf(False)
            return True
        if ah == 0x40:
            h = self._reg('bx')
            f = self.handles.get(h)
            if f is None or not isinstance(f[0], bytearray):
                return False
            n = self._reg('cx')
            data = bytes(self.mu.mem_read(self.lin(self._reg('ds'), self._reg('dx')), n))
            f[0][f[1]:f[1] + n] = data
            f[1] += n
            self._wreg('ax', n)
            self._set_cf(False)
            return True
        if ah == 0x41:
            name = self._path_name()
            self.written.pop(name, None)
            self.deleted.add(name)
            self._set_cf(False)
            return True
        if ah == 0x1A:  # the DTA
            self.dta = (self._reg('ds'), self._reg('dx'))
            return True
        if ah == 0x2F:
            self.mu.reg_write(UC_X86_REG_ES, self.dta[0])
            self._wreg('bx', self.dta[1])
            return True
        if ah in (0x4E, 0x4F):  # find first / next
            import fnmatch
            if ah == 0x4E:
                pat = self._path_name()
                if os.environ.get('SCREENS_DEBUG'):
                    print('find', pat, file=sys.stderr)
                names = sorted((set(self.names) - self.deleted) | set(self.written))
                self.found = [n for n in names if fnmatch.fnmatch(n, pat.replace('?', '?'))]
            if not self.found:
                self._wreg('ax', 0x12)
                self._set_cf(True)
                return True
            n = self.found.pop(0)
            size = len(self.written[n]) if n in self.written else os.path.getsize(self.names[n])
            rec = struct.pack('<BHHI', 0x20, 0, 0x2021, size) + n.encode()[:12].ljust(13, b'\0')
            self.mu.mem_write(self.lin(self.dta[0], self.dta[1] + 0x15), rec)
            self._set_cf(False)
            return True
        return False

    # --- interrupts ---------------------------------------------------------

    def _intr(self, mu, intno, _):
        ax = self._reg('ax')
        if intno == 9:  # the BIOS's keyboard handler, run once at the start
            return
        if intno == 0x10:
            if ax >> 8 == 0x0F:
                self._wreg('ax', (ax & 0xFF00) | 3)
            return
        if intno == 0x33:
            if ax == 0:
                self._wreg('ax', 0xFFFF)
                self._wreg('bx', 2)
            elif ax == 3:
                # A click (`clicks`): down at its place at one read, up
                # at the next.
                if self.clicks:
                    # (x, y) the left button; (x, y, 2) the right.
                    x, y, *button = self.clicks[0]
                    self.mouse = (x, y)
                    self.buttons ^= button[0] if button else 1
                    if not self.buttons:
                        self.clicks.pop(0)
                self._wreg('bx', self.buttons)
                self._wreg('cx', self.mouse[0] * 2)
                self._wreg('dx', self.mouse[1])
            elif ax in (5, 6):
                self._wreg('ax', self.buttons)
                self._wreg('bx', 0)
            elif ax == 0x0B:
                self._wreg('cx', self.motion[0])
                self._wreg('dx', self.motion[1])
                self.motion = (0, 0)
            return
        if intno == 0x16:
            self.waiting = ('int16', ax >> 8)
            mu.emu_stop()
            return
        if intno == 0x21 and os.environ.get('SCREENS_DEBUG') == '2':
            print('int21 ax=%04X bx=%04X cx=%04X dx=%04X' % (ax, self._reg('bx'), self._reg('cx'), self._reg('dx')), file=sys.stderr)
        if intno == 0x21 and self._dos_write():
            return
        if intno == 0x21:
            ah = ax >> 8
            if ah == 0x35:
                # The mouse driver's vector set (0000:5384 then takes the
                # driver as present); the others none.
                self._wreg('bx', 0x10 if ax & 0xFF == 0x33 else 0)
                self.mu.reg_write(UC_X86_REG_ES, 0x0070 if ax & 0xFF == 0x33 else 0)
                return
            if ah == 0x25:
                return
            if ah == 0x2C:  # the time
                self._wreg('cx', 0x0C00)
                self._wreg('dx', 0)
                return
            if ah == 0x2A:  # the date
                self._wreg('cx', 1994)
                self._wreg('dx', 0x0101)
                return
            self.unhandled = ('int21', ax >> 8)
        if intno == 0x1A and ax >> 8 == 0:
            self.timer += 1
            self._wreg('cx', self.timer >> 16)
            self._wreg('dx', self.timer)
            return
        super()._intr(mu, intno, _)

    # --- the game started and a save loaded -----------------------------------

    def start(self):
        """The game's start as far as its screens need: 21EE:0000 (the
        page, the fonts, the cursors, the interface's pictures), then slot
        1's saved game loaded from the title (gen_titleload.py's run: the
        load 25FD:0309 into the world loop's entry, 0000:A2AC, with its
        stubs, taken out again after)."""
        self.init_memory()
        assert self.run_screen(0x21EE, 0) is None, 'the start waited'
        # The texts' far pointer's segment the EMS frame's, as the program's
        # main sets it after the start (0000:108B): an INF's @TEXT is copied
        # there (and a level's triggers read their texts from it).
        self.w16(0xCB8A, self.r16(0x9030))
        saved = self.stub()
        # The working slot as the start leaves it with no ARENA variable in
        # the environment (1194:3A72: 40h).
        self.mu.mem_write(self.lin(self.ds, 0x1A80), b'\x40')
        self.mu.mem_write(self.lin(self.ds, 0x1A6C), b'\x40')
        # The title's Load (0000:117E): the game set up (21EE:016C: the
        # towns' lists at 1AB5, 1004, 1008, the people's records at 1042,
        # ...) before the load menu.
        assert self.run_screen(0x21EE, 0x016C) is None, 'the setup waited'
        self.w16(0x1020, EXTRA[1])
        self.w16(0x1022, EXTRA[0])
        self.w16(0x05C8, self.r16(0x05C8) | 4)
        self.mu.mem_write(self.lin(self.ds, 0xC995), b'\x01')
        for at in (0xC3B0, 0xCD2A):
            self.w16(at, 0)
            self.w16(at + 2, 0)
        self.w16(0xCD42, 0x468B)
        self.w16(0xCD44, LOAD_SEG)
        # The load menu from its slot picked (slot 1, 25FD:028D): the slot's
        # files copied to the working slot's (06A1), then the load
        # (0309), into the world loop's entry.
        self._setup([], {'eax': 1})
        self._run(0x25FD, 0x028D, LOAD_SEG, 0xA2AC)
        # The stack the world loop runs on: the interior's run (0000:0936,
        # called at 0E7A) and the outer loop under it.
        self.world_stack = self.stack_kept()
        self.unstub(saved)
        for at in range(0xFFE0, 0x10000, 2):
            self.w16(at, 0)
        self.mu.mem_write(self.lin(self.ds, 0xC995), b'\x00')
        # The mouse's cursor neither drawn nor taken away (0000:5458,
        # 5422): the screens' pages without it.
        self.mu.mem_write(self.lin(LOAD_SEG, 0x5422), b'\xc3')
        self.mu.mem_write(self.lin(LOAD_SEG, 0x5458), b'\xc3')
        # The game's colours on the screen, as a screen's way back sets
        # them (1194:5E87): PAL.COL read into 929E, written to the DAC.
        self.call_far(0, 0x7530, esi=0x1304, edi=0x929E)
        self.call_far(0, 0x9B4E, esi=0x929E)

    def stub(self, location=True):
        """The sounds left out (no driver), and with `location` the
        location record's build (1194:3AE8, as gen_titleload.py's load has
        it): what was there. (25CA:01E6 isn't stubbed: it counts the
        creature slots marked usable, 4000h, and needs nothing; nor is a
        fight's sound, 23BA:01C7, which loads the creature's .VOC into
        bank 15h as `Machine::fight_sound` does.)"""
        from gen_frame import counter
        stubs = [(0x2F9F, 0x001F, counter(0xFFF2, True))]
        if location:
            stubs.append((0x1194, 0x3AE8, counter(0xFFEE, True)))
        saved = []
        for seg, off, code in stubs:
            lin = self.lin(LOAD_SEG + seg, off)
            saved.append((lin, bytes(self.mu.mem_read(lin, len(code)))))
            self.code_write(lin, code)
        return saved

    def unstub(self, saved):
        for lin, b in saved:
            self.code_write(lin, b)

    def code_write(self, lin, b):
        """Code written, and unicorn's translation of the bytes there
        dropped: otherwise a stub taken out again keeps running from its
        cached translation."""
        self.mu.mem_write(lin, b)
        self.mu.ctl_remove_cache(lin, lin + len(b))

    # --- moving between areas ---------------------------------------------------

    def area_ended(self, count=2_000_000_000):
        """The area left (C398 bit 80h, as its way out sets it): the world
        loop's end (0000:A45D) on the world's stack, then the outer loop
        (0000:0E7D on) into the next area, until its world loop is about
        to run its first frame (0000:A2AC; the frames pace on the timer's
        ticks, 0151, which nothing here advances). That world's stack is
        kept for the next."""
        self.w16(0xC398, self.r16(0xC398) | 0x80)
        ss, sp, words = self.world_stack
        self.mu.mem_write(self.lin(ss, sp), words)
        saved = self.stub(location=False)
        # Where the next world loop starts its frames: its stack then.
        at = self.lin(LOAD_SEG, 0xA2AC)
        def kept(mu, addr, size, _):
            self.world_stack = self.stack_kept()
            self.waiting = ('world',)
            mu.emu_stop()
        h = self.mu.hook_add(UC_HOOK_CODE, kept, None, at, at)
        try:
            r = self.run_screen(0, 0xA45D, count=count, stack=(ss, sp))
        finally:
            self.mu.hook_del(h)
            self.unstub(saved)
        return r

    def enter(self, square, count=2_000_000_000):
        """A building's door (its map square, row << 8 | column << 1)
        clicked from the corner of its square: the wall's trigger
        (0000:43B1, PICKED CB92 the square, the player's place C3D0, C3D4
        40h past the corner), then the area left for the building
        (`area_ended`)."""
        col, row = (square & 0xFF) >> 1, square >> 8
        self.w16(0xC3D0, (col << 7) + 0x40)
        self.w16(0xC3D4, (row << 7) + 0x40)
        self.w16(0xCB92, square)
        saved = self.stub(location=False)
        try:
            assert self.run_screen(0, 0x43B1, count=count) is None, 'the door waited'
        finally:
            self.unstub(saved)
        assert self.r16(0xC398) & 0x80, 'the door led nowhere'
        return self.area_ended(count=count)

    def stack_kept(self):
        """SS, SP and the stack's words above SP (to STACK's top): what
        the screens' runs, on the same stack, write over."""
        ss, sp = self.mu.reg_read(UC_X86_REG_SS), self.mu.reg_read(UC_X86_REG_SP)
        top = STACK[1] if ss == STACK[0] else sp + 0x200
        return ss, sp, bytes(self.mu.mem_read(self.lin(ss, sp), top - sp))

    # --- running a screen -----------------------------------------------------

    def run_screen(self, seg, off, regs=None, count=200_000_000, near=False, stack=None):
        """Far-call (or near-call) seg:off (image-relative) until it waits
        for input with nothing left in `script` (the wait returned) or
        returns (None)."""
        self.waiting = None
        if stack is not None:
            # Going on from inside the game, on its own stack: no return.
            self._setup([], regs or {})
            self.mu.reg_write(UC_X86_REG_SS, stack[0])
            self.mu.reg_write(UC_X86_REG_SP, stack[1])
            end = (STUB_SEG, 0)
        elif near:
            self._setup([0xFFF0], regs or {})
            end = (LOAD_SEG + seg, 0xFFF0)
        else:
            self._setup([0, STUB_SEG], regs or {})
            end = (STUB_SEG, 0)
        mu = self.mu
        mu.reg_write(UC_X86_REG_CS, LOAD_SEG + seg)
        mu.reg_write(UC_X86_REG_IP, off)
        self.fault = None
        hooks = []
        self.polls = polls = {}
        for wseg, woff in self.waits:
            def stop(mu, addr, size, w):
                # A wait is the input asked for from the same place a
                # third time: a loop waiting (not a read on the way).
                ss, sp = mu.reg_read(UC_X86_REG_SS), mu.reg_read(UC_X86_REG_SP)
                caller = (w, self.r16(sp, seg=ss), self.r16(sp + 2, seg=ss))
                if (caller[2] - LOAD_SEG, caller[1]) in RELEASE_WAITS:
                    # A wait for the buttons' release returns at its first
                    # poll with none down: called again and again, it
                    # isn't a wait for input.
                    return
                polls[caller] = polls.get(caller, 0) + 1
                if polls[caller] < 3:
                    return
                polls.clear()
                if self.script:
                    act = self.script.pop(0)
                    if os.environ.get('SCREENS_DEBUG'):
                        print('act', act, 'at', caller, file=sys.stderr)
                    if act[0] == 'click':
                        self.clicks.append(act[1:])
                    elif act[0] == 'rclick':
                        self.clicks.append((*act[1:], 2))
                    elif act[0] == 'keys':
                        # Keys pressed and let go: the key interrupt run for
                        # each (`press`) from the wait, then on.
                        self.pressed = act[1]
                        mu.emu_stop()
                    return
                self.waiting = ('call', w)
                mu.emu_stop()
            lin = self.lin(LOAD_SEG + wseg, woff)
            hooks.append(mu.hook_add(UC_HOOK_CODE, stop, (wseg, woff), lin, lin))
        # Code translated before the hooks were added doesn't see them.
        mu.ctl_flush_tb()
        start = self.lin(LOAD_SEG + seg, off)
        self.pressed = None
        try:
            while True:
                mu.emu_start(start, self.lin(*end), count=count)
                if self.pressed is None:
                    break
                keys, self.pressed = self.pressed, None
                cs, ip = mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP)
                self.press(keys)
                start = self.lin(cs, ip)
        finally:
            for h in hooks:
                mu.hook_del(h)
        if self.waiting is not None:
            return self.waiting
        if self.fault is not None:
            raise Fault(self.fault)
        at = (mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP))
        assert at == end, f'stopped at {at[0] - LOAD_SEG:04X}:{at[1]:04X}'
        return None

    # --- the world's frames -------------------------------------------------

    # The world loop's waits for the timer (0000:A428, A43D: [0151] against
    # the frame's start, BA60), each read of which here is a tick: nothing
    # else advances it.
    TICK_WAITS = (0xA428, 0xA43D)

    def world_frames(self, n, until=None, count=2_000_000_000, each=None):
        """The world loop run on from the top of its frame (0000:A2AC, on
        the stack `start` or `area_ended` kept) for `n` frames, the timer
        ticking at each of its reads in the frame's waits (TICK_WAITS), so
        a frame takes the ticks it waits for (2, or 1 on a level with C39A
        bit 400h). It stops at the top of a frame: after `n`, or the first
        at which `until(self)` holds; or at a wait for input (`waits`,
        `script` as `run_screen`'s); or at the world loop's end (0000:A45D:
        the area left), ('ended',). `each(self)` is called at the top of
        every frame (its first the start's). The frames run: the stack
        then kept for the next run. Returns (why, frames)."""
        ss, sp, words = self.world_stack
        self.mu.mem_write(self.lin(ss, sp), words)
        saved = self.stub(location=False)
        done = [0]
        def tick(mu, addr, size, _):
            self.w16(0x0151, (self.r16(0x0151) + 1) & 0xFFFF)
        def top(mu, addr, size, _):
            if each is not None:
                each(self)
            # A wait is the input asked for from one place three times in a
            # frame: each frame asks the mouse once (the controls' 8FC4).
            getattr(self, 'polls', {}).clear()
            if done[0] >= n or (done[0] and until is not None and until(self)):
                self.world_stack = self.stack_kept()
                self.waiting = ('frames',)
                mu.emu_stop()
                return
            done[0] += 1
        def ended(mu, addr, size, _):
            self.waiting = ('ended',)
            mu.emu_stop()
        hooks = [self.mu.hook_add(UC_HOOK_CODE, tick, None, self.lin(LOAD_SEG, o), self.lin(LOAD_SEG, o))
                 for o in self.TICK_WAITS]
        hooks.append(self.mu.hook_add(UC_HOOK_CODE, top, None, self.lin(LOAD_SEG, 0xA2AC), self.lin(LOAD_SEG, 0xA2AC)))
        hooks.append(self.mu.hook_add(UC_HOOK_CODE, ended, None, self.lin(LOAD_SEG, 0xA45D), self.lin(LOAD_SEG, 0xA45D)))
        try:
            r = self.run_screen(0, 0xA2AC, count=count, stack=(ss, sp))
        finally:
            for h in hooks:
                self.mu.hook_del(h)
            self.unstub(saved)
        return r, done[0]

    def keys_held(self, scans, down=True):
        """Keys pressed (or with `down` false let go) and left so: each
        make (or break) code given to the key interrupt alone, as `press`
        gives it, on the stack under the world's."""
        mu = self.mu
        ss, sp, _ = self.world_stack
        for scan in scans:
            code = scan if down else scan | 0x80
            at = (sp - 0x200) & 0xFFFF
            mu.mem_write(self.lin(ss, at), struct.pack('<H', code | code << 8))
            self._setup([], {})
            mu.reg_write(UC_X86_REG_SS, ss)
            mu.reg_write(UC_X86_REG_SP, at)
            # (The code segment set too: a set-up's call into another
            # segment leaves CS there, and the start is taken in it.)
            mu.reg_write(UC_X86_REG_CS, LOAD_SEG)
            mu.emu_start(self.lin(LOAD_SEG, 0x71BB), self.lin(LOAD_SEG, 0x72B2), count=100_000)
            where = (mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP))
            assert where == (LOAD_SEG, 0x72B2), f'the key interrupt stopped at {where}'

    # The registers a key's interrupt leaves as it found them.
    KEPT = ('ax', 'bx', 'cx', 'dx', 'si', 'di', 'bp', 'sp', 'ss', 'ds', 'es', 'fs', 'gs', 'eflags', 'cs', 'ip')

    def press(self, scans):
        """Each scan code given to the key interrupt (0000:718A from its
        port's read, 71BB, to 72B2: the PIC's end left out, as
        gen_lineedit.py runs it) pressed and then let go, on the stack
        under the game's, the game's registers kept round it."""
        mu = self.mu
        kept = {r: mu.reg_read(globals()['UC_X86_REG_' + r.upper()]) for r in self.KEPT}
        for scan in scans:
            for code in (scan, scan | 0x80):
                sp = (kept['sp'] - 0x200) & 0xFFFF
                mu.mem_write(self.lin(kept['ss'], sp), struct.pack('<H', code | code << 8))
                for r, v in dict(ss=kept['ss'], sp=sp, ds=self.ds, es=self.ds).items():
                    mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)
                mu.reg_write(UC_X86_REG_CS, LOAD_SEG)
                mu.reg_write(UC_X86_REG_IP, 0x71BB)
                mu.emu_start(self.lin(LOAD_SEG, 0x71BB), self.lin(LOAD_SEG, 0x72B2), count=100_000)
                at = (mu.reg_read(UC_X86_REG_CS), mu.reg_read(UC_X86_REG_IP))
                assert at == (LOAD_SEG, 0x72B2), f'the key interrupt stopped at {at}'
        for r, v in kept.items():
            mu.reg_write(globals()['UC_X86_REG_' + r.upper()], v)

    def page(self):
        """The 320 x 200 page the game draws into (A900)."""
        return bytes(self.mu.mem_read(self.lin(self.r16(PAGE_SEG_FIELD), 0), 64000))

    def vga(self):
        return bytes(self.mu.mem_read(0xA0000, 64000))
