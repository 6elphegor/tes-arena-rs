//! The game's sprintf (0000:8D14, formatter 0000:8D6E), for formats
//! whose conversions are `%s`, `%d` and `%%`.

use crate::machine::Machine;
use crate::mem::DS;

/// Bit 0x10 turns off writing "\n" as "\r\n".
const NO_CR: u16 = 0xA902;
/// End of the buffer 0000:7028 converts numbers in (backwards).
const UTOA_END: u16 = 0xA933;

/// An argument of `sprintf`.
#[derive(Clone, Copy, Debug)]
pub enum Arg {
    /// A string, by far pointer (`%s`).
    Str(u16, u16),
    /// A word (`%d`, signed).
    Int(u16),
}

impl Machine {
    /// 0000:8D14. Formats `fmt` with `args` into `dst`, NUL-terminated;
    /// returns the offset of the NUL. A line feed in the format or a
    /// string argument is written as CR LF unless DGROUP A902 bit 0x10
    /// is set.
    pub fn sprintf(&mut self, dst: (u16, u16), fmt: (u16, u16), args: &[Arg]) -> u16 {
        let (dseg, mut di) = dst;
        let cr = self.mem.r16(DS, NO_CR) & 0x10 == 0;
        let mut put = |m: &mut Self, c: u8, lf: bool| {
            if lf && c == b'\n' && cr {
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
                    match (spec, args.next()) {
                        (b'%', _) => put(self, b'%', false),
                        (b's', Some(&Arg::Str(aseg, mut q))) => loop {
                            let c = self.mem.r8(aseg, q);
                            if c == 0 {
                                break;
                            }
                            put(self, c, true);
                            q = q.wrapping_add(1);
                        },
                        // 0000:7062
                        (b'd' | b'i', Some(&Arg::Int(v))) => {
                            let mut v = v;
                            if (v as i16) < 0 {
                                put(self, b'-', false);
                                v = v.wrapping_neg();
                            }
                            for d in self.utoa(v) {
                                put(self, d, false);
                            }
                        }
                        // 0000:7028
                        (b'u', Some(&Arg::Int(v))) => {
                            for d in self.utoa(v) {
                                put(self, d, false);
                            }
                        }
                        (s, a) => unimplemented!("sprintf %{} with {a:?}", s as char),
                    }
                }
                _ => put(self, c, true),
            }
        }
        put(self, 0, false);
        di.wrapping_sub(1)
    }

    /// `sprintf` with string arguments only.
    pub fn sprintf_s(&mut self, dst: (u16, u16), fmt: (u16, u16), args: &[(u16, u16)]) -> u16 {
        let args: Vec<Arg> = args.iter().map(|&(s, o)| Arg::Str(s, o)).collect();
        self.sprintf(dst, fmt, &args)
    }

    /// 0000:7028: writes the decimal digits of `v` backwards into DGROUP,
    /// ending at A933 (they stay there), and returns them in order.
    pub(crate) fn utoa(&mut self, mut v: u16) -> Vec<u8> {
        let mut p = UTOA_END;
        loop {
            self.mem.w8(DS, p, b'0' + (v % 10) as u8);
            p = p.wrapping_sub(1);
            v /= 10;
            if v == 0 {
                break;
            }
        }
        (p.wrapping_add(1)..=UTOA_END).map(|o| self.mem.r8(DS, o)).collect()
    }
}
