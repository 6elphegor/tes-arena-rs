"""What of the original is ported: the original's call graph against the
addresses the port claims.

Usage: coverage.py ACD_unpacked.EXE REPO_DIR OUT.md

Needs capstone (the harness's venv). Run it from anywhere; it reads the
unpacked EXE (never committed) and the port's sources, and writes a
Markdown report holding only addresses, sizes, counts, file names and
short descriptions.

The graph
- Code segments are the segments far calls name (9A off seg).
- Roots: the program's entry (0000:0000), the world and outer loops, and
  every far pointer constant in the image whose segment word the
  relocation table fixes up (far calls, far pointers written as
  immediates, far pointer tables in DGROUP or code), when it points at
  a code segment and decodes.
- From each routine entry a recursive descent follows jumps and
  conditional jumps within it, and takes near calls (E8) and direct far
  calls (9A) as new routine entries. A jump to another routine's entry
  ends the walk there (a tail call).
- Indirect calls and jumps through [reg+disp] are resolved where the
  table holds a run of offsets that decode as code in the same segment:
  up to a `cmp reg, n` bound just before, or without one, while each
  entry starts like a routine (a known one, or code right after a
  return or padding). Calls through a DGROUP word or dword ([N]) take
  its value in the image and every immediate the code stores to it
  (`mov word/dword ptr [N], imm`); discovery repeats until it stops
  growing, since stores found later resolve calls walked earlier. The
  rest are counted (distinct sites) as unresolved.

Coverage
- Every SEG:OFF in src/*.rs is a claim. One in a doc comment of a trait
  method of a `...Hooks` trait is a hook (left to the front end); any
  other one in src/ is ported (a routine's entry, or a piece of one
  described inline); one in view/src is the front end's own
  re-implementation (presentation).
- A routine is ported when a ported claim falls on its entry or inside
  its instruction bytes; else a hook, else the view's, else not covered.

Class of an uncovered routine (a guess from its instructions): DOS I/O
(int 21h), BIOS (int 10h/16h/33h/1Ah), hardware (in/out), drawing
(segment A000h, or rep movs/stos to ES with no other signs), else game
state or undetermined.
"""
import collections
import os
import re
import struct
import sys

import capstone

DGROUP = 0x3BA0

