//! Why the port gave up. A routine that can't go on returns None; where it
//! knows why, it says so here on the way out, in one of three kinds:
//!
//! - `original`: the original itself faults or hangs there (a divide
//!   overflow, a #GP, a loop that never ends, a return into data); the
//!   port stops where the original would have died.
//! - `not_ported`: the original goes on into something the port doesn't
//!   run (a hook set to a routine not ported, a path not followed).
//! - `error`: a real error the original reports too (a file missing, the
//!   heap full).
//!
//! For reports only: nothing in the game reads it, and the None is the
//! same as ever. Kept per thread, so a routine with no machine to hand
//! can still say why; the last one said wins, and a front end clears it
//! (`take`) before the turn it wants to explain.

use std::cell::Cell;
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Original,
    NotPorted,
    Error,
}

impl Kind {
    /// The marker a report prints.
    pub fn marker(self) -> &'static str {
        match self {
            Kind::Original => "ORIGFAULT",
            Kind::NotPorted => "NOTPORTED",
            Kind::Error => "ERROR",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GaveUp {
    pub kind: Kind,
    pub why: &'static str,
}

impl fmt::Display for GaveUp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let what = match self.kind {
            Kind::Original => "the original faults here",
            Kind::NotPorted => "not ported",
            Kind::Error => "an error",
        };
        write!(f, "{what}: {}", self.why)
    }
}

thread_local! {
    static LAST: Cell<Option<GaveUp>> = const { Cell::new(None) };
    /// Every give-up noted on this thread since `noted` last took them
    /// (the first 64): for a test that plays on through the front end's
    /// `take`s and asks afterwards what gave up on the way.
    static ALL: std::cell::RefCell<Vec<GaveUp>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn note<T>(kind: Kind, why: &'static str) -> Option<T> {
    LAST.with(|last| last.set(Some(GaveUp { kind, why })));
    ALL.with(|all| {
        let mut all = all.borrow_mut();
        if all.len() < 64 {
            all.push(GaveUp { kind, why });
        }
    });
    None
}

/// Every give-up noted on this thread since the last call (up to 64),
/// cleared; `take` and `describe` leave them.
pub fn noted() -> Vec<GaveUp> {
    ALL.with(|all| std::mem::take(&mut *all.borrow_mut()))
}

/// None, noting the original faults or hangs here.
pub fn original<T>(why: &'static str) -> Option<T> {
    note(Kind::Original, why)
}

/// None, noting the original goes on into something not ported.
pub fn not_ported<T>(why: &'static str) -> Option<T> {
    note(Kind::NotPorted, why)
}

/// None, noting a real error (the original reports it too).
pub fn error<T>(why: &'static str) -> Option<T> {
    note(Kind::Error, why)
}

/// The last give-up noted, left in place.
pub fn peek() -> Option<GaveUp> {
    LAST.with(|last| last.get())
}

/// The last give-up noted, cleared.
pub fn take() -> Option<GaveUp> {
    LAST.with(|last| last.take())
}

/// What a front end says of a None: the reason noted (cleared, so the
/// next None isn't given it), or that none was.
pub fn describe() -> String {
    match take() {
        Some(g) => g.to_string(),
        None => "no reason noted".to_string(),
    }
}
