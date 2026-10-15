//! The game's sprintf (0000:8D14, formatter 0000:8D6E), for formats
//! whose conversions are `%s`, `%d`, `%u`, `%ld`, `%lu` and `%%`.

use crate::machine::Machine;
use crate::mem::DS;

/// Bit 0x10 turns off writing "\n" as "\r\n".
const NO_CR: u16 = 0xA902;
/// End of the buffer 0000:7028 converts numbers in (backwards).
const UTOA_END: u16 = 0xA933;
/// End of the buffer 0000:75B4 converts dwords in (backwards, in the
/// format's segment).
const LTOA_END: u16 = 0x94F6;

/// An argument of `sprintf`.
#[derive(Clone, Copy, Debug)]
pub enum Arg {
    /// A string, by far pointer (`%s`).
    Str(u16, u16),
    /// A word (`%d`, signed; `%u`).
    Int(u16),
    /// A dword (`%ld`, signed; `%lu`).
    Long(u32),
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
                        // 0000:8DB9: the word through 7028, unsigned.
                        (b'u', Some(&Arg::Int(v))) => {
                            for d in self.utoa(v) {
                                put(self, d, false);
                            }
                        }
                        // 0000:8E39: `%lu` through 0000:75B4, `%ld` through
                        // 75F6 (a minus sign, then the same): the digits
                        // are written backwards into the format's segment,
                        // ending at 94F6, and copied out.
                        (b'l', Some(&Arg::Long(v))) => {
                            let kind = self.mem.r8(fseg, p);
                            p = p.wrapping_add(1);
                            let mut v = v;
                            match kind {
                                b'u' => {}
                                b'd' => {
                                    if (v as i32) < 0 {
                                        v = v.wrapping_neg();
                                        put(self, b'-', false);
                                    }
                                }
                                k => unimplemented!("sprintf %l{}", k as char),
                            }
                            let mut at = LTOA_END;
                            loop {
                                self.mem.w8(fseg, at, b'0' + (v % 10) as u8);
                                at = at.wrapping_sub(1);
                                v /= 10;
                                if v == 0 {
                                    break;
                                }
                            }
                            let mut q = at.wrapping_add(1);
                            while q != LTOA_END.wrapping_add(1) {
                                let d = self.mem.r8(fseg, q);
                                put(self, d, false);
                                q = q.wrapping_add(1);
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
