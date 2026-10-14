//! arena-view: walk a level of Arena, drawn with wgpu, with the port's
//! engine running the game.
//!
//! Usage: arena-view GAME_DIR LEVEL.MIF [--fight] [--people] [--click] [--hour H] [--at COL ROW HEADING] [--wait N] [--shot OUT.png [FRAMES [TURN]]]
//!
//! Keys: W/S or Up/Down forward and back, A/D strafe, Left/Right turn,
//! the mouse turns (click the window to capture it, Esc to release),
//! Space jumps (Shift for a running jump), E or a click (captured) clicks
//! the centre of the view: opens a door. With --shot it renders one
//! picture after turning by TURN (heading units, 512 a turn) and FRAMES
//! frames (default 0) of walking forward, and exits; --wait stands still
//! N frames before the walk, --click clicking first; --at puts the
//! player in a square's centre, facing HEADING, a debugging aid; --hour
//! sets the clock to that hour. --fight brings a random encounter at the
//! start (F does in the window); --people (P) rolls until people come.

mod archive;
mod flats;
mod render;
mod world;

use arena::controls::{ControlHooks, Intent};
use arena::engine::{Engine, EngineHooks, Input};
use render::{Camera, Renderer, Scene};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The front end's hooks: nothing asked of the player yet.
struct Front;

impl ControlHooks for Front {}
impl arena::weapon::WeaponHooks for Front {}
impl arena::clock::ClockHooks for Front {}
impl arena::frame::FrameHooks for Front {}
impl EngineHooks for Front {
    /// The original stepped the changed squares it drew: here those
    /// within the view's reach (C394 squares) step, seen or not.
    fn draw(&mut self, m: &mut arena::machine::Machine) {
        use arena::click::var::{DOOR_AT, DOOR_COL, DOOR_ROW, DOOR_X0, DOOR_X1};
        use arena::mem::DS;
        // What a click in the centre would meet (the original's renderer
        // left the square and, for a door, its span under the cursor).
        if let Some(sq) = world::pick(m, m.mem.r16(DS, 0xC394)) {
            m.mem.w16(DS, arena::controls::var::PICKED, sq);
            let walls = m.mem.r16(DS, arena::mif::var::MAP1_SEG);
            if m.mem.r16(walls, sq) >> 12 == 0xB {
                m.mem.w16(DS, DOOR_X0, 0);
                m.mem.w16(DS, DOOR_X1, 319);
                m.mem.w16(DS, DOOR_COL, (sq & 0xFF) >> 1);
                m.mem.w16(DS, DOOR_ROW, sq >> 8);
                m.mem.w16(DS, DOOR_AT, sq);
            }
        }
        let reach = m.mem.r16(DS, 0xC394) as i32;
        let (col, row) = (m.mem.r16(DS, 0xCCA4) as i32, m.mem.r16(DS, 0xCCA6) as i32);
        for i in 0..0x20 {
            let sq = m.mem.r16(DS, arena::click::var::TEMP_WALLS + 2 * i);
            if sq == 0xFFFF {
                continue;
            }
            let (c, r) = ((sq & 0xFF) as i32 >> 1, (sq >> 8) as i32);
            if (c - col).abs() <= reach && (r - row).abs() <= reach {
                m.temp_walls_begin_draw();
                m.temp_wall_drawn(sq);
            }
        }
    }
}

fn camera(engine: &Engine) -> Camera {
    let v = engine.view();
    Camera { x: v.x as f32, y: v.y as f32, height: v.height as f32 + v.bob as f32, heading: v.heading as f32 }
}

/// What drawing a level needs, read once from the engine after loading.
struct Assets {
    palette: [[u8; 4]; 256],
    textures: Vec<[u8; 4096]>,
    shading: Vec<u8>,
    mesh: Vec<world::Vertex>,
    sheet: world::SpriteSheet,
    archive: archive::Archive,
    /// DITHER.IMG then DITHER2.IMG.
    dither: Vec<u8>,
    /// The sky's .COL as the game keeps it: its name's offset and its
    /// colours, the first eight again past the end.
    colours: Option<(u16, Vec<u8>)>,
    /// What the day and night switch left: the row the textures are
    /// remapped through (0 as loaded), palette entry 71h (the lamps'
    /// glow; PAL.COL's until set) and the light table's segment.
    dark: u32,
    lamp: Option<[u8; 3]>,
    table: u16,
    /// The sky's images loaded, by their offset in the EMS frame.
    sky_images: HashMap<u16, String>,
}

