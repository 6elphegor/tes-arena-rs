//! A menu's boxes (1194:020E and its 3159:0F2A): what showing a menu and
//! putting it away leaves in the game's state, and its loop (3159:10BE..
//! 1199: the keys, the clicks by row, Esc and the right button), run over
//! the mouse's reads the front end gives it (`MenuRead`,
//! `Machine::menu_reads`, `Machine::menu_answered`). Without reads (or
//! where they end before the loop does) the front end's pick is taken.
//!
//! 020E waits for the buttons to be let go (8FC4), flushes the keys
//! (710E) and calls 0F2A with its title (DS:SI), its keys (DS:BX) and its
//! entries (the words at DS:DI: the first taken whatever it is, the rest
//! to a 0), at x and y AX, flags 4006h and margin 5. 0F2A sets flags 9
//! (1 cleared again with A902 bit 800h: no backgrounds kept), measures
//! the title and each entry (0000:9A08: `box_measure`), and gives up
//! (FFFFh, nothing shown) where the widest is over 140h or their heights
//! over C8h. Centred down (flag 4, cleared), the title's top is 64h less
//! half the heights, and it gives up where that runs past C8h. Each box
//! is then laid out (965A, `message_box`: DI FFFEh, its background in the
//! heap's top; FS the widest, flag 8 its width), the title's, then the
//! entries' under one another (each at the last's y plus height, from its
//! background's record), and the mouse kept to the entries (53AE). After
//! the pick, the mouse's limits the screen's, each entry's background put
//! back and freed, then the title's (933C); 020E then dissolves the
//! screen in (4FF7), flushes the keys, waits for them to be let go and
//! limits the mouse to the view's 135h columns.
//!
//! The boxes' x is the AX 020E's wait for the buttons leaves: the
//! cursor's x (8FC4 returns 8F92), with flag 2 only kept at A836.
//!
//! With A902 bit 800h no box keeps a background, and 965A returns where
//! one would have gone: DGROUP:FFFEh (the callers' ES, DI). 0F2A reads
//! each box's record there all the same, the offsets wrapping: the boxes
//! go down from the title's y the word at DS:0000 plus the one at 0004h,
//! each entry's height the one at 0004h, and the mouse is kept to x
//! [FFFEh], width [0002h], from y [0000h] to that plus [0004h]. Putting
//! them away tests the bit again for each box (3159:117F, 11A2) and puts
//! back and frees none. (No code sets the bit: A902 is given 8 whole,
//! 0000:6F34, and only bits 1 and 200h after. Under those limits the
//! wait's cursor, the front end's, runs past its buffers at EBB0h and
//! EDB0h, over DGROUP's top and the first heap block's head.)

use crate::machine::Machine;
use crate::mem::DS;

/// One read of the mouse by the menu's loop (0000:8FC4): its buttons
/// (AFF9: bit 0 left, 1 right) and the cursor's place (8F92, 8F94), and
/// the bytes the keyboard gave the key interrupt (0000:718A) since the
/// read before (a key's make code pressed, its break code let go), as the
/// front end saw them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MenuRead {
    pub buttons: u8,
    pub x: u16,
    pub y: u16,
    pub codes: Vec<u8>,
}

/// The boxes a menu showed: where their backgrounds' records are (965A's
/// DX:AX, a heap block's segment and 0, or DGROUP:FFFEh with none kept),
/// the title's first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuBoxes {
    pub title: (u16, u16),
    pub entries: Vec<(u16, u16)>,
}

impl Machine {
    /// 0000:9A08: the text at `seg`:`si` measured as a box with margin
    /// `margin` (A846, A844 the text, A84C 0, A83A and A83C twice the
    /// margin, then 98A0's pass). Returns (width, height): A83A, A83C.
    pub fn box_measure(&mut self, (seg, si): (u16, u16), margin: u16) -> (u16, u16) {
        self.mem.w16(DS, 0xA846, seg);
        self.mem.w16(DS, 0xA84C, 0);
        self.mem.w16(DS, 0xA844, si);
        let m2 = margin.wrapping_add(margin);
        self.mem.w16(DS, 0xA83A, m2);
        self.mem.w16(DS, 0xA83C, m2);
        self.box_lines();
        (self.mem.r16(DS, 0xA83A), self.mem.r16(DS, 0xA83C))
    }

