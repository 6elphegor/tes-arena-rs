//! Every hook trait method with a default body in src/ has its row in
//! docs/hooks.md (what the default stands for and whether play keeps the
//! original's behaviour), and every row a default: a default added
//! without being looked at fails here.
use std::collections::BTreeSet;
use std::path::Path;

/// The source with comments, strings and character literals blanked.
fn code(s: &str) -> String {
    let b = s.as_bytes();
    let (mut out, mut i) = (String::with_capacity(s.len()), 0);
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                while i < b.len() && b[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'"' => {
                i += 1;
                while i < b.len() && b[i] != b'"' {
                    i += if b[i] == b'\\' { 2 } else { 1 };
                }
                i += 1;
                out.push_str("\"\"");
            }
            b'\'' if b.get(i + 1) == Some(&b'\\') => {
                i += 2;
                while i < b.len() && b[i] != b'\'' {
                    i += 1;
                }
                i += 1;
            }
            b'\'' if b.get(i + 2) == Some(&b'\'') => i += 3,
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    out
}

/// "Trait::method" for each trait method with a body in `s`.
fn defaults(s: &str, out: &mut BTreeSet<String>) {
    let s = code(s);
    let b = s.as_bytes();
    let mut at = 0;
    while let Some(k) = s[at..].find("pub trait ") {
        let start = at + k + "pub trait ".len();
        let name: String = s[start..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        let Some(open) = s[start..].find(['{', ';']).map(|o| start + o) else { break };
        at = open + 1;
        if b[open] != b'{' {
            continue;
        }
        // The trait's body, and in it each `fn` at depth 1.
        let (mut depth, mut i) = (1, open + 1);
        while i < b.len() && depth > 0 {
            if depth == 1 && s[i..].starts_with("fn ") && !b[i - 1].is_ascii_alphanumeric() {
                let m: String = s[i + 3..].chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
                // Its signature runs to `;` (none) or `{` (a default) outside brackets.
                let (mut j, mut nest) = (i + 3, 0i32);
                while j < b.len() {
                    match b[j] {
                        b'(' | b'[' | b'<' => nest += 1,
                        b')' | b']' => nest -= 1,
                        b'>' if b[j - 1] != b'-' => nest -= 1,
                        b';' if nest <= 0 => break,
                        b'{' if nest <= 0 => {
                            out.insert(format!("{name}::{m}"));
                            break;
                        }
                        _ => {}
                    }
                    j += 1;
                }
                i = j;
                continue;
            }
            match b[i] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        at = i;
    }
}

fn walk(dir: &Path, out: &mut BTreeSet<String>) {
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            defaults(&std::fs::read_to_string(&p).unwrap(), out);
        }
    }
}

#[test]
fn every_hook_default_is_in_the_table() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut found = BTreeSet::new();
    walk(&root.join("src"), &mut found);
    let table: BTreeSet<String> = std::fs::read_to_string(root.join("docs/hooks.md"))
        .unwrap()
        .lines()
        .filter_map(|l| l.strip_prefix("| `")?.split('`').next().map(str::to_string))
        .collect();
    let missing: Vec<_> = found.difference(&table).collect();
    let stale: Vec<_> = table.difference(&found).collect();
    assert!(missing.is_empty() && stale.is_empty(), "docs/hooks.md lacks {missing:?}; has no default for {stale:?}");
    assert!(found.len() > 300, "{} defaults", found.len());
}
