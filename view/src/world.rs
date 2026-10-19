//! The world as the renderer sees it: a level loaded through the port's
//! engine, and what drawing it needs read out of the original's memory:
//! the palette, the shading table, the textures (from the EMS banks the
//! INF loader filled) and the meshes built from the map planes.

use arena::bsa::Files;
use arena::engine::Engine;
use arena::exe;
use arena::machine::Machine;
use arena::mem::DS;
use std::path::Path;

/// One vertex: position (map units: x, height, y), texture coordinates
/// in texels (64 a wall texture, where they repeat), the layer (of the
/// wall textures, or with SPRITE of the flats' frames) and a light factor
/// (1 lit, darker for some faces).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub uv: [f32; 2],
    pub layer: u32,
    pub light: f32,
    pub flags: u32,
}

/// Vertex flag: a flat, drawn from the sprite layers, index 0 clear.
pub const SPRITE: u32 = 1;
/// Vertex flag: placed on the screen (x, y in clip space), behind the
/// world, not shaded by the lights: the sky's objects.
pub const SCREEN: u32 = 16;
/// Vertex flags for the sky's bodies (0000:E41A): BODY, behind the sky's
/// objects and not remapped by the night; DARK_SKY, a star, drawn only
/// where the sky under it is dark.
pub const BODY: u32 = 32;
pub const DARK_SKY: u32 = 64;
/// Vertex flag: a wall face whose texels 0 are left unpainted (a door's
/// moved texture: 0000:BAC3 skips them).
pub const SEE_THROUGH: u32 = 128;

/// Vertex flags: a wall face's side as the original numbers it (CCF8),
/// plus one, shifted by WALL_SHIFT: 0 its normal -y (north), 1 +x, 2 +y,
/// 3 -x. Floors, caps and flats have none.
pub const WALL_SHIFT: u32 = 1;

/// Map squares are 128 units across.
pub const SQUARE: f32 = 128.0;

/// The fonts the program's start loads (21EE:0066..008F, each by
/// 0000:7504: the file whole in a heap block of its own, its segment in
/// the fonts' table at 91AC): FONT_A.DAT at [1026], FONT_B.DAT [1028],
/// ARENAFNT.DAT [102C], TEENYFNT.DAT [102E]. The game's routines select a
/// font by copying one of these words to B036 (0000:9AE6), which is how
/// the view tells which font a box is drawn in (`hud::Span::font`).
pub fn fonts_load(m: &mut Machine, dir: &Path) {
    let archive = crate::archive::Archive::open(dir);
    for (k, (name, at)) in [("FONT_A.DAT", 0x1026u16), ("FONT_B.DAT", 0x1028), ("ARENAFNT.DAT", 0x102C), ("TEENYFNT.DAT", 0x102E)].into_iter().enumerate() {
        let Some(d) = archive.read(name) else { continue };
        let Some((seg, _)) = m.heap_alloc(d.len().div_ceil(16).min(0xFFFF) as u16) else { continue };
        for (i, &b) in d.iter().enumerate().take(0xFFFF) {
            m.mem.w8(seg, i as u16, b);
        }
        m.mem.w16(DS, 0x91AC + 2 * k as u16, seg);
        m.mem.w16(DS, at, seg);
    }
}

/// The heap as the original's (0000:00CC over conventional memory from
/// the image's end, the screen's 64 KB a block in use, as the harness
/// lays it out: free 4C00h..8FFEh, the screen's block, free A000h..EFFDh
/// here, the image's segments being the harness's less 1000h). The
/// view's was A000h..F000h alone, 3400h paragraphs short: with the sky's
/// buffers allocated as the game's set-up allocates them (2294:01F1) the
/// LZHUF unpacker's tables (83Eh paragraphs, `mif_decompress`) no longer
/// fitted, and the world map's pictures (OUTPROV.CIF) weren't unpacked.
fn heap(m: &mut Machine) {
    const LOW: u16 = 0x4C00;
    const SCREEN: u16 = 0x8FFF;
    const HIGH: u16 = 0xA000;
    const TOP: u16 = 0xF000;
    m.heap_init(LOW, TOP).expect("heap");
    let header = |m: &mut Machine, seg: u16, next: u16, prev: u16, size: u16, used: u8| {
        m.mem.w16(seg, 0, 0);
        m.mem.w16(seg, 2, next);
        m.mem.w16(seg, 4, 0);
        m.mem.w16(seg, 6, prev);
        m.mem.w16(seg, 8, size);
        m.mem.w8(seg, 0xA, used);
    };
    let low = SCREEN - LOW - 1;
    let high = TOP - HIGH - 3;
    header(m, LOW, SCREEN, 0, low, 0);
    header(m, SCREEN, HIGH, LOW, 0x1000, 1);
    header(m, HIGH, 0, SCREEN, high, 0);
    m.mem.w16(DS, arena::heap::var::FREE, low + high);
}

/// The game at its title, as the original reaches it: the memory as the
/// program finds it (`memory_init`), its start (21EE:0000,
/// `program_start`), main's texts' segment (0000:108B) and working slot
/// (1194:3A51, `working_slot_main`), then the title's Load (0000:1064:
/// the working slot's files fresh, 25FD:0219; 117E: the set-up, 21EE:016C,
/// `game_setup`). A save loaded from here (`areas::load_game_title`) is
/// the title's Load's.
pub fn boot(dir: &Path) -> Engine {
    boot_in(dir, None)
}

/// `boot` with ARENA.BAT's sound driver (-Ssbpdig.adv -IOS220 -IRQS7
/// -DMAS1: the name at 3827, the I/O port, IRQ and DMA channel at 386A,
/// 386C, 386E, as main's command line leaves them, 1194:066A) loaded as
/// that command line loads it, after the working slot's files (076D,
/// `Machine::sound_driver_main`), `driver` answering for it: the
/// level's sounds are then loaded (23BA:0217) and played (23BA:002A)
/// with sound on.
pub fn boot_sound(dir: &Path, driver: &mut dyn arena::digital::DigitalDriver) -> Engine {
    boot_in(dir, Some(driver))
}

fn boot_in(dir: &Path, driver: Option<&mut dyn arena::digital::DigitalDriver>) -> Engine {
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(dir).expect("game directory");
    m.memory_init().expect("the memory");
    m.program_start(&mut files, &mut arena::start::PortStart).expect("the program's start");
    let frame = m.mem.r16(DS, 0x9030);
    m.mem.w16(DS, 0xCB8A, frame);
    m.working_slot_main();
    m.working_files_fresh(&mut files);
    if let Some(driver) = driver {
        for (i, &b) in b"SBPDIG.ADV\0".iter().enumerate() {
            m.mem.w8(DS, arena::digital::var::NAME + i as u16, b);
        }
        for (k, v) in [0x220u16, 7, 1].into_iter().enumerate() {
            m.mem.w16(DS, arena::digital::var::SETTINGS + 2 * k as u16, v);
        }
        m.sound_driver_main(&mut files, driver).expect("the sound driver's load");
    }
    m.game_setup(&mut arena::start::SetupPort(&mut files)).expect("the game's set-up");
    m.store_rng();
    Engine::new(m, files)
}

/// The game as the original reaches its main menu's wait from the
/// program's start (21EE:0000, then main from 0000:1059 with no
/// arguments: `Machine::program_main`, the intro's movies and pictures run
/// to their ends with no input): the state the menu's Load and New go on
/// from (`App::title_engine`). None where the port can't follow it.
pub fn title(dir: &Path) -> Option<Engine> {
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).ok()?).ok()?;
    let mut m = Machine::new(&image);
    let mut files = Files::new(dir).ok()?;
    m.memory_init()?;
    m.program_start(&mut files, &mut arena::start::PortStart)?;
    m.program_main(&mut files, &mut arena::title::NoTitleHooks)?;
    Some(Engine::new(m, files))
}

