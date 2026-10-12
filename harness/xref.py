"""Static helpers over re/ACD_unpacked.EXE: code xrefs and disassembly.

Usage:
  xref.py data OFF...     code that references DGROUP offsets OFF (hex)
  xref.py ptrs OFF...     DGROUP words equal to OFF (pointer tables)
  xref.py dis SEG:OFF [N] disassemble N instructions
  xref.py calls SEG:OFF   far/near call sites of a routine
"""
import struct, sys, os
from capstone import Cs, CS_ARCH_X86, CS_MODE_16
from capstone.x86 import X86_OP_MEM, X86_OP_IMM

EXE = os.path.join(os.path.dirname(__file__), '..', 're', 'ACD_unpacked.EXE')
DGROUP = 0x3BA0
_b = open(EXE, 'rb').read()
_h = struct.unpack('<14H', _b[:28])
IMG = _b[_h[4] * 16:]
RELOCS = {s * 16 + o for o, s in struct.iter_unpack('<2H', _b[_h[12]:_h[12] + 4 * _h[3]])}
CODE_END = DGROUP * 16
DS = IMG[CODE_END:]
md = Cs(CS_ARCH_X86, CS_MODE_16)
md.detail = True
md.skipdata = True

def code_segments():
    """Segments that are targets of relocated far calls/jumps, plus 0."""
    segs = {0}
    for r in RELOCS:
        if r >= 3 and IMG[r - 3] in (0x9a, 0xea):
            seg = struct.unpack_from('<H', IMG, r)[0]
            if seg < DGROUP:
                segs.add(seg)
    return sorted(segs)

SEGS = code_segments()

def seg_of(lin):
    s = max(x for x in SEGS if x * 16 <= lin)
    return s, lin - s * 16

def linear_sweep():
    """(seg, off, insn) for all code; resyncs at segment boundaries."""
    bounds = SEGS + [DGROUP]
    for i, s in enumerate(SEGS):
        start, end = s * 16, bounds[i + 1] * 16
        for ins in md.disasm(IMG[start:end], 0):
            yield s, ins

def data_refs(targets):
    targets = set(targets)
    for s, ins in linear_sweep():
        if ins.id == 0:
            continue
        for op in ins.operands:
            v = None
            if op.type == X86_OP_MEM and op.mem.base == 0 and op.mem.index == 0:
                v = op.mem.disp & 0xffff
            elif op.type == X86_OP_MEM:
                v = op.mem.disp & 0xffff
            elif op.type == X86_OP_IMM:
                v = op.imm & 0xffff
            if v in targets:
                yield s, ins
                break

def dis(seg, off, n=60):
    out = []
    for ins in md.disasm(IMG[seg * 16 + off: seg * 16 + off + n * 8], off):
        out.append(f'{seg:04X}:{ins.address:04X}  {ins.bytes.hex():<14} {ins.mnemonic} {wrap(ins)}{farname(seg, ins)}')
        if len(out) >= n:
            break
    return '\n'.join(out)

def wrap(ins):
    """Operands, with relative branch targets wrapped to 16 bits as the
    CPU does (capstone reports them unwrapped past offset 0xFFFF)."""
    if ins.bytes[0] in (0xe8, 0xe9) or ins.mnemonic.startswith('j') and ins.operands \
            and ins.operands[0].type == X86_OP_IMM:
        return f'0x{ins.operands[0].imm & 0xffff:x}'
    return ins.op_str

def farname(seg, ins):
    if ins.bytes[0] == 0x9a and len(ins.bytes) == 5:
        o, s = struct.unpack_from('<HH', ins.bytes, 1)
        return f'    ; -> {s:04X}:{o:04X}'
    return ''

def call_sites(seg, off):
    lin = seg * 16 + off
    for r in RELOCS:
        if r >= 3 and IMG[r - 3] == 0x9a:
            o = struct.unpack_from('<H', IMG, r - 2)[0]
            s = struct.unpack_from('<H', IMG, r)[0]
            if s * 16 + o == lin:
                yield ('far',) + seg_of(r - 3)
    for s, ins in linear_sweep():
        if s == seg and ins.mnemonic == 'call' and ins.bytes[0] == 0xe8 and ins.operands[0].imm & 0xffff == off:
            yield ('near', s, ins.address)

if __name__ == '__main__':
    cmd, args = sys.argv[1], sys.argv[2:]
    if cmd == 'data':
        for s, ins in data_refs(int(a, 16) for a in args):
            print(f'{s:04X}:{ins.address:04X}  {ins.mnemonic} {ins.op_str}')
    elif cmd == 'ptrs':
        for a in args:
            t = int(a, 16)
            print(a, [hex(i) for i in range(0, len(DS) - 1) if struct.unpack_from('<H', DS, i)[0] == t])
    elif cmd == 'dis':
        s, o = (int(x, 16) for x in args[0].split(':'))
        print(dis(s, o, int(args[1]) if len(args) > 1 else 60))
    elif cmd == 'calls':
        s, o = (int(x, 16) for x in args[0].split(':'))
        for c in call_sites(s, o):
            print(f'{c[0]:4} {c[1]:04X}:{c[2]:04X}')
