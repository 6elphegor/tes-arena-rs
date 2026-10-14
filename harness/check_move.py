"""Run the original's single move (0000:F8DD) from a state the port dumped
(view/examples/wilddungeon.rs with DUMP=dir): DGROUP and the wall and
floor planes at their segments. The hooks into segment 0000 get the
emulator's relocated segment, as the game's are.

Usage: check_move.py ACD_unpacked.EXE DIR
"""
import glob
import os
import struct
import sys

sys.path.insert(0, os.path.dirname(__file__))
from emu import Arena, Fault, LOAD_SEG

EXE, D = sys.argv[1:3]
a = Arena(EXE)
ds = open(os.path.join(D, 'ds.bin'), 'rb').read()
planes = []
for f in glob.glob(os.path.join(D, '*_*.bin')):
    seg = int(f.rsplit('_', 1)[1].split('.')[0], 16)
    planes.append((seg, open(f, 'rb').read()))
HOOKS = (0xCD6A, 0xCD6E, 0xC3B8, 0xC3AC, 0xC3B4, 0xC3A4, 0xC3A8, 0xC3B0, 0xCD2A, 0xCD2E, 0xCD32, 0xCD36,
         0xCD42, 0xCD46, 0xCD4A, 0xCD4E, 0xCD52, 0xCD56, 0xCD5A, 0xCD5E, 0xCD62, 0xCD1A, 0xCD22, 0xCD26, 0xCD3A)
for h in (0, 0x80, 0x100, 0x180):
    a.mu.mem_write(a.lin(a.ds, 0), ds)
    for seg, data in planes:
        assert seg >= 0x6000, hex(seg)
        a.mu.mem_write(seg * 16, data)
    for at in HOOKS:
        off, seg = struct.unpack_from('<HH', ds, at)
        if off or seg:
            a.w16(at + 2, (seg + LOAD_SEG) & 0xFFFF)
    try:
        r = a.call_far(0, 0xF8DD, eax=0x40, ebx=h)
        print(f'original move heading {h:#x}: ax {r:#x} to ({a.r16(0xC3D0):#x}, {a.r16(0xC3D4):#x})')
    except Fault as f:
        print(f'original move heading {h:#x}: fault {f}')
    except AssertionError:
        print(f'original move heading {h:#x}: hang')