/// Loads `level` (a .MIF) from the game in `dir` into an engine and
/// enters it, as examples/play.rs does.
pub fn load(dir: &Path, level: &str) -> Engine {
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(dir).expect("game directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    heap(&mut m);
    fonts_load(&mut m, dir);
    // The boxes' tiles as the program's start registers them (0000:756C:
    // BRASS, BRASS2 at 1034, 103E, MARBLE, MARBLE2 at 1036, 103A), MARBLE2
    // in use (B038): the message boxes and menus are laid out on them.
    for (name, w, at) in [(0x3AC0u16, 8u16, 0x1034u16), (0x3ACA, 8, 0x103E), (0x3AD5, 3, 0x1036), (0x3AF6, 3, 0x103A)] {
        let slot = m.resource_add(&mut files, name, w, w).expect("the boxes' tiles");
        m.mem.w16(DS, at, slot);
    }
    let tiles = m.mem.r16(DS, 0x103A);
    m.mem.w16(DS, 0xB038, tiles);
    // The sound driver's buffer (EB89), FFDCh bytes from the heap as the
    // program's start allocates them (2294:CEDB): the level's sound files
    // are read through it on their way to EMS (23BA:0217, `sound_load`).
    let (dseg, doff) = m.heap_alloc_bytes(0xFFDC).expect("heap");
    m.mem.w16(DS, arena::sound::var::BUFFER, doff);
    m.mem.w16(DS, arena::sound::var::BUFFER + 2, dseg);
    // The EMS frame at F000h, above conventional memory as the EMS
    // driver's page frame is (and the harness's): the floor plane's words
    // have its segment added (1194:04DD), and the ceiling's dark runs step
    // by it plus C00h (CCCE, 0000:D865's EDX bug), so its value shows.
    let buf = 0xF000u16;
    m.mem.w16(DS, 0x9030, buf);
    // EMS on: 32 banks swapped through the frame, the last kept back
    // (21EE:0035): the INF loader caches GLOBAL.BSA's directory there
    // (bank 1Fh), and in the 3D mode (C39A bit 400h) keeps it cached
    // from area to area.
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    m.mem.w16(DS, 0x9034, 1);
    // The bank loads unpack through (the wilderness's blocks, 217B:0189;
    // the city's build): that last bank (21EE:004E: the pages less 1),
    // not bank 0, where the textures are.
    m.mem.w16(DS, 0x1030, 0x1F);
    // GLOBAL.BSA's directory cached in bank 16h (21EE:005E, 0000:6A52),
    // for the whole game: the INF loader caches it in bank 1Fh only once
    // it was dropped (0000:EC05, EF6E: an INF loaded outside the 3D
    // mode), and that bank is the one the loads unpack through.
    m.bsa_cache_dir(&mut files, 0x16);
    // The effects' flats (explosions, missiles, blood, sparkles, dust, the
    // arrow) as flats 0..1Bh in their banks (C8FA: 0Fh, 10h), as the
    // program's start loads them (21EE:0134); the level's come after.
    // (The banks free, as EMS's set-up leaves them: 0000:5CCA.)
    m.ems_reset();
    m.effect_flats(&mut files).expect("the effects' flats");
    let (seg, off) = m.heap_alloc_bytes(0x3E80).expect("heap");
    m.mem.w16(DS, 0xBA54, off);
    m.mem.w16(DS, 0xBA56, seg);
    m.mem.w16(DS, 0xCB8A, buf);
    m.mem.w16(DS, 0xCB88, 0x8000);
    for (i, b) in format!("{level}\0").bytes().enumerate() {
        m.mem.w8(DS, 0xF000 + i as u16, b);
        m.mem.w8(DS, 0xC37F + i as u16, b);
    }
    // The spells (records the casting reads): a new game's, from the
    // working slot's copies of the game's *.65 (1194:3A51, 25FD:0219).
    m.working_slot_main();
    m.working_files_fresh(&mut files);
    m.save_set_slot(0x40);
    m.spells_load(&mut files).expect("spells");
    let (sseg, _) = m.spell_record(0);
    if m.mem.r16(sseg, 0x18) == 0 {
        eprintln!("the spells did not load (SPELLSG.64)");
    }
    // The light table's source as the game starts with it.
    lgt(&mut m, dir, arena::weather::NORMAL_LGT);
    m.mem.w8(DS, 0xC3C6, 0x70);
    if let Some(city) = level.strip_prefix("CITY:") {
        city_load(&mut m, &mut files, dir, city);
        // The city's build left the buffer pointer at the frame (1194:0292);
        // entering the area gives the INF a buffer again (0000:A4C5).
        m.mem.w16(DS, 0xBA54, off);
        m.mem.w16(DS, 0xBA56, seg);
    } else if m.level_load(&mut files, 0xF000).is_none() {
        eprintln!("{level}: no such level in {} (a .MIF in GLOBAL.BSA or the game folder, or CITY:PROVINCE:INDEX)", dir.display());
        std::process::exit(2);
    }
    let info = m.mem.r16(DS, 0xCB84);
    // The 3D world begun as the outer loop begins it for every area
    // (0000:0AA9, `world_begin`: the 3D mode, 2294:0006, sets the world
    // loop's hooks and the level's), the level being loaded already.
    let name = arena::transit::var::LEVEL_NAME;
    // The original begins the world before it loads the level, and
    // world_begin empties the level's trigger squares (C866), which the
    // level's TRIG chunk has filled here already: they are kept.
    let triggers: Vec<u8> = (0..0x101u16).map(|i| m.mem.r8(DS, arena::level::var::TRIGGERS + i)).collect();
    m.world_begin(name, &mut arena::transit::NoAreaHooks);
    for (i, b) in triggers.into_iter().enumerate() {
        m.mem.w8(DS, arena::level::var::TRIGGERS + i as u16, b);
    }
    let (cseg, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x1042, cseg);
    // The towns' lists of taverns, temples and stores, as the game's
    // setup allocates them (21EE:016C: 280h bytes, the temples' at offset
    // 100h and the stores' at 180h of its segment). Left at 0:0, the
    // first entry of a list sits at offset 0, which the lookup (1194:3EB1)
    // returns as "none": the first temple's name came out random.
    // The sky's buffers: the day's colours and the dither (2294:01F1, the
    // set-up's resources, before its allocations below).
    m.sky_buffers(&mut files).expect("the sky's buffers");
    let (seg, off) = m.heap_alloc_bytes(0x280).expect("heap");
    let hi = (seg as u32) << 16;
    m.mem.w32(DS, 0x1AB5, hi | off as u32);
    m.mem.w32(DS, 0x1004, hi | 0x100);
    m.mem.w32(DS, 0x1008, hi | 0x180);
    // The setup's A902 bit 10h (21EE:016C, before any play: the title's
    // Load and a new game run it): the game's sprintf (0000:8D14) writes a
    // line feed alone, not CR LF. With CR LF a haggle's line broken at
    // its line feed (1194:84C3 makes both spaces) wrapped one space late.
    let v = m.mem.r16(DS, 0xA902) | 0x10;
    m.mem.w16(DS, 0xA902, v);
    // A player to walk around with: health 50 of 50, fatigue full
    // ((strength + endurance) x 64, 2901:00A8's maximum) — with less, the
    // first exhaustion's hour of rest (2901:0014) can't make up the hour's
    // loss, and the player collapses again and again.
    m.mem.w16(DS, 0x01FD, 50);
    m.mem.w16(DS, 0x01FF, 50);
    m.mem.w16(DS, 0x0201, (50 + 50) * 64);
    for i in 0..8 {
        m.mem.w8(DS, 0x01A4 + arena::character::field::ATTRS + i, 50);
        m.mem.w8(DS, 0x01A4 + arena::character::field::BASE_ATTRS + i, 50);
    }
    light_table(&mut m);
    // The view's clip (A914..A91A: the whole 320 x 200 screen, as C39C
    // holds it) and from it the centre column and the horizon's row
    // (0000:A5D1); the sky's objects are placed by them.
    for (at, v) in [(0xA914, 0), (0xA916, 0), (0xA918, 0x13F), (0xA91A, 0xC7)] {
        m.mem.w16(DS, at, v);
    }
    let centre = (m.mem.r16(DS, 0xA918) + m.mem.r16(DS, 0xA914)) / 2 + m.mem.r16(DS, 0xCB86);
    m.mem.w16(DS, 0xC3D6, centre);
    let horizon = (m.mem.r16(DS, 0xA91A) + m.mem.r16(DS, 0xA916)) / 2;
    m.mem.w16(DS, 0xC3D8, horizon);
    m.rng.seed = 1994;
    m.store_rng();
    let mut engine = Engine::new(m, files);
    engine.enter_area(info).unwrap_or_else(|| panic!("entering the area: {}", arena::giveup::describe()));
    // The level's lights from its flats, as entering a level makes them
    // (0000:478F, `flat_lights`).
    engine.m.flat_lights();
    engine
}

/// 0000:7428 for a .LGT: the file named at DS:`name` (loose in the game
/// directory) to 3AAC:0240, where the light table is made from.
pub fn lgt(m: &mut Machine, dir: &Path, name: u16) {
    let file = String::from_utf8_lossy(m.mem.cstr(DS, name)).to_ascii_uppercase();
    let Ok(d) = std::fs::read(dir.join(&file)) else {
        eprintln!("{file} not found");
        return;
    };
    for (i, &b) in d.iter().take(0xD00).enumerate() {
        m.mem.w8(0x3AAC, 0x240 + i as u16, b);
    }
}

/// The .LGT loads of the weather's set-up.
pub struct Lgt<'a>(pub &'a Path);

impl arena::light::LightHooks for Lgt<'_> {
    fn lgt(&mut self, m: &mut Machine, name: u16) {
        lgt(m, self.0, name);
    }
}

/// 0000:AAA2, the renderer's set-up: the light table (CCC2) the INF
/// loader copies into each texture bank's shading rows (0000:B174):
/// the .LGT's 13 rows as the image holds them at 3AAC:0240, then two of
/// 16 zeros and the fill colour (C3C6: 70h, as 0000:095A sets it in clear
/// weather); CCD2's rows; and each texture bank's shading rows at F000h,
/// on the machine as the original makes them (`render::shade_tables`;
/// fog's FOG.LGT reaches the banks so too). Returns its segment.
pub fn light_table(m: &mut Machine) -> u16 {
    m.light_table_make().expect("heap")
}

