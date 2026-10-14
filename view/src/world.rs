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

/// Map squares are 128 units across.
pub const SQUARE: f32 = 128.0;

/// Loads `level` (a .MIF) from the game in `dir` into an engine and
/// enters it, as examples/play.rs does.
pub fn load(dir: &Path, level: &str) -> Engine {
    let image = exe::unpack(&std::fs::read(dir.join("ACD.EXE")).expect("ACD.EXE")).expect("unpack");
    let mut m = Machine::new(&image);
    let mut files = Files::new(dir).expect("game directory");
    assert!(m.bsa_open(&mut files), "GLOBAL.BSA");
    m.heap_init(0xA000, 0xF000).expect("heap");
    let (buf, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x9030, buf);
    // EMS on: 32 banks swapped through the frame.
    m.mem.w8(DS, 0x8FA6, 1);
    m.mem.w16(DS, 0x9032, 0x20);
    let (seg, off) = m.heap_alloc_bytes(0x3E80).expect("heap");
    m.mem.w16(DS, 0xBA54, off);
    m.mem.w16(DS, 0xBA56, seg);
    m.mem.w16(DS, 0xCB8A, buf);
    m.mem.w16(DS, 0xCB88, 0x8000);
    for (i, b) in format!("{level}\0").bytes().enumerate() {
        m.mem.w8(DS, 0xF000 + i as u16, b);
        m.mem.w8(DS, 0xC37F + i as u16, b);
    }
    // The spells (records the casting reads): a new game's, slot 65.
    m.save_set_slot(65);
    m.spells_load(&mut files).expect("spells");
    let (sseg, _) = m.spell_record(0);
    if m.mem.r16(sseg, 0x18) == 0 {
        eprintln!("the spells did not load (SPELLSG.65)");
    }
    m.level_load(&mut files, 0xF000).expect("level");
    let info = m.mem.r16(DS, 0xCB84);
    // The world loop's hooks are set: the port runs its own input step
    // and transitions in their place.
    m.mem.w32(DS, 0xC3B4, 0x2826_0002);
    m.mem.w32(DS, 0xC3AC, 0x0000_4470);
    let (cseg, _) = m.heap_alloc(0x1000).expect("heap");
    m.mem.w16(DS, 0x1042, cseg);
    // A player to walk around with.
    m.mem.w16(DS, 0x01FD, 50);
    m.mem.w16(DS, 0x01FF, 50);
    m.mem.w16(DS, 0x0201, 200);
    for i in 0..8 {
        m.mem.w8(DS, 0x01A4 + arena::character::field::ATTRS + i, 50);
        m.mem.w8(DS, 0x01A4 + arena::character::field::BASE_ATTRS + i, 50);
    }
    m.rng.seed = 1994;
    m.store_rng();
    let mut engine = Engine::new(m, files);
    engine.enter_area(info).expect("entering the area");
    engine
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
            eprintln!("the encounter met code not ported");
            return;
        }
    }
}