/// The view's side of the day and night switch (`LightHooks`).
struct Light<'a> {
    dark: &'a mut u32,
    lamp: &'a mut Option<[u8; 3]>,
    table: &'a mut u16,
    sky: SkyLoad<'a>,
}

/// The sky's images loaded as 0000:7428 loads them with DI FFFDh: the
/// file's size allocated in EMS (0000:5C26, which maps the bank), the
/// file read there. Their names are kept for the atlas.
struct SkyLoad<'a> {
    archive: &'a archive::Archive,
    images: &'a mut HashMap<u16, String>,
}

impl arena::skyobj::SkyHooks for SkyLoad<'_> {
    fn load(&mut self, m: &mut arena::machine::Machine, name: u16) -> Option<(u16, u16)> {
        let file = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, name)).to_ascii_uppercase();
        let data = self.archive.read(&file)?;
        if data.is_empty() || data.len() > 0xFFFF {
            return None;
        }
        let (off, _) = m.ems_alloc(data.len() as u16)?;
        let frame = m.mem.r16(arena::mem::DS, 0x9030);
        for (i, &b) in data.iter().enumerate() {
            m.mem.w8(frame, off.wrapping_add(i as u16), b);
        }
        self.images.insert(off, file);
        Some((frame, off))
    }
}

/// The sky's objects drawn this frame (2294:0A5D's draws), as quads.
struct SkyDraw<'a> {
    sheet: &'a world::SpriteSheet,
    images: &'a HashMap<u16, String>,
    focal: f32,
    out: Vec<world::Vertex>,
}

impl SkyDraw<'_> {
    fn quad(&mut self, m: &arena::machine::Machine, name: Option<&String>, frame: usize, x: u16, y: u16) {
        use arena::mem::DS;
        let Some(f) = name.and_then(|n| self.sheet.sky.get(n)) else { return };
        let rect = self.sheet.rects[f.first + frame % f.count];
        let r = |at| m.mem.r16(DS, at) as i16;
        world::sky_quad(&mut self.out, rect, x as i16, y as i16, r(0xC3D6), r(0xC3D8), r(0xA91A), self.focal);
    }
}

impl arena::skyobj::SkyHooks for SkyDraw<'_> {
    fn draw_image(&mut self, m: &mut arena::machine::Machine, seg: u16, off: u16) {
        let name = if seg == m.mem.r16(arena::mem::DS, 0x9030) { self.images.get(&off) } else { None };
        let name = name.cloned().or_else(|| (m.mem.r8(arena::mem::DS, 0x7BD2) == 3).then(|| "VOLCANO3.IMG".to_string()));
        self.quad(m, name.as_ref(), 0, m.mem.r16(seg, off), m.mem.r16(seg, off + 2));
    }
    fn draw_dfa(&mut self, m: &mut arena::machine::Machine, _seg: u16, _off: u16, frame: u16, x: u16, y: u16) {
        self.quad(m, Some(&"VOLCANO2.DFA".to_string()), frame as usize, x, y);
    }
}

impl arena::light::LightHooks for Light<'_> {
    /// The table freed and made again from NORMAL.LGT's rows.
    fn light_table(&mut self, m: &mut arena::machine::Machine) {
        if *self.table != 0 {
            m.heap_free(*self.table);
        }
        *self.table = world::light_table(m);
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
        *self.dark = (level >> 8) as u32;
    }

    fn restore_textures(&mut self, _m: &mut arena::machine::Machine) {
        *self.dark = 0;
    }

    /// 2294:08B9, the sky's objects made again (by day, and before
    /// relighting by night).
    fn sky_objects(&mut self, m: &mut arena::machine::Machine) {
        self.sky.images.clear();
        m.sky_objects(&mut self.sky);
    }

    /// Only entry 71h is written through here; its 6-bit colour made 8-bit.
    fn palette(&mut self, m: &mut arena::machine::Machine, index: u8, at: u16) {
        if index == 0x71 {
            let c = |i: u16| m.mem.r8(arena::mem::DS, at + i) & 0x3F;
            *self.lamp = Some([0, 1, 2].map(|i| c(i) << 2 | c(i) >> 4));
        }
    }
}