/// A city, from `PROVINCE:INDEX[:TERRAIN:WEATHER[:DAY]]`: province 0..7 (8 the
/// Imperial City), the location in it (0..7 city-states, 8..15 towns,
/// 16..31 villages), the terrain class (A50; 1 by default: the climate
/// DA8 is 0 for 1 and 4, 4, the desert, for 3 and 5, else 8) and the
/// weather as the season's roll gives it (0 clear, 1 and 6 overcast with
/// fog, 2 rain, 3 snow, both with fog one day in four, 4 snow with fog, 5
/// rain with fog, 7 snow, fog on a roll), set up as entering the area
/// does (0000:0F05, `weather_set`; the zone is taken as 0) on the day of
/// the year DAY (CCA8, 0 by default: rain and snow then bring fog). The
/// location's record is built from CITYDATA.65 (1194:3B84) and its city
/// from its blocks (1194:0292); it is outdoors (C398 bit 4), and its INF
/// is the one for the climate and weather (0000:0896).
fn city_load(m: &mut Machine, files: &mut Files, dir: &Path, spec: &str) {
    const RECORD: u16 = 0x0A22;
    const PROVINCE_DATA: u16 = 0x1429;
    const PROVINCE_BYTES: usize = 0x4CC;
    let v: Vec<u16> = spec.split(':').map(|x| x.parse().expect("CITY:PROVINCE:INDEX[:TERRAIN:WEATHER[:DAY]]")).collect();
    let (province, index) = (v[0].min(8), v.get(1).copied().unwrap_or(0));
    let data = std::fs::read(dir.join("CITYDATA.65")).expect("CITYDATA.65");
    let p = province as usize;
    for (i, &b) in data[p * PROVINCE_BYTES..(p + 1) * PROVINCE_BYTES].iter().enumerate() {
        m.mem.w8(DS, PROVINCE_DATA + i as u16, b);
    }
    let id = if province == 8 { 0x100 } else { province * 32 + index };
    m.city_build_location(DS, RECORD, id, 0).expect("location record");
    m.mif_build_city(files).expect("city");
    m.mem.w8(DS, arena::weather::var::TERRAIN, v.get(2).copied().unwrap_or(1) as u8);
    m.mem.w8(DS, arena::weather::var::ZONE, 0);
    m.mem.w8(DS, arena::weather::var::ZONE_WEATHER, v.get(3).copied().unwrap_or(0) as u8);
    if let Some(&day) = v.get(4) {
        m.mem.w16(DS, arena::weather::var::DAY, day);
    }
    m.weather_set(&mut Lgt(dir));
    let f = m.mem.r16(DS, 0xC398) | 4;
    m.mem.w16(DS, 0xC398, f);
    m.outdoor_inf();
}

/// 0000:45FC..4631 for a city entered: in the daytime (CCFE 100h..300h)
/// fifteen townspeople come around the player (`spawn_actors`, speed
/// 0Ah), their graphics loaded first (0000:17F9).
/// The wilderness's seven are not done here.
pub fn city_people(engine: &mut Engine) {
    let m = &mut engine.m;
    let t = m.mem.r16(DS, 0xCCFE);
    if m.mem.r16(DS, 0xC398) & 4 == 0 || m.mem.r16(DS, 0xC39A) & 0x200 != 0 || !(0x100..=0x300).contains(&t) {
        return;
    }
    // 0000:17F9: their graphics, for the original's renderer.
    if m.townsfolk_load(&mut engine.files).is_none() {
        eprintln!("the townspeople's graphics gave up: {}", arena::giveup::describe());
    }
    if m.spawn_actors(0xF, 0xA).is_none() {
        eprintln!("the townspeople gave up: {}", arena::giveup::describe());
    }
}

/// 0000:17F9: the townspeople's files, men's (actor +11h 34h) then
/// women's (39h), with `@` the view: men by climate (DA8 / 4: mlgen0@w,
/// @d, @t), women in the desert climate (4) fmgend0@, in snow (BB09 low
/// bits 2) fmgenw0@, else fmgen0@.
pub fn townsfolk(m: &Machine) -> [String; 2] {
    let climate = m.mem.r16(DS, 0x0DA8);
    let name = |at: u16| String::from_utf8_lossy(m.mem.cstr(DS, at)).into_owned();
    let man = name(m.skip_strings(DS, 0x620A, climate >> 2));
    let woman = if climate == 4 {
        0x623D
    } else if m.mem.r8(DS, 0xBB09) & 0x7F == 2 {
        0x624A
    } else {
        0x6231
    };
    [man, name(woman)]
}

/// A debugging aid: rolls random encounters (as the clock does hourly)
/// until creatures come, at most 1000 times, with the area taken as kind
/// 3 (as a dungeon's above 8 is) for the rolls.
pub fn summon(engine: &mut Engine) {
    // The player's square, as a frame sets it.
    engine.m.view_begin();
    let area = engine.m.mem.r8(DS, 0x05E6);
    engine.m.mem.w8(DS, 0x05E6, 3);
    summon_rolls(engine);
    engine.m.mem.w8(DS, 0x05E6, area);
}

/// `summon` until people come (a fight of kind 8000h), sending monsters
/// away (their slots freed) up to 200 times.
pub fn summon_people(engine: &mut Engine) {
    use arena::encounter::var::{CREATURE_SLOTS, FIGHTING, FIGHT_KIND};
    for _ in 0..200 {
        summon(engine);
        let m = &mut engine.m;
        if m.mem.r16(DS, FIGHT_KIND) == 0x8000 {
            return;
        }
        for i in 0..8u16 {
            let at = CREATURE_SLOTS + i * 0x1C + 0xF;
            let f = m.mem.r16(DS, at) | 0x4000;
            m.mem.w16(DS, at, f);
        }
        m.mem.w16(DS, FIGHTING, 0);
    }
    eprintln!("no people came");
}

fn summon_rolls(engine: &mut Engine) {
    let active = |m: &Machine| (0..8u16).any(|i| m.mem.r16(DS, 0x0C57 + i * 0x1C + 0xF) & 0x4000 == 0);
    for _ in 0..1000 {
        if active(&engine.m) {
            return;
        }
        let Engine { m, files, .. } = engine;
        if m.random_encounter(files, &mut arena::encounter::NoFightHooks).is_none() {
            eprintln!("the encounter gave up: {}", arena::giveup::describe());
            return;
        }
    }
}

/// The 256 colours (RGB, 8 bits) of PAL.COL, past its 8-byte header.
pub fn palette(dir: &Path) -> [[u8; 4]; 256] {
    palette_file(dir, "PAL.COL")
}

/// A .COL's 256 colours (RGB, 8 bits), past its 8-byte header.
pub fn palette_file(dir: &Path, name: &str) -> [[u8; 4]; 256] {
    let d = std::fs::read(dir.join(name)).unwrap_or_else(|_| panic!("{name}"));
    let mut p = [[0u8; 4]; 256];
    for (i, c) in p.iter_mut().enumerate() {
        *c = [d[8 + 3 * i], d[9 + 3 * i], d[10 + 3 * i], 255];
    }
    p
}

/// The texture bank the textures start in, and how many there are.
fn texture_banks(m: &Machine) -> (u16, u16) {
    let first = m.mem.r8(DS, arena::info::var::TEXTURE_BANKS) as u16;
    let count = m.mem.r8(DS, arena::info::var::TEXTURE_BANKS + 1) as u16;
    (first, count)
}

/// All the level's textures, row-major 64 x 64 bytes each, numbered from
/// 1 (texture t is element t - 1): 15 to a bank, stored transposed.
pub fn textures(m: &Machine) -> Vec<[u8; 4096]> {
    let mut out = level_textures(m);
    let doors = door_layers(m, &out);
    out.extend(doors);
    out
}

/// How many of `textures` are the level's own (before `door_layers`).
pub fn texture_count(m: &Machine) -> u32 {
    let (_, count) = texture_banks(m);
    (count as u32 * 15).max(1)
}

/// The level's own textures (`textures` without the doors' layers).
fn level_textures(m: &Machine) -> Vec<[u8; 4096]> {
    let (first, count) = texture_banks(m);
    let mut out = Vec::new();
    for b in 0..count {
        let bank = m.ems_bank(first + b);
        for k in 0..15usize {
            let src = &bank[k * 0x1000..(k + 1) * 0x1000];
            let mut t = [0u8; 4096];
            for u in 0..64 {
                for v in 0..64 {
                    t[v * 64 + u] = src[u * 64 + v];
                }
            }
            out.push(t);
        }
    }
    if out.is_empty() {
        out.push([0; 4096]);
    }
    out
}

/// The shading table: 16 rows of 256 colour indices, darkest last, as
/// the INF loader copies it to the top of each texture bank (F000).
pub fn shading(m: &Machine) -> Vec<u8> {
    let (first, _) = texture_banks(m);
    let bank = m.ems_bank(first);
    let mut rows = bank[0xF000..].to_vec();
    if rows.iter().all(|&b| b == 0) {
        // No shading segment: identity rows.
        rows = (0..16).flat_map(|_| 0..=255u8).collect();
    }
    rows
}

/// The ceiling's texture: the slot the *CEILING texture went to (its
/// segment in the frame, CB64), in the first texture bank.
fn ceiling_texture(m: &Machine) -> Option<u32> {
    // None outdoors (C398 bit 4), whatever an interior's INF left at CB64
    // (city INFs have no *CEILING).
    if m.mem.r16(DS, 0xC398) & 4 != 0 {
        return None;
    }
    let seg = m.mem.r16(DS, arena::info::var::CEILING_SEG);
    let frame = m.mem.r16(DS, 0x9030);
    if seg == 0 || seg < frame {
        return None;
    }
    Some(((seg - frame) << 4 >> 12) as u32)
}

