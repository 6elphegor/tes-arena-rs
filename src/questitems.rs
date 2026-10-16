//! The lines the equipment screen's list shows after the player's items
//! (1194:1EB8): what the quests under way give the player to carry
//! (1194:A206), then the artifact and the Staff of Chaos's pieces
//! (2433:083D), as strings one after another.
//!
//! A206 goes over the four quests at 0DCA (29h bytes each) and the four
//! at 0E6E (34h each); a quest under way (+4 bits 6-7 01b) gives a line
//! by its stage (+4 bits 9-11) and its "carried" bit (+4 20h), each
//! counted at 126C:
//! - a person to take along (stage 3, or for the second four 5 with the
//!   bit): their name (1194:0807 of race +25h >> 5, sex +28h, the seed
//!   +1Eh + +8 dword, its low word plus +6 and +26h), the words at 6E04
//!   and the place's kind (+27h among the strings at DATA:4931)
//!   (1194:A30F);
//! - for the second four at stage 4 with the bit, a person by name the
//!   same way but of sex 0 and the seed's low byte plus +2Fh, the name's
//!   last letter written over (the original's `dec di` past a name that
//!   ends at its NUL), then 6E04's words and the string +2Fh less 18h of
//!   2A62;
//! - an item (stage 0, or another with the bit; stage 6 of the second
//!   four never): the item's name with its first letter made a capital
//!   (1194:A357: the string (+4 bits 12-15) * 10 + byte +6 of DATA:4A58
//!   for the first four, word +6 of DATA:4A3B for the second);
//!
//! and ends the strings with a zero word. 2433:083D then writes, from
//! the same place, the artifact's name (string F77 of 4C12, when F7F is
//! set) and "You currently have %d piece(s)..." of 4C92 with F77 (when
//! F77 is), counting them at 12CE, which it returns.

use crate::bsa::Files;
use crate::format::Arg;
use crate::machine::Machine;
use crate::mem::{DATA, DS};

/// DGROUP variables.
pub mod var {
    /// The quests' lines counted (1194:A206's) and the artifact's lines
    /// (2433:083D's).
    pub const COUNT: u16 = 0x126C;
    pub const ARTIFACT_COUNT: u16 = 0x12CE;
    /// The staff's pieces held, and the artifact held.
    pub const PIECES: u16 = 0x0F77;
    pub const ARTIFACT: u16 = 0x0F7F;
}

impl Machine {
    /// 1194:5585 over DS `seg`: past `n` strings at `si`, each scanned for
    /// its NUL 3000 bytes at most.
    fn qi_skip(&self, seg: u16, mut si: u16, n: u16) -> u16 {
        for _ in 0..n {
            for k in 1..=0xBB8 {
                let c = self.mem.r8(seg, si);
                si = si.wrapping_add(1);
                if c == 0 || k == 0xBB8 {
                    break;
                }
            }
        }
        si
    }

    /// 0000:9CA0: the string at `seg`:`si` copied to DS:`di`; DI past its
    /// NUL.
    fn qi_copy(&mut self, seg: u16, mut si: u16, mut di: u16) -> u16 {
        loop {
            let c = self.mem.r8(seg, si);
            self.mem.w8(DS, di, c);
            si = si.wrapping_add(1);
            di = di.wrapping_add(1);
            if c == 0 {
                return di;
            }
        }
    }

    /// 1194:A357: string `n` of DATA:`si` copied to `di` with its first
    /// byte 20h less (a capital); DI past the NUL.
    fn qi_capital(&mut self, si: u16, n: u16, di: u16) -> u16 {
        let si = self.qi_skip(DATA, si, n);
        let c = self.mem.r8(DATA, si);
        self.mem.w8(DATA, si, c.wrapping_sub(0x20));
        let di = self.qi_copy(DATA, si, di);
        let c = self.mem.r8(DATA, si);
        self.mem.w8(DATA, si, c.wrapping_add(0x20));
        di
    }

    /// The seed 1194:A30F and A2AA make a quest's person's name from: +1Eh
    /// and +8 added, their low word plus +6.
    fn qi_seed(&self, si: u16) -> u32 {
        let s = self.mem.r32(DS, si.wrapping_add(0x1E)).wrapping_add(self.mem.r32(DS, si.wrapping_add(8)));
        let lo = (s as u16).wrapping_add(self.mem.r16(DS, si.wrapping_add(6)));
        (s & 0xFFFF_0000) | lo as u32
    }

