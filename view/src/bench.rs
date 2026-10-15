//! `--bench TURNS [SETUP]`: the window's turns (`App::step`, the engine
//! copied for its replay, the frame, the view's frame) and redraws (the
//! layer composed, the sky and lights, the billboards, the GPU's draw to
//! an offscreen target, waited for) run headless and timed (`prof`; set
//! `ARENA_PROFILE`). SETUP is comma-separated: `at:COL:ROW:HEADING`,
//! `click`, `wait:N`, `walk:N`; the timers restart after it. Each timed
//! turn walks forward, turning right one turn in four for 30.

use crate::{prof, App};
use arena::mem::DS;
use arena::movement::var::{HEADING, X, Y};
use std::time::Instant;

pub fn run(app: &mut App, turns: usize, setup: &str, started: Instant) {
    println!("start-up: {:.1} ms", started.elapsed().as_secs_f64() * 1e3);
    let (w, h) = (640u32, 400u32);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).expect("adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).expect("device");
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut renderer = prof::time("renderer.new", || app.assets.renderer(&device, &queue, format));
    let overlay = crate::hud::Overlay::new(&device, format);
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bench"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let aspect = w as f32 / h as f32;
    let mut remade = app.remade;
    let step = |app: &mut App| {
        // The game's questions and boxes left, so the turns go on.
        app.pending = None;
        app.boxes.clear();
        prof::time("step", || app.step());
    };
    for s in setup.split(',').filter(|s| !s.is_empty()) {
        if let Some(at) = s.strip_prefix("at:") {
            let v: Vec<u16> = at.split(':').map(|x| x.parse().expect("at:COL:ROW:HEADING")).collect();
            app.engine.m.mem.w16(DS, X, v[0] << 7 | 0x40);
            app.engine.m.mem.w16(DS, Y, v[1] << 7 | 0x40);
            app.engine.m.mem.w16(DS, HEADING, v[2]);
            app.engine.world_loop_start();
            step(app);
        } else if s == "click" {
            app.click = true;
            step(app);
        } else if let Some(n) = s.strip_prefix("wait:").or(s.strip_prefix("walk:")) {
            app.keys.forward = s.starts_with("walk");
            for _ in 0..n.parse::<usize>().expect("wait:N / walk:N") {
                step(app);
            }
            app.keys.forward = false;
        } else {
            panic!("--bench setup: {s}?");
        }
    }
    if app.remade != remade {
        renderer = app.assets.renderer(&device, &queue, format);
        remade = app.remade;
    }
    println!("setup done ({:.1} ms since start-up)", started.elapsed().as_secs_f64() * 1e3);
    prof::report();
    prof::reset();
    let t0 = Instant::now();
    for i in 0..turns {
        app.keys.forward = true;
        app.keys.turn_right = (i / 30) % 4 == 3;
        step(app);
        if app.remade != remade {
            renderer = prof::time("renderer.new", || app.assets.renderer(&device, &queue, format));
            remade = app.remade;
        }
        let r = Instant::now();
        let cam = app.compose();
        let (sky, lights) = prof::time("redraw.drawn", || app.assets.drawn(&mut app.engine, aspect));
        let ticks = i * 4;
        let sprites = prof::time("redraw.sprites", || app.assets.sprites(&mut app.engine, cam, ticks, aspect));
        if prof::on() {
            prof::add("redraw.cpu", r.elapsed());
        }
        prof::time("redraw.gpu", || {
            renderer.set_sky(sky);
            renderer.set_lights(lights);
            renderer.set_window(crate::interface::window(&app.engine.m));
            renderer.draw(&device, &queue, &view, w, h, cam, &sprites);
            overlay.draw(&device, &queue, &view, &app.screen);
            device.poll(wgpu::Maintain::Wait);
        });
    }
    let total = t0.elapsed().as_secs_f64();
    let v = app.engine.view();
    println!("EMS banks kept: {}", app.engine.m.ems.len());
    println!("{turns} turns in {:.2} s ({:.2} ms a turn and redraw); end at ({:#x}, {:#x}) heading {:#x}", total, total * 1e3 / turns.max(1) as f64, v.x, v.y, v.heading);
    prof::report();
}