/// The meshes of the level: walls, raised platforms, floors and the
/// ceiling, as triangles.
pub fn mesh(m: &Machine) -> Vec<Vertex> {
    let w = m.mem.r16(DS, 0xC357).min(128);
    let d = m.mem.r16(DS, 0xC359).min(128);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let floors = m.mem.r16(DS, arena::mif::var::FLOOR_SEG);
    let frame = m.mem.r16(DS, 0x9030);
    let ceiling = m.mem.r16(DS, arena::movement::var::CEILING) as f32;
    let ceiling_tex = ceiling_texture(m);
    let at = |col: u16, row: u16| (row << 8) | (col << 1);
    let wall = |col: i32, row: i32| -> u16 {
        if col < 0 || row < 0 || col >= w as i32 || row >= d as i32 {
            return 0xFFFF;
        }
        m.mem.r16(walls, at(col as u16, row as u16))
    };
    // A solid, full-height wall (for hiding the faces between them).
    let solid = |v: u16| -> bool { v == 0xFFFF || (v != 0 && v & 0x8000 == 0 && (v >> 8) as u8 == v as u8) };
    let mut out = Vec::new();
    let mut quad = |p: [[f32; 3]; 4], uv: [[f32; 2]; 4], layer: u32, light: f32, flags: u32| {
        for i in [0, 1, 2, 0, 2, 3] {
            out.push(Vertex { pos: p[i], uv: uv[i], layer, light, flags });
        }
    };
    let side = |s: u32| (s + 1) << WALL_SHIFT;
    for row in 0..d as i32 {
        for col in 0..w as i32 {
            let x0 = col as f32 * SQUARE;
            let z0 = row as f32 * SQUARE;
            let (x1, z1) = (x0 + SQUARE, z0 + SQUARE);
            let v = wall(col, row);
            // The sides of a box from `lo` to `hi` facing open squares.
            // `lit`: the faces hide lights behind them (a side for the
            // shader's test, 0000:B7A9), which kinds 0Ah and 0Dh's never
            // do: B7A9 returns at once for CCF8 FFFFh, which a kind 0Ah's
            // face is drawn with (D27A after CF20's BA7F 2), and for
            // CB37 0Dh.
            let mut sides = |lo: f32, hi: f32, layer: u32, cull: &dyn Fn(u16) -> bool, lit: bool| {
                let th = 64.0 * (hi - lo) / ceiling.max(1.0);
                let uv = [[0.0, th], [64.0, th], [64.0, 0.0], [0.0, 0.0]];
                let side = |s: u32| if lit { side(s) } else { 0 };
                if !cull(wall(col, row - 1)) {
                    quad([[x1, lo, z0], [x0, lo, z0], [x0, hi, z0], [x1, hi, z0]], uv, layer, 0.85, side(0));
                }
                if !cull(wall(col, row + 1)) {
                    quad([[x0, lo, z1], [x1, lo, z1], [x1, hi, z1], [x0, hi, z1]], uv, layer, 0.85, side(2));
                }
                if !cull(wall(col - 1, row)) {
                    quad([[x0, lo, z0], [x0, lo, z1], [x0, hi, z1], [x0, hi, z0]], uv, layer, 1.0, side(3));
                }
                if !cull(wall(col + 1, row)) {
                    quad([[x1, lo, z1], [x1, lo, z0], [x1, hi, z0], [x1, hi, z1]], uv, layer, 1.0, side(1));
                }
            };
            let mut has_floor = true;
            if v != 0 && v & 0x8000 == 0 {
                if (v >> 8) as u8 == v as u8 {
                    // A solid wall: texture by its low 6 bits.
                    let t = (v & 0x3F) as u32;
                    sides(0.0, ceiling, t.saturating_sub(1), &solid, true);
                    has_floor = false;
                } else {
                    // A raised platform: the box tables give the side and
                    // cap textures; its heights come from the high byte.
                    let side = m.mem.r8(DS, arena::info::var::BOX_SIDES + (v & 0xF)) as u32;
                    let cap = m.mem.r8(DS, arena::info::var::BOX_CAPS + ((v >> 4) & 0xF)) as u32;
                    let (top, bottom) = m.move_wall_height(v);
                    let (lo, hi) = (bottom as i16 as f32, top as i16 as f32);
                    sides(lo, hi, side.saturating_sub(1), &solid, true);
                    let uv = [[0.0, 0.0], [64.0, 0.0], [64.0, 64.0], [0.0, 64.0]];
                    quad([[x0, hi, z0], [x1, hi, z0], [x1, hi, z1], [x0, hi, z1]], uv, cap.saturating_sub(1), 1.0, 0);
                    quad([[x0, lo, z1], [x1, lo, z1], [x1, lo, z0], [x0, lo, z0]], uv, cap.saturating_sub(1), 0.7, 0);
                }
            } else if v != 0 && matches!(v >> 12, 9 | 0xA | 0xD) {
                // The other wall kinds, drawn as walls for now. Doors
                // (0Bh) move: `doors`.
                let t = (v & 0x3F) as u32;
                sides(0.0, ceiling, t.saturating_sub(1), &|_| false, v >> 12 == 9);
            }
            if has_floor {
                let f = m.mem.r16(floors, at(col as u16, row as u16));
                let kind = f.wrapping_sub(frame) >> 8;
                let uv = [[0.0, 0.0], [64.0, 0.0], [64.0, 64.0], [0.0, 64.0]];
                if kind < 0xC {
                    quad([[x0, 0.0, z1], [x1, 0.0, z1], [x1, 0.0, z0], [x0, 0.0, z0]], uv, kind as u32, 0.9, 0);
                }
                if let Some(t) = ceiling_tex {
                    quad([[x0, ceiling, z0], [x1, ceiling, z0], [x1, ceiling, z1], [x0, ceiling, z1]], uv, t, 0.8, 0);
                }
            }
        }
    }
    out
}

/// The doors (wall kind 0Bh) as they stand this frame, drawn as the
/// original's renderer draws them (src/doordraw.rs): a face on each edge
/// the door shares with a square that isn't a full wall, as that
/// square's face (0000:D0A1 draws a wall's faces from the open squares
/// beside it). Open (high byte bits 1-3, 0..7, as `Machine::door_drawn`
/// steps them), a door of kind 0 (low byte bits 6-7) swings its face out
/// into that square about the face's first corner by (open + 1) * 16 of
/// 512 to a turn (0000:D344, `doordraw::swing_end`); kinds 1..3 keep the
/// face where it is and draw the texture moved over, the uncovered
/// texels clear (0000:BAF3, `door_layers`).
///
/// Left out: the original swings only the first face drawn after the
/// door was met (C398 bit 1000h, cleared by that face), and the frame a
/// closing door shuts it still draws at 16 (the bit set by the step
/// before, CCC4 0).
pub fn doors(m: &Machine) -> Vec<Vertex> {
    use arena::doordraw::{swing_end, FACES};
    let w = m.mem.r16(DS, 0xC357).min(128);
    let d = m.mem.r16(DS, 0xC359).min(128);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let ceiling = m.mem.r16(DS, arena::movement::var::CEILING) as f32;
    let base = texture_count(m);
    let moved = moved_doors(m);
    let mut out = Vec::new();
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, (row << 8) | (col << 1));
            if v >> 12 != 0xB {
                continue;
            }
            let tex = ((v & 0x3F) as u32).saturating_sub(1);
            let kind = ((v & 0xFF) >> 6) as u8;
            let open = (v >> 9) & 7;
            let layer = match moved.iter().position(|&k| k == (tex, kind)) {
                Some(i) if open != 0 => base + i as u32 * 7 + open as u32 - 1,
                _ => tex,
            };
            // Each neighbour, and the side of it the shared edge is.
            for ((dc, dr), side) in [((-1, 0), 1usize), ((1, 0), 3), ((0, -1), 2), ((0, 1), 0)] {
                let (c, r) = (col as i32 + dc, row as i32 + dr);
                let n = if c < 0 || r < 0 || c >= w as i32 || r >= d as i32 { 0xFFFF } else { m.mem.r16(walls, (r << 8 | c << 1) as u16) };
                if n == 0xFFFF || (n != 0 && n & 0x8000 == 0 && n as u8 == (n >> 8) as u8) {
                    continue;
                }
                let (ox, oy) = ((c << 7) as u16, (r << 7) as u16);
                let ((hx, hy), (fx, fy)) = FACES[side];
                let hinge = (ox.wrapping_add(hx), oy.wrapping_add(hy));
                let mut far = (ox.wrapping_add(fx), oy.wrapping_add(fy));
                if kind == 0 && open != 0 {
                    far = swing_end(m, hinge, far, open);
                }
                let (hx, hz) = (hinge.0 as i16 as f32, hinge.1 as i16 as f32);
                let (fx, fz) = (far.0 as i16 as f32, far.1 as i16 as f32);
                // The texture's first column at the face's first corner.
                let p = [[hx, 0.0, hz], [fx, 0.0, fz], [fx, ceiling, fz], [hx, ceiling, hz]];
                let uv = [[0.0, 64.0], [64.0, 64.0], [64.0, 0.0], [0.0, 0.0]];
                for i in [0, 1, 2, 0, 2, 3] {
                    out.push(Vertex { pos: p[i], uv: uv[i], layer, light: if side % 2 == 0 { 0.85 } else { 1.0 }, flags: SEE_THROUGH });
                }
            }
        }
    }
    out
}