/// The 256 colours (RGB, 8 bits) of PAL.COL, past its 8-byte header.
pub fn palette(dir: &Path) -> [[u8; 4]; 256] {
    let d = std::fs::read(dir.join("PAL.COL")).expect("PAL.COL");
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
    let mut quad = |p: [[f32; 3]; 4], uv: [[f32; 2]; 4], layer: u32, light: f32| {
        for i in [0, 1, 2, 0, 2, 3] {
            out.push(Vertex { pos: p[i], uv: uv[i], layer, light, flags: 0 });
        }
    };
    for row in 0..d as i32 {
        for col in 0..w as i32 {
            let x0 = col as f32 * SQUARE;
            let z0 = row as f32 * SQUARE;
            let (x1, z1) = (x0 + SQUARE, z0 + SQUARE);
            let v = wall(col, row);
            // The sides of a box from `lo` to `hi` facing open squares.
            let mut sides = |lo: f32, hi: f32, layer: u32, cull: &dyn Fn(u16) -> bool| {
                let th = 64.0 * (hi - lo) / ceiling.max(1.0);
                let uv = [[0.0, th], [64.0, th], [64.0, 0.0], [0.0, 0.0]];
                if !cull(wall(col, row - 1)) {
                    quad([[x1, lo, z0], [x0, lo, z0], [x0, hi, z0], [x1, hi, z0]], uv, layer, 0.85);
                }
                if !cull(wall(col, row + 1)) {
                    quad([[x0, lo, z1], [x1, lo, z1], [x1, hi, z1], [x0, hi, z1]], uv, layer, 0.85);
                }
                if !cull(wall(col - 1, row)) {
                    quad([[x0, lo, z0], [x0, lo, z1], [x0, hi, z1], [x0, hi, z0]], uv, layer, 1.0);
                }
                if !cull(wall(col + 1, row)) {
                    quad([[x1, lo, z1], [x1, lo, z0], [x1, hi, z0], [x1, hi, z1]], uv, layer, 1.0);
                }
            };
            let mut has_floor = true;
            if v != 0 && v & 0x8000 == 0 {
                if (v >> 8) as u8 == v as u8 {
                    // A solid wall: texture by its low 6 bits.
                    let t = (v & 0x3F) as u32;
                    sides(0.0, ceiling, t.saturating_sub(1), &solid);
                    has_floor = false;
                } else {
                    // A raised platform: the box tables give the side and
                    // cap textures; its heights come from the high byte.
                    let side = m.mem.r8(DS, arena::info::var::BOX_SIDES + (v & 0xF)) as u32;
                    let cap = m.mem.r8(DS, arena::info::var::BOX_CAPS + ((v >> 4) & 0xF)) as u32;
                    let (top, bottom) = m.move_wall_height(v);
                    let (lo, hi) = (bottom as i16 as f32, top as i16 as f32);
                    sides(lo, hi, side.saturating_sub(1), &solid);
                    let uv = [[0.0, 0.0], [64.0, 0.0], [64.0, 64.0], [0.0, 64.0]];
                    quad([[x0, hi, z0], [x1, hi, z0], [x1, hi, z1], [x0, hi, z1]], uv, cap.saturating_sub(1), 1.0);
                    quad([[x0, lo, z1], [x1, lo, z1], [x1, lo, z0], [x0, lo, z0]], uv, cap.saturating_sub(1), 0.7);
                }
            } else if v != 0 && matches!(v >> 12, 9 | 0xA | 0xD) {
                // The other wall kinds, drawn as walls for now. Doors
                // (0Bh) move: `doors`.
                let t = (v & 0x3F) as u32;
                sides(0.0, ceiling, t.saturating_sub(1), &|_| false);
            }
            if has_floor {
                let f = m.mem.r16(floors, at(col as u16, row as u16));
                let kind = f.wrapping_sub(frame) >> 8;
                let uv = [[0.0, 0.0], [64.0, 0.0], [64.0, 64.0], [0.0, 64.0]];
                if kind < 0xC {
                    quad([[x0, 0.0, z1], [x1, 0.0, z1], [x1, 0.0, z0], [x0, 0.0, z0]], uv, kind as u32, 0.9);
                }
                if let Some(t) = ceiling_tex {
                    quad([[x0, ceiling, z0], [x1, ceiling, z0], [x1, ceiling, z1], [x0, ceiling, z1]], uv, t, 0.8);
                }
            }
        }
    }
    out
}

