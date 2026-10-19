# Hook trait defaults

Every engine hook trait method with a default body (`src/`), what it stands
for in the original, and whether play keeps the original's behaviour.
`tests/hookdefaults.rs` fails when a default is added or removed without
its row here.

Verdicts:
- **overridden**: a front-end impl used in play (`view/src`) or an engine
  adapter replaces the default.
- **port**: the default runs the ported routine.
- **original**: the default is what the original does there (a RETF, no
  CyberMan, a flag the routine leaves).
- **stub**: a test's stand-in hook; the default lets the port run.
- **elsewhere**: the view does the routine's work outside the hook.
- **present**: drawing, fades, sound or input waits only; no game state.
- **unreached**: no port path calls it in the state that matters.
- **left**: the original's behaviour is lost in play.

The last column checks each **overridden** default's override against
the original routine:
- **faithful**: it runs the port (or does what the routine does) and
  leaves the game's state as the original does; what checks it.
- **present**: drawing, the player's answers or a wait: no game state
  beyond what the port sets; the screen tests that check it.
- **elsewhere**: the view does the routine's work outside the hook.
- **gpu-only**: used only for the view's own GPU world (--gpu-world);
  with the engine's page the drawing runs the routine itself.
- **fixed**: it differed from the original in play; what was wrong and
  how it is now.

