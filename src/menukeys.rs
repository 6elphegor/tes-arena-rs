//! The menu keys of play (2542:0100, called from the frame body once a
//! frame): the keys that open the game's screens or flip its switches.
//!
//! Each key is asked in turn whether it was pressed since last asked
//! (`Machine::key_pressed`, 2542:05B8), and the first that was is acted on
//! and ends the check: a frame takes at most one menu key, and the keys
//! after it are not asked (their latches stay as they were). In order:
//!
//! | Key (scan) | Handler | What it does |
//! |---|---|---|
//! | Esc (01), the CyberMan's 20h | 000A | the options screen |
//! | A (1E), the CyberMan's button 1 | 0137 | the weapon readied or put away (unless C39A bit 4000h: then the keys after it are asked) |
//! | C (2E) | 026D | the spellbook (056A), unless 5CA bit 8 |
//! | P (19) | 0279 | picking pockets armed |
//! | S (1F) | 037C | the status box (04B1) |
//! | U (16) | 034D | the list of items to use |
//! | M (32), the CyberMan's 10h | 045A / 03BF | the automap; with Shift held (ACDF, ACEB) the world map |
//! | F1 (3B), the CyberMan's 08h | 00BE | a touch spell waiting cast on the player; else the character sheet (1194:0CF8) if the player has a name (01AD) |
//! | F8 (42) | 01C5 | the compass shown or hidden (5C8 bit 2000h) |
//! | L (26) | 02A3 | the logbook |
//! | R (13) | 01E6 | camping (0205's checks, then 1194:424E) |
//!
//! The interface bar's buttons (2542:06CE's table at 5BD2) run the same
//! handlers: 0486 (as A), 0380 (the map), 0279, 037C, 026D, 02A3, 034D,
//! 01E6 and 00BE (the portrait, as F1).
//!
//! What only changes the game's state is done here; the screens are the
//! front end's, named by the `MenuKey` returned.

use crate::bsa::Files;
use crate::machine::Machine;
use crate::mem::DS;

/// The scan codes the check asks about.
pub mod key {
    pub const ESC: u16 = 0x01;
    pub const A: u16 = 0x1E;
    pub const C: u16 = 0x2E;
    pub const P: u16 = 0x19;
    pub const S: u16 = 0x1F;
    pub const U: u16 = 0x16;
    pub const M: u16 = 0x32;
    pub const F1: u16 = 0x3B;
    pub const F8: u16 = 0x42;
    pub const L: u16 = 0x26;
    pub const R: u16 = 0x13;
}

/// DGROUP variables.
pub mod var {
    /// The Shift keys' state bytes (DS:ACB5 + 2Ah, + 36h).
    pub const LSHIFT: u16 = 0xACDF;
    pub const RSHIFT: u16 = 0xACEB;
    /// The player's name (record +9): the character sheet only with one.
    pub const NAME: u16 = 0x01AD;
}

/// What the key check leaves the front end to do (or what it did).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKey {
    /// No menu key this frame, or one whose handler did nothing (C with
    /// the spellbook barred, F1 with neither a touch spell nor a sheet).
    None,
    /// Esc: the options screen (000A).
    Options,
    /// A: the weapon readied or put away, done.
    Ready,
    /// C: the spellbook (056A).
    Spellbook,
    /// P: picking pockets armed, done.
    Pickpocket,
    /// S: the status box (04B1).
    Status,
    /// U: the use list (034D).
    Use,
    /// M: the automap (045A).
    Automap,
    /// Shift+M: the world map (03BF).
    TravelMap,
    /// F1: the touch spell waiting cast on the player, done.
    TouchCast,
    /// F1: the character sheet (1194:0CF8).
    Sheet,
    /// F8: the compass shown or hidden, done.
    Compass,
    /// L: the logbook (02A3).
    Log,
    /// R: camping (01E6).
    Camp,
}