/// The doors (wall kind 0Bh) as they stand this frame: shut, a panel on
/// each side not against a full wall; opening (high byte bits 1-3, 0..7, as `Machine::
/// door_drawn` steps them), each panel swung into the square about its
/// left edge (as seen from outside) by 90 degrees fully open. How the
/// original's renderer drew a door ajar (by CCC4) isn't ported; this is
/// a stand-in.
pub fn doors(m: &Machine) -> Vec<Vertex> {
    let w = m.mem.r16(DS, 0xC357).min(128);
    let d = m.mem.r16(DS, 0xC359).min(128);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let ceiling = m.mem.r16(DS, arena::movement::var::CEILING) as f32;
    let mut out = Vec::new();
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, (row << 8) | (col << 1));
            if v >> 12 != 0xB {
                continue;
            }
            let layer = ((v & 0x3F) as u32).saturating_sub(1);
            let angle = ((v >> 9) & 7) as f32 / 7.0 * std::f32::consts::FRAC_PI_2;
            let (x0, z0) = (col as f32 * SQUARE, row as f32 * SQUARE);
            let (x1, z1) = (x0 + SQUARE, z0 + SQUARE);
            // Each side: the neighbour it faces, its hinge, its far edge,
            // the way into the square, and its light.
            let sides = [
                ((0, -1), (x1, z0), (x0, z0), (0.0, 1.0), 0.85),
                ((0, 1), (x0, z1), (x1, z1), (0.0, -1.0), 0.85),
                ((-1, 0), (x0, z0), (x0, z1), (1.0, 0.0), 1.0),
                ((1, 0), (x1, z1), (x1, z0), (-1.0, 0.0), 1.0),
            ];
            for ((dc, dr), (hx, hz), (ex, ez), (nx, nz), light) in sides {
                // None against a full wall.
                let (c, r) = (col as i32 + dc, row as i32 + dr);
                let n = if c < 0 || r < 0 || c >= w as i32 || r >= d as i32 { 0xFFFF } else { m.mem.r16(walls, (r << 8 | c << 1) as u16) };
                if n == 0xFFFF || (n != 0 && n & 0x8000 == 0 && n as u8 == (n >> 8) as u8) {
                    continue;
                }
                let (c, s) = (angle.cos(), angle.sin());
                let (fx, fz) = (hx + (ex - hx) * c + nx * SQUARE * s, hz + (ez - hz) * c + nz * SQUARE * s);
                let p = [[hx, 0.0, hz], [fx, 0.0, fz], [fx, ceiling, fz], [hx, ceiling, hz]];
                let th = 64.0;
                let uv = [[0.0, th], [64.0, th], [64.0, 0.0], [0.0, 0.0]];
                for i in [0, 1, 2, 0, 2, 3] {
                    out.push(Vertex { pos: p[i], uv: uv[i], layer, light, flags: 0 });
                }
            }
        }
    }
    out
}

/// The map square under the centre of the view, as the original's
/// renderer leaves the square under the cursor (CB92) for a click: the
/// first square that isn't empty along the player's heading, within
/// `reach` squares; None past it or the map's edge.
pub fn pick(m: &Machine, reach: u16) -> Option<u16> {
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
        if m.mem.r16(walls, sq) != 0 {
            return Some(sq);
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
}

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
    sheet: SpriteSheet,
}