# What a routine is, where it was looked at by hand (the frontier's top).
NOTES = {
    '0000:5104': 'rain/snow drops drawn over the view (F81 bit 2, not full daylight) [drawing]',
    '0000:D0A1': "the drawing's walk over floor squares (deep floors CCCE) [renderer]",
    '3159:06A0': "window library: a window's handler called through es:[di+33] [UI]",
    '3159:0F2A': 'window library: the menu/list window 1194:020E builds [UI]',
    '0000:CBFF': "an object's corners for the renderer's lists (A872..A888) [renderer]",
    '1194:5C34': 'a text window drawn (buffer 1044) [UI]',
    '1194:2A69': "creation's attribute screen loop around the ported rolls/checks (2AC7, 2CD0, 2D28) [UI]",
    '0000:965A': 'message box drawn (from 1194:59CC) [UI]',
    '0000:965E': 'message box drawn [UI]',
    '1194:5CF4': 'window library state (B7C4) [UI]',
    '0FFD:0C49': 'sound/music driver interface (segment 0FFD) [driver]',
    '0FFD:0821': 'sound/music driver interface [driver]',
    '0FFD:0AF7': 'sound/music driver interface [driver]',
    '1194:6001': 'waiting for a key or click (8F92/8F94 mouse, AFF9 key) [input]',
    '1194:5FC0': 'waiting for input [input]',
    '1194:5E21': 'waiting for input [input]',
    '1194:5536': 'waiting for a key or click (710E, 8FC4, 2F26:02A1) [input]',
    '1194:17D8': "the inventory screen's list drawn [UI]",
    '1194:223A': "the inventory screen's list drawn [UI]",
    '0000:E41A': 'the sun and moons placed and drawn (clear weather, time 100h..300h) [sky drawing]',
    '2F26:00F2': 'keyboard/mouse reading (key buffer 8BEC) [input]',
    '2F26:0445': 'key handling [input]',
    '2F26:0214': 'key state [input]',
    '2F26:04B0': 'key handling [input]',
    '1194:5CD9': 'EMS bank [1030] mapped (the port has ems_map) [infrastructure]',
    '0000:56C8': 'an image drawn, clipped [drawing]',
    '0000:56CC': 'an image drawn, clipped (A836..A83C) [drawing]',
    '0000:77EC': 'waiting for input [input]',
    '0000:36B6': 'a text box drawn [UI]',
    '0000:DC67': 'renderer',
    '0000:CD30': 'renderer',
    '0000:CD34': 'renderer (CCF8 wall sides, CB37)',
    '0000:CFBA': 'map flats placed for the drawing (D806, C38E) [renderer]',
    '1194:4943': "a file's 6-byte records searched for a two-byte key [file I/O]",
    '25FD:05A5': "a save slot's four files checked (list at 5F5A) [file I/O]",
    '222C:0185': "the class quiz's scroll screen (question.txt, scroll01.dfa) [UI]",
    '25FD:0718': 'the load/save screen (loadsave.img, names.dat) [UI]',
    '1194:157E': "the character sheet's values shown (12E6/12F5 draw, DE1) [UI]",
    '1194:12E6': 'a value drawn at (5Ch, 71h) [UI]',
    '0000:4F72': 'the walls plane copied 80h x 80h into a buffer (A900) for a map image [presentation?]',
    '24FA:00FF': "people's equipment step: lock level 13CC from the corpse record at [4F00] (CORPSE_OFF), then the flagged parts [game state; ported inline: loot.rs, creature.rs]",
    '2AA1:0556': "a bare-handed hit's damage from the gauntlets (1194:7449) and the table at 8270h [game state; ported inline: combat.rs FIST_DAMAGE, 2AA1:04A5]",
    '0000:E084': "the heading's slope, the body of 0000:E06A [game math; ported inline: travel.rs travel_heading]",
    '1194:8960': 'the date from the clock 5CC (day of the year, 7E900h minutes a year) [game state; ported inline: text.rs, shops.rs, use_item.rs]',
    '25CA:0233': "a fight's start: with 5C8 bit 1000h a free entry in the list at 622 [game state; ported inline: encounter.rs (25CA:006A)]",
    '2CDD:0658': 'a value at +66 changed and clamped to the maximum at +68, which the cap overwrites [game state; ported inline: spell.rs 2CDD:0604]',
    '2CDD:0691': 'the same for another pair [game state; ported inline: spell.rs 2CDD:0604]',
    '2CDD:06CC': 'the same for another pair [game state; ported inline: spell.rs 2CDD:0604]',
    # Classified by hand in the triage after the first audit.
    '0000:85AE': "the 3D polygon set-up: points projected (AF7F..AF85), outcodes, then the clipper [renderer]",
    '0000:8755': "a polygon clipped against one plane (Sutherland-Hodgman over AF74/AF78/AF7A, outcodes AF76/AF77) [renderer]",
    '0000:8950': "a point's outcode against the view's planes [renderer]",
    '0000:4D2F': "the keys held (652, 12 of them) drawn as icons [drawing]",
    '0000:A561': "the renderer's buffers allocated (CD86, CD82, CD7E, CD76/CD7A) and its set-up (AAA2) [renderer]",
    '0000:3796': "the automap's centre (126C/126E) kept inside the map (C357/C359) [UI: automap screen 0000:2CFE]",
    '0000:B5A5': "a quad's corners flagged inside a rectangle (+12h bits F000h) [renderer]",
    '292B:0530': "eight images of the list at 7BAA drawn (FD5 flags) [drawing]",
    '242A:0031': "CITYDATA's province records checked; a bad one shows the text at 4A1E [I/O check]",
    '0000:6D76': "the formatter's fraction digits (three, after a '.') [text formatting]",
    '1194:E539': "a place's name: the name list 3276:3CB6 then a suffix [game text; ported inline: talk.rs names_a]",
    '2B39:17B8': "a 3D object's file loaded and its points projected [renderer]",
    '2F26:0026': "the CyberMan's turn (C3DA) [input hardware]",
    '2F26:003E': "the CyberMan's move (CCBA, 5DE) [input hardware]",
    '0000:98A0': "a text's lines laid out (colour codes 0Ch, tabs 09h) [text drawing]",
    '1194:A206': "the logbook's quest lines listed (records at 0DCA, 0E6E) [UI]",
    '0000:CD6C': "a door's wall drawn (C398 bit 1000h set for the hit test near the player) [renderer]",
    '0000:DDAA': "a sprite's shading row from the lights near it (A836) [renderer]",
    '1194:1988': "the character sheet's status texts [UI]",
}
DATA = 0x3276


def load(path):
    b = open(path, 'rb').read()
    h = struct.unpack('<14H', b[:28])
    img = b[h[4] * 16:]
    relocs = [seg * 16 + off for off, seg in struct.iter_unpack('<2H', b[h[12]:h[12] + 4 * h[3]])]
    return img, relocs


