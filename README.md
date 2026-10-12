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
  and spell points (segment 1194). `character::field` is the one place
  character/creature record offsets are named.
- `src/progression.rs` – levels from XP, level-up, XP awards (segment 2686).
- `src/combat.rs` – to-hit, hit location, criticals, damage, weapon wear
  (segment 2AA1).
- `src/loot.rs` – item and treasure generation (1194:E8xx-F3xx, 24FA).
- `src/shop.rs` – shop stock, potions and magic items (1194:FAD9, B7FA,
  B848).
- `src/haggle.rs` – haggling when buying or selling (1194:8020); the
  player's answers come from a `HaggleIo`. Temple and guild services
  (modes 5, 6) are not ported.
- `src/item.rs` – the 19-byte item slot layout shared by loot, inventory
  and combat.
- `src/inventory.rs` – inventory slots, equipment lookups (armour per
  location, weapon, shields, missile), class usability, add/remove/sort.
- `src/city.rs` – city block plans and block file names from a city's
  seed, location records and templates (1194:0292, 3B84). Loading the
  MIFs is in `src/mif.rs`.
- `src/bsa.rs`, `src/mif.rs` – GLOBAL.BSA access (including the XOR
  decryption of .INF/.TXT entries), the MIF decompressor (LZHUF), city
  block loading, the wall pass and whole cities (1194:0292).
- `src/level.rs` – the .MIF level loader (0000:E87C) used for city
  templates, interiors and dungeons: header, level selection, planes,
  locks, triggers, lights, text and info chunks.
- `src/heap.rs` – the game's heap: startup setup, first-fit and last-fit
  allocation, free with merging. Loaders and the decompressor allocate
  from it as the original does; set it up with `heap_init` first.
- `src/dungeon.rs` – dungeon records and seeds, level count, layout,
  stairs, whole levels built from random1.mif, main-quest dungeon
  selection.
- `src/creature.rs` – NPC and monster generation, corpse loot,
  monster spawning.
- `src/save.rs` – saved games: slot file names, paths, the engine save
  (SAVEENGN.nn, decrypted), the level save (SAVEGAME.nn: screenshot,
  palette, state and planes) and spell files. Tests with a save need
  `ARENA_SAVE` set to its folder.
- `src/travel.rs` – fast travel: days and distance between two places,
  over the terrain map and the weather (1194:2E69), and the monthly
  weather roll (1194:2FB3).
- `src/wild.rs` – the wilderness grid around a location (217B:0004).
- `src/log.rs` – the calendar (dates from the minute counter) and
  logbook entries; appending them to LOG.nn is not ported.
- `src/names.rs` – random NPC names from NAMECHNK.DAT (1194:080B);
  `expand_npc_named` builds a whole NPC with its name.
- `examples/city_map.rs`, `examples/dungeon_map.rs` – build a city or a
  dungeon from your game files with the ported code and write its map as
  a PPM image: `cargo run --release --example city_map -- ../ESarena 8 0
  imperial.ppm`. `examples/character_sheet.rs` loads a saved game and
  prints the character, date, logbook, inventory and spells; `examples/save_view.rs`
  writes a save's stored screenshot and the map of the level it is in;
  `examples/wild_map.rs` draws a location's wilderness grid and four of
  its blocks as terrain; `examples/travel.rs` plans a journey.
- `harness/dos.py` – a minimal emulated DOS file layer (int 21h) so file
  routines can run unmodified against the user's own game files, and
  `init_heap` to set up the game's heap.
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
   the original faults or hangs, return `None`.
3. Add cases to a `harness/gen_*.py` and regenerate its vectors. Each case
   records the writes made before the call, the returned registers and
   every changed byte; the Rust test replays it and must match exactly.

Pitfalls:

- capstone mislabels sign extensions in 16-bit code: opcode 99 prints as
  `cdq` but is `cwd`, and 98 prints as `cwde` but is `cbw`.
- unicorn caches translated code, so patching code bytes after it has run
  has no effect. Stub calls with fixed code that reads its result from
  data (see `harness/gen_combat.py`).
- The image's RNG seed is 0, a fixed point of the LCG. Seed the machine
  before running anything that rerolls.
- Private helpers on `Machine` share one namespace across modules; give
  them module-specific names.
- DGROUP holds relocated segment words, which differ between the
  emulator (loaded at 1000h) and the port (image-relative). Keep test
  pointers in memory the case writes, and convert stored segment values
  back to image-relative before diffing.
- capstone reports near branch targets past offset 0xFFFF unwrapped;
  `xref.py` wraps them (a `call 0x1533c` in segment 0 is 0000:533C).
- unicorn stops silently on a faulting `div` and on an exhausted
  instruction budget; check where it stopped before trusting a result.
- Reroll loops that can only end on a different RNG value hang when the
  seed sits on a short cycle; use `random::CycleCheck` and return None.
- Vector runs that decompress under unicorn are slow (an hour for
  `gen_level.py`): write output line-buffered and make long generators
  resumable.
- In an agent worktree the game is not at `../ESarena`; set `ARENA_EXE`
  or the game-dependent tests skip silently.

## Running tests

    cargo test

Loader tests read `../ESarena/ACD.EXE`, or the path in `ARENA_EXE`, and skip
if it is absent. Regenerating vectors needs `pip install unicorn capstone`:

    python harness/gen_rules.py re/ACD_unpacked.EXE tests/vectors/rules.txt
