//! The view's side of the game's light (`LightHooks`): the textures'
//! row, the lamps' colour and the fog's for the window (`LightState`),
//! 0000:0390's loads made inside it (`LightNow`), the sky's images loaded
//! as the game loads them (`GameSkyLoad`), and the light an area's entry
//! makes, handed to the assets made next (`ENTRY_LIGHT`).

use crate::world;
use std::collections::HashMap;
use std::path::PathBuf;

/// The view's side of the day and night switch and the fog
/// (`LightHooks`): the row the textures are remapped through (0 as
/// loaded), palette entry 71h (the lamps' glow; PAL.COL's until set), the
/// light table's segment, and this frame's fog colour (filling the view
/// for the sky).
pub struct LightState {
    pub dir: PathBuf,
    pub dark: u32,
    pub lamp: Option<[u8; 3]>,
    pub table: u16,
    pub fog: Option<u8>,
    /// The dither and the day's colours the sky's steps asked for
    /// (2294:0341, 0428), loaded into the game's memory after the frame
    /// (`Assets::view_frame`), where the files are.
    pub dither: Option<u16>,
    pub colours: Option<u16>,
    /// The relight's level (0000:027A, 02F2), its remapping of the sky's
    /// images and the water's frames in the game's memory done after the
    /// frame with the files (`Assets::sky_loads`).
    pub relight: Option<u16>,
    /// 0000:DFDB with BA17 asked (the day's textures as loaded, put back:
    /// `water_frames`), done with the files after the frame.
    pub restore: bool,
    /// 2294:08B9 asked (the sky's objects made again), done with the
    /// files after the frame.
    pub objects: bool,
}

impl LightState {
    pub fn new(dir: &std::path::Path, m: &arena::machine::Machine) -> Self {
        LightState { dir: dir.to_path_buf(), dark: 0, lamp: None, table: m.mem.r16(arena::mem::DS, 0xCCC2), fog: None, dither: None, colours: None, relight: None, restore: false, objects: false }
    }
}

thread_local! {
    /// The sky's images in EMS, named by their offset in the frame (for
    /// the GPU world's atlas), as the game loads them (`GameSkyLoad`):
    /// kept across the assets made again, emptied as the sky's objects
    /// are made again.
    pub static SKY_NAMES: std::cell::RefCell<HashMap<u16, String>> = std::cell::RefCell::new(HashMap::new());
    /// The light the area's entry made (`Areas::entered`, 0000:46AF's
    /// 0390 with its loads), for the assets made next (`Assets::make`),
    /// which then make none of the entry's loads themselves.
    pub static ENTRY_LIGHT: std::cell::RefCell<Option<LightState>> = const { std::cell::RefCell::new(None) };
}

/// The sky's images loaded as the game loads them (0000:7428 with DI
/// FFFDh, `Machine::file_load_ems`), their names kept (`SKY_NAMES`).
pub struct GameSkyLoad<'a> {
    pub files: &'a mut arena::bsa::Files,
}

impl arena::skyobj::SkyHooks for GameSkyLoad<'_> {
    fn load(&mut self, m: &mut arena::machine::Machine, name: u16) -> Option<(u16, u16)> {
        let file = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, name)).to_ascii_uppercase();
        let at = m.file_load_ems(self.files, name);
        if let Some((_, off)) = at {
            SKY_NAMES.with(|n| n.borrow_mut().insert(off, file));
        }
        at
    }
}

/// 2294:08B9 with the game's loads (`GameSkyLoad`): the sky's objects
/// made again, their images' names forgotten first.
pub fn sky_objects_made(m: &mut arena::machine::Machine, files: &mut arena::bsa::Files) {
    SKY_NAMES.with(|n| n.borrow_mut().clear());
    m.sky_objects(&mut GameSkyLoad { files });
}

impl LightState {
    /// 0000:02F2's relight to `level` asked (the shader's `dark`).
    pub fn relight(&mut self, level: u16) {
        self.dark = (level >> 8) as u32;
        self.relight = Some(level);
    }

    /// The table freed and made again from the .LGT named at `name`.
    pub fn table(&mut self, m: &mut arena::machine::Machine, name: u16) {
        world::lgt(m, &self.dir, name);
        if self.table != 0 {
            m.heap_free(self.table);
        }
        self.table = world::light_table(m);
    }
}

impl arena::light::LightHooks for LightState {
    /// 2294:0341's dither load, for after the frame.
    fn sky(&mut self, _m: &mut arena::machine::Machine, step: arena::sky::SkyStep) {
        if step.dither.is_some() {
            self.dither = step.dither;
        }
    }

    fn light_table(&mut self, m: &mut arena::machine::Machine) {
        self.table(m, arena::weather::NORMAL_LGT);
    }

