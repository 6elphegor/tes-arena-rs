//! The game's heap (segment 0000): a list of blocks, each led by a
//! 16-byte header in the paragraph before its data:
//! - +0 far pointer to the next header (0:0 ends the list),
//! - +4 far pointer to the previous one,
//! - +8 size in paragraphs, not counting the header,
//! - +A bit 0 set while the block is in use.
//!
//! Headers sit at offset 0 of their paragraph in practice. The routines
//! nonetheless mix the stored offsets with offset 0 in places, and each
//! port does exactly what its original does with them.
//!
//! Two allocators share the list: first fit (0000:51F2), which reports a
//! corrupt list or exhausted heap through a hook and an int3, and last
//! fit (9CD6), which returns 0 instead. Unlike first fit, last fit does
//! not update the following block's back pointer when it splits a block,
//! so a later free may merge into a stale neighbour.

use crate::machine::Machine;
use crate::mem::DS;

/// DGROUP variables.
pub mod var {
    /// Far pointer to the first header (offset, segment).
    pub const FIRST: u16 = 0x0157;
    /// Top of memory (paragraph), from startup.
    pub const TOP: u16 = 0x015D;
    /// Non-zero when startup shrinks the DOS block (int 21h AH 4Ah to
    /// the heap's top less this, 0000:00DF). 0 in the image, and no
    /// instruction of the program writes it: the shrink never runs. Not
    /// a bug (#60): an option the build left off; the heap runs to the
    /// top of memory, which the game keeps for itself either way.
    pub const SHRINK: u16 = 0x002B;
    /// Free paragraphs, kept loosely: an exact fit subtracts the size,
    /// a split the size plus the new header, a free the size plus one
    /// per merge.
    pub const FREE: u16 = 0x8F6E;
    /// Far pointer called with an error code before the int3 (not ported).
    pub const ERROR_HOOK: u16 = 0x8F70;
}
use var::*;

/// Error codes passed to the hook.
pub const CORRUPT: u16 = 7;
pub const NO_MEMORY: u16 = 8;

/// List steps after which a walk must be going round a cycle: more
/// than twice the paragraphs in memory, so some header has repeated
/// (a corrupted list; the original loops for ever).
const WALK_LIMIT: u32 = 0x22_0000;

const NEXT: u16 = 0;
const PREV: u16 = 4;
const SIZE: u16 = 8;
const FLAGS: u16 = 0xA;

impl Machine {
    /// 0000:00CC (startup): one free block from the first header to
    /// 3 paragraphs short of TOP. `start` is the first header's segment
    /// (DGROUP 0159; offset 0157 stays as the image has it). None where
    /// startup would shrink its DOS block: never, SHRINK being 0 always.
    pub fn heap_init(&mut self, start: u16, top: u16) -> Option<()> {
        self.mem.w16(DS, FIRST + 2, start);
        self.mem.w16(DS, TOP, top);
        let paras = top.wrapping_sub(start).wrapping_sub(3);
        if self.mem.r16(DS, SHRINK) != 0 {
            return crate::giveup::not_ported("0000:00CC: startup shrinking its DOS block (SHRINK is 0 in every run of the original): not ported");
        }
        let (seg, off) = self.heap_far(DS, FIRST);
        for i in 0..4 {
            self.mem.w16(seg, off.wrapping_add(2 * i), 0);
        }
        self.mem.w16(seg, off.wrapping_add(SIZE), paras);
        self.mem.w8(seg, off.wrapping_add(FLAGS), 0);
        self.mem.w16(DS, FREE, paras);
        Some(())
    }

