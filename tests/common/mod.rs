pub mod blow;
pub mod controls;

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

/// The game's directory: ARENA_EXE's, or ESarena/ next to this crate.
#[allow(dead_code)]
pub fn game_dir() -> std::path::PathBuf {
    std::env::var("ARENA_EXE")
        .ok()
        .and_then(|p| std::path::Path::new(&p).parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| concat!(env!("CARGO_MANIFEST_DIR"), "/../ESarena").into())
}
