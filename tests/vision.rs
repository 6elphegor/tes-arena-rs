//! The dream speech's bank put back (src/vision.rs, 2F75:014E). The
//! vision itself is checked frame by frame in the view (it writes
//! VISION.EMS); this file holds its fixed mode's test.
mod common;

use arena::machine::Machine;
use arena::mem::DS;

/// Fixed (kept bug #51): putting the speech away maps back the bank
/// VISION.EMS kept (8FA8's word, the bank times 4: shifted right by 2);
/// the original shifts it right by 4.
#[test]
fn fixed_speech_maps_back_the_bank_it_kept() {
    use arena::fixes::{Fixes, VISION_BANK};
    let Some(img) = common::image() else { return };
    for bank in [1u16, 5, 0x19] {
        let mut m = Machine::new(&img);
        m.mem.w16(DS, 0x8F41, bank << 2);
        assert_eq!(m.speech_bank(), bank >> 2, "the original");
        m.fixes = Fixes::only(VISION_BANK);
        assert_eq!(m.speech_bank(), bank, "fixed");
    }
}