/// The textures (from 0, as `textures` numbers them) and kinds (1..3) of
/// the level's doors that draw their texture moved over as they open,
/// in order: `door_layers` holds seven layers for each, open 1..7.
fn moved_doors(m: &Machine) -> Vec<(u32, u8)> {
    let w = m.mem.r16(DS, 0xC357).min(128);
    let d = m.mem.r16(DS, 0xC359).min(128);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let mut set = std::collections::BTreeSet::new();
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, (row << 8) | (col << 1));
            let kind = ((v & 0xFF) >> 6) as u8;
            if v >> 12 == 0xB && kind != 0 {
                set.insert((((v & 0x3F) as u32).saturating_sub(1), kind));
            }
        }
    }
    set.into_iter().collect()
}

/// The layers after the level's textures: for each of `moved_doors`, its
/// texture moved over as 0000:BAF3 moves it, open 1..7 (row-major, as
/// `textures`).
pub fn door_layers(m: &Machine, textures: &[[u8; 4096]]) -> Vec<[u8; 4096]> {
    let mut out = Vec::new();
    for (tex, kind) in moved_doors(m) {
        let t = textures.get(tex as usize).copied().unwrap_or([0; 4096]);
        // Back to the banks' column-major order.
        let mut cols = [0u8; 4096];
        for u in 0..64 {
            for v in 0..64 {
                cols[u * 64 + v] = t[v * 64 + u];
            }
        }
        for open in 1..=7 {
            let moved = arena::doordraw::door_texture(&cols, 0, kind, open);
            let mut layer = [0u8; 4096];
            for u in 0..64 {
                for v in 0..64 {
                    layer[v * 64 + u] = moved[u * 64 + v];
                }
            }
            out.push(layer);
        }
    }
    out
}

/// The map square under the centre of the view, as the original's
/// renderer leaves the square under the cursor (CB92) for a click: the
/// first square that isn't empty along the player's heading, within
/// `reach` squares; None past it or the map's edge.
pub fn pick(m: &Machine, reach: u16) -> Option<u16> {
    pick_at(m, reach).map(|(sq, _)| sq)
}

/// `pick`, with how far along the line the square was met.
pub fn pick_at(m: &Machine, reach: u16) -> Option<(u16, f32)> {
    pick_where(m, reach, |_, _| true)
}

/// `pick_at`, meeting only the squares `meets` takes.
fn pick_where(m: &Machine, reach: u16, meets: impl Fn(u16, u16) -> bool) -> Option<(u16, f32)> {
    let w = m.mem.r16(DS, 0xC357).min(128) as i32;
    let d = m.mem.r16(DS, 0xC359).min(128) as i32;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    use arena::movement::var::{HEADING, X, Y};
    let (x, y) = (m.mem.r16(DS, X) as f32, m.mem.r16(DS, Y) as f32);
    let a = m.mem.r16(DS, HEADING) as f32 / 512.0 * std::f32::consts::TAU;
    let (dx, dy) = (a.sin(), a.cos());
    let start = (x as i32 >> 7, y as i32 >> 7);
    let mut t = 0.0;
    while t < reach as f32 * SQUARE {
        t += 4.0;
        let (col, row) = ((x + dx * t) as i32 >> 7, (y + dy * t) as i32 >> 7);
        if col < 0 || row < 0 || col >= w || row >= d {
            return None;
        }
        if (col, row) == start {
            continue;
        }
        let sq = (row << 8 | col << 1) as u16;
        let v = m.mem.r16(walls, sq);
        if v != 0 && meets(sq, v) {
            return Some((sq, t));
        }
    }
    None
}

/// The level's flats by number (from 0, FLAT_BASE being the level's
/// first), decoded where the loader read a file the decoders know.
pub fn flats(m: &Machine) -> Vec<Option<crate::flats::Flat>> {
    let n = m.mem.r16(DS, arena::info::var::FLAT_NUMBER).min(0x100);
    (0..n)
        .map(|f| {
            let off = m.mem.r16(DS, arena::info::var::FLAT_DATA + f * 4) as usize;
            let bank = m.mem.r16(DS, arena::info::var::FLAT_DATA + f * 4 + 2);
            if off == 0xFFFF {
                return None;
            }
            let data = m.ems_bank(bank);
            crate::flats::decode(m.mem.r8(DS, arena::info::var::FLAT_KINDS + f), &data[off..])
        })
        .collect()
}

/// Where a frame is in the sprite atlas, and its size.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// An image's frames in the atlas (from `first`, `count` of them).
#[derive(Clone, Copy, Debug)]
pub struct Frames {
    pub first: usize,
    pub count: usize,
}

/// Every flat image the renderer draws, packed in shelves into one
/// atlas of palette indices `ATLAS` wide.
pub struct SpriteSheet {
    pub h: u32,
    pub pixels: Vec<u8>,
    pub rects: Vec<Rect>,
    /// The level's flats, by number.
    pub flats: Vec<Option<Frames>>,
    /// The creatures' images, by type: views 1..5 (front to back) and 6.
    pub creatures: Vec<[Option<Frames>; 6]>,
    /// The people's images by file name (upper case), with their CFA
    /// header's height and offsets (+2, +6, +8).
    pub people: std::collections::HashMap<String, (Frames, [i16; 3])>,
    /// The sky's images by file name (upper case): every name 2294:08B9
    /// can make, and the volcano's (292B:029A).
    pub sky: std::collections::HashMap<String, Frames>,
}

/// The files the sky's objects can be: mount001..011.img, desert1..4,
/// temp01..10, cloud01..17 and the volcano near and far.
fn sky_files() -> Vec<String> {
    let mut v: Vec<String> = (1..=11).map(|i| format!("MOUNT0{i:02}.IMG")).collect();
    v.extend((1..=4).map(|i| format!("DESERT{i}.IMG")));
    v.extend((1..=10).map(|i| format!("TEMP{i:02}.IMG")));
    v.extend((1..=17).map(|i| format!("CLOUD{i:02}.IMG")));
    v.extend(["VOLCANO2.DFA".to_string(), "VOLCANO3.IMG".to_string()]);
    // The lightning's bolts (0000:E5C9 loads them into the sky's bank).
    v.extend((1..=6).map(|i| format!("LGLIT0{i}.CFA")));
    // The sky's bodies (0000:E5C9 too): the sun, the moons' phases, the
    // stars.
    v.extend(["SUN.IMG".to_string(), "MOON01.DFA".to_string(), "MOON02.DFA".to_string()]);
    v.extend((1..=8).map(|i| format!("STAR{i}.IMG")));
    v
}

/// The atlas entry of the stars' single pixels: colours 40h..4Fh, one
/// texel each (a cluster's pixel, 2294:03A1).
pub const STAR_PIXELS: &str = "#STAR PIXELS";

/// Whether a file is a person's image: a view (`01PLTWLK.CFA`: a woman's
/// with 1 first, the view 1..5, then the look and the action, or a
/// weapon or shield) or a townsperson's (`MLGEN01W.CFA`, `FMGEN01.CFA`).
fn is_person_file(name: &str) -> bool {
    let b = name.as_bytes();
    let view = b.len() > 6 && matches!(b[0], b'0' | b'1') && (b'1'..=b'5').contains(&b[1]) && b[2].is_ascii_alphabetic();
    (view || name.starts_with("MLGEN") || name.starts_with("FMGEN")) && name.ends_with(".CFA")
}

pub const ATLAS: u32 = 4096;

struct Packer {
    x: u32,
    y: u32,
    shelf: u32,
    pixels: Vec<u8>,
    rects: Vec<Rect>,
}

impl Packer {
    fn add(&mut self, f: &crate::flats::Flat) -> Frames {
        let first = self.rects.len();
        let (w, h) = (f.w as u32, f.h as u32);
        for frame in &f.frames {
            if self.x + w > ATLAS {
                self.y += self.shelf;
                self.x = 0;
                self.shelf = 0;
            }
            let r = Rect { x: self.x, y: self.y, w, h };
            let need = ((r.y + h) * ATLAS) as usize;
            if self.pixels.len() < need {
                self.pixels.resize(need, 0);
            }
            for row in 0..f.h {
                let at = ((r.y as usize + row) * ATLAS as usize) + r.x as usize;
                self.pixels[at..at + f.w].copy_from_slice(&frame[row * f.w..(row + 1) * f.w]);
            }
            self.rects.push(r);
            self.x += w;
            self.shelf = self.shelf.max(h);
        }
        Frames { first, count: f.frames.len() }
    }
}

/// The creatures' file name patterns (`rat@.cfa`, ... with `@` the view
/// 1..6), from the table in the image, by type from 1.
pub fn creature_names(m: &Machine) -> Vec<String> {
    let needle = b"rat@.cfa\0";
    let mut at = None;
    for o in 0..0xFFF0u16 {
        if (0..needle.len() as u16).all(|i| m.mem.r8(DS, o + i) == needle[i as usize]) {
            at = Some(o);
            break;
        }
    }
    let Some(mut o) = at else { return vec![] };
    let mut names = Vec::new();
    while names.len() < 24 {
        let s: String = (0..13).map(|i| m.mem.r8(DS, o + i)).take_while(|&c| c != 0).map(|c| c as char).collect();
        if !s.contains('@') {
            break;
        }
        o += s.len() as u16 + 1;
        names.push(s);
    }
    names
}

/// One file read and decoded (`kind` as `flats::decode` takes it), with
/// the header words at +2, +6 and +8 (a person's: the weapon's placement
/// over it).
fn decode_file(archive: &crate::archive::Archive, name: &str, kind: u8) -> Option<(crate::flats::Flat, [i16; 3])> {
    let d = archive.read(name)?;
    let f = crate::flats::decode(kind, &d)?;
    let at = |o: usize| d.get(o..o + 2).map_or(0, |b| i16::from_le_bytes([b[0], b[1]]));
    Some((f, [at(2), at(6), at(8)]))
}