/// What the check leaves to code not ported here.
pub trait MenuKeyHooks {
    /// 2F26:04E2: whether the CyberMan's button `mask` is down (the
    /// carry). Never without one.
    fn cyber_button(&mut self, _m: &mut Machine, _mask: u8) -> bool {
        false
    }
    /// 2F26:03ED: the CyberMan's buttons (DL). None without one.
    fn cyber_buttons(&mut self, _m: &mut Machine) -> u8 {
        0
    }
    /// 29CF:0000 with AX 1 and FS:SI the player: the spell waiting cast
    /// on the player (`resolve_cast`); whether one was waiting.
    fn touch_cast(&mut self, m: &mut Machine, files: &mut Files) -> Option<bool> {
        // On the player: no quest target to give up.
        m.resolve_cast(files, crate::cast::Touch::Record(DS, crate::character::PLAYER), &mut |_, _| {})
    }
}

/// The hooks with no CyberMan and the port's casting.
pub struct NoMenuKeyHooks;
impl MenuKeyHooks for NoMenuKeyHooks {}

impl Machine {
    /// 2542:0100. The menu key pressed this frame, if any, acted on (see
    /// the module's table). None where the touch cast fails.
    pub fn menu_keys(&mut self, files: &mut Files, hooks: &mut dyn MenuKeyHooks) -> Option<MenuKey> {
        if self.key_pressed(key::ESC) || hooks.cyber_button(self, 0x20) {
            return Some(MenuKey::Options);
        }
        let mut ready = self.key_pressed(key::A);
        if !ready && hooks.cyber_buttons(self) & 1 != 0 {
            // The CyberMan's button held: waited for until let go.
            while hooks.cyber_buttons(self) & 1 != 0 {}
            ready = true;
        }
        if ready && self.ready_weapon() {
            return Some(MenuKey::Ready);
        }
        if self.key_pressed(key::C) {
            // 026D: unless the spellbook is barred.
            return Some(if self.mem.r16(DS, 0x05CA) & 8 != 0 { MenuKey::None } else { MenuKey::Spellbook });
        }
        if self.key_pressed(key::P) {
            self.pilfer_arm();
            return Some(MenuKey::Pickpocket);
        }
        if self.key_pressed(key::S) {
            return Some(MenuKey::Status);
        }
        if self.key_pressed(key::U) {
            return Some(MenuKey::Use);
        }
        if self.key_pressed(key::M) || hooks.cyber_button(self, 0x10) {
            let shift = self.mem.r8(DS, var::LSHIFT) | self.mem.r8(DS, var::RSHIFT);
            return Some(if shift != 0 { MenuKey::TravelMap } else { MenuKey::Automap });
        }
        if self.key_pressed(key::F1) || hooks.cyber_button(self, 0x08) {
            return self.portrait(files, hooks);
        }
        if self.key_pressed(key::F8) {
            let f = self.mem.r16(DS, 0x05C8) ^ 0x2000;
            self.mem.w16(DS, 0x05C8, f);
            return Some(MenuKey::Compass);
        }
        if self.key_pressed(key::L) {
            return Some(MenuKey::Log);
        }
        if self.key_pressed(key::R) {
            return Some(MenuKey::Camp);
        }
        Some(MenuKey::None)
    }

    /// 2542:0137..0144 (and the bar's 0486): the weapon readied or put
    /// away: unless C39A bit 4000h, 5E0 bit 10h toggled and the hands'
    /// frame 1Eh. Whether it was.
    pub fn ready_weapon(&mut self) -> bool {
        if self.mem.r16(DS, crate::movement::var::LEVEL_FLAGS) & 0x4000 != 0 {
            return false;
        }
        let f = self.mem.r16(DS, 0x05E0) ^ 0x10;
        self.mem.w16(DS, 0x05E0, f);
        self.mem.w16(DS, crate::weapon::var::FRAME, 0x1E);
        true
    }

    /// 2542:00BE (F1, the bar's portrait): a touch spell waiting is cast
    /// on the player; with none, the character sheet if the player has a
    /// name, where the player stands checked first for its equipment
    /// page's drop button (00E5: 1194:5F13, `drop_place`).
    pub fn portrait(&mut self, files: &mut Files, hooks: &mut dyn MenuKeyHooks) -> Option<MenuKey> {
        if hooks.touch_cast(self, files)? {
            return Some(MenuKey::TouchCast);
        }
        if self.mem.r8(DS, var::NAME) == 0 {
            return Some(MenuKey::None);
        }
        self.drop_place();
        Some(MenuKey::Sheet)
    }
}
