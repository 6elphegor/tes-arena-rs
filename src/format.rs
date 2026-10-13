//! The game's sprintf (0000:8D14, formatter 0000:8D6E), for formats
//! whose only conversions are `%s` and `%%`.

use crate::machine::Machine;
use crate::mem::DS;

/// Bit 0x10 turns off writing "\n" as "\r\n".
const NO_CR: u16 = 0xA902;

impl Machine {
    /// 0000:8D14. Formats `fmt` with the strings `args` (far pointers)
    /// into `dst`, NUL-terminated; returns the offset of the NUL. A
    /// line feed in the format or an argument is written as CR LF unless
    /// DGROUP A902 bit 0x10 is set.
    pub fn sprintf_s(&mut self, dst: (u16, u16), fmt: (u16, u16), args: &[(u16, u16)]) -> u16 {
        let (dseg, mut di) = dst;
        let cr = self.mem.r16(DS, NO_CR) & 0x10 == 0;
        let mut put = |m: &mut Self, c: u8| {
            if c == b'\n' && cr {
                m.mem.w8(dseg, di, b'\r');
                di = di.wrapping_add(1);
            }
            m.mem.w8(dseg, di, c);
            di = di.wrapping_add(1);
        };
        let (fseg, mut p) = fmt;
        let mut args = args.iter();
        loop {
            let c = self.mem.r8(fseg, p);
            p = p.wrapping_add(1);
            match c {
                0 => break,
                b'%' => {
                    let spec = self.mem.r8(fseg, p);
                    p = p.wrapping_add(1);
                    match spec {
                        b'%' => put(self, b'%'),
                        b's' => {
                            let &(aseg, mut q) = args.next().expect("an argument per %s");
                            loop {
                                let c = self.mem.r8(aseg, q);
                                if c == 0 {
                                    break;
                                }
                                put(self, c);
                                q = q.wrapping_add(1);
                            }
                        }
                        _ => unimplemented!("sprintf %{}", spec as char),
                    }
                }
                _ => put(self, c),
            }
        }
        put(self, 0);
        di.wrapping_sub(1)
    }
}
