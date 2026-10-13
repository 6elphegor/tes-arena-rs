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
- `src/attack.rs` – a blow at the player (2AA1:00D8): a creature's damage
  range and special attacks or a person's weapon, wear on the armour hit
  and broken items ("Your %s is broken."), the magic shield, and death.
- `src/strike.rs` – a landing swing (0000:39D8): every actor in reach in
  front of the player is struck; people die at once (0000:3AED: gold,
  and maybe the guards); the player's strike on a creature-slot actor
  (0000:3B6D, 2AA1:0000): blood, criticals, damage and weapon
  enchantments; and the kill handler all damage goes through (2AA1:06A0):
  death, corpses moved off doors, experience, quest targets that die or
  give up. `GameHooks` gives the effects engine these handlers.
- `src/spell.rs` – a cast applied to its targets (29CF:0941): their
  defences, then the spell's three effects (2CDD:000C and some 25
  handlers): damage and healing (through the magic shield and elemental
  resistance, 29CF:0B35), status effects, cures (2EBD:0004), timed spell
  flags, attribute, HP, SP and fatigue changes and transfers, instant
  death, levitation; and an actor's cast (29CF:0145), launched at the
  player or taking effect on itself. A projectile spawned by an effect
  is not ported.
- `src/projectile.rs` – spells in flight and arrows (segment 29CF): the
  launch (29CF:02E2) with its sprite object, light and column list
  (0000:FDEA, E0CA, BFC0), the per-frame mover (29CF:0441) with its
  hit tests against actors, the player and walls (0000:F4EB),
  explosions, arrows striking, and the targets of a spell's impact
  (29CF:09B8, 2CDD:08D9), which then takes effect (`spell.rs`).
- `src/format.rs` – the game's sprintf for `%s` and `%d` formats (line
  feeds become CR LF).
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
  decryption of .INF/.TXT entries and the directory cached in EMS), the
  MIF decompressor (LZHUF), city block loading, the wall pass and whole
  cities (1194:0292).
- `src/level.rs` – the .MIF level loader (0000:E87C) used for city
  templates, interiors and dungeons: header, level selection, planes,
  locks, triggers, lights, text and info chunks; reloading a level on
  stairs (0000:EBB5); EMS (0000:5C8A), modelled as 64 KB banks swapped
  through the frame.
