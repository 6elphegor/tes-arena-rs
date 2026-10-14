//! Following the game from area to area. When a frame leaves the area
//! (`Frame::Left`: a building's door clicked, an interior's way out), the
//! area's run ends and the outer loop picks the next (src/transit.rs);
//! the view loads its level as the original's run does, and then makes
//! its assets again.
//!
//! Cities and interiors are followed. The wilderness's set-up (217B:0084)
//! and the dungeons' making (2294:0ED4, 0000:28BC) aren't wired here.

use crate::world;
use arena::bsa::Files;
use arena::engine::Engine;
use arena::machine::Machine;
use arena::mem::DS;
use arena::transit::{AreaHooks, Run};
use std::path::Path;

/// The view's side of the area changes.
pub struct Areas<'a> {
    pub dir: &'a Path,
    /// The game's files, while the outer loop runs (the world's entry
    /// uses the engine's).
    pub files: Option<&'a mut Files>,
    /// Something the original does here failed in the port.
    pub failed: bool,
}

/// The location record the view's cities use (as `world::city_load`).
const RECORD: u16 = 0x0A22;

impl AreaHooks for Areas<'_> {
    fn normal_lgt(&mut self, m: &mut Machine) {
        world::lgt(m, self.dir, arena::weather::NORMAL_LGT);
    }

    /// 1194:3AE8 as the view's cities need it: the Imperial City's
    /// record (province 8) stays the Imperial City, and the terrain
    /// (record +2Eh, A50), which the original reads from the map file
    /// (1194:3D91), stays the one the view was started with.
    fn location(&mut self, m: &mut Machine, id: u16) {
        let id = if m.mem.r8(DS, RECORD + 0x24) == 8 { 0x100 } else { id };
        let terrain = m.mem.r8(DS, arena::weather::var::TERRAIN);
        if m.city_build_location(DS, RECORD, id, terrain).is_none() {
            self.failed = true;
        }
        m.mem.w8(DS, arena::weather::var::TERRAIN, terrain);
    }

    fn weather(&mut self, m: &mut Machine) {
        m.weather_set(&mut world::Lgt(self.dir));
    }

    /// The city built; its build leaves the buffer pointer (BA54) at the
    /// frame, which entering the area gives a buffer again (0000:A4C5).
    fn build_city(&mut self, m: &mut Machine) {
        let (off, seg) = (m.mem.r16(DS, 0xBA54), m.mem.r16(DS, 0xBA56));
        let Some(files) = self.files.as_deref_mut() else { return };
        if m.mif_build_city(files).is_none() {
            self.failed = true;
        }
        m.mem.w16(DS, 0xBA54, off);
        m.mem.w16(DS, 0xBA56, seg);
    }

    /// 0000:468B's part the view needs: indoors, the level's lights
    /// from its flats (0000:478F; outdoors the night makes them).
    fn entered(&mut self, m: &mut Machine) {
        if m.mem.r16(DS, 0xC398) & 4 == 0 {
            m.flat_lights();
            m.mem.w16(DS, 0x7A46, 0);
        }
    }

    fn spawn(&mut self, m: &mut Machine, n: u16) {
        if m.spawn_actors(n, 0xA).is_none() {
            eprintln!("the townspeople met code not ported");
        }
    }
}

/// The area the view starts in, as the outer loop would have entered it:
/// a city (5E6 0) or an interior (4).
pub fn start(engine: &mut Engine, city: bool) -> Run {
    let area = if city { 0 } else { 4 };
    engine.m.mem.w8(DS, arena::trigger::var::AREA, area);
    engine.m.mem.w8(DS, arena::transit::var::ENTERED, area);
    if city { Run::City } else { Run::Interior(0xC37F) }
}

/// The area `run` was left: its run ends, the outer loop picks the next
/// and it is entered. Returns the new run, or None where the game ends
/// there or the port can't follow (reported).
pub fn next(engine: &mut Engine, dir: &Path, run: Run) -> Option<Run> {
    // The INF loader keeps GLOBAL.BSA's directory cached in EMS bank
    // 1Fh in the 3D mode (C39A bit 400h, 0000:EF6E), but every plane a
    // level load reads goes through that bank too (0000:EB0B), so the
    // next area's INF would be looked up in plane bytes. Where the
    // original drops the cache between areas isn't found yet; the view
    // drops it here (0000:6ABA).
    engine.m.mem.w16(DS, arena::bsa::var::DIR_CACHE, 0xFFFF);
    let mut files = std::mem::take(&mut engine.files);
    let r = {
        let h = &mut Areas { dir, files: Some(&mut files), failed: false };
        let m = &mut engine.m;
        m.world_leave(h);
        m.world_end(h);
        m.area_end(run, h);
        let next = m.area_next(h);
        match next {
            Run::City | Run::Interior(_) | Run::Start => {
                let name = m.area_begin(next, h);
                m.world_begin(name, h);
                // The view has no sound: the INF's sounds (the hook CD4A,
                // 23BA:0217, which the 3D mode set) aren't loaded.
                m.mem.w32(DS, 0xCD4A, 0);
                (!h.failed).then_some(next)
            }
            other => {
                eprintln!("the next area isn't followed yet: {other:?}");
                None
            }
        }
    };
    engine.files = files;
    let next = r?;
    let h = &mut Areas { dir, files: None, failed: false };
    if engine.world_enter(h).is_none() || h.failed {
        eprintln!("entering the area met code not ported");
        return None;
    }
    let name = String::from_utf8_lossy(engine.m.mem.cstr(DS, arena::transit::var::LEVEL_NAME)).into_owned();
    eprintln!("entered {next:?} ({name})");
    Some(next)
}

