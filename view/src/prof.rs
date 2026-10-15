//! Named timers for seeing where the view's time goes. They cost one
//! check of a flag unless `ARENA_PROFILE` is set; then each `time` adds
//! its span to its name's count, total and worst, and `report` prints
//! them (`--bench`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

thread_local! {
    static ON: bool = std::env::var_os("ARENA_PROFILE").is_some();
    static TIMES: RefCell<BTreeMap<&'static str, (u32, Duration, Duration)>> = const { RefCell::new(BTreeMap::new()) };
}

/// Whether the timers are on.
pub fn on() -> bool {
    ON.with(|o| *o)
}

/// `f` run, its time added to `name`'s when the timers are on.
pub fn time<R>(name: &'static str, f: impl FnOnce() -> R) -> R {
    if !on() {
        return f();
    }
    let t = Instant::now();
    let r = f();
    add(name, t.elapsed());
    r
}

/// A span added to `name`'s.
pub fn add(name: &'static str, d: Duration) {
    TIMES.with(|t| {
        let mut t = t.borrow_mut();
        let e = t.entry(name).or_insert((0, Duration::ZERO, Duration::ZERO));
        e.0 += 1;
        e.1 += d;
        e.2 = e.2.max(d);
    });
}

/// The timers forgotten.
pub fn reset() {
    TIMES.with(|t| t.borrow_mut().clear());
}

/// Each name's count, mean and worst, and its total.
pub fn report() {
    TIMES.with(|t| {
        println!("{:<22} {:>7} {:>11} {:>11} {:>11}", "timer", "count", "mean µs", "max µs", "total ms");
        for (name, (n, total, max)) in t.borrow().iter() {
            let mean = total.as_secs_f64() * 1e6 / (*n).max(1) as f64;
            println!("{name:<22} {n:>7} {mean:>11.1} {:>11.1} {:>11.1}", max.as_secs_f64() * 1e6, total.as_secs_f64() * 1e3);
        }
    });
}