impl Assets {
    /// Also enters the area's sky (0000:464A: its objects made).
    fn new(dir: &std::path::Path, engine: &mut Engine) -> Self {
        let flats = world::flats(&engine.m);
        let archive = archive::Archive::open(dir);
        let sheet = world::sprite_sheet(&flats, &archive, &world::creature_names(&engine.m));
        let mut sky_images = HashMap::new();
        engine.m.area_sky(&mut SkyLoad { archive: &archive, images: &mut sky_images });
        Assets {
            palette: world::palette(dir),
            textures: world::textures(&engine.m),
            shading: world::shading(&engine.m),
            mesh: world::mesh(&engine.m),
            sheet,
            dither: ["DITHER.IMG", "DITHER2.IMG"].iter().flat_map(|n| {
                let mut d = archive.read(n).unwrap_or_default();
                d.resize(800, 0);
                d
            }).collect(),
            archive,
            colours: None,
            dark: 0,
            lamp: None,
            table: engine.m.mem.r16(arena::mem::DS, 0xCCC2),
            sky_images,
        }
    }

    fn renderer(&self, device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Renderer {
        let scene = Scene { palette: self.palette, textures: &self.textures, shading: &self.shading, mesh: &self.mesh, sprites: &self.sheet, dither: &self.dither };
        Renderer::new(device, queue, format, &scene)
    }

    /// The sky after a frame (2294:0428, then 0000:0390 with 2294:0341;
    /// the .COL loads here): outdoors, palette entries 1..8 from the day's
    /// colours at the window for the time of day, the textures darkened by
    /// night and the lamps lit.
    fn sky(&mut self, m: &mut arena::machine::Machine) -> render::Sky {
        use arena::sky::var::{COLOURS, DITHER, DITHER_AFTERNOON, WINDOW};
        let outdoors = m.mem.r16(arena::mem::DS, 0xC398) & 4 != 0;
        if !outdoors {
            return render::Sky::default();
        }
        m.sky_colours();
        let sky = SkyLoad { archive: &self.archive, images: &mut self.sky_images };
        m.outdoor_light(&mut Light { dark: &mut self.dark, lamp: &mut self.lamp, table: &mut self.table, sky });
        let name = m.mem.r16(arena::mem::DS, COLOURS);
        if self.colours.as_ref().map(|c| c.0) != Some(name) {
            let file = String::from_utf8_lossy(m.mem.cstr(arena::mem::DS, name)).into_owned();
            // An 8-byte header, then the colours.
            let mut c = self.archive.read(&file).map(|d| d.get(8..).unwrap_or_default().to_vec()).unwrap_or_default();
            c.resize(0x300, 0);
            let first: Vec<u8> = c[..0x18].to_vec();
            c.extend(first);
            self.colours = Some((name, c));
        }
        let c = &self.colours.as_ref().unwrap().1;
        let w = m.mem.r16(arena::mem::DS, WINDOW) as usize;
        let mut colours = [[0u8; 3]; 8];
        for (i, col) in colours.iter_mut().enumerate() {
            for (k, v) in col.iter_mut().enumerate() {
                *v = *c.get(w + 3 * i + k).unwrap_or(&0);
            }
        }
        render::Sky { outdoors, afternoon: m.mem.r16(arena::mem::DS, DITHER) == DITHER_AFTERNOON, colours, dark: self.dark, lamp: self.lamp }
    }

    /// The lights after a frame (the drawing's set-up, 0000:A2EC..A303 and
    /// E106): the player's light at the player, the near list made again
    /// if the player moved to another place (`lights_near`), then the
    /// near and the moving lights. None in full daylight (C398 bit 8000h).
    fn lights(&self, m: &mut arena::machine::Machine) -> render::Lights {
        use arena::light::var::*;
        use arena::mem::DS;
        if m.mem.r16(DS, 0xC398) & 0x8000 != 0 {
            return render::Lights::default();
        }
        let (x, y) = (m.mem.r16(DS, 0xC3D0), m.mem.r16(DS, 0xC3D4));
        m.mem.w16(DS, NEAR, x);
        m.mem.w16(DS, NEAR + 2, y);
        m.lights_near();
        m.moving_lights();
        let at = |m: &arena::machine::Machine, list: u16, i: u16| {
            let e = list + 6 * i;
            [m.mem.r16(DS, e), m.mem.r16(DS, e + 2), m.mem.r16(DS, e + 4)]
        };
        let mut lights: Vec<[u16; 3]> = (0..m.mem.r16(DS, NEAR_COUNT).min(64)).map(|i| at(m, NEAR, i)).collect();
        lights.extend((0..m.mem.r16(DS, MOVING_COUNT).min(32)).map(|i| at(m, MOVING, i)));
        lights.truncate(64);
        render::Lights { lights, rows: [m.mem.r16(DS, LIMIT), m.mem.r16(DS, DARK), m.mem.r16(DS, FAR)] }
    }

    /// This frame's billboards, animations stepping every `ANIMATE` ticks,
    /// and the sky's objects (2294:0A5D) for a view `aspect` wide.
    fn sprites(&self, engine: &mut Engine, cam: Camera, ticks: usize, aspect: f32) -> Vec<world::Vertex> {
        let mut v = world::sprites(&engine.m, &self.sheet, cam.heading, ticks / ANIMATE);
        v.extend(world::doors(&engine.m));
        let focal = (30f32).to_radians().tan() * aspect;
        let mut draw = SkyDraw { sheet: &self.sheet, images: &self.sky_images, focal, out: Vec::new() };
        if engine.m.sky_draw(&mut draw).is_some() {
            v.extend(draw.out);
        }
        v
    }
}

/// Timer ticks an animation frame lasts.
const ANIMATE: usize = 4;

fn main() {
    let mut args: Vec<String> = std::env::args().collect();
    let fight = args.iter().any(|a| a == "--fight");
    let click = args.iter().any(|a| a == "--click");
    let people = args.iter().any(|a| a == "--people");
    args.retain(|a| a != "--fight" && a != "--click" && a != "--people");
    let mut wait = 0;
    if let Some(i) = args.iter().position(|a| a == "--wait") {
        wait = args.get(i + 1).and_then(|v| v.parse().ok()).expect("--wait N");
        args.drain(i..i + 2);
    }
    let mut hour = None;
    if let Some(i) = args.iter().position(|a| a == "--hour") {
        hour = Some(args[i + 1].parse::<u32>().expect("--hour H"));
        args.drain(i..i + 2);
    }
    let mut at = None;
    if let Some(i) = args.iter().position(|a| a == "--at") {
        let v: Vec<u16> = args[i + 1..i + 4].iter().map(|v| v.parse().expect("--at COL ROW HEADING")).collect();
        at = Some((v[0], v[1], v[2]));
        args.drain(i..i + 4);
    }
    if args.len() < 3 {
        eprintln!("usage: arena-view GAME_DIR LEVEL.MIF [--fight] [--people] [--click] [--hour H] [--at COL ROW HEADING] [--wait N] [--shot OUT.png [FRAMES [TURN]]]");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let mut engine = world::load(&dir, &args[2]);
    if let Some(h) = hour {
        // The clock at that hour of the first day (its calendar fields
        // agreeing, so no hour passes at once), and the sky's time.
        use arena::clock::var::{HOUR, MINUTE, MINUTES};
        engine.m.mem.w32(arena::mem::DS, MINUTES, h % 24 * 60);
        engine.m.mem.w16(arena::mem::DS, HOUR, (h % 24) as u16);
        engine.m.mem.w16(arena::mem::DS, MINUTE, 0);
        engine.m.sky_time().expect("sky time");
    }
    world::city_people(&mut engine);
    if let Some((col, row, heading)) = at {
        use arena::movement::var::{HEADING, X, Y};
        engine.m.mem.w16(arena::mem::DS, X, col << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, Y, row << 7 | 64);
        engine.m.mem.w16(arena::mem::DS, HEADING, heading);
    }
    if fight {
        world::summon(&mut engine);
    }
    if people {
        world::summon_people(&mut engine);
    }
    if args.get(3).map(String::as_str) == Some("--shot") {
        let out = args.get(4).expect("--shot needs a file");
        let frames: usize = args.get(5).map_or(0, |v| v.parse().expect("frames"));
        let turn: i16 = args.get(6).map_or(0, |v| v.parse().expect("turn"));
        shot(&dir, engine, out, click, wait, frames, turn);
        return;
    }
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let assets = Assets::new(&dir, &mut engine);
    let mut app = App { assets, start: Instant::now(), engine, gpu: None, keys: Default::default(), look: 0.0, captured: false, click: false, next: Instant::now() };
    event_loop.run_app(&mut app).expect("run");
}

/// Renders one frame offscreen to a PNG, after `frames` frames of
/// walking forward.
fn shot(dir: &std::path::Path, mut engine: Engine, out: &str, click: bool, wait: usize, frames: usize, turn: i16) {
    let mut hooks = Front;
    // One frame turning (or not) before the walk: the actors pick their
    // sprites in a frame.
    let t = engine.frame_ticks();
    engine.tick(t);
    let intent = Intent { look: turn, ..Default::default() };
    engine.frame(Input::Steer(&intent), &mut hooks).expect("frame");
    if click {
        centre_click(&mut engine);
    }
    for _ in 0..wait {
        let t = engine.frame_ticks();
        engine.tick(t);
        engine.frame(Input::Steer(&Intent::default()), &mut hooks).expect("frame");
    }
    for _ in 0..frames {
        let t = engine.frame_ticks();
        engine.tick(t);
        let intent = Intent { forward: 256, ..Default::default() };
        engine.frame(Input::Steer(&intent), &mut hooks).expect("frame");
    }
    let (w, h) = (640u32, 400u32);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).expect("device");
    let mut assets = Assets::new(dir, &mut engine);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = assets.renderer(&device, &queue, format);
    r.set_sky(assets.sky(&mut engine.m));
    r.set_lights(assets.lights(&mut engine.m));
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("shot"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let cam = camera(&engine);
    let sprites = assets.sprites(&mut engine, cam, (wait + frames) * 4, w as f32 / h as f32);
    r.draw(&device, &queue, &target.create_view(&Default::default()), w, h, cam, &sprites);
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (w * h * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
        },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = buf.slice(..).get_mapped_range().to_vec();
    let file = std::fs::File::create(out).expect("create");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w, h);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().expect("png").write_image_data(&data).expect("png");
    let v = engine.view();
    println!("{out}: at ({:#x}, {:#x}) heading {:#x}, {} triangles", v.x, v.y, v.heading, assets.mesh.len() / 3);
}

/// A left click in the centre of the view (`Machine::view_click`), on
/// what the last frame's drawing left under it.
fn centre_click(engine: &mut Engine) {
    engine.m.set_mouse(160, 70, 0);
    if engine.m.view_click(&mut engine.files, &mut Front).is_none() {
        eprintln!("the click met something not ported");
    }
}

struct Gpu {
    window: Arc<winit::window::Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: Renderer,
}

#[derive(Default)]
struct Keys {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    turn_left: bool,
    turn_right: bool,
    jump: bool,
    run: bool,
}

struct App {
    assets: Assets,
    start: Instant,
    engine: Engine,
    gpu: Option<Gpu>,
    keys: Keys,
    look: f64,
    captured: bool,
    /// A click waiting for the next frame.
    click: bool,
    next: Instant,
}

impl App {
    /// One engine frame with the keys and the mouse as an Intent.
    fn step(&mut self) {
        let k = &self.keys;
        let axis = |p: bool, n: bool| (p as i16 - n as i16) * 256;
        let look = self.look.round() as i16;
        self.look -= look as f64;
        let intent = Intent {
            forward: axis(k.forward, k.back),
            strafe: axis(k.right, k.left),
            turn: axis(k.turn_right, k.turn_left),
            look,
            jump: k.jump,
            run: k.run,
        };
        if std::mem::take(&mut self.click) {
            centre_click(&mut self.engine);
        }
        let t = self.engine.frame_ticks();
        self.engine.tick(t);
        match self.engine.frame(Input::Steer(&intent), &mut Front) {
            Some(arena::engine::Frame::Running) => {}
            other => eprintln!("the frame ended: {other:?}"),
        }
    }
}

impl winit::application::ApplicationHandler for App {
    fn resumed(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let window = Arc::new(
            el.create_window(winit::window::Window::default_attributes().with_title("Arena").with_inner_size(winit::dpi::LogicalSize::new(960, 600)))
                .expect("window"),
        );
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("adapter");
        let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).expect("device");
        let caps = surface.get_capabilities(&adapter);
        // Palette colours as they are: a non-sRGB format if there is one.
        let format = caps.formats.iter().copied().find(|f| !f.is_srgb()).unwrap_or(caps.formats[0]);
        let size = window.inner_size();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);
        let renderer = self.assets.renderer(&device, &queue, format);
        self.gpu = Some(Gpu { window, surface, device, queue, config, renderer });
    }

    fn window_event(&mut self, el: &winit::event_loop::ActiveEventLoop, _id: winit::window::WindowId, event: winit::event::WindowEvent) {
        use winit::event::{ElementState, WindowEvent};
        use winit::keyboard::{KeyCode, PhysicalKey};
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = &mut self.gpu {
                    g.config.width = size.width.max(1);
                    g.config.height = size.height.max(1);
                    g.surface.configure(&g.device, &g.config);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, .. } if !self.captured => {
                if let Some(g) = &self.gpu {
                    let ok = g
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::Locked)
                        .or_else(|_| g.window.set_cursor_grab(winit::window::CursorGrabMode::Confined))
                        .is_ok();
                    g.window.set_cursor_visible(!ok);
                    self.captured = ok;
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: winit::event::MouseButton::Left, .. } => self.click = true,
            WindowEvent::KeyboardInput { event, .. } => {
                let down = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    let k = &mut self.keys;
                    match code {
                        KeyCode::KeyW | KeyCode::ArrowUp => k.forward = down,
                        KeyCode::KeyS | KeyCode::ArrowDown => k.back = down,
                        KeyCode::KeyA => k.left = down,
                        KeyCode::KeyD => k.right = down,
                        KeyCode::ArrowLeft => k.turn_left = down,
                        KeyCode::ArrowRight => k.turn_right = down,
                        KeyCode::Space => k.jump = down,
                        KeyCode::KeyF if down => world::summon(&mut self.engine),
                        KeyCode::KeyP if down => world::summon_people(&mut self.engine),
                        KeyCode::KeyE if down => self.click = true,
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => k.run = down,
                        KeyCode::Escape if down => {
                            if let Some(g) = &self.gpu {
                                let _ = g.window.set_cursor_grab(winit::window::CursorGrabMode::None);
                                g.window.set_cursor_visible(true);
                            }
                            self.captured = false;
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let cam = camera(&self.engine);
                if let Some(g) = &mut self.gpu {
                    let Ok(frame) = g.surface.get_current_texture() else {
                        g.surface.configure(&g.device, &g.config);
                        return;
                    };
                    let view = frame.texture.create_view(&Default::default());
                    let ticks = (self.start.elapsed().as_secs_f64() * 18.2) as usize;
                    let aspect = g.config.width as f32 / g.config.height.max(1) as f32;
                    let sprites = self.assets.sprites(&mut self.engine, cam, ticks, aspect);
                    g.renderer.set_sky(self.assets.sky(&mut self.engine.m));
                    g.renderer.set_lights(self.assets.lights(&mut self.engine.m));
                    g.renderer.draw(&g.device, &g.queue, &view, g.config.width, g.config.height, cam, &sprites);
                    frame.present();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _el: &winit::event_loop::ActiveEventLoop, _id: winit::event::DeviceId, event: winit::event::DeviceEvent) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            if self.captured {
                // About a quarter turn for 400 pixels.
                self.look += delta.0 * 128.0 / 400.0;
            }
        }
    }

    fn about_to_wait(&mut self, el: &winit::event_loop::ActiveEventLoop) {
        // The game runs at its own pace (18.2 timer ticks a second, a frame
        // taking `frame_ticks`); drawing goes as fast as the display.
        let now = Instant::now();
        if now >= self.next {
            self.step();
            let ticks = self.engine.frame_ticks() as f64;
            self.next = now + Duration::from_secs_f64(ticks / 18.2);
        }
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
        el.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(self.next));
    }
}
