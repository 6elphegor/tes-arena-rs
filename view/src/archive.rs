//! Reading the game's files by name, as the renderer needs them outside
//! the machine: a loose file in the game directory (any case), else a
//! member of GLOBAL.BSA (a count word, the members one after another, and
//! a directory of 18-byte entries at the end: a 12-byte name, then the
//! size as a long at +14).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Archive {
    /// The game directory's files by upper-case name, as it was listed
    /// when the archive was opened (the first of a name in the listing's
    /// order, where two differ only in case).
    loose: HashMap<String, PathBuf>,
    bsa: Vec<u8>,
    members: HashMap<String, (usize, usize)>,
}

impl Archive {
    pub fn open(dir: &Path) -> Self {
        let bsa = std::fs::read(dir.join("GLOBAL.BSA")).unwrap_or_default();
        let mut members = HashMap::new();
        if bsa.len() >= 2 {
            let n = u16::from_le_bytes([bsa[0], bsa[1]]) as usize;
            if let Some(dir_at) = bsa.len().checked_sub(n * 18) {
                let mut at = 2;
                for i in 0..n {
                    let e = &bsa[dir_at + i * 18..dir_at + (i + 1) * 18];
                    let name: String = e[..12].iter().take_while(|&&c| c != 0).map(|&c| c as char).collect();
                    let size = u32::from_le_bytes([e[14], e[15], e[16], e[17]]) as usize;
                    members.insert(name.to_ascii_uppercase(), (at, size));
                    at += size;
                }
            }
        }
        let mut loose = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                loose.entry(e.file_name().to_string_lossy().to_ascii_uppercase()).or_insert_with(|| e.path());
            }
        }
        Archive { loose, bsa, members }
    }

    /// The members' names (upper case).
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.members.keys().map(String::as_str)
    }

    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        let upper = name.to_ascii_uppercase();
        if let Some(path) = self.loose.get(&upper) {
            return std::fs::read(path).ok();
        }
        let &(at, size) = self.members.get(&upper)?;
        self.bsa.get(at..at + size).map(<[u8]>::to_vec)
    }
}
