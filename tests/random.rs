//! Differential test against vectors produced by running the original
//! machine code (harness/gen_random.py).
use arena::random::{ArenaRandom, Table};

#[test]
fn matches_original() {
    let text = include_str!("vectors/random.txt");
    let (mut ranges, mut tables) = (0, 0);
    for line in text.lines() {
        let mut parts = line.split(" | ");
        let head: Vec<&str> = parts.next().unwrap().split(' ').collect();
        let n = |s: &str| s.parse::<u32>().unwrap();
        match head[0] {
            "range" => {
                let mut r = ArenaRandom::new(n(head[1]));
                let got = r.range(n(head[2]) as u16, n(head[3]) as u16).unwrap();
                assert_eq!(got as u32, n(head[4]), "{line}");
                assert_eq!(r.seed, n(head[5]), "{line}");
                ranges += 1;
            }
            "table" => {
                let mut words: Vec<u16> = head[4..].iter().map(|s| n(s) as u16).collect();
                words.resize(0x8000, 0);
                let mut r = ArenaRandom {
                    seed: n(head[1]),
                    table: Some(Table { words, offset: 0, remaining: n(head[3]) as u16, refill: n(head[2]) as u16 }),
                };
                let want: Vec<u32> = parts.next().unwrap().split(' ').map(n).collect();
                let got: Vec<u32> = want.iter().map(|_| r.next() as u32).collect();
                assert_eq!(got, want, "{line}");
                assert_eq!(r.seed, n(parts.next().unwrap()), "{line}");
                tables += 1;
            }
            _ => unreachable!(),
        }
    }
    assert!(ranges > 0 && tables > 0);
}