class Graph:
    def __init__(self, img, relocs):
        self.img = img
        self.relocs = set(relocs)
        self.md = capstone.Cs(capstone.CS_ARCH_X86, capstone.CS_MODE_16)
        self.cache = {}
        # Code segments: far call targets' segments.
        segs = set()
        for p in relocs:
            if p >= 3 and img[p - 3] == 0x9A:
                segs.add(struct.unpack_from('<H', img, p)[0])
        self.segs = sorted(s for s in segs if s * 16 < len(img) and s not in (DGROUP, DATA))
        self.entries = {}          # (seg, off) -> set of caller entries
        self.unresolved = 0
        self.unresolved_sites = set()
        self.tables = 0
        # Immediates stored to DGROUP words and dwords (`mov word/dword
        # ptr [N], imm`), for the pointers called through them.
        self.ptr_writes = collections.defaultdict(set)
        self.known = set()

    def seg_end(self, seg):
        later = [s for s in self.segs if s > seg]
        return min((later[0] * 16 if later else len(self.img)), seg * 16 + 0x10000, len(self.img))

    def insn(self, seg, off):
        key = (seg, off)
        if key not in self.cache:
            lin = seg * 16 + off
            code = self.img[lin:lin + 15]
            self.cache[key] = next(self.md.disasm(code, off), None)
        return self.cache[key]

    def decodes(self, seg, off, n=4):
        """A plausible code start: n instructions decode, none of them
        the zero-filled `add [bx+si], al` run."""
        o = off
        for _ in range(n):
            if seg * 16 + o >= self.seg_end(seg):
                return False
            i = self.insn(seg, o)
            if i is None or i.bytes[:2] == b'\x00\x00' or i.mnemonic in ('hlt', 'int3', '(bad)'):
                return False
            if i.mnemonic in ('ret', 'retf', 'iret', 'jmp'):
                return True
            o = (o + i.size) & 0xFFFF
        return True

    def start_like(self, seg, off):
        """A routine could start at seg:off: a known one, or code right
        after a return or a padding nop."""
        if (seg, off) in self.known:
            return True
        lin = seg * 16 + off
        return lin > 0 and self.img[lin - 1] in (0xC3, 0xCB, 0xC2, 0xCA, 0xCF, 0x90) and self.decodes(seg, off)

    def pointer(self, seg, i):
        """Targets of `call word ptr [N]` (near, in this segment) and
        `lcall [N]` (far) through a DGROUP variable: its value in the
        image and the immediates the code stores to it."""
        m = re.fullmatch(r'(?:word ptr |dword ptr )?\[(0x[0-9a-f]+)\]', i.op_str)
        if not m:
            return []
        n = int(m.group(1), 16)
        g = DGROUP * 16 + n
        out = set()
        if i.mnemonic == 'call':
            cands = set(self.ptr_writes[(n, 'w')])
            if g + 2 <= len(self.img):
                cands.add(struct.unpack_from('<H', self.img, g)[0])
            out = {(seg, t) for t in cands if self.start_like(seg, t)}
        else:
            cands = set()
            for v in self.ptr_writes[(n, 'd')]:
                cands.add((v >> 16, v & 0xFFFF))
            for o in self.ptr_writes[(n, 'w')]:
                for sg in self.ptr_writes[(n + 2, 'w')]:
                    cands.add((sg, o))
            if g + 4 <= len(self.img):
                o, sg = struct.unpack_from('<HH', self.img, g)
                cands.add((sg, o))
            out = {(sg, o) for sg, o in cands if sg in self.segs and self.start_like(sg, o)}
        return sorted(out)

    def table(self, seg, i, seen):
        """Targets of `call/jmp word ptr [reg+disp]` with a table base
        (disp >= 100h) and a bound (`cmp reg, n` in the 40 bytes before):
        the first n+1 words, kept while they decode as code in the same
        segment (cs: or ds: tables). Anything else is left unresolved."""
        m = re.search(r'word ptr (cs:)?\[([a-z]+)(?: \+ ([a-z]+))?(?: \+ (0x[0-9a-f]+|\d+))?\]', i.op_str)
        if not m or not m.group(4):
            return []
        disp = int(m.group(4), 0)
        if disp < 0x100:
            return []
        bound = None
        for o in sorted((o for o in seen if i.address - 40 <= o < i.address), reverse=True):
            c = seen[o]
            cm = re.match(r'([a-z]+), (0x[0-9a-f]+|\d+)$', c.op_str)
            if c.mnemonic == 'cmp' and cm:
                bound = int(cm.group(2), 0)
                break
        unbounded = bound is None
        if unbounded:
            bound = 63
        elif bound > 64:
            return []
        base = seg * 16 if m.group(1) else DGROUP * 16
        out = []
        for k in range(bound + 1):
            p = base + disp + 2 * k
            if p + 2 > len(self.img):
                break
            t = struct.unpack_from('<H', self.img, p)[0]
            if not self.decodes(seg, t) or (unbounded and not self.start_like(seg, t)):
                break
            out.append(t)
        return out if len(out) >= 2 else []

    def walk(self, seg, off, stop_at_entries):
        """The instructions of the routine at seg:off, and its callees."""
        seen = {}
        callees = set()
        todo = [off]
        end = self.seg_end(seg)
        while todo:
            o = todo.pop()
            while True:
                if o in seen or seg * 16 + o >= end:
                    break
                if o != off and (seg, o) in stop_at_entries:
                    callees.add((seg, o))
                    break
                i = self.insn(seg, o)
                if i is None:
                    break
                seen[o] = i
                b, mn = i.bytes, i.mnemonic
                if mn == 'mov':
                    pw = re.fullmatch(r'(word|dword) ptr \[(0x[0-9a-f]+)\], (0x[0-9a-f]+|\d+)', i.op_str)
                    if pw:
                        self.ptr_writes[(int(pw.group(2), 16), pw.group(1)[0])].add(int(pw.group(3), 0))
                nxt = (o + i.size) & 0xFFFF
                op = b[0]
                prefix = 0
                while op in (0x26, 0x2E, 0x36, 0x3E, 0x64, 0x65, 0x66, 0x67, 0xF2, 0xF3) and prefix < len(b) - 1:
                    prefix += 1
                    op = b[prefix]
                if mn in ('ret', 'retf', 'iret', 'hlt'):
                    break
                if op == 0x9A and len(b) - prefix >= 5:
                    t_off, t_seg = struct.unpack_from('<HH', b, prefix + 1)
                    if t_seg in self.segs:
                        callees.add((t_seg, t_off))
                    o = nxt
                    continue
                if op == 0xEA and len(b) - prefix >= 5:
                    t_off, t_seg = struct.unpack_from('<HH', b, prefix + 1)
                    if t_seg in self.segs:
                        callees.add((t_seg, t_off))
                    break
                if mn == 'call':
                    if i.op_str.startswith('0x'):
                        callees.add((seg, int(i.op_str, 16) & 0xFFFF))
                    elif self.pointer(seg, i):
                        self.tables += 1
                        callees.update(self.pointer(seg, i))
                    else:
                        t = self.table(seg, i, seen)
                        if t:
                            self.tables += 1
                            callees.update((seg, x) for x in t)
                        else:
                            self.unresolved_sites.add((seg, o, mn, i.op_str))
                    o = nxt
                    continue
                if mn == 'lcall':
                    t = self.pointer(seg, i)
                    if t:
                        self.tables += 1
                        callees.update(t)
                    else:
                        self.unresolved_sites.add((seg, o, mn, i.op_str))
                    o = nxt
                    continue
                if mn == 'jmp':
                    if i.op_str.startswith('0x'):
                        t = int(i.op_str, 16) & 0xFFFF
                        if (seg, t) in stop_at_entries and t != off:
                            callees.add((seg, t))
                        else:
                            todo.append(t)
                    else:
                        t = self.table(seg, i, seen)
                        if t:
                            self.tables += 1
                            todo.extend(t)
                        else:
                            self.unresolved_sites.add((seg, o, mn, i.op_str))
                    break
                if mn == 'ljmp':
                    self.unresolved_sites.add((seg, o, mn, i.op_str))
                    break
                if mn.startswith('j') or mn.startswith('loop') or mn == 'jcxz':
                    if i.op_str.startswith('0x'):
                        todo.append(int(i.op_str, 16) & 0xFFFF)
                o = nxt
        return seen, callees

    def build(self, roots):
        # Discovery, again until it stops growing: the pointers stored by
        # code found later resolve calls walked earlier.
        size = -1
        for _ in range(8):
            entries = {}
            todo = list(roots)
            while todo:
                r = todo.pop()
                if r in entries or r[0] not in self.segs:
                    continue
                entries[r] = set()
                _, callees = self.walk(r[0], r[1], set())
                for c in callees:
                    todo.append(c)
            self.known = set(entries)
            if len(entries) == size:
                break
            size = len(entries)
        # Bodies with the full entry set (tail calls end a body), callers;
        # the indirect sites counted on this pass only.
        self.unresolved = 0
        self.unresolved_sites = set()
        self.tables = 0
        stop = set(entries)
        self.bodies = {}
        self.callees = {}
        for e in entries:
            seen, callees = self.walk(e[0], e[1], stop)
            self.bodies[e] = seen
            self.callees[e] = callees
            for c in callees:
                if c in entries:
                    entries[c].add(e)
        self.entries = entries
        self.unresolved = len(self.unresolved_sites)


