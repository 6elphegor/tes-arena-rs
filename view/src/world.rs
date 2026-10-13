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
            } else if v != 0 && matches!(v >> 12, 9 | 0xA | 0xB | 0xD) {
                // Doors and the other wall kinds, drawn as walls for now
                // (an open door, bit 200h, as nothing).
                if !(v >> 12 == 0xB && v & 0x200 != 0) {
                    let t = (v & 0x3F) as u32;
                    sides(0.0, ceiling, t.saturating_sub(1), &|_| false);
                }
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

/// The flats' frames as sprite layers: the first layer of each flat (by
/// number), and every frame padded to the largest size.
pub struct SpriteSheet {
    pub w: usize,
    pub h: usize,
    pub layers: Vec<Vec<u8>>,
    pub first: Vec<Option<u32>>,
}

pub fn sprite_sheet(flats: &[Option<crate::flats::Flat>]) -> SpriteSheet {
    let w = flats.iter().flatten().map(|f| f.w).max().unwrap_or(1);
    let h = flats.iter().flatten().map(|f| f.h).max().unwrap_or(1);
    let mut layers = Vec::new();
    let mut first = Vec::new();
    for f in flats {
        let Some(f) = f else {
            first.push(None);
            continue;
        };
        first.push(Some(layers.len() as u32));
        for frame in &f.frames {
            let mut l = vec![0u8; w * h];
            for y in 0..f.h {
                l[y * w..y * w + f.w].copy_from_slice(&frame[y * f.w..(y + 1) * f.w]);
            }
            layers.push(l);
        }
    }
    if layers.is_empty() {
        layers.push(vec![0; w * h]);
    }
    SpriteSheet { w, h, layers, first }
}

/// Billboards for the flats standing in the map (MAP1 words 8xxx: flat
/// low byte + FLAT_BASE), at their squares' centres, facing a camera
/// turned to `heading`; animations at frame `tick`. A flat is a unit
/// per pixel, raised by its record's Y.
pub fn sprites(m: &Machine, flats: &[Option<crate::flats::Flat>], sheet: &SpriteSheet, heading: f32, tick: usize) -> Vec<Vertex> {
    let w = m.mem.r16(DS, 0xC357).min(128);
    let d = m.mem.r16(DS, 0xC359).min(128);
    let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
    let base = m.mem.r16(DS, arena::info::var::FLAT_BASE);
    let a = heading / 512.0 * std::f32::consts::TAU;
    // The camera's right, along which a billboard spreads.
    let (rx, rz) = (a.cos(), -a.sin());
    let mut out = Vec::new();
    for row in 0..d {
        for col in 0..w {
            let v = m.mem.r16(walls, (row << 8) | (col << 1));
            if v >> 12 != 8 {
                continue;
            }
            let n = ((v & 0xFF) + base) as usize;
            let (Some(Some(f)), Some(Some(layer))) = (flats.get(n), sheet.first.get(n)) else {
                continue;
            };
            let lift = m.mem.r8(DS, arena::info::var::FLATS + n as u16 * 5) as i8 as f32;
            let (cx, cz) = (col as f32 * SQUARE + SQUARE / 2.0, row as f32 * SQUARE + SQUARE / 2.0);
            let hw = f.w as f32 / 2.0;
            let (lo, hi) = (lift, lift + f.h as f32);
            let layer = layer + (tick % f.frames.len()) as u32;
            let p = [
                [cx - rx * hw, lo, cz - rz * hw],
                [cx + rx * hw, lo, cz + rz * hw],
                [cx + rx * hw, hi, cz + rz * hw],
                [cx - rx * hw, hi, cz - rz * hw],
            ];
            let (u, t) = (f.w as f32, f.h as f32);
            let uv = [[0.0, t], [u, t], [u, 0.0], [0.0, 0.0]];
            for i in [0, 1, 2, 0, 2, 3] {
                out.push(Vertex { pos: p[i], uv: uv[i], layer, light: 1.0, flags: SPRITE });
            }
        }
    }
    out
}