    /// 0000:51F2. First fit: allocates `paras` paragraphs and returns
    /// the far pointer DX:AX (the data segment, one past the header, and
    /// the header's offset). A larger block is split, its remainder
    /// becoming a new free block. None where the original reports an
    /// error: a free block of 0xA000 paragraphs or more, a fitting block
    /// of size 0, or a next pointer whose segment ORed with its offset
    /// exceeds its segment (all CORRUPT), or nothing fits (NO_MEMORY);
    /// also where a corrupted list cycles (WALK_LIMIT). Fixed (#59), the
    /// ordering check compares the header just left's segment with the
    /// next one's, as meant: a list that runs backwards is CORRUPT.
    pub fn heap_alloc(&mut self, paras: u16) -> Option<(u16, u16)> {
        let fixed = self.fixed(crate::fixes::HEAP_LINKS);
        let ax = paras;
        let (mut ds, mut si) = self.heap_far(DS, FIRST);
        // BX is 0 on the first check, then (segment | offset) of the
        // header just reached: the ordering check meant to compare
        // neighbouring segments tests that instead.
        let mut bx = 0u16;
        let mut steps = 0u32;
        let found = loop {
            steps += 1;
            if steps > WALK_LIMIT {
                return crate::giveup::original("0000:51F2: a broken heap chain: the original walks it for ever");
            }
            if bx > ds {
                return crate::giveup::error("0000:51F2: the heap reports an error");
            }
            if self.mem.r8(ds, si.wrapping_add(FLAGS)) & 1 == 0 {
                let size = self.mem.r16(ds, si.wrapping_add(SIZE));
                if size >= 0xA000 {
                    return crate::giveup::error("0000:51F2: the heap reports an error");
                }
                if ax <= size {
                    break (ds, si);
                }
            }
            let prev = ds;
            (ds, si) = self.heap_far(ds, si.wrapping_add(NEXT));
            bx = ds | si;
            if bx == 0 {
                return crate::giveup::error("0000:51F2: the heap reports an error");
            }
            if fixed {
                bx = prev;
            }
        };
        let (ds, si) = found;
        let size = self.mem.r16(ds, si.wrapping_add(SIZE));
        if size == 0 {
            return crate::giveup::error("0000:51F2: the heap reports an error");
        }
        let taken = if ax == size {
            let f = self.mem.r8(ds, si.wrapping_add(FLAGS)) | 1;
            self.mem.w8(ds, si.wrapping_add(FLAGS), f);
            ax
        } else {
            let new = ds.wrapping_add(ax).wrapping_add(1);
            self.heap_split(ds, si, new, ax);
            // The block after the new one points back at it (through
            // offset 0 of the next pointer's segment).
            let next = self.mem.r16(new, NEXT + 2);
            if next != 0 {
                self.mem.w16(next, PREV + 2, new);
            }
            ax.wrapping_add(1)
        };
        let free = self.mem.r16(DS, FREE).wrapping_sub(taken);
        self.mem.w16(DS, FREE, free);
        Some((ds.wrapping_add(1), si))
    }

    /// Both allocators' split: the header at `ds`:`si` is copied to
    /// `new`:0, then the block keeps `ax` paragraphs (its header written
    /// at offset 0), and the copy, now free space after it, loses ax + 1.
    fn heap_split(&mut self, ds: u16, si: u16, new: u16, ax: u16) {
        for i in 0..16 {
            let b = self.mem.r8(ds, si.wrapping_add(i));
            self.mem.w8(new, i, b);
        }
        self.mem.w16(ds, SIZE, ax);
        self.mem.w16(ds, NEXT + 2, new);
        let f = self.mem.r8(ds, FLAGS) | 1;
        self.mem.w8(ds, FLAGS, f);
        self.mem.w16(new, PREV + 2, ds);
        let s = self.mem.r16(new, SIZE).wrapping_sub(ax.wrapping_add(1));
        self.mem.w16(new, SIZE, s);
    }

    /// 0000:51EC (and far 51E2): `heap_alloc` of `bytes` rounded up to
    /// paragraphs (in 16 bits: 0xFFF1 and up round to 0).
    pub fn heap_alloc_bytes(&mut self, bytes: u16) -> Option<(u16, u16)> {
        self.heap_alloc(bytes.wrapping_add(15) >> 4)
    }

    /// 0000:9CD6 (far 9CD2). Last fit: allocates `bytes` (rounded up to
    /// paragraphs) from the last free block in the list that fits, and
    /// returns the data segment (AX is 0), or 0 if none fits. The
    /// following block's back pointer is left as it was. None where a
    /// corrupted list cycles (WALK_LIMIT). Fixed (#59), it points back at
    /// the split's new block, as first fit's split makes it.
    pub fn heap_alloc_high(&mut self, bytes: u16) -> Option<u16> {
        let ax = bytes.wrapping_add(15) >> 4;
        let mut dx = 0u16;
        let (mut ds, mut si) = self.heap_far(DS, FIRST);
        for steps in 0.. {
            if steps >= WALK_LIMIT {
                return crate::giveup::original("0000:9CD6: a broken heap chain: the original walks it for ever");
            }
            if self.mem.r8(ds, si.wrapping_add(FLAGS)) & 1 == 0 && ax <= self.mem.r16(ds, si.wrapping_add(SIZE)) {
                dx = ds;
            }
            (ds, si) = self.heap_far(ds, si.wrapping_add(NEXT));
            if ds | si == 0 {
                break;
            }
        }
        if dx == 0 {
            return Some(0);
        }
        // SI is 0 here (the list ended at 0:0): the header is used at
        // offset 0.
        let ds = dx;
        let taken = if ax == self.mem.r16(ds, SIZE) {
            let f = self.mem.r8(ds, FLAGS) | 1;
            self.mem.w8(ds, FLAGS, f);
            ax
        } else {
            let new = ds.wrapping_add(ax).wrapping_add(1);
            self.heap_split(ds, 0, new, ax);
            if self.fixed(crate::fixes::HEAP_LINKS) {
                let next = self.mem.r16(new, NEXT + 2);
                if next != 0 {
                    self.mem.w16(next, PREV + 2, new);
                }
            }
            ax.wrapping_add(1)
        };
        let free = self.mem.r16(DS, FREE).wrapping_sub(taken);
        self.mem.w16(DS, FREE, free);
        Some(ds.wrapping_add(1))
    }