def far_pointer_roots(g):
    roots = set()
    for p in g.relocs:
        if p < 2 or p + 2 > len(g.img):
            continue
        seg = struct.unpack_from('<H', g.img, p)[0]
        off = struct.unpack_from('<H', g.img, p - 2)[0]
        if seg in g.segs and g.decodes(seg, off):
            roots.add((seg, off))
    return roots


CLAIM = re.compile(r'\b([0-9A-F]{4}):([0-9A-F]{4})\b')
# A bare offset in a comment: four upper-case hex digits standing alone
# (not 0x.., not part of a longer number, not followed by h).
BARE = re.compile(r'(?<![0-9A-Za-z_:])([0-9A-F]{4})(?![0-9A-Za-z_:])')


class Claims(collections.defaultdict):
    """The claims by address, with the bare offsets still to resolve."""

    def __init__(self):
        super().__init__(set)
        self.bare = []


def claims(repo):
    """(seg, off) -> set of kinds: 'ported', 'hook', 'view'; where; and
    the entries the port names: the first address of a `///` block right
    above a `fn` in src/ outside the hooks traits."""
    # Bare offsets in comments ("8575: the menu") are kept in `out.bare`,
    # resolved later against the routine entries of the segments the same
    # file names.
    out = Claims()
    where = collections.defaultdict(set)
    named_entries = set()
    for sub, kind_default in (('src', 'ported'), ('view/src', 'view')):
        d = os.path.join(repo, sub)
        for name in sorted(os.listdir(d)):
            if not name.endswith('.rs'):
                continue
            in_hooks = False
            depth = 0
            hooks_depth = None
            block_first = None
            segs = set()
            bare = []
            for line in open(os.path.join(d, name), encoding='utf-8'):
                s = line.strip()
                if s.startswith('///'):
                    if block_first is None:
                        m0 = CLAIM.search(s)
                        block_first = (int(m0.group(1), 16), int(m0.group(2), 16)) if m0 and s.index(m0.group(0)) < 8 else False
                elif s.startswith('#['):
                    pass
                else:
                    if block_first and kind_default == 'ported' and not in_hooks and re.match(r'(pub(\(crate\))? )?fn ', s):
                        named_entries.add(block_first)
                    block_first = None
                if re.match(r'pub(\(crate\))? trait \w*Hooks\b', s):
                    in_hooks = True
                    hooks_depth = depth
                depth += line.count('{') - line.count('}')
                if in_hooks and hooks_depth is not None and depth <= hooks_depth and '}' in line:
                    in_hooks = False
                kind = kind_default
                if kind_default == 'ported' and in_hooks and s.startswith('///'):
                    kind = 'hook'
                for m in CLAIM.finditer(line):
                    a = (int(m.group(1), 16), int(m.group(2), 16))
                    out[a].add(kind)
                    where[a].add(f'{sub}/{name}')
                    segs.add(a[0])
                if '//' in line:
                    comment = line[line.index('//'):]
                    for m in BARE.finditer(CLAIM.sub(' ', comment)):
                        bare.append((int(m.group(1), 16), kind))
            out.bare.extend((segs, off, kind, f'{sub}/{name}') for off, kind in bare)
    return out, where, named_entries