    /// 1194:A30F: the quest's person, the words at 6E04 and its place's
    /// kind; DI past the NUL.
    fn qi_person(&mut self, files: &mut Files, si: u16, di: u16) -> Option<u16> {
        let race = self.mem.r8(DS, si.wrapping_add(0x25)) >> 5;
        let sex = self.mem.r8(DS, si.wrapping_add(0x28));
        let s = self.qi_seed(si);
        let lo = (s as u16).wrapping_add(self.mem.r16(DS, si.wrapping_add(0x26)));
        let mut di = self.npc_name(files, race, sex, (s & 0xFFFF_0000) | lo as u32, DS, di)?;
        if self.mem.r8(DS, di) != 0 {
            di = di.wrapping_sub(1);
        }
        let di = self.qi_copy(DS, 0x6E04, di).wrapping_sub(1);
        let n = self.mem.r8(DS, si.wrapping_add(0x27)) as u16;
        let from = self.qi_skip(DATA, 0x4931, n);
        Some(self.qi_copy(DATA, from, di))
    }

    /// 1194:A206 then 2433:083D: the lines at DS:`di` (126C and 12CE
    /// counting them); the number of the artifact's lines (12CE).
    pub fn quest_item_names(&mut self, files: &mut Files, mut di: u16) -> Option<u16> {
        let count = |m: &mut Self| {
            let n = m.mem.r16(DS, var::COUNT).wrapping_add(1);
            m.mem.w16(DS, var::COUNT, n);
        };
        for i in 0..4u16 {
            let si = 0x0DCA + i * 0x29;
            if self.quest_kind(si) != 1 {
                continue;
            }
            let st = self.quest_stage(si);
            let carried = self.mem.r16(DS, si.wrapping_add(4)) & 0x20 != 0;
            if st == 3 {
                count(self);
                di = self.qi_person(files, si, di)?;
            } else if st == 0 || carried {
                count(self);
                let sort = (self.mem.r16(DS, si.wrapping_add(4)) >> 12) as u8;
                let n = (sort.wrapping_mul(10) as u16).wrapping_add(self.mem.r8(DS, si.wrapping_add(6)) as u16);
                di = self.qi_capital(0x4A58, n, di);
            }
        }
        for i in 0..4u16 {
            let si = 0x0E6E + i * 0x34;
            if self.quest_kind(si) != 1 {
                continue;
            }
            let st = self.quest_stage(si);
            let carried = self.mem.r16(DS, si.wrapping_add(4)) & 0x20 != 0;
            match st {
                6 => {}
                3 => {
                    count(self);
                    di = self.qi_person(files, si, di)?;
                }
                5 | 4 if !carried => {}
                5 => {
                    count(self);
                    di = self.qi_person(files, si, di)?;
                }
                4 => {
                    count(self);
                    let race = self.mem.r8(DS, si.wrapping_add(0x25)) >> 5;
                    let s = self.qi_seed(si);
                    let k = self.mem.r16(DS, si.wrapping_add(0x2F));
                    let lo = (s as u16 & 0xFF00) | (s as u8).wrapping_add(k as u8) as u16;
                    let d = self.npc_name(files, race, 0, (s & 0xFFFF_0000) | lo as u32, DS, di)?;
                    let d = self.qi_copy(DS, 0x6E04, d.wrapping_sub(1)).wrapping_sub(1);
                    let from = self.qi_skip(DS, 0x2A62, k.wrapping_sub(0x18) & 0xFF);
                    di = self.qi_copy(DS, from, d);
                }
                _ if st == 0 || carried => {
                    count(self);
                    let n = self.mem.r16(DS, si.wrapping_add(6));
                    di = self.qi_capital(0x4A3B, n, di);
                }
                _ => {}
            }
        }
        self.mem.w16(DS, di, 0);
        // 2433:083D.
        self.mem.w16(DS, var::ARTIFACT_COUNT, 0);
        let pieces = self.mem.r8(DS, var::PIECES) as u16;
        if self.mem.r8(DS, var::ARTIFACT) != 0 {
            let from = self.qi_skip(DS, 0x4C12, pieces);
            di = self.qi_copy(DS, from, di);
            let n = self.mem.r16(DS, var::ARTIFACT_COUNT).wrapping_add(1);
            self.mem.w16(DS, var::ARTIFACT_COUNT, n);
        }
        if pieces != 0 {
            let n = self.mem.r16(DS, var::ARTIFACT_COUNT).wrapping_add(1);
            self.mem.w16(DS, var::ARTIFACT_COUNT, n);
            self.sprintf((DS, di), (DS, 0x4C92), &[Arg::Int(pieces)]);
        }
        Some(self.mem.r16(DS, var::ARTIFACT_COUNT))
    }
}