    /// 0000:6C36 (far 6C32 with the segment in AX, 6C26 with a far
    /// pointer on the stack). Frees the block whose data starts at
    /// `seg` (0 does nothing), merging it into a free previous block and
    /// then a free next one. Nothing checks that the block was in use.
    pub fn heap_free(&mut self, seg: u16) {
        if seg == 0 {
            return;
        }
        let mut ds = seg.wrapping_sub(1);
        let si = 0u16;
        let mut bx = self.mem.r16(ds, SIZE);
        let f = self.mem.r8(ds, FLAGS) & 0xFE;
        self.mem.w8(ds, FLAGS, f);
        let (es, di) = self.heap_far(ds, PREV);
        if es | di != 0 && self.mem.r8(es, di.wrapping_add(FLAGS)) & 1 == 0 {
            let (ns, no) = self.heap_far(ds, NEXT);
            if ns | no != 0 {
                self.mem.w16(ns, no.wrapping_add(PREV), di);
                self.mem.w16(ns, no.wrapping_add(PREV + 2), es);
            }
            let next = self.mem.r32(ds, NEXT);
            self.mem.w32(es, di.wrapping_add(NEXT), next);
            let grow = self.mem.r16(ds, SIZE).wrapping_add(1);
            bx = bx.wrapping_add(1);
            let s = self.mem.r16(es, di.wrapping_add(SIZE)).wrapping_add(grow);
            self.mem.w16(es, di.wrapping_add(SIZE), s);
            // The merged block is taken to be at es:0, whatever di was.
            ds = es;
        }
        let (es, di) = self.heap_far(ds, si.wrapping_add(NEXT));
        if es | di != 0 && self.mem.r8(es, di.wrapping_add(FLAGS)) & 1 == 0 {
            let next = self.mem.r32(es, di.wrapping_add(NEXT));
            self.mem.w32(ds, si.wrapping_add(NEXT), next);
            let grow = self.mem.r16(es, di.wrapping_add(SIZE)).wrapping_add(1);
            bx = bx.wrapping_add(1);
            let s = self.mem.r16(ds, si.wrapping_add(SIZE)).wrapping_add(grow);
            self.mem.w16(ds, si.wrapping_add(SIZE), s);
            let (ns, no) = self.heap_far(ds, si.wrapping_add(NEXT));
            if ns | no != 0 {
                self.mem.w16(ns, no.wrapping_add(PREV), si);
                self.mem.w16(ns, no.wrapping_add(PREV + 2), ds);
            }
        }
        let free = self.mem.r16(DS, FREE).wrapping_add(bx);
        self.mem.w16(DS, FREE, free);
    }

    /// Far pointer (segment, offset) stored offset first at `seg`:`off`.
    fn heap_far(&self, seg: u16, off: u16) -> (u16, u16) {
        (self.mem.r16(seg, off.wrapping_add(2)), self.mem.r16(seg, off))
    }

    /// Whether the data at `seg` is an in-use block of the list of at least
    /// `paras` paragraphs (its header the paragraph before), for a front
    /// end writing into a buffer the game's words name.
    pub fn heap_block_used(&self, seg: u16, paras: u16) -> bool {
        let (mut ds, mut si) = self.heap_far(DS, FIRST);
        for _ in 0..0x10000 {
            if (ds | si) == 0 {
                return false;
            }
            if ds.wrapping_add(1) == seg && si == 0 {
                return self.mem.r8(ds, FLAGS) & 1 != 0 && self.mem.r16(ds, SIZE) >= paras;
            }
            (ds, si) = self.heap_far(ds, si.wrapping_add(NEXT));
        }
        false
    }

    /// 2294:11F2. Checks the heap list: every header must have a non-zero
    /// size under 0x6000 paragraphs, zero offsets in both links and a
    /// flags byte of 0 or 1. False where the original stops at an int3.
    /// A list that cycles is followed up to WALK_LIMIT steps.
    pub fn heap_check(&self) -> bool {
        let (mut ds, mut si) = self.heap_far(DS, FIRST);
        for _ in 0..WALK_LIMIT {
            let size = self.mem.r16(ds, si.wrapping_add(SIZE));
            let flags = self.mem.r8(ds, si.wrapping_add(FLAGS));
            if size == 0
                || self.mem.r16(ds, si.wrapping_add(NEXT)) != 0
                || self.mem.r16(ds, si.wrapping_add(PREV)) != 0
                || size >= 0x6000
                || flags > 1
            {
                return false;
            }
            if self.mem.r32(ds, si.wrapping_add(NEXT)) == 0 {
                return true;
            }
            (ds, si) = self.heap_far(ds, si.wrapping_add(NEXT));
        }
        false
    }
}