def resolve_bare(cl, g):
    """The routine entries named only by a bare offset in a comment of a
    file that names their segment: address -> files. Not claims: such a
    mention is as often a routine left out ("stubbed: 9466, 545C") or a
    variable as one ported inline, so they are listed for checking."""
    out = collections.defaultdict(set)
    for segs, off, kind, f in cl.bare:
        for sg in segs:
            a = (sg, off)
            if a in g.entries and not cl.get(a):
                out[a].add(f)
    return out


def classify(g, e):
    body = g.bodies[e]
    text = ' '.join(f'{i.mnemonic} {i.op_str}' for i in body.values())
    tags = []
    if re.search(r'\bint 0x21\b', text):
        tags.append('DOS I/O')
    if re.search(r'\bint (0x10|0x16|0x33|0x1a)\b', text):
        tags.append('BIOS')
    if re.search(r'\b(in|out|insb|outsb|insw|outsw) ', text + ' '):
        tags.append('hardware')
    if re.search(r'0xa000\b', text):
        tags.append('drawing')
    elif re.search(r'rep (movs|stos)', text) and not tags:
        tags.append('drawing?')
    return tags


FILE = re.compile(rb'^[A-Za-z0-9_@$]{1,8}\.[A-Za-z0-9$@]{1,3}\x00')


def describe(g, e):
    """Files named, DGROUP variables touched (a few), callee count."""
    files, vars_ = [], collections.Counter()
    for o, i in g.bodies[e].items():
        # An immediate the relocation table fixes up is a segment, not an
        # offset.
        if e[0] * 16 + o + i.size - 2 in g.relocs:
            continue
        for m in re.finditer(r'0x([0-9a-f]{3,4})\b', i.op_str):
            v = int(m.group(1), 16)
            p = DGROUP * 16 + v
            if 'ptr [' in i.op_str and f'[0x{m.group(1)}]' in i.op_str:
                vars_[v] += 1
            elif i.mnemonic == 'mov' and re.match(r'(si|di|dx|bx), 0x', i.op_str) and p < len(g.img) and g.img[p - 1] == 0:
                fm = FILE.match(g.img[p:p + 13])
                if fm:
                    files.append(fm.group(0)[:-1].decode().lower())
    bits = []
    if files:
        bits.append('files ' + ', '.join(sorted(set(files))[:3]))
    if vars_:
        bits.append('vars ' + ' '.join(f'{v:04X}' for v, _ in vars_.most_common(4)))
    return '; '.join(bits)