| Hook | Where | Original | Verdict | Note | Override |
|---|---|---|---|---|---|
| `AreaHooks::ask_leave` | src/transit.rs:115 | 26F7:0000 | overridden | the engine: Exit<'_> | faithful: the engine's `Exit` runs the travel map's port (travelmap vectors) |
| `AreaHooks::automap` | src/transit.rs:109 | 292B:095E | overridden | the view: areas.rs Areas<'_> | faithful: `states_keep` (states vectors) |
| `AreaHooks::border` | src/transit.rs:149 | 0000:3503 | overridden | the view: areas.rs Areas<'_> | faithful: `automap_load` (automap vectors) |
| `AreaHooks::build_city` | src/transit.rs:96 | 1194:0292 | overridden | the view: areas.rs Areas<'_> | faithful: `mif_build_city` (city vectors) |
| `AreaHooks::build_dungeon` | src/transit.rs:102 | 2294:0ED4 and 0000:28BC | overridden | the view: areas.rs Areas<'_> | faithful: `dungeon_create` with the seed (dungeon vectors) |
| `AreaHooks::city_screen` | src/transit.rs:106 | 2294:11F2 | present | 2294:11F2 is the heap check (int3 on a bad heap); nothing to keep | |
| `AreaHooks::ems_free` | src/transit.rs:166 | 0000:5D70 | port | fixed by this audit: was empty, so entering an area never freed the EMS bank table (`ems_reset`) | |
| `AreaHooks::ending` | src/transit.rs:87 | 1194:D459 | overridden | the view: areas.rs Areas<'_> | faithful: `Machine::vision` (its pages clicked through, `ClickThrough`); the window shows the farewell's vision (`Creation::ending`, vision.txt frame by frame) |
| `AreaHooks::entered` | src/transit.rs:180 | 0000:468B | overridden | the view: areas.rs Areas<'_> | faithful: `area_entered_inside` / `city_entered` (entered, cityinfo vectors; state tests from the load); outdoors 46AF's `outdoor_light` with its loads (`LightNow`), the light handed to the assets (was the assets' own, after the load: savefiles.txt `loadplace` for the street and the wilderness) |
| `AreaHooks::hands` | src/transit.rs:159 | 0000:068A | overridden | the view: areas.rs Areas<'_> | faithful: `sky_reset`, then 06B8's day's colours (`sky_colours_load`; was the assets' own, after the load) (state tests, savefiles.txt `loadplace`) |
| `AreaHooks::hook_2631` | src/transit.rs:162 | 0000:2631 | port | fixed by this audit: was empty, so a player loaded or entering inside a wall stayed in it (`player_unstick`) | |
| `AreaHooks::hook_2974` | src/transit.rs:139 | 0000:2974 | overridden | the view: areas.rs Areas<'_> | faithful: `dungeon_first_level` (dungeon vectors) |
| `AreaHooks::leave_dungeon` | src/transit.rs:129 | 2433:0199 | port | the default runs the port | |
| `AreaHooks::leave_interior` | src/transit.rs:123 | 1194:D55F and 2433:0199 | port | the default runs the port | |
| `AreaHooks::leave_screen` | src/transit.rs:119 |  | present | the screens shown on the way out; the view draws its own | |
| `AreaHooks::location` | src/transit.rs:92 | 1194:3AE8 | overridden | the view: areas.rs Areas<'_> | faithful: `city_location` (city vectors) |
| `AreaHooks::map_keep` | src/transit.rs:121 | 0000:4564..4568 | overridden | the engine: Exit<'_> | faithful: `map_keep` with the stairs' hooks' colours |
| `AreaHooks::map_given_back` | src/transit.rs:128 | 0000:457C..4586, 45A4..45A9 | port | the default runs the port (`view_window`, `world_back`); the transit vectors' stub counts it | |
| `AreaHooks::mouse_wait` | src/transit.rs:146 | 0000:8FC4 | present | waiting for the buttons up; input only | |
| `AreaHooks::normal_lgt` | src/transit.rs:89 | 0000:742C | overridden | the view: areas.rs Areas<'_> | faithful: NORMAL.LGT loaded as 0000:742C loads it |
| `AreaHooks::restore_level` | src/transit.rs:151 | 25FD:034B and 0000:16F7 | unreached | only with 5C8 bit 4, which `title_load` clears after its own `save_restore` | |
| `AreaHooks::sky` | src/transit.rs:137 | 0000:464A | overridden | the view: areas.rs Areas<'_> | faithful: `area_sky` with the game's loads (`file_load_ems`, 7428 with DI FFFDh), else PAL.COL to 929E; was the assets' own after the entry, with the view's reads |
| `AreaHooks::slots_usable` | src/transit.rs:154 | 25CA:01E6 | port | the default runs the port | |
| `AreaHooks::spawn` | src/transit.rs:134 | 0000:1793 | overridden | the view: areas.rs Areas<'_> | faithful: `spawn_actors` (actors vectors) |
| `AreaHooks::sync` | src/transit.rs:82 | 2F9F:0438 | port | the default runs the port | |
| `AreaHooks::townsfolk_graphics` | src/transit.rs:104 | 0000:17F9 | overridden | the view: areas.rs Areas<'_> | faithful: `townsfolk_load` (actors vectors; playsky exact) |
| `AreaHooks::weather` | src/transit.rs:94 | 0000:0F05 | overridden | the view: areas.rs Areas<'_> | faithful: `weather_set` (state tests in rain and snow) |
| `AreaHooks::wild_done` | src/transit.rs:100 | 217B:04DC | overridden | the view: areas.rs Areas<'_> | faithful: `wild_done` |
| `AreaHooks::wild_empty` | src/transit.rs:142 | 217B:0184 | overridden | the view: areas.rs Areas<'_> | faithful: `wild_shift`, `wild_quarter_ok` (wild vectors; playwild) |
| `AreaHooks::wild_setup` | src/transit.rs:98 | 217B:0084 | overridden | the view: areas.rs Areas<'_> | faithful: `wild_setup` (wild vectors) |
| `AreaHooks::world_free` | src/transit.rs:175 | 0000:A5A0 | port | `draw_buffers_free`: the renderer's buffers (A561's, `draw_buffers_alloc`) and the light table freed, as the area's entry made them | |
| `AreaHooks::world_screen` | src/transit.rs:171 |  | overridden | the view: areas.rs Areas<'_> | faithful: `automap_keep`; the screens' fades are the window's (areafade.txt) |
| `AutomapScreenHooks::palette` | src/automapscreen.rs:20 | 0000:6E3C | overridden | the view: automap.rs Colours | faithful: the window's colours as the VGA's (playautomap's state) |
| `CastHooks::message_box` | src/cast.rs:42 | 1194:59C8 | overridden | the view: panels.rs Said | present: the cast's box in the equipment page (screens.txt) |
| `CelestialHooks::draw` | src/celestial.rs:35 | 0000:E4D2 | overridden | the view: main.rs CelestialDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `CelestialHooks::moon_phase` | src/celestial.rs:31 | 0000:E6CB | overridden | the view: main.rs CelestialDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `CelestialHooks::star_image` | src/celestial.rs:41 |  | overridden | the view: main.rs CelestialDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `CelestialHooks::star_pixel` | src/celestial.rs:38 |  | overridden | the view: main.rs CelestialDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `ClockHooks::encounter` | src/clock.rs:67 | 1194:887A | overridden | the view: main.rs Front<'_> | faithful: `random_encounter` (encounter vectors) |
| `ClockHooks::encounter_minute` | src/clock.rs:73 | 1194:86F2 | overridden | the view: main.rs Front<'_> | faithful: `quest_ambush` (ambush vectors); its fight's music is `FightHooks::fight_music`'s |
| `ClockHooks::exhausted` | src/clock.rs:80 | 2901:0014 | original | Some(true): the port's `exhaustion` runs | |
| `ClockHooks::exhaustion_box` | src/clock.rs:85 | 1194:59C8 | overridden | the view: main.rs Front<'_> | present: the game's box |
| `ClockHooks::level_up_screen` | src/clock.rs:94 | 2686:0319 | overridden | the view: main.rs Front<'_> | faithful: `level_up_around`, the points spent on the attribute screen (levelup vectors, screens.txt, worldframes playlevelup) |
| `ClockHooks::quests_daily` | src/clock.rs:54 | 1194:99F8 | overridden | the view: main.rs Front<'_> | faithful: `quests_daily` with its texts (hooktest) |
| `ClockHooks::quests_hourly` | src/clock.rs:60 | 1194:98E8 | overridden | the view: main.rs Front<'_> | faithful: `quests_hourly` with its texts (hooktest) |
| `ClockHooks::view_cleared` | src/clock.rs:88 | 0000:4EFD | present | the view filled with 9288's colour, which the port sets | |
| `ControlHooks::bars` | src/controls.rs:138 | 2826:0008, 2901:00A8 | overridden | the view: main.rs Front<'_> | fixed: the controls skipped the bars as drawing, but they leave 126C (their last x, which a flat brought to life writes over at 25CA:02F3) and 9288; now `bars_state` at 2826:0008 (playswim) |
| `ControlHooks::bar_click` | src/controls.rs:175 | 2542:00BA | overridden | the view: main.rs Front<'_> | faithful: the window's click on a bar button taken in the frame's input step (`player_steer`, where the classic controls' 2826:02E2 takes it), its handler run there (camping's too); the screen it asks for run inside the frame (`Talk::screen`: the turn stops there, the screen opens on the game as it asked, and the turn runs again as it ends with the game it left, the rest of the frame after it), but the travel map and the options screen after the frame; whether a click was taken (a click on the bar ends the controls there, 2826:00CB: no jump) (busy worldframes cases, playbarjump) |
| `ControlHooks::cast_at_target` | src/controls.rs:151 | 29CF:0000 | port | the default runs the port | |
| `ControlHooks::floor_pick` | src/controls.rs:138 | 2826:0B52 | port | fixed by this audit: answered no floor, so wall and floor spells never took (`Machine::floor_pick`, 3000 vectors) | |
| `ControlHooks::look` | src/controls.rs:158 | 2826:0C15 | elsewhere | the view looks from its own right click (`look_at`) | |
| `ControlHooks::message_box` | src/controls.rs:155 | 1194:42EA, 59C8 | present | the box drawn; front ends with a screen override it | |
| `ControlHooks::spell_counters` | src/controls.rs:172 | 2826:0BB2 | elsewhere | drawn by the view's HUD from DC0/DC2 | |
| `ControlHooks::take_click` | src/controls.rs:152 | 2826:0128 | overridden | the view: main.rs Front<'_> | faithful: the window's click taken in the frame's input step, after the frame's actors, as the classic controls' centre handler takes it (`view_click`; areaplay vectors) |
| `ControlHooks::wall_trigger` | src/controls.rs:145 | 0000:43B1 | overridden | the view: main.rs Front<'_> | faithful: `wall_trigger` (trigger vectors) |
| `ControlHooks::wild_recentre` | src/controls.rs:167 | 217B:0265 | overridden | the view: main.rs Front<'_> | faithful: `wild_shift` (wild vectors) |
| `DungeonPicHooks::click` | src/dungeonpic.rs:46 | 1194:564D | overridden | the view: travel.rs PictureHooks | present: the quest dungeon's picture (dungeonscreens) |
| `DungeonPicHooks::fade` | src/dungeonpic.rs:34 | 1194:55B2 | present | a fade; the view fades through `Film` | |
| `DungeonPicHooks::line` | src/dungeonpic.rs:39 | 0000:5364 | overridden | the view: travel.rs PictureHooks | present: the quest dungeon's picture (dungeonscreens) |
| `DungeonPicHooks::picture` | src/dungeonpic.rs:37 | 0000:5828 | overridden | the view: travel.rs PictureHooks | present: the quest dungeon's picture (dungeonscreens) |
| `DungeonPicHooks::shown` | src/dungeonpic.rs:42 | 0000:9466 | overridden | the view: travel.rs PictureHooks | present: the quest dungeon's picture (dungeonscreens) |
| `EndHooks::begin` | src/staffend.rs:26 | 1194:DAF0 | present | the scenes' screen and song | |
| `EndHooks::clear` | src/staffend.rs:55 | 0000:9A98 | present | the view cleared | |
| `EndHooks::end` | src/staffend.rs:53 | 1194:DB26 | present | the game's screen and the area's song back | |
| `EndHooks::fade` | src/staffend.rs:33 | 1194:55B2 | present | a fade | |
| `EndHooks::house` | src/staffend.rs:38 | 1194:DA25 | overridden | the view: main.rs EndFront<'_, '_> | present: HOUSE.IMG as a still with its text (now screen-checked: house.txt) |
| `EndHooks::line` | src/staffend.rs:46 | 0000:5364 | elsewhere | the view's still of HOUSE.IMG draws the lines as 0000:5364 does (`Movie::draw`: centred on A0h from row 32h, 0Ah apart), screen-checked against the original (gen_house.py, house.txt) | |
| `EndHooks::movie` | src/staffend.rs:28 | 2294:0895 | overridden | the view: main.rs EndFront<'_, '_> | present: the movies (not screen-checked) |
| `EndHooks::shown` | src/staffend.rs:49 | 1194:DAC4 | present | the screen shown and a key waited for | |
| `EndHooks::speech` | src/staffend.rs:31 | 1194:D459 | overridden | the view: main.rs EndFront<'_, '_> | present: the movies with their texts, brought in as D459 brings them (`Movie::lead_in`, checked on the dreams) |
| `EndHooks::text` | src/staffend.rs:41 | 1194:A36C | port | the default runs the port | |
| `EngineHooks::creature_flats` | src/engine.rs:70 | 0000:CF96, 25CA:0271 | overridden | the view: main.rs Front<'_> | fixed: the view ran the hook for every flat the walk reached after the frame, so a square brought to life earlier in the walk brought its creature again, and the controls' bars (2826:0008) came before it; now at the drawing, a square whose word is gone skipped (playswim) |
| `EngineHooks::draw` | src/engine.rs:49 | 0000:C980 | overridden | the view: main.rs Front<'_> | fixed: was the view's stand-in (every projectile and changed square in reach stepped, the click hit-tested on the GPU, a door's span the whole width): with the page the engine now takes the drawing's own walk (`render::Walked`: C23A's steps, D3F9's, the square and door under the cursor, the hit test 0000:C4BB with the hook CD3A); the stand-in is left for --gpu-world. playghostclick's state matches |
| `EngineHooks::draw_setup` | src/engine.rs:65 | 0000:D4B4 | elsewhere | the drawing's set-up is the renderer's (`render::draw`); fog's step runs on the machine (`fog_step`) | |
| `EngineHooks::drawn` | src/engine.rs:88 |  | overridden | the view: main.rs Front<'_> | fixed: a box opened after the frame's drawing (a click's, from the physics) saved what it covers (0000:8F2A) from the screen at the frame's start; now from the page as this frame drew it, the 3D rows over the interface (transitpalace's bank 11h) |
| `EngineHooks::present` | src/engine.rs:74 | 0000:A7B4 | present | the frame put on the screen | |
| `EngineHooks::rain` | src/engine.rs:72 |  | elsewhere | as snow (`rain_drops`) | |
| `EngineHooks::sky` | src/engine.rs:68 |  | overridden | the view: main.rs Front<'_> | faithful: `fog_step` on the game's memory (playfog, playfogfirst exact; state tests) |
| `EngineHooks::snow` | src/engine.rs:71 | 0000:E26B, E1A2 | elsewhere | the drops move in the engine before the hook (`snow_drops`); the hook only tells the front end | |
| `EngineHooks::sound` | src/engine.rs:54 |  | port | the default runs the port | |
| `EngineHooks::step` | src/engine.rs:59 |  | stub | a test's stand-in; false lets the engine run the step | |
| `EngineHooks::transition` | src/engine.rs:42 |  | original | C3AC is 0000:4470, a RETF in the 3D mode | |
| `FightHooks::fight_graphics` | src/encounter.rs:48 | 0000:1AF9 | port | the default runs the port | |
| `FightHooks::fight_music` | src/encounter.rs:53 | 23BA:01C7 | port | fixed by the overrides audit: was empty, so a monster fight's sound (the creature's .VOC, 43D7's list by 100D) never reached bank 15h at AFC8 nor the file slot 45F0 that 23BA:0217 keeps and 012B plays (`fight_sound`, soundload vectors); the driver's call 85h stays the front end's | |
| `FightHooks::jagar` | src/encounter.rs:55 | 1194:D45D | overridden | the view: main.rs FightFront<'_, '_> | present: JAGAR.FLC with the text, as a movie brought in as D459 brings it (`Movie::lead_in`, checked on the dreams) |
| `FightHooks::view_restored` | src/encounter.rs:58 |  | present | and put back | |
| `FightHooks::view_saved` | src/encounter.rs:57 | 0000:A611 and A6B5 | present | the view saved around a fight's start | |
| `FrameHooks::actor_click` | src/frame.rs:90 | 1194:DB6C | overridden | the view: main.rs Front<'_> | faithful: `actor_click` (talk vectors; playghostclick) |
| `FrameHooks::area_music` | src/frame.rs:100 | 2294:04F5 | port | the default runs the port | |
| `FrameHooks::automap_mark` | src/frame.rs:95 | 0000:3491 | port | the default runs the port | |
| `FrameHooks::effect_words` | src/frame.rs:62 | 0000:5011 | overridden | the view: main.rs Front<'_> | faithful: `effect_words`, drawn as 5011 draws them (hooktest) |
| `FrameHooks::faces` | src/frame.rs:64 | 0000:4C86, 292B:0530 | overridden | the view: main.rs Front<'_> | faithful: `faces_draw` (faces vectors) |
| `FrameHooks::guards_arrive` | src/frame.rs:84 |  | port | the default runs the port | |
| `FrameHooks::interface` | src/frame.rs:55 | 0000:4C83, 4DD6 | overridden | the view: main.rs Front<'_> | faithful: `interface_setup` (state tests) |
| `FrameHooks::no_spells_picture` | src/frame.rs:60 | 292B:0341..0358 | port | the default runs the port (`no_spells_picture`); the frame vectors stub 7428 and 599C | fixed: NOSPELL.IMG was neither loaded to 1044 nor drawn (titleplay creation) |
| `FrameHooks::key_d327` | src/frame.rs:126 | 1194:D327 | port | the default runs the port | |
| `FrameHooks::level_text_draw` | src/frame.rs:76 | 0000:05B5 | overridden | the view: main.rs Front<'_> | present: the level's text drawn by the window, ADB8 left as 9E0A leaves it (riddle screens) |
| `FrameHooks::menu_keys` | src/frame.rs:112 | 2542:0100 | overridden | the view: main.rs Front<'_> | faithful: `Machine::menu_keys` in the frame's body (0000:4952), its switches and camping (2542:01E6) done there; the screen a key asks for run inside the body (`Talk::screen`), the rest of the frame after it, but the travel map and the options screen after the frame (busy worldframes cases, the `...end` cases ending with that frame) |
| `FrameHooks::message_box` | src/frame.rs:68 | 1194:59C8 | overridden | the view: main.rs Front<'_> | present: the game's box (screens.txt) |
| `FrameHooks::message_box_number` | src/frame.rs:70 | 1194:A36C | overridden | the view: main.rs Front<'_> | faithful: the text port then its box (entry boxes in screens.txt) |
| `FrameHooks::outdoor_light` | src/frame.rs:141 | 0000:4B84, 0390 | overridden | the view: main.rs Front<'_> | faithful: `outdoor_light` with its loads made inside it (`LightNow`), in the frame body where the original runs it; was the view's after the frame, so a title load's first frame lost its relight (savefiles.txt `loadframe` lines) |
| `FrameHooks::projectile_lights_end` | src/frame.rs:66 | 292B:000E | overridden | the view: main.rs Front<'_> | faithful: `projectile_lights_end` |
| `FrameHooks::quest_carry` | src/frame.rs:117 | 0000:4D00 | port | the default runs the port | |
| `FrameHooks::rest` | src/frame.rs:133 | 1194:4635 | overridden | the view: main.rs Front<'_> | faithful: `rest_frame` (rest vectors, screens.txt) |
| `FrameHooks::riddle` | src/frame.rs:80 | 292B:05A2 | overridden | the view: main.rs Front<'_> | faithful: the game's editor, `riddle_answered` (riddle vectors, screens.txt) |
| `FrameHooks::sky_colours` | src/frame.rs:145 | 0000:4C7E, 2294:0428 | overridden | the view: main.rs Front<'_> | faithful: `sky_colours`, `sky_colours_load` in the frame body (was after the frame) |
| `FrameHooks::town_welcome` | src/frame.rs:124 | 0000:4B02 | overridden | the view: main.rs Front<'_> | faithful: `Machine::town_welcome_box` (the load by its save path, 965E's state with the original's AX and BX); what it covers kept from the screen last shown (`Assets::shown`), as the page at the frame's start (tests/vectors/welcome.txt, worldtest's `the_welcome_keeps_the_screen_as_the_original`, the box's working words included) |
| `FrameHooks::townsfolk_graphics` | src/frame.rs:108 | 0000:17F9 | overridden | the view: main.rs Front<'_> | faithful: `townsfolk_load` |
| `FrameHooks::weapon_graphics` | src/frame.rs:104 | 0000:3C4C | overridden | the view: main.rs Front<'_> | faithful: `weapon_graphics_load` (state tests) |
| `FxHooks::spell_reflected` | src/effects.rs:172 | 0000:27E5 | overridden | the engine: GameHooks | faithful: the engine's |
| `GuildHooks::click` | src/guild.rs:108 |  | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::haggle` | src/guild.rs:100 | 1194:8020 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::message_box` | src/guild.rs:90 | 1194:59CC | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::screen` | src/guild.rs:80 | 1194:4B60 and 4BFF | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::spell_screen` | src/guild.rs:104 | 1194:68A3 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::spellmaker` | src/guild.rs:115 |  | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::stock_file` | src/guild.rs:83 | 25FD:0844, 0000:600C, 67C4 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::stock_list` | src/guild.rs:93 | 1194:5734 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `GuildHooks::stock_write` | src/guild.rs:88 | 1194:A82D | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `HaggleIo::expand` | src/haggle.rs:129 | 2DA2:000A | port | the default runs the port | |
| `HaggleIo::picture` | src/haggle.rs:141 | 1194:4B8E | overridden | the view: questui.rs HaggleTalk<'_> | present: the haggle's lines and the player's offers (screens.txt) |
| `HaggleIo::say` | src/haggle.rs:136 | 1194:49FF | overridden | the view: questui.rs HaggleTalk<'_> | present: the haggle's lines and the player's offers (screens.txt) |
| `HaggleIo::shown` | src/haggle.rs:147 | 1194:4E54, 4E88, 5FC0 | overridden | the view: questui.rs HaggleTalk<'_> | present: the haggle's dialogs' state as the original's (worldframes.txt's playstore and playguild state lines) |
| `HaggleIo::type_offer` | src/haggle.rs:120 | 1194:810D, 4E88 | overridden | the view: questui.rs HaggleTalk<'_> | present: the haggle's lines and the player's offers (screens.txt) |
| `LevelUpHooks::body` | src/levelup.rs:119 | 1194:2462 | port | the default runs the port (`equip_frames`); the stubbed levelup vectors stub it | |
| `LevelUpHooks::dac` | src/levelup.rs:94 | 0000:6E3C | overridden | the view: main.rs Front<'_> | faithful: the colours shown, as the travel map's keep reads them (worldframes playlevelup) |
| `LevelUpHooks::keep_screen` | src/levelup.rs:106 | 1194:158F | port | the default runs the port (`sheet_keep`) | |
| `LevelUpHooks::message_box` | src/levelup.rs:141 | 1194:59CC | overridden | the view: main.rs Front<'_> | present: the game's box (screens.txt) |
| `LevelUpHooks::points_screen` | src/levelup.rs:124 | 1194:124A | port | the default runs the port (`Machine::points_screen`) | |
| `LevelUpHooks::points_screen_end` | src/levelup.rs:136 | 1194:1241 | port | the default runs the port (`Machine::points_screen_end`) | |
| `LevelUpHooks::restore` | src/levelup.rs:144 | 0000:541E, A6B5 | port | the default runs the port (`world_back`) | |
| `LevelUpHooks::screen` | src/levelup.rs:112 | 0000:9A98, 9426, 1194:55B2 | port | the default runs the port (the clip, the page cleared, 929E black; the fade the front end's) | |
| `LevelUpHooks::sound` | src/levelup.rs:89 | 23BA:012B | port | the default runs the port | |
| `LevelUpHooks::spend` | src/levelup.rs:132 | 1194:122C | overridden | the view: main.rs Front<'_> | faithful: answered on the attribute screen, the clicks replayed (`attribute_arrow`), `spend_in_turn` without one (worldframes playlevelup, screens.txt) |
| `LevelUpHooks::world_keep` | src/levelup.rs:99 | 0000:541E, A611 | port | the default runs the port (`world_keep`, the colours `dac`) | |
| `LightHooks::fog_fill` | src/light.rs:63 | 0000:9A6E | overridden | the view: main.rs LightState | faithful: the fill colour kept for the page (playfog) |
| `LightHooks::fog_light_table` | src/light.rs:60 | 2294:0C36 | overridden | the view: main.rs LightState | faithful: FOG.LGT's table (playfogfirst) |
| `LightHooks::lgt` | src/light.rs:57 | 0000:7428 | overridden | the view: main.rs LightState, world.rs Lgt<'_> | faithful: the .LGT loaded |
| `LightHooks::light_table` | src/light.rs:39 | 2294:0B8A | overridden | the view: areas.rs Areas<'_>, main.rs LightState | faithful: the table made by `shade_tables` (state tests, EMS banks) |
| `LightHooks::palette` | src/light.rs:51 | 0000:9B5B | overridden | the view: main.rs LightState | faithful: entry 71h kept for the window's DAC (screens) |
| `LightHooks::relight` | src/light.rs:44 | 0000:027A and 02F2 | overridden | the view: main.rs LightState | faithful: the sky's and water's remapping done after the frame with the files (playnight exact, state tests) |
| `LightHooks::restore_textures` | src/light.rs:46 | 0000:DFDB | overridden | the view: main.rs LightState | faithful: done after the frame with the files (state tests) |
| `LightHooks::sky` | src/light.rs:53 | 2294:0341 | overridden | the view: main.rs LightState | faithful: the dither loaded after the frame (playnight, state tests) |
| `LightHooks::sky_objects` | src/light.rs:48 | 2294:08B9 | overridden | the view: main.rs LightState | faithful: `sky_objects` (playsky exact) |
| `LightHooks::weather_palette` | src/light.rs:66 | 0000:7534 and 9B56 | elsewhere | PAL.COL's palette: the view sets its palette from its assets | |
| `LoadHooks::automap_area` | src/load.rs:121 | 0000:3503 | port | the default runs the port | |
| `LoadHooks::automap_keep` | src/load.rs:81 | 23F1:0178 | port | the default runs the port | |
| `LoadHooks::citydata` | src/load.rs:93 | 242A:000E | port | the default runs the port | |
| `LoadHooks::copy_file` | src/load.rs:111 | 25FD:056F | port | the default runs the port | |
| `LoadHooks::exists` | src/load.rs:73 | 0000:600C | port | the default runs the port | |
| `LoadHooks::interior_song` | src/load.rs:126 | 25FD:051F | port | the default runs the port | |
| `LoadHooks::level_automap` | src/load.rs:116 | 292B:09AC | port | the default runs the port | |
| `LoadHooks::loading` | src/load.rs:85 | 25FD:0803 | present | "Loading..." shown | |
| `LoadHooks::message` | src/load.rs:78 | 1194:59C8 | elsewhere | the view's save menu shows the empty slot's words (`SaveUi::empty_slot`) | |
| `LoadHooks::mode_3d` | src/load.rs:132 | 2294:0006 | stub | false: the port's `mode_3d` runs (the vectors stub it) | |
| `LoadHooks::renderer` | src/load.rs:108 | 0000:AAA2 | overridden | the view: areas.rs Areas<'_>; the default runs `light_table_make` | faithful: the light table made (titleload vectors) |
| `LoadHooks::screen` | src/load.rs:101 | 0000:A5D1 | present | the view's screen set up | |
| `LoadHooks::sky_graphics` | src/load.rs:106 | 0000:E5C9 | overridden | the view: areas.rs Areas<'_> | faithful: `sky_images` where the load runs it (25FD:0417, AA5C); was the assets' own after the load (savefiles.txt `loadplace`) |
| `LoadHooks::slot_files` | src/load.rs:88 | 25FD:06A1 | overridden | the view: areas.rs Areas<'_> | faithful: `slot_files_in` (titleload vectors) |
| `LoadHooks::sound_off` | src/load.rs:70 | 23BA:000C | present | the sound stopped | |
| `LoadHooks::view_update` | src/load.rs:103 | 0000:DA8E | present | the view updated | |
| `LootHooks::last_piece` | src/loot_window.rs:75 | 1194:D986 | overridden | the view: main.rs LootFront<'_, '_> | present: the loot window's boxes and lists (screens.txt) |
| `LootHooks::list` | src/loot_window.rs:58 | 1194:5734 | overridden | the view: main.rs LootFront<'_, '_> | present: the loot window's boxes and lists (screens.txt) |
| `LootHooks::message_box` | src/loot_window.rs:55 | 1194:59CC | overridden | the view: main.rs LootFront<'_, '_> | present: the loot window's boxes and lists (screens.txt) |
| `LootHooks::piece_told` | src/loot_window.rs:70 | 1194:D363 | port | the default runs the port | |
| `LootHooks::piles` | src/loot_window.rs:79 | 2433:0000 | port | the default runs the port | |
| `LootHooks::sound` | src/loot_window.rs:65 | 23BA:012B | port | the default runs the port | |
| `LootHooks::text` | src/loot_window.rs:63 | 1194:A374 | overridden | the view: main.rs LootFront<'_, '_> | present: the loot window's boxes and lists (screens.txt) |
| `MapHooks::list` | src/mapscreen.rs:34 | 1194:5730 | overridden | the view: travel.rs Hooks<'_> | present: the game's list (screens.txt) |
| `MapHooks::quest_dungeon` | src/mapscreen.rs:28 | 292B:0362 | overridden | the view: travel.rs Hooks<'_> | faithful: the picture shown after the screen (dungeonpic vectors, screens) |
| `MapHooks::type_name` | src/mapscreen.rs:31 | 1194:5C30 | overridden | the view: travel.rs Hooks<'_> | present: the game's editor (screens.txt); the prompt's box state round it (`named_shown`, `named_closed`; msgbox vectors) |
| `MenuHooks::automap_keep` | src/savemenu.rs:111 | 23F1:0145 | port | the default runs the port | |
| `MenuHooks::centred` | src/savemenu.rs:88 |  | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuHooks::click` | src/savemenu.rs:92 | 1194:564D | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuHooks::delete_matching` | src/savemenu.rs:102 | 1194:4D91 | port | the default runs the port | |
| `MenuHooks::fill` | src/savemenu.rs:87 | 0000:9A6E | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuHooks::font` | src/savemenu.rs:71 | 0000:9AE6 | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuHooks::load_screen` | src/savemenu.rs:115 | 2294:11F2 | present | 2294:11F2, the heap check | |
| `MenuHooks::map_bank` | src/savemenu.rs:73 | 0000:5C86 | port | the default runs the port | |
| `MenuHooks::message` | src/savemenu.rs:99 | 1194:59C8 | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuHooks::mouse_wait` | src/savemenu.rs:66 | 0000:8FC4 | original | AFF9 cleared, as the wait leaves it | |
| `MenuHooks::names` | src/savemenu.rs:82 | 0000:7428 | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuHooks::picture` | src/savemenu.rs:78 | 0000:7428 | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuHooks::show` | src/savemenu.rs:90 | 0000:9466 and 541E | present | the screen shown | |
| `MenuHooks::states_keep` | src/savemenu.rs:107 | 292B:095E | port | the default runs the port | |
| `MenuHooks::type_name` | src/savemenu.rs:97 | 0000:9EE5 | overridden | the view: saveui.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `MenuKeyHooks::cyber_button` | src/menukeys.rs:96 | 2F26:04E2 | original | no CyberMan: never down | |
| `MenuKeyHooks::cyber_buttons` | src/menukeys.rs:100 | 2F26:03ED | original | no CyberMan: none | |
| `MenuKeyHooks::touch_cast` | src/menukeys.rs:105 | 29CF:0000 | port | the default runs the port | |
| `NewGameHooks::creation_done` | src/newgame.rs:107 | 0000:5D70 | port | fixed by this audit: `ems_reset` | |
| `NewGameHooks::faded` | src/newgame.rs:129 | 1194:55B6 | port | the default runs the port (`sheet_black`); the newgame vectors stub it | fixed: creation's end didn't fade (titleplay creation) |
| `NewGameHooks::game_screen` | src/newgame.rs:111 | 2294:11F2 | present | 2294:11F2, the heap check | |
| `NewGameHooks::hands` | src/newgame.rs:116 | 0000:3C4C | overridden | the title's set-up: start.rs SetupPort (`weapon_graphics_load`); a new game's, the view loads the hands | faithful: programstart vectors (setup) |
| `NewGameHooks::level_flats` | src/newgame.rs:121 | 21EE:0134 | overridden | the title's set-up: start.rs SetupPort (`effect_flats`, C38E); a new game's, the view's world load | faithful: programstart vectors (setup) |
| `NewGameHooks::message_box` | src/newgame.rs:99 | 1194:59CC | overridden | the view: create.rs Hooks<'_> | present: creation's boxes (createshot.txt) |
| `NewGameHooks::palette` | src/newgame.rs:104 | 1194:1695 | present | the attribute screen's palette | |
| `NewGameHooks::parchment_drop` | src/newgame.rs:123 | 0000:9326 | port | the default runs the port (`resource_drop` of [1038]); the newgame vectors stub it | fixed: the parchment's tiles weren't added or dropped (titleplay creation) |
| `NewGameHooks::resources` | src/newgame.rs:114 | 2294:01F1 | overridden | the title's set-up: start.rs SetupPort (`game_resources`); a new game's, the view's assets | faithful: programstart vectors (setup) |
| `NewGameHooks::roll_screen` | src/newgame.rs:112 | 1194:12BE, 157E | port | the default runs the port (`corner_kept`, `arrows_made`, `sheet_texts`); the newgame vectors stub them | fixed: 1297 left 34h, bank [1032] and bank 5 not written (titleplay creation) |
| `NewGameHooks::sheet_keep` | src/newgame.rs:134 | 1194:158F | port | the default runs the port (`sheet_keep`); the newgame vectors stub it | fixed: creation's end kept no sheet (5E0 bit 200h, the race's digit; titleplay creation) |
| `NewGameHooks::spells` | src/newgame.rs:118 | 29CF:0691 | overridden | the title's set-up: start.rs SetupPort (`spells_load`); a new game's, world.rs | faithful: programstart vectors (setup) |
| `NewGameHooks::stats_screen` | src/newgame.rs:102 | 1194:0DC7 | port | the default names the clothes (16AA's 1D22, `sheet_clothes`); the drawing is the view's (`create.rs`) | fixed: the clothes' names stayed a man's (titleplay creation) |
| `NoteHooks::draw_text` | src/mapnotes.rs:68 | 0000:9E0A | overridden | the view: automap.rs Collect, inputtest.rs Collect | present: the automap's notes drawn (screens.txt) |
| `NoteHooks::redraw` | src/mapnotes.rs:62 | 0000:2F10 | present | the map redrawn | |
| `NoteHooks::restore` | src/mapnotes.rs:60 | 0000:3698 | present | the screen region put back | |
| `NoteHooks::type_text` | src/mapnotes.rs:66 | 0000:9EE9 | overridden | the view: main.rs Front<'_> | present: the game's editor (lineedit vectors) |
| `OptionsHooks::automap_write` | src/options.rs:67 | 23F1:0145 | port | the default runs the port | |
| `OptionsHooks::centred` | src/options.rs:81 |  | overridden | the view: options.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `OptionsHooks::click` | src/options.rs:89 | 1194:564D | overridden | the view: inputtest.rs Clicks, options.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `OptionsHooks::confirm` | src/options.rs:103 | 1194:48DF | overridden | the view: options.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `OptionsHooks::cyber` | src/options.rs:93 | 2F26:04E2 | original | no CyberMan | |
| `OptionsHooks::delete_matching` | src/options.rs:108 | 1194:4D91 | port | the default runs the port | |
| `OptionsHooks::fill` | src/options.rs:80 | 0000:9A6E | overridden | the view: options.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `OptionsHooks::font` | src/options.rs:65 | 0000:9AE6 | overridden | the view: options.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `OptionsHooks::hide` | src/options.rs:83 | 0000:541E | present | the mouse hidden | |
| `OptionsHooks::load_menu` | src/options.rs:99 | 25FD:0242 and 0002 | elsewhere | the view runs its load menu itself (`SaveUi`) | |
| `OptionsHooks::map_bank` | src/options.rs:71 | 0000:5C86 | port | the default runs the port | |
| `OptionsHooks::music_volume` | src/options.rs:114 | 0FFD:0DE8 | elsewhere | the view sets its music's volume itself | |
| `OptionsHooks::picture` | src/options.rs:76 | 0000:7428 | overridden | the view: options.rs Script<'_> | present: the options and save screens answered by the player (screens.txt) |
| `OptionsHooks::save_menu` | src/options.rs:100 |  | elsewhere | and its save menu | |
| `OptionsHooks::show` | src/options.rs:85 |  | present | the screen shown | |
| `OptionsHooks::unhide` | src/options.rs:84 |  | present | and shown | |
| `PhysicsHooks::input` | src/movement.rs:799 |  | overridden | the engine: Classic<'_>, Physics<'_, '_> | faithful: the engine's controls |
| `PhysicsHooks::transition` | src/movement.rs:804 |  | overridden | the engine: Physics<'_, '_> | faithful: the engine's |
| `QuestHooks::audience` | src/quest.rs:175 |  | overridden | the view: main.rs Audience<'_, '_>, questui.rs QuestFront<'_, '_> | present: the audience's movies (not screen-checked) |
| `QuestHooks::bios_ticks` | src/quest.rs:126 | 0000:9D60 | original | the machine's BIOS ticks | |
| `QuestHooks::city_info` | src/quest.rs:111 | 1194:3EC3 | port | the default runs the port | |
| `QuestHooks::dialog` | src/quest.rs:166 | 1194:4E88 | overridden | the view: questui.rs QuestFront<'_, '_> | present: the game's boxes, menus and windows (palace screens) |
| `QuestHooks::entry_save` | src/quest.rs:89 | 26F7:0C0A | port | the default runs the port | |
| `QuestHooks::expand` | src/quest.rs:84 | 2DA2:000A | overridden | the view: questui.rs QuestFront<'_, '_> | faithful: the text port |
| `QuestHooks::finale` | src/quest.rs:100 | 1194:1034 | overridden | the view: questui.rs QuestFront<'_, '_> | faithful: `level_up_screen` with the cursor's column (by reading the code) |
| `QuestHooks::font` | src/quest.rs:67 | 0000:9AE6 | present | fonts selected | |
| `QuestHooks::frame_load` | src/quest.rs:158 | 0000:7428 | port | the default runs the port | |
| `QuestHooks::informant_haggle` | src/quest.rs:152 | 1194:8020 | overridden | the view: questui.rs QuestFront<'_, '_> | faithful: `haggle` with the player's answers (haggle vectors) |
| `QuestHooks::inventory_full` | src/quest.rs:104 | 1194:42EA | original | 1194:42EA: fonts selected and 9288 60h; the text is 19DE's own | |
| `QuestHooks::log_entry` | src/quest.rs:94 | 27E3:000A | port | the default runs the port | |
| `QuestHooks::menu` | src/quest.rs:142 | 1194:020E | overridden | the view: questui.rs QuestFront<'_, '_> | present: the game's boxes, menus and windows (palace screens) |
| `QuestHooks::message_box` | src/quest.rs:69 | 1194:59CC | overridden | the view: questui.rs QuestFront<'_, '_> | present: the game's boxes, menus and windows (palace screens) |
| `QuestHooks::music` | src/quest.rs:121 | 2294:04C0 | port | the default runs the port | |
| `QuestHooks::notice` | src/quest.rs:170 | 1194:4D31 | overridden | the view: questui.rs QuestFront<'_, '_> | present: the game's boxes, menus and windows (palace screens) |
| `QuestHooks::npc_name` | src/quest.rs:180 | 1194:080B | port | the default runs the port | |
| `QuestHooks::picture` | src/quest.rs:163 | 1194:4B8E | overridden | the view: questui.rs QuestFront<'_, '_> | present: the game's boxes, menus and windows (palace screens) |
| `QuestHooks::province_load` | src/quest.rs:72 | 1194:2E18 | port | the default runs the port | |
| `QuestHooks::province_save` | src/quest.rs:77 | 26F7:0CF1 | port | the default runs the port | |
| `QuestHooks::substitute` | src/quest.rs:137 | 2DA2:000A | overridden | the view: questui.rs QuestFront<'_, '_> | faithful: the text port |
| `QuestHooks::text` | src/quest.rs:65 | 1194:A374 | overridden | the view: questui.rs QuestFront<'_, '_> | faithful: the text port |
| `QuestHooks::text_at` | src/quest.rs:132 | 1194:A374 | overridden | the view: questui.rs QuestFront<'_, '_> | faithful: the text port |
| `QuestHooks::text_shown` | src/quest.rs:81 | 1194:A374 | overridden | the view: questui.rs QuestFront<'_, '_> | faithful: the text port and its box |
| `QuestHooks::travel` | src/quest.rs:116 | 1194:2E69 | port | the default runs the port | |
| `RestHooks::advance` | src/rest.rs:129 | 0000:142D | stub | false: the game's clock runs the minutes | |
| `RestHooks::dream` | src/rest.rs:108 | 1194:D45D | overridden | the view: main.rs Front<'_> | fixed: was CHAOSVSN.FLC in 33h for every dream; 1194:D3AF shows only the first (5DCh) so, the others on VISION.FLC in 2Fh, as the view now does, each brought in as D459 brings it (`Movie::lead_in`: faded to black in 8, cleared); tests/vectors/dream.txt (gen_dream.py) state by state, screentest's `the_dreams_as_the_original_shows_them` |
| `RestHooks::hours_text` | src/rest.rs:106 | 1194:4659 / 452F | overridden | the view: main.rs Front<'_> | present: drawn as 1194:4659 draws it (screens.txt) |
| `RestHooks::menu` | src/rest.rs:97 | 1194:020E | overridden | the view: main.rs Front<'_> | present: the game's menu (screens.txt); its boxes' state (`menu_boxes`, `menu_closed`; msgbox vectors) |
| `RestHooks::message` | src/rest.rs:104 | 1194:59C8 | overridden | the view: main.rs Front<'_> | present: the game's box (screens.txt) |
| `RestHooks::room_file` | src/rest.rs:120 | 1194:BA91 | port | the default runs the port | |
| `RestHooks::room_load` | src/rest.rs:115 | 1194:C04C | port | the default runs the port | |
| `RestHooks::screen` | src/rest.rs:125 |  | present | the screen redrawn after the rest | |
| `RestHooks::sky` | src/rest.rs:110 | 2294:0341 | port | the default runs the port | |
| `RestHooks::type_hours` | src/rest.rs:102 | 1194:4800 | overridden | the view: main.rs Front<'_> | present: the game's editor (lineedit vectors); the prompt's box state round it (`prompt_shown`, `prompt_closed`; msgbox vectors) |
| `SaveHooks::delete` | src/save_write.rs:53 | 0000:9DC6 | overridden | the view: main.rs SaveDir<'_> | faithful: the file deleted |
| `SaveHooks::palette` | src/save_write.rs:59 | 0000:6E3C | overridden | the view: main.rs SaveDir<'_> | present: the window's palette for the save's |
| `SaveHooks::write` | src/save_write.rs:56 |  | overridden | the view: main.rs SaveDir<'_> | faithful: the save's files written (save vectors); SAVEGAME's screen bytes black, the window's picture not being the game's |
| `ShopHooks::banner` | src/shops.rs:46 | 1194:4BFF | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::buffer_freed` | src/shops.rs:105 | 1194:5E49 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' screens' state as the original's (worldframes.txt's playstore and playguild state lines), for a game whose page is its own block |
| `ShopHooks::buffer_made` | src/shops.rs:107 | 1194:5E54 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' screens' state as the original's (worldframes.txt's playstore and playguild state lines), for a game whose page is its own block |
| `ShopHooks::dialog` | src/shops.rs:64 | 1194:4E88 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::dialog_at` | src/shops.rs:69 |  | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::expand` | src/shops.rs:79 | 2DA2:000A | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::gold_left` | src/shops.rs:52 | 1194:545C | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::list` | src/shops.rs:74 | 1194:5734 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::menu` | src/shops.rs:59 | 1194:020E | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::message` | src/shops.rs:38 | 1194:4D31 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::picture` | src/shops.rs:47 | 1194:4B8E | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::quest_check` | src/shops.rs:88 | 1194:D961 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::quests` | src/shops.rs:93 | 1194:8F1E | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `ShopHooks::screen_back` | src/shops.rs:102 | 1194:54B3 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' screens' state as the original's (worldframes.txt's playstore and playguild state lines), for a game whose page is its own block |
| `ShopHooks::screen_saved` | src/shops.rs:98 | 1194:5496 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' screens' state as the original's (worldframes.txt's playstore and playguild state lines), for a game whose page is its own block |
| `ShopHooks::shop_screen` | src/shops.rs:42 | 1194:4B60 | overridden | the view: shopui.rs ShopFront<'_, '_> | faithful: `shop_screen`, before the temple's name is made (playtempleheal's EMS) |
| `ShopHooks::strip_back` | src/shops.rs:56 | 1194:5FC0 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `SkyHooks::bolt` | src/skyobj.rs:52 | 0000:78A8 | overridden | the view: main.rs SkyDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `SkyHooks::draw_dfa` | src/skyobj.rs:46 | 0000:5652 | overridden | the view: main.rs SkyDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `SkyHooks::draw_image` | src/skyobj.rs:42 | 0000:5828 | overridden | the view: main.rs SkyDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `SkyHooks::fill` | src/skyobj.rs:49 | 0000:9A6E | overridden | the view: main.rs SkyDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself |
| `SkyHooks::fog` | src/skyobj.rs:37 | 2294:0B01 | overridden | the view: main.rs SkyDraw<'_> | gpu-only: the GPU world's sky only: with the page the drawing runs the sky itself (playsky, playbolt, playvolcano exact) |
| `SkyHooks::load` | src/skyobj.rs:31 | 0000:7428 | overridden | the view: main.rs SkyLoad | faithful: the sky's images into EMS as 0000:7428 loads them |
| `SkyHooks::sound` | src/skyobj.rs:54 | 23BA:002A | port | the default runs the port | |
| `SpellPageHooks::click` | src/spellpage.rs:69 | 0000:8FC4 | overridden | the view: panels.rs Replay | present: the spell page answered by the player (screens.txt) |
| `SpellPageHooks::confirm` | src/spellpage.rs:76 | 1194:48E9 | overridden | the view: panels.rs Replay | present: the spell page answered by the player (screens.txt) |
| `SpellPageHooks::leave` | src/spellpage.rs:80 | 1194:55B2 and 53E3 | present | faded out, the equipment screen back | |
| `SpellPageHooks::page` | src/spellpage.rs:66 | 1194:68D7 | overridden | the view: panels.rs Replay | present: the spell page answered by the player (screens.txt) |
| `SpellPageHooks::type_name` | src/spellpage.rs:74 | 1194:65FD | overridden | the view: panels.rs Replay | present: the spell page answered by the player (screens.txt) |
| `SpellPageHooks::write` | src/spellpage.rs:83 | 25FD:0844, 0000:9DD2 | port | the default runs the port | |
| `SpellmakerHooks::click` | src/spellmaker.rs:64 | 1194:5651 | overridden | the view: shopui.rs LiveMaker<'_, '_>, spellmaker.rs Maker | present: the spellmaker's screen answered by the player (screens.txt) |
| `SpellmakerHooks::form` | src/spellmaker.rs:94 |  | present | an effect's form shown | |
| `SpellmakerHooks::list` | src/spellmaker.rs:77 | 1194:5734 | overridden | the view: shopui.rs LiveMaker<'_, '_>, spellmaker.rs Maker | present: the spellmaker's screen answered by the player (screens.txt) |
| `SpellmakerHooks::menu` | src/spellmaker.rs:82 | 1194:020E | overridden | the view: shopui.rs LiveMaker<'_, '_>, spellmaker.rs Maker | present: the spellmaker's screen answered by the player (screens.txt) |
| `SpellmakerHooks::message_box` | src/spellmaker.rs:87 | 1194:59CC | overridden | the view: shopui.rs LiveMaker<'_, '_>, spellmaker.rs Maker | present: the spellmaker's screen answered by the player (screens.txt) |
| `SpellmakerHooks::screen` | src/spellmaker.rs:92 |  | overridden | the view: spellmaker.rs Maker | present: the spellmaker's (screens.txt) |
| `SpellmakerHooks::spells_write` | src/spellmaker.rs:97 | 29CF:06CB | port | the default runs the port | |
| `SpellmakerHooks::type_name` | src/spellmaker.rs:90 | 0000:9EE5 | overridden | the view: shopui.rs LiveMaker<'_, '_>, spellmaker.rs Maker | present: the spellmaker's screen answered by the player (screens.txt) |
| `StairsHooks::ask_leave` | src/hooks.rs:64 | 26F7:0000 | overridden | the view: main.rs Front<'_> | faithful: the travel map (travelmap vectors, dungeonscreens) |
| `StairsHooks::automap_keep` | src/hooks.rs:43 | 292B:095E | overridden | the view: areas.rs Kept, main.rs Front<'_> | faithful: `states_keep` |
| `StairsHooks::automap_load` | src/hooks.rs:46 | 292B:09AC | overridden | the view: areas.rs Kept, main.rs Front<'_> | faithful: `states_load` |
| `StairsHooks::level_piles` | src/hooks.rs:55 | 2433:0000 | port | the default runs the port | |
| `StairsHooks::level_screen` | src/hooks.rs:60 | 0000:368E | overridden | the view: areas.rs Kept, main.rs Front<'_> | faithful: `automap_keep`, `automap_load` (playdown) |
| `StairsHooks::light_table` | src/hooks.rs:40 | 2294:0B8A | overridden | the view: main.rs Front<'_> | faithful: NORMAL.LGT's table (playdown exact) |
| `StairsHooks::map_keep` | src/hooks.rs:70 | 0000:4564..4568 | overridden | the view: main.rs Front<'_> | faithful: `map_keep` with the screen's colours (A39E) |
| `StairsHooks::quest_level` | src/hooks.rs:50 | 1194:9FAC | port | the default runs the port | |
| `StoreHooks::armor_lines` | src/store.rs:123 | 1194:F204 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `StoreHooks::greeting` | src/store.rs:106 | 1194:4BFF | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `StoreHooks::haggle` | src/store.rs:114 | 1194:8020 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `StoreHooks::message_box` | src/store.rs:108 | 1194:59CC | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `StoreHooks::repair_instead` | src/store.rs:128 | 1194:C6AC | stub | None: the repairs run | |
| `StoreHooks::text` | src/store.rs:133 | 1194:A374 | port | the default runs the port | |
| `StoreHooks::weapon_lines` | src/store.rs:119 | 1194:F142 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TalkHooks::dungeon_entrance` | src/talk.rs:195 | 0000:4EFD | overridden | the view: main.rs Front<'_> | faithful: `wild_dungeon_entry` |
| `TalkHooks::expand` | src/talk.rs:135 | 2DA2:000A | stub | None: the port's `expand` runs | |
| `TalkHooks::list` | src/talk.rs:115 | 1194:5734 | overridden | the view: main.rs Front<'_> | present: the game's list (screens.txt) |
| `TalkHooks::loot` | src/talk.rs:125 | 1194:7B16 | overridden | the view: main.rs Front<'_> | faithful: `loot_window` (lootwin vectors, screens.txt) |
| `TalkHooks::palace` | src/talk.rs:155 | 1194:8F1E | overridden | the view: main.rs Front<'_> | faithful: `quests_talk` (sidequest vectors, palace screens) |
| `TalkHooks::quest_chest` | src/talk.rs:165 | 2294:0F33 | overridden | the view: main.rs Front<'_> | faithful: `quest_chest` (mainquest vectors, hooktest) |
| `TalkHooks::quest_item` | src/talk.rs:146 | 1194:9F04 | overridden | the view: main.rs Front<'_> | faithful: `quest_item_taken` (hooktest) |
| `TalkHooks::quest_offer` | src/talk.rs:160 | 1194:CD9A | port | the default runs the port | |
| `TalkHooks::quest_person` | src/talk.rs:150 | 1194:D593 | overridden | the view: main.rs Front<'_> | faithful: `quest_person_talk` |
| `TalkHooks::shop` | src/talk.rs:174 |  | overridden | the view: main.rs Front<'_> | faithful: the shops' ports with the player's answers (shops vectors, screens.txt) |
| `TalkHooks::sound` | src/talk.rs:129 | 23BA:002A | port | the default runs the port | |
| `TalkHooks::square_text` | src/talk.rs:208 | 0000:06BE | port | the default runs the port | |
| `TalkHooks::text` | src/talk.rs:111 | 1194:A374 | stub | false: the text port runs | |
| `TalkHooks::touch_cast` | src/talk.rs:140 | 29CF:0000 | port | the default runs the port | |
| `TalkHooks::way` | src/talk.rs:120 | 23F1:00A3 | overridden | the view: main.rs Front<'_> | faithful: `note_add` (look vectors) |
| `TalkHooks::wild_flat` | src/talk.rs:191 | 2294:0DD0 | port | the default runs the port | |
| `TalkHooks::wild_way` | src/talk.rs:201 | 217B:0682 | overridden | the view: main.rs Front<'_> | faithful: `wild_way` (wildtalk vectors) |
| `TavernHooks::files` | src/tavern.rs:61 | 1194:C0B1 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TavernHooks::greeting` | src/tavern.rs:63 | 1194:C12A | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TavernHooks::haggle` | src/tavern.rs:94 | 1194:C14E | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TavernHooks::message_box` | src/tavern.rs:88 | 1194:59CC | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TavernHooks::quest` | src/tavern.rs:65 | 1194:CED7 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TavernHooks::quests_talk` | src/tavern.rs:69 | 1194:8F1E | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TavernHooks::room_file` | src/tavern.rs:84 | 1194:BA91 | port | the default runs the port | |
| `TavernHooks::room_load` | src/tavern.rs:79 | 1194:C04C | port | the default runs the port | |
| `TavernHooks::rumours` | src/tavern.rs:100 | 1194:E3F7 | stub | false: the port's rumours run | |
| `TavernHooks::tavern_quest` | src/tavern.rs:73 | 1194:90B9 | overridden | the view: shopui.rs ShopFront<'_, '_> | present: the shops' questions answered by the player and drawn as the screen tests check (screens.txt); the ports run as the default would |
| `TavernHooks::text` | src/tavern.rs:105 | 1194:A374 | stub | false: the text port runs | |
| `TextHooks::menu` | src/text.rs:54 | 1194:020E | overridden | the view: main.rs Front<'_> | present: the game's menu (screens.txt); its boxes' state (`menu_boxes`, `menu_closed`; msgbox vectors) |
| `TextHooks::message_box` | src/text.rs:43 | 1194:59CC | overridden | the view: main.rs Front<'_>, panels.rs Said | present: the game's box (screens.txt) |
| `TextHooks::message_box_far` | src/text.rs:47 | 1194:59CC | overridden | the view: panels.rs Said | present: the special item's box (screens.txt) |
| `TravelHooks::advance` | src/travelmap.rs:105 | 0000:142D | stub | false: the game's clock runs the minutes | |
| `TravelHooks::animation` | src/travelmap.rs:100 | 1194:2F69 | present | the travel animation loaded; the view plays its own | |
| `TravelHooks::animation_frame` | src/travelmap.rs:102 | 0000:5526 | overridden | the view: travel.rs Hooks<'_> | present: the journey's frames, one a game tick (the original's as fast as its loop) |
| `TravelHooks::arrival_delete` | src/travelmap.rs:128 | 25FD:0844 | port | the default runs the port | |
| `TravelHooks::arrival_log` | src/travelmap.rs:119 | 26F7:08F7 | stub | false: `travel_arrival` runs | |
| `TravelHooks::arrival_text` | src/travelmap.rs:124 | 1194:A36C | port | the default runs the port | |
| `TravelHooks::arrival_write` | src/travelmap.rs:133 | 25FD:0844 | port | the default runs the port | |
| `TravelHooks::bank` | src/travelmap.rs:89 | 0000:5C86 | port | the default runs the port | |
| `TravelHooks::close` | src/travelmap.rs:147 | 3159:0006 | present | the map's window closed | |
| `TravelHooks::days` | src/travelmap.rs:96 | 1194:2E69 | port | the default runs the port | |
| `TravelHooks::dungeon_location` | src/travelmap.rs:116 | 1194:3B0C | overridden | the view: travel.rs Hooks<'_> | faithful: `city_climate`, `dungeon_build_location` |
| `TravelHooks::journey_stub` | src/travelmap.rs:143 |  | stub | a test's stand-in | |
| `TravelHooks::location` | src/travelmap.rs:114 | 1194:3AE8 | overridden | the view: travel.rs Hooks<'_> | faithful: `city_location` |
| `TravelHooks::message_box` | src/travelmap.rs:78 | 1194:5E1D | overridden | the view: travel.rs Hooks<'_> | present: the map's box (screens.txt) |
| `TravelHooks::pick_stub` | src/travelmap.rs:138 |  | stub | a test's stand-in | |
| `TravelHooks::province` | src/travelmap.rs:110 | 1194:2E14 / 2E18 | port | the default runs the port | |
| `TravelHooks::question` | src/travelmap.rs:80 | 1194:48DB | overridden | the view: travel.rs Hooks<'_> | fixed: was answered Yes without asking: 1194:48DB asks it (020E: "Yes", "No", keys YN), as the view now does (`Asking::Question`); screen-checked (worldunwell) |
| `TravelHooks::resource` | src/travelmap.rs:87 | 0000:56C8 | present | the map's pictures loaded for drawing; the view loads its own | |
| `TravelHooks::screen` | src/travelmap.rs:85 |  | present | a fade | |
| `TravelHooks::show_bank` | src/travelmap.rs:93 | 1194:4D74 | present | a screen bank shown | |
| `TitleHooks::fade_out` | src/title.rs:72 | 1194:55B2 | present | the screen faded out; the view's creation screens draw the intro | |
| `TitleHooks::movie_frame` | src/title.rs:68 | 0000:6546 | present | a movie's frame drawn and shown; no key or click ends it (the view's intro plays the movies itself) | |
| `TitleHooks::picture` | src/title.rs:77 | 0000:5828 | present | a picture shown, faded or dissolved in | |
| `TitleHooks::wait` | src/title.rs:80 | 0000:13BF | present | the intro's waits; none ended by a key | |
| `VisionHooks::fade_out` | src/vision.rs:107 | 1194:55B2 | present | the screen faded out; the view's ending shows the vision | |
| `VisionHooks::frame` | src/vision.rs:102 | 0000:618F | present | the speech's movie's frame drawn and shown | |
| `VisionHooks::line` | src/vision.rs:105 | 0000:5364 | present | a line of the speech's page drawn | |
| `VisionHooks::mouse` | src/vision.rs:97 | 0000:9012 | overridden | the view: vision.rs ClickThrough | elsewhere: a click at each page, as the vectors' run clicks; by default no button, the place kept |
| `TriggerHooks::artifact_audience` | src/trigger.rs:100 | 1194:A36C | overridden | the view: main.rs Front<'_> | faithful: the text port and its box |
| `TriggerHooks::automap_write` | src/trigger.rs:110 | 23F1:0145 and 013C | port | the default runs the port | |
| `TriggerHooks::blackout` | src/trigger.rs:106 | 1194:55B2 | overridden | the view: main.rs Front<'_>, screentest.rs DoorHooks | present: the fade the window plays (areafade.txt) |
| `TriggerHooks::enter_city` | src/trigger.rs:129 | 0000:4EFD and 946A | present | the view's window and the screen cleared | |
| `TriggerHooks::enter_dungeon` | src/trigger.rs:131 | 217B:04DC | overridden | the view: main.rs Front<'_> | faithful: `wild_done` |
| `TriggerHooks::guards` | src/trigger.rs:83 | 0000:3EBE | port | the default runs the port | |
| `TriggerHooks::leave_city` | src/trigger.rs:123 | 217B:027A | port | the default runs the port (`wild_leave_city`; the view's Front<'_> the same) | |
| `TriggerHooks::leave_interior` | src/trigger.rs:116 | 1194:D55F and 2433:0199 | port | the default runs the port | |
| `TriggerHooks::message_box` | src/trigger.rs:77 | 1194:59C8 | overridden | the view: main.rs Front<'_> | present: the game's box (screens.txt) |
| `TriggerHooks::palace_audience` | src/trigger.rs:95 | 0000:41A4 | overridden | the view: main.rs Front<'_> | faithful: `emperor_audience` (audience vectors), its movies the window's |
| `TriggerHooks::sound` | src/trigger.rs:79 | 23BA:002A | port | the default runs the port | |
| `TriggerHooks::sounds_off` | src/trigger.rs:104 | 2F9F:0438 | present | the sounds stopped | |
| `TriggerHooks::tavern_room` | src/trigger.rs:88 | 1194:C048 | port | the default runs the port | |
| `UseHooks::broken` | src/use_item.rs:134 | 2AA1:021B | port | the default runs the port | |
| `UseHooks::cant_use` | src/use_item.rs:146 | 1194:1BE0 | overridden | the view: panels.rs Said | present: the equipment page's boxes and panel lines (screens.txt) |
| `UseHooks::cast` | src/use_item.rs:130 | 29CF:00FA | port | the default runs the port | |
| `UseHooks::clash` | src/use_item.rs:149 | 1194:1B78 | overridden | the view: panels.rs Said | present: the equipment page's boxes and panel lines (screens.txt) |
| `UseHooks::message_box` | src/use_item.rs:143 | 1194:59C8 | overridden | the view: panels.rs Said | present: the equipment page's boxes and panel lines (screens.txt) |
| `UseHooks::pick` | src/use_item.rs:139 | 24FA:0B78 | overridden | the view: panels.rs Said | present: the equipment page's boxes and panel lines (screens.txt) |
| `WeaponHooks::building_closed` | src/weapon.rs:62 | 1194:4FB8 | port | the default runs the port | |
| `WeaponHooks::fire_bow` | src/weapon.rs:67 | 29CF:037F | port | the default runs the port | |
| `WeaponHooks::wall_trigger` | src/weapon.rs:57 | 0000:43B1 | overridden | the view: main.rs Front<'_> | faithful: `wall_trigger` (trigger vectors) |
| `WildHooks::load_block` | src/wild.rs:69 | 217B:0189 | port | the default runs the port | |
| `WildHooks::weather` | src/wild.rs:79 | 0000:0F05 | overridden | the view: areas.rs Wild<'_> | faithful: `weather_set` |
| `WildHooks::write` | src/wild.rs:74 | 0000:9DD2 | port | the default runs the port | |
| `StartHooks::video_mode` | src/start.rs:22 | 0000:6F21 | original | INT 10h AH=0Fh's mode before the start's 13h: 3, the text mode DOS leaves (the harness's too) | |
| `StartHooks::mouse` | src/start.rs:27 | 0000:5384 | original | a mouse driver present (the harness's has one): the pointer's drawing [B02C] chosen (9B04) | |
