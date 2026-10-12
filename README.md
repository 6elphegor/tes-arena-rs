# arena-port

A routine-by-routine Rust port of The Elder Scrolls: Arena (ACD.EXE, CD v1.07).
No game code or data is included: everything is loaded from your own copy.

## Layout

- `src/exe.rs` – unpacks ACD.EXE (PKLITE 1.12, extra compression, large
  relocation table) and checks it is the expected build.
- `src/mem.rs` – flat real-mode memory with the image at linear 0, so
  segments overlap as they really do (3276:92A0 is DGROUP 3BA0:0000).
- `src/machine.rs` – memory plus RNG; ported routines are methods on it.
- `src/random.rs` – random number generator (0000:6E10, 6EA8, 6EEA).
- `src/dice.rs`, `src/scale.rs` – dice rolls and percent/256-scale conversions.
- `src/character.rs` – character creation stats, attribute modifiers, hit
  and spell points (segment 1194).
- `harness/emu.py` – runs original routines under unicorn (far, near or a
  code range); CPU faults are reported as `Fault`.
- `harness/xref.py` – static helpers: data xrefs, pointer tables,
  disassembly, call sites.
- `harness/gen_*.py` – produce test vectors from the original code.
- `tests/` – differential tests against those vectors, plus loader tests.
- `tools/` – the original C/Python unpacking scripts; `re/` holds the rebuilt
  `ACD_unpacked.EXE` for disassembly (git-ignored).

## Porting a routine

1. Disassemble it from `re/ACD_unpacked.EXE` (rebuild with
   `tools/rebuild_mz.py` if missing).
2. Port it, keeping 16-bit semantics explicit (`wrapping_*`, exact widths)
   and keeping the original's bugs. State goes in a typed struct if
   understood, otherwise in `Memory` at the original segment:offset. Where
   the original faults, return `None`.
3. Add cases to a `harness/gen_*.py` and regenerate its vectors. Each case
   records the writes made before the call, the returned registers and
   every changed byte; the Rust test replays it and must match exactly.

Note: capstone prints opcode 99 as `cdq` in 16-bit code; it is `cwd`.

## Running tests

    cargo test

Loader tests read `../ESarena/ACD.EXE`, or the path in `ARENA_EXE`, and skip
if it is absent. Regenerating vectors needs `pip install unicorn capstone`:

    python harness/gen_rules.py re/ACD_unpacked.EXE tests/vectors/rules.txt