    /// 1194:020E up to its pick: the keys flushed, the menu with its
    /// title at DS:`si` and the entries in the table at DS:`di` shown at
    /// the cursor's x (see the module's notes; the keys at BX are the
    /// pick's).
    /// `screen` the page as the front end has it, for the backgrounds.
    /// Some(None) where 0F2A gives up (FFFFh, nothing shown); None where
    /// the heap has no room.
    pub fn menu_boxes(&mut self, si: u16, di: u16, screen: Option<&[u8]>) -> Option<Option<MenuBoxes>> {
        let ax = self.mem.r16(DS, 0x8F92);
        self.key_flush();
        let mut n = 1u16;
        while self.mem.r16(DS, di.wrapping_add(n.wrapping_mul(2))) != 0 {
            n = n.wrapping_add(1);
        }
        let entries: Vec<u16> = (0..n).map(|i| self.mem.r16(DS, di.wrapping_add(i.wrapping_mul(2)))).collect();
        let mut flags = 0x4006u16 | 9;
        if self.mem.r16(DS, 0xA902) & 0x800 != 0 {
            flags &= !1;
        }
        let (mut wide, mut high) = self.box_measure((DS, si), 5);
        for &e in &entries {
            let (w, h) = self.box_measure((DS, e), 5);
            high = high.wrapping_add(h);
            wide = wide.max(w);
        }
        if wide > 0x140 || high > 0xC8 {
            return Some(None);
        }
        flags &= !4;
        let mut y = 0x64u16.wrapping_sub(high >> 1);
        if y.wrapping_add(high) > 0xC8 {
            return Some(None);
        }
        let rec = |m: &Machine, (seg, off): (u16, u16), at: u16| m.mem.r16(seg, off.wrapping_add(at));
        let title = self.menu_box((DS, si), (ax, y), flags, wide, screen)?;
        y = rec(self, title, 2).wrapping_add(rec(self, title, 6));
        let mut boxes = MenuBoxes { title, entries: Vec::new() };
        for &e in &entries {
            let at = self.menu_box((DS, e), (ax, y), flags, wide, screen)?;
            y = y.wrapping_add(rec(self, at, 6));
            boxes.entries.push(at);
        }
        let (first, last) = (boxes.entries[0], boxes.entries[boxes.entries.len() - 1]);
        let (x, top, w) = (rec(self, first, 0), rec(self, first, 2), rec(self, first, 4));
        let bottom = rec(self, last, 2).wrapping_add(rec(self, last, 6));
        self.mouse_limits(x, top, w, bottom.wrapping_sub(top));
        Some(Some(boxes))
    }

    /// One of 0F2A's boxes (965A: DX 5, DI FFFEh, FS the widest): where
    /// its background's record is (EAX: segment, offset).
    fn menu_box(&mut self, text: (u16, u16), at: (u16, u16), flags: u16, wide: u16, screen: Option<&[u8]>) -> Option<(u16, u16)> {
        self.message_box(text, at, flags, 5, (DS, 0xFFFE), (wide, 0), screen)
    }

    /// 1194:020E after its pick: with `boxes` (0F2A showed them) its
    /// mouse limits the screen (3159:1155), each entry's background put
    /// back and freed and the title's (933C); then, shown or not, 020E's
    /// dissolve (4FF7 over the screen), the keys flushed (710E) and the
    /// mouse limited to the view (0282: 53AE with 0, 0, 135h, C8h). The
    /// page's showing (9466) and the wait for the keys' release (ACB6)
    /// are the front end's.
    pub fn menu_closed(&mut self, boxes: Option<&MenuBoxes>) {
        if let Some(boxes) = boxes {
            self.mouse_limits(0, 0, 0x140, 0xC8);
            for &(seg, _) in &boxes.entries {
                if self.mem.r16(DS, 0xA902) & 0x800 == 0 {
                    self.box_restore(seg);
                }
            }
            if self.mem.r16(DS, 0xA902) & 0x800 == 0 {
                self.box_restore(boxes.title.0);
            }
        }
        self.dissolve();
        self.key_flush();
        self.mouse_limits(0, 0, 0x135, 0xC8);
    }