def main():
    exe, repo, out = sys.argv[1:4]
    img, relocs = load(exe)
    g = Graph(img, relocs)
    named = {(0x0000, 0x0000): 'program entry', (0x0000, 0xA240): 'world loop',
             (0x0000, 0x0ED5): 'outer loop', (0x0000, 0x0CAD): 'outer loop (area run)',
             (0x2826, 0x0002): 'input step', (0x0000, 0x10C9): 'start-up / main menu'}
    cl, where, port_entries = claims(repo)
    roots = set(named) | far_pointer_roots(g)
    g.build(roots)
    graph_only = set(g.entries)
    # The port's own named entries the graph didn't reach (code reached
    # through pointers set at run time): added as roots, and counted.
    extra = {e for e in port_entries if e[0] in g.segs and e not in graph_only and g.decodes(*e)}
    g.build(roots | extra)
    added = len(g.entries) - len(graph_only)
    bare = resolve_bare(cl, g)

    by_seg = collections.defaultdict(list)
    for (s, o) in g.entries:
        by_seg[s].append(o)
    for s in by_seg:
        by_seg[s].sort()

    def routine_of(a):
        s, o = a
        if (s, o) in g.entries:
            return (s, o)
        for e_off in reversed(by_seg.get(s, [])):
            if e_off <= o:
                body = g.bodies[(s, e_off)]
                if o in body:
                    return (s, e_off)
                break
        # Inside any routine's instructions in that segment.
        for e_off in by_seg.get(s, []):
            if o in g.bodies[(s, e_off)]:
                return (s, e_off)
        return None

    status = {}
    unplaced = collections.Counter()
    for a, kinds in cl.items():
        r = routine_of(a)
        if r is None:
            if a[0] in g.segs:
                unplaced['ported' if 'ported' in kinds else ('hook' if 'hook' in kinds else 'view')] += 1
            continue
        cur = status.get(r)
        for k in ('ported', 'hook', 'view'):
            if k in kinds:
                if cur is None or ['ported', 'hook', 'view'].index(k) < ['ported', 'hook', 'view'].index(cur):
                    cur = k
        status[r] = cur

    size = {e: sum(i.size for i in g.bodies[e].values()) for e in g.entries}
    total = len(g.entries)
    covered = [e for e in g.entries if status.get(e) == 'ported']
    hooks = [e for e in g.entries if status.get(e) == 'hook']
    viewed = [e for e in g.entries if status.get(e) == 'view']
    rest = [e for e in g.entries if e not in status]
    rest_set = set(rest)
    cls = {e: classify(g, e) for e in rest}
    # Reached only through code the port leaves to the front end (a hook's
    # or the view's routine, or presentation below them): the front end's.
    front = {e for e in g.entries if status.get(e) in ('hook', 'view')} | {e for e in rest if cls[e]}
    changed = True
    while changed:
        changed = False
        for e in rest:
            if e in front:
                continue
            callers = g.entries[e]
            if callers and all(c in front for c in callers):
                front.add(e)
                if not cls[e]:
                    cls[e] = ['under a hook']
                changed = True
    game = [e for e in rest if not cls[e]]
    pres = [e for e in rest if cls[e]]

    # Frontier: uncovered game-state routines called from covered ones;
    # weight: the uncovered code reachable from them.
    def subtree(e):
        seen, todo, n = set(), [e], 0
        while todo:
            x = todo.pop()
            if x in seen or x in status:
                continue
            seen.add(x)
            n += size.get(x, 0)
            todo.extend(c for c in g.callees.get(x, ()) if c in g.entries)
        return n, len(seen)
    frontier = []
    for e in game:
        callers = g.entries[e]
        if any(status.get(c) == 'ported' for c in callers):
            frontier.append((subtree(e), e))
    frontier.sort(reverse=True)

    def fmt(e):
        return f'{e[0]:04X}:{e[1]:04X}'

    claims_in = collections.defaultdict(set)
    for a, files in where.items():
        r = routine_of(a)
        if r is not None:
            claims_in[r] |= {f.split('/')[-1] for f in files}

    def context(e):
        files = set()
        for c in g.entries[e]:
            files |= claims_in.get(c, set())
        return ', '.join(sorted(files)[:3])

    def note(e):
        return NOTES.get(fmt(e), '')

    # Likely ported inline: an uncovered routine whose distinctive
    # constants (immediates and addresses >= 100h, not segments) show as
    # hex literals in its callers' port files.
    texts = {}

    def port_text(f):
        if f not in texts:
            for sub in ('src', 'view/src'):
                q = os.path.join(repo, sub, f)
                if os.path.exists(q):
                    texts[f] = open(q, encoding='utf-8').read().lower()
                    break
            else:
                texts[f] = ''
        return texts[f]

    def constants(e):
        out = set()
        for o, i in g.bodies[e].items():
            if e[0] * 16 + o + i.size - 2 in g.relocs:
                continue
            for m in re.finditer(r'0x([0-9a-f]{3,8})\b', i.op_str):
                v = int(m.group(1), 16)
                if v >= 0x100 and v != DGROUP:
                    out.add(v)
        return out

    def inline(e):
        files = set()
        for c in g.entries[e]:
            files |= claims_in.get(c, set())
        if not files:
            return False
        cs = constants(e)
        if not cs:
            return False
        text = ' '.join(port_text(f) for f in files)
        found = sum(1 for v in cs if re.search(r'0x0*%x\b' % v, text))
        return found >= min(2, len(cs))

    likely = {e for e in game if inline(e)}
    open_game = [e for e in game if e not in likely]
    frontier = [f for f in frontier if f[1] not in likely]

    def kb(es):
        return sum(size[e] for e in es)

    lines = []
    w = lines.append
    w('# What of the original is ported')
    w('')
    w('Generated by `tools/coverage.py` (run it with the harness venv\'s python:')
    w('`coverage.py re/ACD_unpacked.EXE . docs/COVERAGE.md`). It holds only')
    w('addresses, sizes, counts, file names and short descriptions.')
    w('')
    w('## How it is counted')
    w('')
    w('The original\'s call graph is built from the unpacked ACD.EXE by recursive')
    w('descent from the program entry, the world and outer loops, and every far')
    w('pointer constant the relocation table fixes up (far calls, far pointers')
    w('written as immediates or stored in tables), following near and far calls;')
    w('indirect calls through tables are resolved where the table reads as a run')
    w('of code offsets (bounded by a `cmp` before the call, or each entry')
    w('starting like a routine), and calls through a DGROUP pointer [N] by its')
    w('value in the image and the immediates the code stores to it.')
    w('The port\'s claims are the SEG:OFF addresses in its')
    w('sources: in a `...Hooks` trait\'s docs a hook (left to the front end),')
    w('elsewhere in `src/` ported, in `view/src/` the front end\'s own version.')
    w('A routine counts as ported when a ported address falls on its entry or')
    w('inside its instructions.')
    w('')
    w('Limits, honestly: indirect calls the heuristic could not resolve are')
    w(f'not followed ({g.unresolved} sites; {g.tables} table sites resolved), so')
    w('code reached only through pointers set at run time (hooks, handler')
    w('tables built in code, overlay-style dispatch) can be missing from')
    w('"reachable"; routine sizes count decoded instruction bytes only (data')
    w('inside code is not counted); a routine "ported" may be ported in part')
    w('(its doc names what is left as hooks); addresses claimed in the port')
    w(f'that fall in no routine the graph found: {sum(unplaced.values())}')
    w(f'({dict(unplaced)}), a measure of what the graph misses; and routines the')
    w(f'port names as entries that the graph alone did not reach, added as roots:')
    w(f'{added} routines came in with them.')
    w('')
    w('"Likely ported inline" marks an uncovered routine whose distinctive')
    w('constants (immediates and addresses of 100h and more) show as hex literals')
    w('in the port files that claim its callers: the port implements many')
    w('callees inside their caller\'s port without naming their address. It is')
    w('a heuristic both ways.')
    w('')
    w('"Under a hook" marks an uncovered routine whose callers are all routines')
    w('the port leaves to the front end (hooks, the view\'s own versions, I/O or')
    w('drawing): the renderer\'s internals mostly land there.')
    w('')
    w('## Summary')
    w('')
    w('| | routines | bytes |')
    w('|---|---:|---:|')
    w(f'| reachable | {total} | {kb(g.entries)} |')
    w(f'| ported (in whole or part) | {len(covered)} | {kb(covered)} |')
    w(f'| hooks (front end\'s) | {len(hooks)} | {kb(hooks)} |')
    w(f'| the view\'s own version | {len(viewed)} | {kb(viewed)} |')
    w(f'| not covered: game state or undetermined | {len(game)} | {kb(game)} |')
    w(f'| &nbsp;&nbsp;of which likely ported inline in a caller | {len(likely)} | {kb(likely)} |')
    w(f'| &nbsp;&nbsp;of which open | {len(open_game)} | {kb(open_game)} |')
    w(f'| not covered: presentation / I/O / hardware | {len(pres)} | {kb(pres)} |')
    w('')
    seg_rows = collections.defaultdict(lambda: [0, 0, 0, 0])
    for e in g.entries:
        r = seg_rows[e[0]]
        r[0] += 1
        r[1] += size[e]
        if status.get(e) in ('ported', 'hook', 'view'):
            r[2] += 1
            r[3] += size[e]
    w('By segment:')
    w('')
    w('| segment | routines | bytes | covered | covered bytes |')
    w('|---|---:|---:|---:|---:|')
    for s in sorted(seg_rows):
        r = seg_rows[s]
        w(f'| {s:04X} | {r[0]} | {r[1]} | {r[2]} | {r[3]} |')
    w('')
    w('## The frontier: uncovered game-state routines the port reaches')
    w('')
    w('Routines not covered, with no sign of I/O or drawing, called from a')
    w('ported routine (where the port stops, stubs or leaves a hook); ranked by')
    w('the uncovered code reachable from them.')
    w('')
    w('| routine | bytes | uncovered below (bytes / routines) | callers (port files) | what it is | evidence |')
    w('|---|---:|---:|---|---|---|')
    for (n, k), e in frontier[:60]:
        callers = ', '.join(fmt(c) for c in sorted(g.entries[e])[:4])
        w(f'| {fmt(e)} | {size[e]} | {n} / {k} | {callers} ({context(e)}) | {note(e)} | {describe(g, e)} |')
    w('')
    w('## Game state still to port')
    w('')
    w('Looked at by hand (the frontier\'s first 140): nearly all of it is')
    w('presentation the port leaves to its own front end: the renderer\'s')
    w('internals, a window and menu library (segment 3159), input reading')
    w('(2F26, the waits in 1194), the sound and music driver interface (0FFD),')
    w('message boxes and screens. The ones that read as game state turned out')
    w('ported inside their callers\' ports, without their own address in the')
    w('docs:')
    w('')
    w('| routine | bytes | callers (port files) | verdict |')
    w('|---|---:|---|---|')
    for k, v in NOTES.items():
        if '[game' not in v:
            continue
        e = tuple(int(x, 16) for x in k.split(':'))
        if e in g.entries:
            callers = ', '.join(fmt(c) for c in sorted(g.entries[e])[:3])
            w(f'| {k} | {size[e]} | {callers} ({context(e)}) | {v} |')
    w('')
    w('Not yet looked at: the open frontier past the hand-checked ones (no')
    w('I/O or drawing signs, not marked inline, a ported caller), largest')
    w('first; candidates, not findings:')
    w('')
    w('| routine | bytes | uncovered below (bytes / routines) | callers (port files) | evidence |')
    w('|---|---:|---:|---|---|')
    shown = 0
    for (n, k), e in frontier:
        if fmt(e) in NOTES:
            continue
        callers = ', '.join(fmt(c) for c in sorted(g.entries[e])[:3])
        w(f'| {fmt(e)} | {size[e]} | {n} / {k} | {callers} ({context(e)}) | {describe(g, e)} |')
        shown += 1
        if shown >= 40:
            break
    w('')
    w('## Named in passing')
    w('')
    w('Uncovered routines whose offset appears bare (no segment) in a comment')
    w('of a port file that names their segment. Such a mention is as often a')
    w('routine ported inline as one deliberately left out ("stubbed: ...") or a')
    w('variable that happens to share the number, so they are not counted as')
    w(f'ported; {len(bare)} to check, largest first:')
    w('')
    w('| routine | bytes | files |')
    w('|---|---:|---|')
    for e in sorted(bare, key=lambda e: (-size[e], e)):
        if e in rest_set:
            w(f'| {fmt(e)} | {size[e]} | {", ".join(sorted(os.path.basename(f) for f in bare[e]))} |')
    w('')
    w('## All uncovered routines')
    w('')
    w('Class: game (no I/O or drawing signs; game state or undetermined) or the')
    w('signs found. Evidence: files named, DGROUP variables most touched.')
    w('')
    w('| routine | bytes | class | callers | what it is | evidence |')
    w('|---|---:|---|---|---|---|')
    for e in sorted(rest):
        callers = ', '.join(fmt(c) for c in sorted(g.entries[e])[:3])
        more = '' if len(g.entries[e]) <= 3 else f' (+{len(g.entries[e]) - 3})'
        c = ', '.join(cls[e]) or ('likely inline' if e in likely else 'game')
        w(f'| {fmt(e)} | {size[e]} | {c} | {callers}{more} | {note(e)} | {describe(g, e)} |')
    open(out, 'w').write('\n'.join(lines) + '\n')
    print(f'reachable {total} ({kb(g.entries)} bytes); ported {len(covered)} ({kb(covered)}); '
          f'hooks {len(hooks)}; view {len(viewed)}; uncovered game {len(game)} ({kb(game)}); '
          f'presentation {len(pres)} ({kb(pres)}); likely inline {len(likely)} ({kb(likely)}); '
          f'unresolved indirect {g.unresolved}; '
          f'unplaced claims {dict(unplaced)}; added by port entries {added}')
    for a in os.environ.get('WHO', '').split():
        e = tuple(int(x, 16) for x in a.split(':'))
        print('callers of', a, [fmt(c) for c in g.entries.get(e, ())], 'root' if e in roots else '', 'port entry' if e in extra else '')
    for (n, k), e in frontier[:int(os.environ.get('SHOW', 15))]:
        print(f'  frontier {fmt(e)} size {size[e]} below {n}/{k} [{context(e)}] {note(e)} | {describe(g, e)}')


if __name__ == '__main__':
    main()