/// A file decoded for the sheet (`decode_file`), shared by the sheets.
type Decoded = Option<std::rc::Rc<(crate::flats::Flat, [i16; 3])>>;
/// A file's name and what `decode_file` made of it.
type Made = (String, Option<(crate::flats::Flat, [i16; 3])>);

thread_local! {
    /// The files decoded so far, by name. The game's files don't change
    /// during a run, and every area's sheet holds the same creatures,
    /// people and sky, so only the first sheet decodes them.
    static DECODED: std::cell::RefCell<std::collections::HashMap<String, Decoded>> = Default::default();
}

/// `files` decoded, in their order: from `DECODED`, the rest decoded on
/// threads at once (decoding a file reads nothing but the file).
fn decoded(archive: &crate::archive::Archive, files: &[(String, u8)]) -> Vec<Decoded> {
    let missing: Vec<&(String, u8)> = DECODED.with(|c| {
        let c = c.borrow();
        let mut seen = std::collections::HashSet::new();
        files.iter().filter(|(n, _)| !c.contains_key(n) && seen.insert(n.as_str())).collect()
    });
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get()).clamp(1, 16);
    let chunk = missing.len().div_ceil(threads).max(1);
    let made: Vec<Made> = std::thread::scope(|s| {
        let parts: Vec<_> = missing
            .chunks(chunk)
            .map(|part| s.spawn(move || part.iter().map(|(name, kind)| (name.clone(), decode_file(archive, name, *kind))).collect::<Vec<_>>()))
            .collect();
        parts.into_iter().flat_map(|h| h.join().expect("decoding the sheet's files")).collect()
    });
    DECODED.with(|c| {
        let mut c = c.borrow_mut();
        for (name, d) in made {
            c.insert(name, d.map(std::rc::Rc::new));
        }
        files.iter().map(|(n, _)| c.get(n).cloned().flatten()).collect()
    })
}

pub fn sprite_sheet(flats: &[Option<crate::flats::Flat>], archive: &crate::archive::Archive, names: &[String]) -> SpriteSheet {
    let mut p = Packer { x: 0, y: 0, shelf: 0, pixels: Vec::new(), rects: Vec::new() };
    let flats = flats.iter().map(|f| f.as_ref().map(|f| p.add(f))).collect();
    // The files after the level's flats, decoded first and packed in this
    // order: the creatures' views, the people, the sky's.
    let mut people: Vec<&str> = archive.names().filter(|n| is_person_file(n)).collect();
    people.sort_unstable();
    let sky = sky_files();
    let mut files: Vec<(String, u8)> = Vec::new();
    for name in names {
        files.extend((1..=6).map(|i| (name.replace('@', &i.to_string()), 0x80)));
    }
    files.extend(people.iter().map(|n| (n.to_string(), 0x80)));
    for name in &sky {
        let kind = if name.ends_with(".DFA") {
            1
        } else if name.ends_with(".CFA") {
            0x80
        } else {
            0
        };
        files.push((name.clone(), kind));
    }
    let made = decoded(archive, &files);
    let mut made = made.iter();
    let mut creatures = Vec::new();
    for _ in names {
        let mut views = [None; 6];
        for v in views.iter_mut() {
            if let Some(d) = made.next().expect("a view") {
                *v = Some(p.add(&d.0));
            }
        }
        creatures.push(views);
    }
    let mut people_frames = std::collections::HashMap::new();
    for name in people {
        if let Some(d) = made.next().expect("a person") {
            let frames = p.add(&d.0);
            people_frames.insert(name.to_string(), (frames, d.1));
        }
    }
    let mut sky_frames = std::collections::HashMap::new();
    for name in sky {
        if let Some(d) = made.next().expect("a sky's") {
            let frames = p.add(&d.0);
            sky_frames.insert(name, frames);
        }
    }
    let strip = crate::flats::Flat { w: 16, h: 1, frames: vec![(0x40..0x50).collect()] };
    let frames = p.add(&strip);
    sky_frames.insert(STAR_PIXELS.to_string(), frames);
    let h = ((p.pixels.len() as u32).div_ceil(ATLAS)).max(1);
    let mut pixels = p.pixels;
    pixels.resize((h * ATLAS) as usize, 0);
    SpriteSheet { h, pixels, rects: p.rects, flats, creatures, people: people_frames, sky: sky_frames }
}

/// The graphics list of the people in a fight (2686:0423, at 1044,
/// names with `@` for the view), while one is on (FFF 8000h).
fn people_list(m: &Machine) -> Vec<String> {
    let mut names = Vec::new();
    if m.mem.r16(DS, arena::encounter::var::FIGHT_KIND) != 0x8000 {
        return names;
    }
    let mut at = arena::outfit::var::LIST;
    while m.mem.r8(DS, at) != 0 && names.len() < 8 {
        let mut s = String::new();
        while m.mem.r8(DS, at) != 0 {
            s.push(m.mem.r8(DS, at) as char);
            at = at.wrapping_add(1);
        }
        at = at.wrapping_add(1);
        names.push(s);
    }
    names
}

/// A billboard: the frame `rect` standing at (x, y) from `lo` up, `scale`
/// units a pixel (see `size`), facing a camera whose right is (rx, rz).
#[allow(clippy::too_many_arguments)]
fn billboard(out: &mut Vec<Vertex>, rect: Rect, x: f32, y: f32, lo: f32, scale: f32, (rx, rz): (f32, f32), mirror: bool) {
    billboard_at(out, rect, x, y, lo, 0.0, scale, (rx, rz), mirror)
}

/// `billboard`, its centre moved `shift` along the camera's right.
#[allow(clippy::too_many_arguments)]
fn billboard_at(out: &mut Vec<Vertex>, rect: Rect, x: f32, y: f32, lo: f32, shift: f32, scale: f32, (rx, rz): (f32, f32), mirror: bool) {
    let (x, y) = (x + rx * shift, y + rz * shift);
    let hw = rect.w as f32 * scale / 2.0;
    let hi = lo + rect.h as f32 * scale;
    let p = [
        [x - rx * hw, lo, y - rz * hw],
        [x + rx * hw, lo, y + rz * hw],
        [x + rx * hw, hi, y + rz * hw],
        [x - rx * hw, hi, y - rz * hw],
    ];
    // Texel coordinates, kept just inside the frame.
    const IN: f32 = 0.01;
    let (mut u0, mut u1) = (rect.x as f32 + IN, (rect.x + rect.w) as f32 - IN);
    if mirror {
        std::mem::swap(&mut u0, &mut u1);
    }
    let (v0, v1) = (rect.y as f32 + IN, (rect.y + rect.h) as f32 - IN);
    let uv = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(Vertex { pos: p[i], uv: uv[i], layer: 0, light: 1.0, flags: SPRITE });
    }
}

/// This frame's billboards, facing a camera turned to `heading`:
/// - the flats standing in the map (MAP1 words 8xxx: flat low byte +
///   FLAT_BASE; else the floor plane's low byte less 1, on the square's
///   platform) at their squares' centres, raised by the flat record's
///   Y, animations at frame `tick`;
/// - the creatures in the actor slots (not inactive, 4000h): sprite C0h +
///   view (0..4, mirrored with flag 2) is the type's view file, C5h
///   (dying) its sixth, any other a flat (the corpse); +D the frame.
pub fn sprites(m: &Machine, sheet: &SpriteSheet, heading: f32, tick: usize) -> Vec<Vertex> {
    sprites_tagged(m, sheet, heading, tick).0
}

/// What a billboard shows, for the hit test: an actor slot (its offset)
/// or a flat standing in the map (its square, its number from 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Slot(u16),
    Flat { col: u16, row: u16, n: u16 },
}