    /// The menu's loop over the front end's `reads` (`menu_reads`, taken)
    /// run on a copy between `menu_boxes` and `menu_closed`, its keys at
    /// DS:`bx` (FFFFh none): where they reach its end the copy is kept and
    /// its pick returned; otherwise the front end's `pick` as it is.
    pub fn menu_answered(&mut self, boxes: &MenuBoxes, bx: u16, pick: u16) -> u16 {
        if let Some(reads) = self.menu_reads.take() {
            let mut run = self.clone();
            let ax = run.menu_loop(boxes, bx, &reads);
            if std::env::var_os("ARENA_LIST_DEBUG").is_some() {
                eprintln!("menu loop over {} reads: {:?} (pick given {:04X})", reads.len(), ax, pick);
            }
            if let Some(ax) = ax {
                *self = run;
                return ax;
            }
        }
        pick
    }

    /// 3159:10BE..1199: the menu's loop. The mouse read (8FC4; the
    /// CyberMan's poll, 2F26:02A1, nothing without one), then each pass:
    /// the left button down ends it; the right button (020E's flags always
    /// have 4000h) leaves it; a key typed (7138), Esc leaving, others
    /// made capitals (0000:9D74, 0000:9D78) and looked for among the entries' keys (one
    /// byte each at DS:`bx`, as many as the entries, whatever they hold),
    /// picking that entry; otherwise the mouse read again, the cursor
    /// drawn and put back (5454, 541E; the page shown, 9466, the front
    /// end's). Ended, the cursor's place is kept while the buttons are
    /// read until all are let go, then put back (1134..1151); left, the
    /// pick is FFFFh; otherwise a key's entry, or the count less one of
    /// the entries whose boxes start at or above the cursor (1164..1199,
    /// FFFFh above them all). None where the reads end first.
    pub fn menu_loop(&mut self, boxes: &MenuBoxes, bx: u16, reads: &[MenuRead]) -> Option<u16> {
        let mut reads = reads.iter();
        let n = boxes.entries.len() as u16;
        self.menu_read(reads.next()?)?;
        let mut keyed = 0xFFFFu16;
        let left = 'pass: loop {
            let b = self.mem.r8(DS, 0xAFF9);
            if b & 1 != 0 {
                break false;
            }
            if b & 2 != 0 {
                break true;
            }
            let al = self.key_read();
            if al != 0 {
                if al == 0x1B {
                    break true;
                }
                let up = if al.is_ascii_lowercase() { al - 0x20 } else { al };
                if bx != 0xFFFF {
                    for i in 0..n {
                        if self.mem.r8(DS, bx.wrapping_add(i)) == up {
                            keyed = i;
                            break 'pass false;
                        }
                    }
                }
            }
            self.menu_read(reads.next()?)?;
            self.cursor_drawn();
            self.cursor_hidden();
        };
        let (x, y) = (self.mem.r16(DS, 0x8F92), self.mem.r16(DS, 0x8F94));
        loop {
            self.menu_read(reads.next()?)?;
            if self.mem.r8(DS, 0xAFF9) == 0 {
                break;
            }
        }
        self.mem.w16(DS, 0x8F94, y);
        self.mem.w16(DS, 0x8F92, x);
        if left {
            return Some(0xFFFF);
        }
        if keyed != 0xFFFF {
            return Some(keyed);
        }
        let cy = self.mem.r16(DS, 0x8F94);
        let mut dx = 0xFFFFu16;
        for &(seg, off) in &boxes.entries {
            if cy >= self.mem.r16(seg, off.wrapping_add(2)) {
                dx = dx.wrapping_add(1);
            }
        }
        Some(dx)
    }

    /// 0000:8FC4 as the menu's loop reads the mouse: the bytes the key
    /// interrupt took since (`key_event`), the buttons (AFF9) and the
    /// cursor's place (8F92, 8F94, kept within its limits) as `r` has them.
    fn menu_read(&mut self, r: &MenuRead) -> Option<()> {
        for &c in &r.codes {
            self.key_event(c)?;
        }
        self.mem.w8(DS, 0xAFF9, r.buttons);
        self.mem.w16(DS, 0x8F92, r.x);
        self.mem.w16(DS, 0x8F94, r.y);
        self.mouse_clamp();
        Some(())
    }
}