- `src/info.rs` – the level .INF loader (0000:EC03): which textures the
  map uses, texture files read into EMS banks, wall and floor roles
  (stairs, chasms, triggers, box sides and caps, ceiling), flat
  records, sounds and texts, and the flat files read whole into EMS
  (0000:BED8, through the EMS allocator 0000:5C26).
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
- `src/movement.rs` – the movement step and collision (0000:F8DD,
  F478, F60A, F697, FB3A): the 16 x 16 cell mask around a square,
  sliding along walls, triggers, climbing out of low areas, and level
  changes on stairs. Creatures that walk into a closed door open it
  unless it is locked (the game's lock check for creatures). The
  physics frame (0000:F57C): falling and the floor height (F9C4: raised
  squares, water and pits, bobbing), with the input step and level
  transitions as `PhysicsHooks`.
- `src/controls.rs` – the player's controls (2826:0002, the physics
  frame's input hook). The original steers with the mouse: the view is a
  3 x 3 grid of regions (forward, back, turn, strafe, centre), and the
  cursor's distance from a region's outer edge sets the speed; the arrow
  keys call the same handlers with the cursor at the edge. Also jumping,
  the levitation bob, and cancelling a spell waiting for a target. The
  front end reports the mouse and keys (`set_mouse`, `set_key`) and calls
  `player_input`, or drives the same movement with modern controls
  (`player_steer` with an `Intent`: forward, strafe, turn, mouse look,
  jump). `controls::Classic` plugs the controls into the physics frame.
- `src/click.rs` – a left click in the view (2826:077E): spells waiting
  for a target (removing or raising walls, digging or filling pits,
  Open and Lock on doors), doors (opened with a key, or picked by a
  lockpicking roll), and the click left for the actors. What is under the
  cursor comes from the renderer (the wall square, the door's span and
  square, and the floor square through `ControlHooks::floor_pick`).
- `src/weapon.rs` – the player's weapon each frame (0000:38CE): drawing
  and sheathing it, and swinging it. A swing starts with a right-button
  drag of 40h mickeys or more (the mouse's motion counters are input,
  `Machine::mouse_motion`); its direction picks the swing; every fifth
  frame it lands (`strike::swing_hits`), and with no actor in reach it
  hits the square ahead (0000:3D43): breaking into a closed building in a
  town (a crime that may bring the guards), or bashing a breakable flat
  or a locked door by the blow's damage and strength. A bow shoots an
  arrow aimed by the cursor (`projectile::fire_missile`, 29CF:037F);
  drawing the hands is the renderer's.
- `src/frame.rs` – the world loop's frame body (0000:481E): the detail
  and debug keys, the acceleration's decay, walking and turning speeds
  from speed and fatigue, the wilderness flats, levitation and the light
  radius, the weapon, the message and guards' countdowns, drowning, a
  holiday's message, breath in deep water, the night's creatures in
  towns, the clock (a minute every 90 timer ticks) and the hurt flash.
  Drawing, sound, menus and message boxes are `FrameHooks`.
- `src/cast.rs` – the player casting (segment 29CF): from the
  spellbook (29CF:071F: silence, the cost by level, the crime of casting
  at someone in town), a spell takes effect on the player or waits for a
  target with its prompt; a click resolves it (29CF:0000): a touch on an
  actor or the player, an aimed spell launched along the cursor's
  direction (`projectile::launch_player`, which loads the casting hands'
  graphic), or an area around the player.
- `src/encounter.rs` – fights starting (segment 25CA): the fight set up
  for monsters or people (25CA:0000, the graphics and music as hooks),
  and the creatures placed in the free creature slots around the player
  (25CA:006A: a random free square near the player, facing the player);
  the guards arriving (1194:33C0: one to three named guards of the
  province's race, rank and level, in armour with a weapon and shield);
  random encounters (1194:887A: the area's chance, a row of the table by
  area and night, an entry by the player's level; monsters, or people
  with their equipment, 24FA:0313). The quests' encounters (86F2) are
  not ported.
- `src/engine.rs` – one frame of the 3D world (the world loop 0000:A240
  without its drawing): actors and projectiles, physics with the input
  step (`Input::Classic` or `Input::Steer`), the state the start of the
  original's drawing sets (`view_begin`, 0000:C923: the player's square
  CB90, which the encounters, the actors and the clicks read) with the
  `EngineHooks::draw` hook in place of its walk over the squares, the
  wall spells' squares, the frame body, a pending INF; `enter_area` sets
  a loaded level up (the state half of 0000:A4ED). `Engine::view` gives the renderer the
  player's position, heading, light, time of day and message;
  `examples/play.rs` runs it on a level with scripted modern controls.
- `src/actors.rs` – the people and creatures of a level and their frame
  (0000:1854): idle walking, facing and chasing the player, side steps
  when blocked, line of sight, respawning near the player, animation
  states and the sprite seen, blows at the player, and casting (people
  pick an affordable spell, creatures one of theirs). Sound and the draw
  list are not ported.
- `src/clock.rs` – the game clock (0000:142D): minutes pass one at a
  time, the calendar is recomputed and each changed field's handler runs:
  the minute tick (auras, timed spell flags, effects, the drain check,
  countdowns, level-up, fatigue, encounter checks), the day (weather,
  Knights' repairs), the hour; and the sky's time of day. Quests,
  encounters, the level-up screen and exhaustion go through `ClockHooks`.
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
  its blocks as terrain; `examples/travel.rs` plans a journey;
  `examples/walk.rs` lets a walker wander a level with the movement code
  and draws its path.
- `harness/dos.py` – a minimal emulated DOS file layer (int 21h) so file
  routines can run unmodified against the user's own game files,
  `init_heap` to set up the game's heap, and `enable_ems` for EMS bank
  switching (int 67h 4E02).
- `harness/emu.py` – runs original routines under unicorn (far, near or a
  code range); CPU faults are reported as `Fault`.
- `harness/xref.py` – static helpers: data xrefs, pointer tables,
  disassembly, call sites.
- `harness/gen_*.py` – produce test vectors from the original code.
- `tests/` – differential tests against those vectors, plus loader tests.
- `tools/` – the original C/Python unpacking scripts; `re/` holds the rebuilt
  `ACD_unpacked.EXE` for disassembly (git-ignored).

## The renderer (view/)

`arena-view` is a new wgpu renderer, not a port of the original's: the
port's engine runs the game, and the renderer draws what is in its memory.
The core crate stays free of dependencies; the view crate is a workspace
member using wgpu, winit, glam and png.

- `world.rs` loads a level the way `examples/play.rs` does. It reads the
  palette (PAL.COL), the textures from the EMS banks the INF loader filled
  (64 x 64, stored transposed, 15 to a bank) and the shading table (F000 in
  the first texture bank). It builds triangles from the map planes:
  - solid walls (equal bytes), with the faces between walls culled;
  - raised platforms (the box side and cap tables, with heights from
    `move_wall_height`);
  - doors and the other wall kinds;
  - floors (FLOOR words), and the *CEILING texture.
- `flats.rs` decodes the flats the INF loader read into the flat banks:
  - raw .IMGs;
  - .DFA animations: a run-length coded first frame, then chunks of
    updates to it;
  - .CFAs: run-length coded rows of packed values, high bits first,
    coloured through the header's table.

  `archive.rs` reads files by name (loose, or from GLOBAL.BSA) for the
  creatures' CFAs: the type's name pattern from the table in the image
  (`rat@.cfa` ...), views 1 (front) to 5 (back), and 6.
- `world::sprite_sheet` packs every frame into one atlas.
  `world::sprites` makes billboards, a unit per pixel:
  - the map's flats (MAP1 words 8xxx), in their squares, raised by the
    flat record's Y;
  - the creatures in the actor slots: sprite C0h + view, mirrored with
    flag 2, at the actor's frame.

  The examples `flats` (every frame on one sheet), `formats` (what the
  decoders miss, by level) and `actors` (the slots in use) check them.
- `render.rs` and `shader.wgsl` keep the textures as palette indices,
  shade them by distance through the original's table, then colour them
  with the palette.
- `main.rs` turns the keys and the mouse into an `Intent` for
  `Engine::frame`, run at the game's pace. Its `draw` hook steps the
  changed squares within the view's reach. The original stepped only
  the squares it drew. F (or `--fight`) rolls random encounters until
  creatures come, a debugging aid.

```
cargo run --release -p arena-view -- GAME_DIR LEVEL.MIF
cargo run --release -p arena-view -- GAME_DIR LEVEL.MIF [--fight] [--wait N] --shot out.png [FRAMES [TURN]]
```

Not drawn yet: people (their graphics come from a list, 0000:1AF9),
cities (their flats load elsewhere), doors as they open, chasms and
water, the sky, and the interface. How a dungeon's placed creatures
(map flats naming creature files, such as rat.dfa) come to life is not
found yet.

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