/// `sprites`, with each target's vertices (a range of the list).
fn sprites_tagged(m: &Machine, sheet: &SpriteSheet, heading: f32, tick: usize) -> (Vec<Vertex>, Vec<(Target, std::ops::Range<usize>)>) {
    let mut tags = Vec::new();
    let w = m.mem.r16(DS, 0xC357).min(128);
    let d = m.mem.r16(DS, 0xC359).min(128);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let base = m.mem.r16(DS, arena::info::var::FLAT_BASE);
    let a = heading / 512.0 * std::f32::consts::TAU;
    // The camera's right, along which a billboard spreads.
    let right = (a.cos(), -a.sin());
    let mut out = Vec::new();
    let frame_of = |f: Frames, k: usize| sheet.rects[f.first + k % f.count];
    let flat = |n: usize| sheet.flats.get(n).copied().flatten();
    let lift = |n: usize| m.mem.r8(DS, arena::info::var::FLATS + n as u16 * 5) as i8 as f32;
    // The units a pixel the original draws a flat at (0000:C0F1): it
    // projects the image's size at a depth of its own, a quarter of the
    // true one for a flat whose record (F:, +3) has bit 4 (C140, C347:
    // the trees), half again for an actor's view (sprite C0h on) or an
    // actor with flag 4 (C1A2, C17F, C351), so 2/3 the size.
    let size = |sprite: u8, flags: u16| {
        let mut s = 1.0;
        if sprite < 0xC0 && m.mem.r8(DS, arena::info::var::FLATS + sprite as u16 * 5 + 3) & 4 != 0 {
            s *= 4.0;
        }
        if sprite >= 0xC0 || flags & 4 != 0 {
            s /= 1.5;
        }
        s
    };
    let floors = m.mem.r16(DS, arena::mif::var::FLOOR_SEG);
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, (row << 8) | (col << 1));
            // A flat in the floor plane (its low byte less 1, as the
            // lights read it, 0000:0810, and a click takes it, 292B:0853:
            // Black Gate's hammer) stands on the square's platform, if any.
            let fb = m.mem.r8(floors, (row << 8) | (col << 1));
            // A creature's flat (bit 2 of its high byte, 25CA:0200) isn't
            // drawn: the drawing calls the hook CD56 instead (0000:CF96,
            // `creature_flats_seen`).
            let (n, base_y) = if v >> 12 == 8 {
                if v & 0x200 != 0 {
                    continue;
                }
                (((v & 0xFF) + base) as usize, 0.0)
            } else if fb != 0 {
                let top = if v != 0 && v & 0x8000 == 0 && (v >> 8) as u8 != v as u8 { m.move_wall_height(v).0 as i16 as f32 } else { 0.0 };
                // Its low byte less 1 only over a platform (the square's
                // word positive), as the drawing takes it (0000:CFBA, the
                // floor's flat, `render::flat_square`).
                let raised = (v as i16) > 0;
                ((fb as u16 - raised as u16 + base) as usize, top)
            } else {
                continue;
            };
            if let Some(f) = flat(n) {
                let (x, y) = (col as f32 * SQUARE + SQUARE / 2.0, row as f32 * SQUARE + SQUARE / 2.0);
                let start = out.len();
                billboard(&mut out, frame_of(f, tick), x, y, base_y + lift(n), size(n as u8, 0), right, false);
                tags.push((Target::Flat { col, row, n: n as u16 }, start..out.len()));
            }
        }
    }
    let seg = m.mem.r16(DS, 0x1042);
    let list = people_list(m);
    let towns = townsfolk(m);
    // The slot being drawn and where its vertices start, tagged once the
    // next one begins.
    let mut open: Option<(u16, usize)> = None;
    let mut close = |open: &mut Option<(u16, usize)>, end: usize| {
        if let Some((si, start)) = open.take() {
            if end > start {
                tags.push((Target::Slot(si), start..end));
            }
        }
    };
    for slot in 0..23u16 {
        close(&mut open, out.len());
        let si = 0x0AB3 + slot * 0x1C;
        let flags = m.mem.r16(DS, si + 0xF);
        let rec = m.mem.r16(DS, si + 0x12);
        // (The first record is at offset 0.)
        if flags & 0x4000 != 0 {
            continue;
        }
        open = Some((si, out.len()));
        let (x, z, y) = (m.mem.r16(DS, si) as f32, m.mem.r16(DS, si + 2) as i16 as f32, m.mem.r16(DS, si + 4) as f32);
        let sprite = m.mem.r8(DS, si + 0xC);
        let frame = m.mem.r8(DS, si + 0xD) as usize;
        let mirror = flags & 2 != 0;
        let sc = size(sprite, flags);
        // A person in a fight (0000:1C9A): the walk, or from frame 6 the
        // attack with the weapon over it.
        // A townsperson (0000:1CAC): the file by +11h, the frame as it is.
        if slot < 15 && (0xC0..0xC5).contains(&sprite) {
            let file = match m.mem.r8(DS, si + 0x11) {
                0x34 => &towns[0],
                0x39 => &towns[1],
                _ => continue,
            };
            let name = file.replace('@', &(sprite - 0xC0 + 1).to_string()).to_ascii_uppercase();
            if let Some(&(f, _)) = sheet.people.get(&name) {
                billboard(&mut out, frame_of(f, frame), x, y, -z, sc, right, mirror);
            }
            continue;
        }
        let person = slot >= 15 && m.mem.r16(seg, rec.wrapping_add(0x40D)) & 0x1000 == 0;
        if person && (0xC0..0xC5).contains(&sprite) && !list.is_empty() {
            let view = (sprite - 0xC0 + 1).to_string();
            let file = |k: usize| list.get(k).and_then(|n| sheet.people.get(&n.replace('@', &view).to_ascii_uppercase())).copied();
            if frame < 6 {
                if let Some((f, _)) = file(0) {
                    billboard(&mut out, frame_of(f, frame), x, y, -z, sc, right, mirror);
                }
            } else if let Some((f, a)) = file(1) {
                billboard(&mut out, frame_of(f, frame - 6), x, y, 0.0, sc, right, mirror);
                if m.mem.r16(seg, rec.wrapping_add(0x40D)) & 0x400 != 0 {
                    if let Some((wf, w)) = file(2) {
                        let z = (w[2] - a[2]) + (w[0] - a[0]) + 0xE;
                        let shift = ((w[1] - a[1]) + 4) as f32;
                        let (z, shift) = (z as f32 * sc, shift * sc);
                        billboard_at(&mut out, frame_of(wf, frame - 6), x, y, -z, if mirror { -shift } else { shift }, sc, right, mirror);
                    }
                }
            }
            continue;
        }
        let art = if (0xC0..=0xC5).contains(&sprite) {
            let t = m.mem.r8(seg, rec + 4) as usize;
            sheet.creatures.get(t.wrapping_sub(1)).and_then(|v| v[(sprite - 0xC0) as usize])
        } else {
            flat(sprite as usize)
        };
        if let Some(f) = art {
            billboard(&mut out, frame_of(f, if frame == 0xFF { f.count - 1 } else { frame }), x, y, -z, sc, right, mirror);
        }
    }
    close(&mut open, out.len());
    // The projectiles (12 slots of 1Ch at 7DBC, +11h the frames to live,
    // 0 free): spells in flight and their bursts, arrows; drawn as the
    // actors are (0000:C0F1), their flat (+C, from the effects' flats
    // 0..1Bh the program's start loads) at frame +D.
    for i in 0..arena::projectile::var::NSLOTS {
        let si = arena::projectile::var::SLOTS + i * arena::projectile::var::SLOT_SIZE;
        if m.mem.r8(DS, si + 0x11) == 0 {
            continue;
        }
        let (x, z, y) = (m.mem.r16(DS, si) as f32, m.mem.r16(DS, si + 2) as i16 as f32, m.mem.r16(DS, si + 4) as f32);
        let sprite = m.mem.r8(DS, si + 0xC);
        let frame = m.mem.r8(DS, si + 0xD) as usize;
        let flags = m.mem.r16(DS, si + 0xF);
        // A burst inside the wall it struck: the original draws a square's
        // objects with the square, over its walls (the walk, back to
        // front, 0000:C980), so it shows; here it is brought towards the
        // player to just outside that square.
        let (x, y) = out_of_wall(m, x, y);
        if let Some(f) = flat(sprite as usize) {
            billboard(&mut out, frame_of(f, if frame == 0xFF { f.count - 1 } else { frame }), x, y, -z, size(sprite, flags), right, flags & 2 != 0);
        }
    }
    (out, tags)
}

/// (x, y) moved along the line to the player until it leaves a solid
/// square (a wall, not a square of flats), if it is in one, and 8 units
/// on, clear of the face.
fn out_of_wall(m: &Machine, x: f32, y: f32) -> (f32, f32) {
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let solid = |x: f32, y: f32| {
        let (c, r) = ((x as i32) >> 7, (y as i32) >> 7);
        if !(0..128).contains(&c) || !(0..128).contains(&r) {
            return false;
        }
        let v = m.mem.r16(walls, ((r as u16) << 8) | ((c as u16) << 1));
        v != 0 && v >> 12 != 8
    };
    if !solid(x, y) {
        return (x, y);
    }
    let (px, py) = (m.mem.r16(DS, 0xC3D0) as f32, m.mem.r16(DS, 0xC3D4) as f32);
    let (dx, dy) = (px - x, py - y);
    let len = (dx * dx + dy * dy).sqrt().max(1.0);
    let (ux, uy) = (dx / len, dy / len);
    let mut t = 0.0;
    while t < len && solid(x + ux * t, y + uy * t) {
        t += 2.0;
    }
    // Clear of the face, not on it.
    let t = (t + 8.0).min(len);
    (x + ux * t, y + uy * t)
}

