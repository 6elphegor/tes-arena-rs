import struct, sys
exe, img_path, out = sys.argv[1:4]
b = open(exe, 'rb').read()
img = open(img_path, 'rb').read()[:305520]
W = lambda p: struct.unpack_from('<H', b, p)[0]
p, seg, rel = 166321, 0, []
while True:
    c = W(p); p += 2
    if c == 0xffff: break
    for _ in range(c): rel.append((W(p), seg)); p += 2
    seg += 0x0fff
ss, sp, cs, ip = struct.unpack_from('<4H', b, p)
hdr_len = 0x1c + 4 * len(rel)
hdr_par = (hdr_len + 15) // 16
total = hdr_par * 16 + len(img)
orig = struct.unpack('<14H', b[:28])
# keep the original program's total memory demand
orig_mem = (orig[1] + 512 * (orig[2] - 1) - orig[4] * 16) + orig[5] * 16
minalloc = max(0, (orig_mem - len(img) + 15) // 16)
h = struct.pack('<2s13H', b'MZ', total % 512, (total + 511) // 512, len(rel), hdr_par,
                minalloc, 0xffff, ss, sp, 0, ip, cs, 0x1c, 0)
h += b''.join(struct.pack('<2H', o, s) for o, s in rel)
h = h.ljust(hdr_par * 16, b'\0')
open(out, 'wb').write(h + img)
print(f'relocs={len(rel)} SS:SP={ss:04X}:{sp:04X} CS:IP={cs:04X}:{ip:04X} size={total}')