impl Packer {
    fn add(&mut self, f: &crate::flats::Flat) -> Frames {
        let first = self.sheet.rects.len();
        let (w, h) = (f.w as u32, f.h as u32);
        for frame in &f.frames {
            if self.x + w > ATLAS {
                self.y += self.shelf;
                self.x = 0;
                self.shelf = 0;
            }
            let r = Rect { x: self.x, y: self.y, w, h };
            let need = ((r.y + h) * ATLAS) as usize;
            if self.sheet.pixels.len() < need {
                self.sheet.pixels.resize(need, 0);
            }
            for row in 0..f.h {
                let at = ((r.y as usize + row) * ATLAS as usize) + r.x as usize;
                self.sheet.pixels[at..at + f.w].copy_from_slice(&frame[row * f.w..(row + 1) * f.w]);
            }
            self.sheet.rects.push(r);
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

pub fn sprite_sheet(flats: &[Option<crate::flats::Flat>], archive: &crate::archive::Archive, names: &[String]) -> SpriteSheet {
    let sheet = SpriteSheet { h: 0, pixels: vec![], rects: vec![], flats: vec![], creatures: vec![], people: Default::default() };
    let mut p = Packer { x: 0, y: 0, shelf: 0, sheet };
    for f in flats {
        let frames = f.as_ref().map(|f| p.add(f));
        p.sheet.flats.push(frames);
    }
    for name in names {
        let mut views = [None; 6];
        for (i, v) in views.iter_mut().enumerate() {
            let file = name.replace('@', &(i + 1).to_string());
            if let Some(f) = archive.read(&file).as_deref().and_then(crate::flats::cfa) {
                *v = Some(p.add(&f));
            }
        }
        p.sheet.creatures.push(views);
    }
    let mut people: Vec<&str> = archive.names().filter(|n| is_person_file(n)).collect();
    people.sort_unstable();
    for name in people {
        let Some(d) = archive.read(name) else { continue };
        if let Some(f) = crate::flats::cfa(&d) {
            let at = |o: usize| i16::from_le_bytes([d[o], d[o + 1]]);
            let frames = p.add(&f);
            p.sheet.people.insert(name.to_string(), (frames, [at(2), at(6), at(8)]));
        }
    }
    let mut sheet = p.sheet;
    sheet.h = ((sheet.pixels.len() as u32).div_ceil(ATLAS)).max(1);
    sheet.pixels.resize((sheet.h * ATLAS) as usize, 0);
    sheet
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

/// A billboard: the frame `rect` standing at (x, y) from `lo` up, a unit
/// per pixel, facing a camera whose right is (rx, rz).
fn billboard(out: &mut Vec<Vertex>, rect: Rect, x: f32, y: f32, lo: f32, (rx, rz): (f32, f32), mirror: bool) {
    billboard_at(out, rect, x, y, lo, 0.0, (rx, rz), mirror)
}

/// `billboard`, its centre moved `shift` along the camera's right.
#[allow(clippy::too_many_arguments)]
fn billboard_at(out: &mut Vec<Vertex>, rect: Rect, x: f32, y: f32, lo: f32, shift: f32, (rx, rz): (f32, f32), mirror: bool) {
    let (x, y) = (x + rx * shift, y + rz * shift);
    let hw = rect.w as f32 / 2.0;
    let hi = lo + rect.h as f32;
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
///   FLAT_BASE) at their squares' centres, raised by the flat record's
///   Y, animations at frame `tick`;
/// - the creatures in the actor slots (not inactive, 4000h): sprite C0h
///   + view (0..4, mirrored with flag 2) is the type's view file, C5h
///   (dying) its sixth, any other a flat (the corpse); +D the frame.
pub fn sprites(m: &Machine, sheet: &SpriteSheet, heading: f32, tick: usize) -> Vec<Vertex> {
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
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, (row << 8) | (col << 1));
            if v >> 12 != 8 {
                continue;
            }
            let n = ((v & 0xFF) + base) as usize;
            if let Some(f) = flat(n) {
                let (x, y) = (col as f32 * SQUARE + SQUARE / 2.0, row as f32 * SQUARE + SQUARE / 2.0);
                billboard(&mut out, frame_of(f, tick), x, y, lift(n), right, false);
            }
        }
    }
    let seg = m.mem.r16(DS, 0x1042);
    let list = people_list(m);
    for slot in 0..23u16 {
        let si = 0x0AB3 + slot * 0x1C;
        let flags = m.mem.r16(DS, si + 0xF);
        let rec = m.mem.r16(DS, si + 0x12);
        // (The first record is at offset 0.)
        if flags & 0x4000 != 0 {
            continue;
        }
        let (x, z, y) = (m.mem.r16(DS, si) as f32, m.mem.r16(DS, si + 2) as i16 as f32, m.mem.r16(DS, si + 4) as f32);
        let sprite = m.mem.r8(DS, si + 0xC);
        let frame = m.mem.r8(DS, si + 0xD) as usize;
        let mirror = flags & 2 != 0;
        // A person in a fight (0000:1C9A): the walk, or from frame 6 the
        // attack with the weapon over it.
        let person = slot >= 15 && m.mem.r16(seg, rec.wrapping_add(0x40D)) & 0x1000 == 0;
        if person && (0xC0..0xC5).contains(&sprite) && !list.is_empty() {
            let view = (sprite - 0xC0 + 1).to_string();
            let file = |k: usize| list.get(k).and_then(|n| sheet.people.get(&n.replace('@', &view).to_ascii_uppercase())).copied();
            if frame < 6 {
                if let Some((f, _)) = file(0) {
                    billboard(&mut out, frame_of(f, frame), x, y, -z, right, mirror);
                }
            } else if let Some((f, a)) = file(1) {
                billboard(&mut out, frame_of(f, frame - 6), x, y, 0.0, right, mirror);
                if m.mem.r16(seg, rec.wrapping_add(0x40D)) & 0x400 != 0 {
                    if let Some((wf, w)) = file(2) {
                        let z = (w[2] - a[2]) + (w[0] - a[0]) + 0xE;
                        let shift = ((w[1] - a[1]) + 4) as f32;
                        billboard_at(&mut out, frame_of(wf, frame - 6), x, y, -z as f32, if mirror { -shift } else { shift }, right, mirror);
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
            billboard(&mut out, frame_of(f, if frame == 0xFF { f.count - 1 } else { frame }), x, y, -z, right, mirror);
        }
    }
    out
}
