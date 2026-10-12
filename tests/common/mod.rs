use arena::exe;

/// The user's ACD.EXE: ARENA_EXE, or ESarena/ next to this crate.
pub fn image() -> Option<exe::Image> {
    let path = std::env::var("ARENA_EXE")
        .unwrap_or_else(|_| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena/ACD.EXE").into());
    match std::fs::read(&path) {
        Ok(b) => Some(exe::unpack(&b).expect("unpack")),
        Err(_) => {
            eprintln!("skipping: {path} not found");
            None
        }
    }
}