/// The renderer's hit test (0000:C4BB..C533) for a click in the centre of
/// the view, while one is left for the actors (C39A bit 80h or 1000h):
/// the nearest billboard the click's line meets before a wall (only
/// across with C39A bit 1000h, as the original then tests only the x),
/// the eye at `eye` (x, y, height) facing `heading`; the line turns
/// `yaw` (radians, right positive) from it and rises `slope` a unit of
/// distance (both 0 through the centre of the view; `click_ray`). A map flat
/// counts only when its record (FLATS, 5 bytes) has +1 or bit 1 of +3, or
/// with C39A bit 400h. The original keeps the last sprite drawn over the
/// cursor, drawing far to near, which is the nearest.
pub fn hit(m: &Machine, sheet: &SpriteSheet, eye: (f32, f32, f32), heading: f32, tick: usize, (yaw, slope): (f32, f32)) -> Option<Target> {
    let level = m.mem.r16(DS, arena::movement::var::LEVEL_FLAGS);
    if level & 0x1080 == 0 {
        return None;
    }
    let (verts, tags) = sprites_tagged(m, sheet, heading, tick);
    let a = heading / 512.0 * std::f32::consts::TAU;
    let (fx, fz) = (a.sin(), a.cos());
    let (rx, rz) = (a.cos(), -a.sin());
    // The wall in the way: a square holding flats (in the walls, or on
    // the floor: a platform with one on it) is passed through.
    let floors = m.mem.r16(DS, arena::mif::var::FLOOR_SEG);
    let wall = pick_where(m, m.mem.r16(DS, 0xC394), |sq, v| v >> 12 != 8 && m.mem.r8(floors, sq) == 0).map_or(f32::MAX, |(_, t)| t);
    let mut best: Option<(f32, Target)> = None;
    for (target, range) in tags {
        if let Target::Flat { n, .. } = target {
            let rec = arena::info::var::FLATS + n * 5;
            if level & 0x400 == 0 && m.mem.r8(DS, rec + 1) == 0 && m.mem.r8(DS, rec + 3) & 1 == 0 {
                continue;
            }
        }
        for q in verts[range].chunks(6) {
            // Corners: bottom left, bottom right, top right.
            let (p0, p1, p2) = (q[0].pos, q[1].pos, q[2].pos);
            let t = (p0[0] - eye.0) * fx + (p0[2] - eye.1) * fz;
            if t <= 0.0 || t >= wall || best.is_some_and(|(b, _)| t >= b) {
                continue;
            }
            // Along the click's line, turned `yaw`: depth t is t / cos(yaw)
            // along it, and the line is tan(yaw) x t to the right there.
            let side = yaw.tan() * t;
            let (px, pz) = (eye.0 + fx * t + rx * side, eye.1 + fz * t + rz * side);
            let width = (p1[0] - p0[0]) * rx + (p1[2] - p0[2]) * rz;
            let u = (px - p0[0]) * rx + (pz - p0[2]) * rz;
            if !(0.0..=width).contains(&u) {
                continue;
            }
            if level & 0x1000 == 0 && !(p0[1]..=p2[1]).contains(&(eye.2 + slope * t)) {
                continue;
            }
            best = Some((t, target));
        }
    }
    best.map(|(_, t)| t)
}

/// The creatures' flats the drawing reaches (0000:CF96: a map flat with
/// bit 2 of its high byte goes to the hook CD56, 25CA:0271, instead of
/// being drawn), as (column, row, the word's offset in MAP1_SEG).
///
/// Approximate: the original's ray caster reaches the squares its rays
/// pass; here a square is reached when its centre is inside the view's
/// columns (45 degrees either side, 160 columns a unit of tan) and the
/// line to it crosses no wall, nearest first.
pub fn creature_flats_seen(m: &Machine) -> Vec<(u16, u16, u16)> {
    let w = m.mem.r16(DS, 0xC357).min(128) as i32;
    let d = m.mem.r16(DS, 0xC359).min(128) as i32;
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    use arena::movement::var::{HEADING, X, Y};
    let (x, y) = (m.mem.r16(DS, X) as f32, m.mem.r16(DS, Y) as f32);
    let a = m.mem.r16(DS, HEADING) as f32 / 512.0 * std::f32::consts::TAU;
    let (fx, fy) = (a.sin(), a.cos());
    let start = (x as i32 >> 7, y as i32 >> 7);
    let mut seen = Vec::new();
    for row in 0..d {
        for col in 0..w {
            let di = (row << 8 | col << 1) as u16;
            let v = m.mem.r16(walls, di);
            if v >> 12 != 8 || v & 0x200 == 0 {
                continue;
            }
            let (cx, cy) = (col as f32 * SQUARE + SQUARE / 2.0, row as f32 * SQUARE + SQUARE / 2.0);
            let (dx, dy) = (cx - x, cy - y);
            let ahead = dx * fx + dy * fy;
            let side = dx * fy - dy * fx;
            if (col, row) != start && (ahead <= 0.0 || side.abs() > ahead) {
                continue;
            }
            let dist = (dx * dx + dy * dy).sqrt();
            let mut t = 4.0;
            let mut open = true;
            while t < dist {
                let (c, r) = ((x + dx / dist * t) as i32 >> 7, (y + dy / dist * t) as i32 >> 7);
                t += 4.0;
                if (c, r) == start || (c, r) == (col, row) || c < 0 || r < 0 || c >= w || r >= d {
                    continue;
                }
                let u = m.mem.r16(walls, (r << 8 | c << 1) as u16);
                if u != 0 && u >> 12 != 8 {
                    open = false;
                    break;
                }
            }
            if open {
                seen.push((dist, (col as u16, row as u16, di)));
            }
        }
    }
    seen.sort_by(|a, b| a.0.total_cmp(&b.0));
    seen.into_iter().map(|(_, s)| s).collect()
}

/// The view's half height (100 of the 320 x 200 screen's rows) over its
/// focal length: the original projects a point at depth d and height h
/// to h * 160 / d rows and columns alike (0000:E032: CCF4 and CCF6 hold
/// A0h).
pub const TAN_HALF: f32 = 100.0 / 160.0;

/// The line through the click (CB60, CB62: the cursor + 4, 2826:0A52)
/// as (yaw, slope) for `hit`: the view is drawn at 160 rows a unit of
/// slope (`TAN_HALF`, see `render`), its columns at that scale over a
/// target `aspect` wide; the centre column C3D6, the eye line row C3D8.
pub fn click_ray(m: &Machine, aspect: f32) -> (f32, f32) {
    let r = |at: u16| m.mem.r16(DS, at) as i16 as f32;
    let rows = 100.0 / TAN_HALF;
    let aspect = if aspect > 0.0 { aspect } else { 1.6 };
    let cols = rows * (320.0 / 200.0) / aspect;
    let x = r(arena::click::var::ACTOR_CLICK) - r(0xC3D6);
    let y = r(arena::click::var::ACTOR_CLICK + 2) - r(0xC3D8);
    ((x / cols).atan(), -y / rows)
}

/// A map flat clicked: the scratch record the original draws it through
/// (FLAT_DRAWN, 0000:D18E) filled as it fills it: the square's centre,
/// z the flat's lift (+ the level's ceiling D80C negated with bit 20h of
/// its +3; the floor's height the drawing adds, BP, is taken as 0), the
/// flat's number, no flags, and an animated flat's frame (its frame count
/// +0Eh, from CA97, the frame the frame counter CCBC gives) or FFh.
pub fn flat_drawn(m: &mut Machine, col: u16, row: u16, n: u16) {
    use arena::talk::var::FLAT_DRAWN as D;
    let rec = arena::info::var::FLATS + n * 5;
    let mut z = m.mem.r8(DS, rec) as i8 as i16 as u16;
    if m.mem.r8(DS, rec + 3) & 0x20 != 0 {
        z = z.wrapping_sub(m.mem.r16(DS, 0xD80C));
    }
    m.mem.w16(DS, D + 0xF, 0);
    m.mem.w16(DS, D + 0x16, 0);
    m.mem.w16(DS, D + 2, z);
    m.mem.w8(DS, D + 0xD, 0xFF);
    m.mem.w8(DS, D + 0xC, n as u8);
    let frames = m.mem.r8(DS, 0xCA97 + n);
    if frames != 0 {
        m.mem.w8(DS, D + 0xE, frames);
        let k = (frames & 0x7F).max(1);
        let f = (m.mem.r16(DS, 0xCCBC) as u8) % k;
        m.mem.w8(DS, D + 0xD, f);
    }
    m.mem.w16(DS, D, col << 7 | 0x40);
    m.mem.w16(DS, D + 4, row << 7 | 0x40);
}

/// A sky object drawn as the original would draw `rect` with its top
/// left at screen (`x`, `y`) of its 320 x 200 view, as a quad in clip
/// space: columns from the centre (C3D6) at 160 a unit of tan (the
/// original's scale, CCF4 A0h), turned to this view's by `focal` (the
/// tangent of half the field across); rows from the horizon (C3D8, the
/// row above it the last), 100 to the half view; rows past `bottom` cut.
#[allow(clippy::too_many_arguments)]
pub fn sky_quad(out: &mut Vec<Vertex>, rect: Rect, x: i16, y: i16, centre: i16, horizon: i16, bottom: i16, focal: f32) {
    sky_quad_flags(out, rect, x, y, centre, horizon, bottom, focal, 0);
}

/// `sky_quad` with more vertex flags (BODY, DARK_SKY).
#[allow(clippy::too_many_arguments)]
pub fn sky_quad_flags(out: &mut Vec<Vertex>, rect: Rect, x: i16, y: i16, centre: i16, horizon: i16, bottom: i16, focal: f32, flags: u32) {
    let rows = (bottom as i32 + 1 - y as i32).clamp(0, rect.h as i32);
    if rows == 0 {
        return;
    }
    let cx = |c: f32| (c - centre as f32) / 160.0 / focal;
    let cy = |r: f32| (horizon as f32 + 1.0 - r) / 100.0;
    let (x0, x1) = (cx(x as f32), cx(x as f32 + rect.w as f32));
    let (y0, y1) = (cy(y as f32), cy(y as f32 + rows as f32));
    let p = [[x0, y1, 0.0], [x1, y1, 0.0], [x1, y0, 0.0], [x0, y0, 0.0]];
    const IN: f32 = 0.01;
    let (u0, u1) = (rect.x as f32 + IN, (rect.x + rect.w) as f32 - IN);
    let (v0, v1) = (rect.y as f32 + IN, (rect.y + rows as u32) as f32 - IN);
    let uv = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(Vertex { pos: p[i], uv: uv[i], layer: 0, light: 1.0, flags: SPRITE | SCREEN | flags });
    }
}