    fn fog_light_table(&mut self, m: &mut arena::machine::Machine) {
        self.table(m, arena::weather::FOG_LGT);
    }

    fn lgt(&mut self, m: &mut arena::machine::Machine, name: u16) {
        world::lgt(m, &self.dir, name);
    }

    fn fog_fill(&mut self, m: &mut arena::machine::Machine) {
        self.fog = Some(m.mem.r8(arena::mem::DS, arena::weather::var::FILL_COLOUR));
    }

    /// 0000:027A: the sky's objects made again, when there are any (their
    /// images are then remapped with the textures: the shader's `dark`).
    fn relight(&mut self, m: &mut arena::machine::Machine, level: u16) {
        use arena::skyobj::var::{CLOUD_COUNT, OBJECT_COUNT};
        if m.mem.r8(arena::mem::DS, OBJECT_COUNT) | m.mem.r8(arena::mem::DS, CLOUD_COUNT) != 0 {
            m.mem.w8(arena::mem::DS, OBJECT_COUNT, 0);
            m.mem.w8(arena::mem::DS, CLOUD_COUNT, 0);
            arena::light::LightHooks::sky_objects(self, m);
        }
        self.relight(level);
    }

    fn restore_textures(&mut self, _m: &mut arena::machine::Machine) {
        self.dark = 0;
        self.restore = true;
    }

    /// 2294:08B9, the sky's objects made again (by day, and before
    /// relighting by night): asked, for when there are files.
    fn sky_objects(&mut self, _m: &mut arena::machine::Machine) {
        self.objects = true;
    }

    /// Only entry 71h is written through here; its 6-bit colour made 8-bit.
    fn palette(&mut self, m: &mut arena::machine::Machine, index: u8, at: u16) {
        if index == 0x71 {
            let c = |i: u16| m.mem.r8(arena::mem::DS, at + i) & 0x3F;
            self.lamp = Some([0, 1, 2].map(|i| c(i) << 2 | c(i) >> 4));
        }
    }
}

/// `LightState` with the files: 0000:0390's loads made as it asks for
/// them, inside it as the original makes them (2294:0341's dither,
/// 027A's sky's images made again then 02F2's relighting of them and of
/// the water's frames, DFDB's textures put back).
pub struct LightNow<'a> {
    pub light: &'a mut LightState,
    pub files: &'a mut arena::bsa::Files,
}

impl arena::light::LightHooks for LightNow<'_> {
    fn sky(&mut self, m: &mut arena::machine::Machine, step: arena::sky::SkyStep) {
        self.light.sky(m, step);
        if let Some(name) = self.light.dither.take() {
            if m.sky_dither_load(self.files, name).is_none() {
                eprintln!("the sky's dither did not load");
            }
        }
    }

    fn light_table(&mut self, m: &mut arena::machine::Machine) {
        self.light.light_table(m);
    }

    fn fog_light_table(&mut self, m: &mut arena::machine::Machine) {
        self.light.fog_light_table(m);
    }

    fn lgt(&mut self, m: &mut arena::machine::Machine, name: u16) {
        self.light.lgt(m, name);
    }

    fn fog_fill(&mut self, m: &mut arena::machine::Machine) {
        self.light.fog_fill(m);
    }

    /// 0000:027A (the sky's objects made again, when there are any), then
    /// 02F2: the sky's images and the water's frames relit.
    fn relight(&mut self, m: &mut arena::machine::Machine, level: u16) {
        use arena::skyobj::var::{CLOUD_COUNT, OBJECT_COUNT};
        if m.mem.r8(arena::mem::DS, OBJECT_COUNT) | m.mem.r8(arena::mem::DS, CLOUD_COUNT) != 0 {
            m.mem.w8(arena::mem::DS, OBJECT_COUNT, 0);
            m.mem.w8(arena::mem::DS, CLOUD_COUNT, 0);
            arena::light::LightHooks::sky_objects(self, m);
        }
        self.light.relight(level);
        self.light.relight = None;
        m.sky_images_relight(level);
        if m.water_relight(self.files, level).is_none() {
            eprintln!("the water's frames did not load");
        }
    }

    fn restore_textures(&mut self, m: &mut arena::machine::Machine) {
        self.light.restore_textures(m);
        if std::mem::take(&mut self.light.restore) && m.water_frames(self.files, 0xBA17).is_none() {
            eprintln!("the water's frames did not load");
        }
    }

    fn sky_objects(&mut self, m: &mut arena::machine::Machine) {
        sky_objects_made(m, self.files);
    }

    fn palette(&mut self, m: &mut arena::machine::Machine, index: u8, at: u16) {
        self.light.palette(m, index, at);
    }

    fn weather_palette(&mut self, m: &mut arena::machine::Machine) {
        self.light.weather_palette(m);
    }
}
