//! arena-view: walk a level of Arena, drawn with wgpu, with the port's
//! engine running the game.
//!
//! Usage: arena-view GAME_DIR LEVEL.MIF [--shot OUT.png [FRAMES [TURN]]]
//!
//! Keys: W/S or Up/Down forward and back, A/D strafe, Left/Right turn,
//! the mouse turns (click the window to capture it, Esc to release),
//! Space jumps (Shift for a running jump). With --shot it renders one
//! picture after turning by TURN (heading units, 512 a turn) and FRAMES
//! frames (default 0) of walking forward, and exits.

mod render;
mod world;

use arena::controls::{ControlHooks, Intent};
use arena::engine::{Engine, EngineHooks, Input};
use render::{Camera, Renderer, Scene};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// The front end's hooks: nothing asked of the player yet.
struct Front;

impl ControlHooks for Front {}
impl arena::weapon::WeaponHooks for Front {}
impl arena::clock::ClockHooks for Front {}
impl arena::frame::FrameHooks for Front {}
impl EngineHooks for Front {}

fn camera(engine: &Engine) -> Camera {
    let v = engine.view();
    Camera { x: v.x as f32, y: v.y as f32, height: v.height as f32 + v.bob as f32, heading: v.heading as f32 }
}

fn scene_parts(dir: &std::path::Path, engine: &Engine) -> ([[u8; 4]; 256], Vec<[u8; 4096]>, Vec<u8>, Vec<world::Vertex>) {
    (world::palette(dir), world::textures(&engine.m), world::shading(&engine.m), world::mesh(&engine.m))
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: arena-view GAME_DIR LEVEL.MIF [--shot OUT.png [FRAMES]]");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let engine = world::load(&dir, &args[2]);
    if args.get(3).map(String::as_str) == Some("--shot") {
        let out = args.get(4).expect("--shot needs a file");
        let frames: usize = args.get(5).map_or(0, |v| v.parse().expect("frames"));
        let turn: i16 = args.get(6).map_or(0, |v| v.parse().expect("turn"));
        shot(&dir, engine, out, frames, turn);
        return;
    }
    let event_loop = winit::event_loop::EventLoop::new().expect("event loop");
    let mut app = App { dir, engine, gpu: None, keys: Default::default(), look: 0.0, captured: false, next: Instant::now() };
    event_loop.run_app(&mut app).expect("run");
}

/// Renders one frame offscreen to a PNG, after `frames` frames of
/// walking forward.
fn shot(dir: &std::path::Path, mut engine: Engine, out: &str, frames: usize, turn: i16) {
    let mut hooks = Front;
    if turn != 0 {
        let t = engine.frame_ticks();
        engine.tick(t);
        let intent = Intent { look: turn, ..Default::default() };
        engine.frame(Input::Steer(&intent), &mut hooks).expect("frame");
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
    let (pal, tex, shade, mesh) = scene_parts(dir, &engine);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = Renderer::new(&device, &queue, format, &Scene { palette: pal, textures: &tex, shading: &shade, mesh: &mesh });
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
    r.draw(&device, &queue, &target.create_view(&Default::default()), w, h, camera(&engine));
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
    println!("{out}: at ({:#x}, {:#x}) heading {:#x}, {} triangles", v.x, v.y, v.heading, mesh.len() / 3);
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
    dir: PathBuf,
    engine: Engine,
    gpu: Option<Gpu>,
    keys: Keys,
    look: f64,
    captured: bool,
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
        let (pal, tex, shade, mesh) = scene_parts(&self.dir, &self.engine);
        let renderer = Renderer::new(&device, &queue, format, &Scene { palette: pal, textures: &tex, shading: &shade, mesh: &mesh });
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
                    g.renderer.draw(&g.device, &g.queue, &view, g.config.width, g.config.height, cam);
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
