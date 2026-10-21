//! The spellbook's page of a spell of the player's (src/spellpage.rs,
//! 1194:51D1): the fixed mode's Delete. (The page's faithful runs are
//! checked by the view's screen vectors.)
mod common;

use arena::bsa::Files;
use arena::machine::Machine;
use arena::mem::DS;
use arena::spellpage::SpellPageHooks;

/// Clicks Delete (answering yes) then Exit, noting the spell each page
/// shows (its number on the list).
struct Script {
    clicks: Vec<(u8, u16, u16)>,
    shown: Vec<u8>,
}

impl SpellPageHooks for Script {
    fn page(&mut self, m: &mut Machine) {
        let mark = m.mem.r8(DS, 0x13C8) as u16;
        self.shown.push(m.mem.r8(DS, 0x050B + mark));
    }
    fn click(&mut self, _m: &mut Machine) -> (u8, u16, u16) {
        if self.clicks.is_empty() { (1, 0x120, 0xC0) } else { self.clicks.remove(0) }
    }
    fn confirm(&mut self, _m: &mut Machine, _si: u16) -> u16 {
        0
    }
}

/// The spells shown when spell `mark` of the list 5, 6, 7 is looked at
/// and deleted, with `fixes`; and the list after.
fn deleted(img: &arena::exe::Image, files: &mut Files, fixes: arena::fixes::Fixes, mark: u8) -> (Vec<u8>, Vec<u8>) {
    let mut m = Machine::new(img);
    m.fixes = fixes;
    m.heap_init(0xA000, 0xF000).unwrap();
    assert!(m.bsa_open(files));
    let (frame, _) = m.heap_alloc(0x1000).unwrap();
    m.mem.w16(DS, 0x9030, frame);
    m.mem.w16(DS, 0xA900, 0x9000);
    m.mem.w8(DS, 0x1A6C, 1);
    m.mem.w8(DS, 0x050A, 3);
    for (i, n) in [5u8, 6, 7].into_iter().enumerate() {
        m.mem.w8(DS, 0x050B + i as u16, n);
    }
    m.mem.w8(DS, 0x1296, mark);
    let mut sc = Script { clicks: vec![(1, 0xD0, 0xC0)], shown: Vec::new() };
    m.spell_page(files, &mut sc).expect("the page");
    let n = m.mem.r8(DS, 0x050A) as u16;
    (sc.shown, (0..n).map(|i| m.mem.r8(DS, 0x050B + i)).collect())
}

/// Fixed (kept bug #38): after a spell is deleted the page shows the one
/// now in its place, or the new last one when the last was deleted. The
/// original shows nothing again after the last (its page stays up), and
/// after the first moves on past the one now there.
#[test]
fn fixed_a_deleted_spell_s_page_is_replaced() {
    use arena::fixes::{Fixes, SPELL_DELETED};
    let Some(img) = common::image() else { return };
    let Ok(mut files) = Files::new(&common::game_dir()) else { return };
    let fixed = Fixes::only(SPELL_DELETED);
    assert_eq!(deleted(&img, &mut files, Fixes::NONE, 2), (vec![7], vec![5, 6]), "the deleted page left up");
    assert_eq!(deleted(&img, &mut files, fixed, 2), (vec![7, 6], vec![5, 6]));
    assert_eq!(deleted(&img, &mut files, Fixes::NONE, 0), (vec![5, 7], vec![6, 7]), "6 passed over");
    assert_eq!(deleted(&img, &mut files, fixed, 0), (vec![5, 6], vec![6, 7]));
    assert_eq!(deleted(&img, &mut files, Fixes::NONE, 1), (vec![6, 7], vec![5, 7]));
    assert_eq!(deleted(&img, &mut files, fixed, 1), (vec![6, 7], vec![5, 7]), "the middle as before");
}
