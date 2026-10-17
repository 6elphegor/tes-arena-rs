//! The view's play against the original's world frames
//! (harness/gen_worldframes.py, tests/vectors/worldframes.txt): from the
//! dungeon both sides make (`into_dungeon`), the same keys held, the
//! player's place frame by frame, and what the 2D drawing leaves on the
//! screen in play (the interface, the compass, the bars: the pixels the
//! original's frame didn't draw as the 3D view) against the view's layer.

use super::*;

const VECTORS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/worldframes.txt");

/// The case's track: per frame, level, x, height, y, heading.
fn track(name: &str) -> Vec<[u16; 5]> {
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    text.lines()
        .map(|l| l.split(" | ").collect::<Vec<_>>())
        .filter(|f| f.len() == 3 && f[0] == "track" && f[1] == name)
        .map(|f| {
            let v: Vec<u16> = f[2].split(' ').map(|x| u16::from_str_radix(x, 16).unwrap()).collect();
            [v[1], v[2], v[3], v[4], v[5]]
        })
        .collect()
}

/// The player's state as a track's (the physics' place, BA62..BA67).
fn state(a: &App) -> [u16; 5] {
    let m = &a.engine.m;
    [m.mem.r8(DS, arena::movement::var::LEVEL) as u16, m.mem.r16(DS, 0xBA62), m.mem.r16(DS, 0xBA64), m.mem.r16(DS, 0xBA66), m.mem.r16(DS, arena::movement::var::HEADING)]
}

/// The player at the middle of a square, facing `heading`.
fn at_square(a: &mut App, row: u16, col: u16, heading: u16) {
    let m = &mut a.engine.m;
    for base in [arena::movement::var::X, 0xBA62] {
        m.mem.w16(DS, base, (col << 7) + 0x40);
        m.mem.w16(DS, base + 4, (row << 7) + 0x40);
    }
    m.mem.w16(DS, arena::movement::var::HEADING, heading);
}

/// Facing `heading` at ten at night of the save's day, as
/// gen_worldframes.py's `night` sets it.
fn night(a: &mut App, heading: u16) {
    use arena::clock::var::{HOUR, MINUTE, MINUTES, SKY_TIME};
    let m = &mut a.engine.m;
    m.mem.w16(DS, arena::movement::var::HEADING, heading);
    let t = m.mem.r32(DS, MINUTES);
    m.mem.w32(DS, MINUTES, t - t % 1440 + 22 * 60);
    m.mem.w16(DS, HOUR, 22);
    m.mem.w16(DS, MINUTE, 0);
    m.mem.w16(DS, SKY_TIME, ((22 * 60) << 10) / 1440);
}

/// Where a case is played: the dungeon `into_dungeon` makes, or the main
/// quest's Crypt of Hearts (`into_crypt`).
#[derive(Clone, Copy)]
enum Place {
    Dungeon,
    Crypt,
    Street,
}

/// From the guild to the street, then into the Crypt of Hearts the way
/// the outer loop enters a named level (0000:0993): the next area 2, no
/// interior, its .MIF named at 0A36.
fn into_crypt(a: &mut App) {
    area_left(a);
    let m = &mut a.engine.m;
    m.mem.w8(DS, arena::trigger::var::AREA, 2);
    m.mem.w16(DS, arena::transit::var::INTERIOR, 0);
    for (i, &c) in b"41649375.MIF\0".iter().enumerate() {
        m.mem.w8(DS, 0x0A36 + i as u16, c);
    }
    area_left(a);
}

/// Into the place, `setup` run, then `play` driving frames and
/// collecting the track: the app as the case leaves it.
fn played(place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) -> Option<(App, Vec<[u16; 5]>)> {
    let dir = game_dir()?;
    let Some(save) = save_copy() else {
        eprintln!("skipping: no ARENA_SAVE");
        return None;
    };
    // Loaded as `loaded` loads, but with no frame run in the guild: the
    // harness leaves it straight from the load, so what the world loop's
    // first frame would change is still the load's (the load's 5CA bit
    // 4000h, which 0000:48BE clears and which keeps 0000:4767 from making
    // the next level's lights; the fall's floor E8BC, still 0, which
    // drops the player a step on the street's door square).
    let mut a = app(&dir, &Start { hour: None, load: Some((save.to_path_buf(), 1)), save_dir: Some(save.to_path_buf()), ..Default::default() });
    let mut d = Driver::new(&mut a);
    clear(&mut d, &mut a);
    match place {
        Place::Dungeon => into_dungeon(&mut a),
        Place::Crypt => into_crypt(&mut a),
        Place::Street => area_left(&mut a),
    }
    a.fade.clear();
    a.fade_old = None;
    setup(&mut a);
    let mut seen = vec![state(&a)];
    play(&mut a, &mut d, &mut seen);
    // The harness's last frame, run again with the page's writes watched
    // (gen_worldframes.py `play`): the page it leaves is the case's.
    d.frame(&mut a);
    Some((a, seen))
}

/// Into the place, `setup` run, then `play` driving frames and
/// collecting the track; both against the original's.
fn world_case(name: &str, place: Place, setup: impl FnOnce(&mut App), play: impl FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)) {
    let Some((mut a, seen)) = played(place, setup, play) else { return };
    let want = track(name);
    assert_eq!(seen, want, "{name}: the player's frames differ from the original's");
    check_fade_in("worldframes.txt", name, &mut a);
    let (idx, dac) = layer(&mut a);
    assert!(check_in(VECTORS, name, &idx, &dac), "{name}: differs from the original's");
}

fn frames(a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>, n: usize, until: impl Fn(&App) -> bool) {
    for _ in 0..n {
        d.frame(a);
        seen.push(state(a));
        if until(a) {
            break;
        }
    }
}

/// The dungeon's first frames, nothing held.
#[test]
fn play_in_a_dungeon_as_the_original_draws_it() {
    world_case("playdungeon", Place::Dungeon, |_| {}, |a, d, seen| frames(a, d, seen, 3, |_| false));
}

/// The street outside the guild, its first frames, nothing held.
#[test]
fn play_in_the_street_as_the_original_draws_it() {
    let (place, setup, play) = case("playstreet").unwrap();
    world_case("playstreet", place, setup, play);
}

/// The first level's down stairs walked into with the up arrow held from
/// the square south of them, facing north: the second level.
#[test]
fn down_a_dungeons_stairs_as_the_original_goes() {
    world_case(
        "playdown",
        Place::Dungeon,
        |a| at_square(a, 11, 10, 0x100),
        |a, d, seen| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, 2, |_| false);
        },
    );
}

/// The Crypt of Hearts entered, turned north and walked on into the square
/// in front of its start, whose trigger shows the INF's first text; let
/// go, a frame more: the text over the view.
#[test]
fn the_crypt_of_hearts_first_text_as_the_original_draws_it() {
    world_case(
        "playcrypt",
        Place::Crypt,
        |a| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100),
        |a, d, seen| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, 0x3C82) != 0);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, 1, |_| false);
        },
    );
}

/// South of the crypt's door at row 14, column 4, facing it, the door
/// given a lock of level 5; clicked in the middle of the view two frames
/// on: not armed, the lock's words on the message line.
#[test]
fn a_locked_door_clicked_as_the_original_answers_it() {
    world_case(
        "playlock",
        Place::Crypt,
        |a| {
            at_square(a, 13, 4, 0);
            let m = &mut a.engine.m;
            let n = m.mem.r8(DS, arena::movement::var::LOCK_COUNT);
            let rec = arena::movement::var::LOCKS + 3 * n as u16;
            m.mem.w16(DS, rec, 0x0E04);
            m.mem.w8(DS, rec + 2, 5);
            m.mem.w8(DS, arena::movement::var::LOCK_COUNT, n + 1);
        },
        |a, d, seen| {
            frames(a, d, seen, 2, |_| false);
            // Down for the frame whose controls read it, then up, as the
            // original's mouse reads give it (its controls act on the
            // next frame's).
            a.input(In::Cursor(160.0 * 3.0 + 1.0, 100.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 10, |a| a.engine.m.mem.r16(DS, 0x061E) != 0);
            frames(a, d, seen, 1, |_| false);
        },
    );
}

/// The view's world drawn offscreen at 320 x 200 (as `shot` draws it),
/// read back: a pixel's colour, RGB.
fn rendered(a: &mut App) -> Option<Vec<[u8; 3]>> {
    let (w, h) = (hud::W as u32, hud::H as u32);
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default(), None)).ok()?;
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = a.assets.renderer(&device, &queue, format);
    let (sky, lights) = a.assets.drawn(&mut a.engine, w as f32 / h as f32);
    let cam = camera(&a.engine);
    let ticks = a.engine.m.bios_ticks as usize;
    let sprites = a.assets.sprites(&mut a.engine, cam, ticks, w as f32 / h as f32);
    r.set_sky(sky);
    r.set_lights(lights);
    r.set_window(interface::window(&a.engine.m));
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("world"),
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    r.draw(&device, &queue, &view, w, h, cam, &sprites);
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("readback"),
        size: (w * h * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo { texture: &target, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
        wgpu::TexelCopyBufferInfo { buffer: &buf, layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) } },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::Maintain::Wait);
    let data = buf.slice(..).get_mapped_range().to_vec();
    Some(data.chunks(4).map(|p| [p[0], p[1], p[2]]).collect())
}

/// The view's 3D world against the original's, measured: with
/// ARENA_SCREENS (gen_worldframes.py --dump's NAME.full, NAME.mask and
/// NAME.dac there) and a GPU, each case's 3D pixels (the mask's 0s) as
/// the view's renderer draws them against the original's page, by colour;
/// printed, with NAME-3d.png (the original's, the view's, and the pixels
/// that differ: grey a near colour, white a far one). A report, not a
/// test of exactness: the view's renderer is its own (polygons, not the
/// original's columns), and nothing here fails on what it measures.
#[test]
fn the_3d_view_measured_against_the_originals() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    type Play = Box<dyn FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)>;
    type Case = (&'static str, Place, Box<dyn FnOnce(&mut App)>, Play);
    let cases: Vec<Case> = vec![
        ("playdungeon", Place::Dungeon, Box::new(|_| {}), Box::new(|a, d, seen| frames(a, d, seen, 3, |_| false))),
        (
            "playdown",
            Place::Dungeon,
            Box::new(|a| at_square(a, 11, 10, 0x100)),
            Box::new(|a, d, seen| {
                a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
                frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0);
                a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
                frames(a, d, seen, 2, |_| false);
            }),
        ),
        (
            "playcrypt",
            Place::Crypt,
            Box::new(|a| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100)),
            Box::new(|a, d, seen| {
                a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
                frames(a, d, seen, 20, |a| a.engine.m.mem.r8(DS, 0x3C82) != 0);
                a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
                frames(a, d, seen, 1, |_| false);
            }),
        ),
    ];
    for (name, place, setup, play) in cases {
        let (Ok(full), Ok(mask), Ok(odac)) = (std::fs::read(dump.join(format!("{name}.full"))), std::fs::read(dump.join(format!("{name}.mask"))), std::fs::read(dump.join(format!("{name}.dac")))) else {
            eprintln!("{name}: no dump");
            continue;
        };
        let Some((mut a, _)) = played(place, setup, play) else { return };
        let Some(px) = rendered(&mut a) else {
            eprintln!("no GPU adapter: nothing measured");
            return;
        };
        // The original's pixel in the view's colours for its index (the
        // view's palette is PAL.COL's 8 bits, the DAC's 6 its top ones),
        // where the DAC holds those colours.
        let pal = a.assets.dac();
        let same_dac = (0..256).all(|k| (0..3).all(|c| pal[k][c] >> 2 == odac[3 * k + c]));
        if !same_dac {
            eprintln!("{name}: the original's DAC isn't the view's palette; colours compared as the DAC's");
        }
        let orig = |i: usize| {
            let k = full[i] as usize;
            if same_dac {
                [pal[k][0], pal[k][1], pal[k][2]]
            } else {
                [odac[3 * k] << 2, odac[3 * k + 1] << 2, odac[3 * k + 2] << 2]
            }
        };
        let dist = |p: [u8; 3], q: [u8; 3]| p.iter().zip(&q).map(|(&x, &y)| (x as i32 - y as i32).abs()).max().unwrap_or(0);
        let (mut n, mut same, mut near) = (0usize, 0usize, 0usize);
        let mut bands = [(0usize, 0usize); 20];
        let mut out = vec![0u8; hud::W * 3 * hud::H * 3];
        for i in 0..hud::W * hud::H {
            let (x, y) = (i % hud::W, i / hud::W);
            let o = orig(i);
            let v = px[i];
            let d = dist(o, v);
            let three = mask[i] == 0;
            if three {
                n += 1;
                bands[y / 10].0 += 1;
                if d == 0 {
                    same += 1;
                    bands[y / 10].1 += 1;
                } else if d <= 16 {
                    near += 1;
                }
            }
            let diff = if !three { [0x40, 0, 0x40] } else if d == 0 { [0, 0, 0] } else if d <= 16 { [0x80, 0x80, 0x80] } else { [0xFF, 0xFF, 0xFF] };
            for (k, c) in [o, v, diff].into_iter().enumerate() {
                let at = (y * hud::W * 3 + k * hud::W + x) * 3;
                out[at..at + 3].copy_from_slice(&c);
            }
        }
        let pct = |k: usize| 100.0 * k as f64 / n.max(1) as f64;
        eprintln!("{name}: {n} 3D pixels, {:.1}% the same colour, {:.1}% near (within 16 a channel)", pct(same), pct(near));
        let rows: Vec<String> = bands.iter().enumerate().filter(|(_, b)| b.0 > 0).map(|(k, b)| format!("{}-{}: {:.0}%", k * 10, k * 10 + 9, 100.0 * b.1 as f64 / b.0 as f64)).collect();
        eprintln!("{name}: same by rows {}", rows.join(", "));
        // The view's picture moved by (dx, dy) against the original's: how
        // much more matches, for an offset in the drawing.
        let mut best = (0i32, 0i32, same);
        for dy in -3i32..=3 {
            for dx in -6i32..=6 {
                let k = (0..hud::W * hud::H)
                    .filter(|&i| {
                        let (x, y) = ((i % hud::W) as i32 + dx, (i / hud::W) as i32 + dy);
                        mask[i] == 0 && (0..hud::W as i32).contains(&x) && (0..hud::H as i32).contains(&y) && px[y as usize * hud::W + x as usize] == orig(i)
                    })
                    .count();
                if k > best.2 {
                    best = (dx, dy, k);
                }
            }
        }
        eprintln!("{name}: best with the view's moved by ({}, {}): {:.1}% the same", best.0, best.1, pct(best.2));
        let file = std::fs::File::create(dump.join(format!("{name}-3d.png"))).expect("create");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), hud::W as u32 * 3, hud::H as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().expect("png").write_image_data(&out).expect("png");
    }
}

type Play = Box<dyn FnOnce(&mut App, &mut Driver, &mut Vec<[u16; 5]>)>;
type Case = (Place, Box<dyn FnOnce(&mut App)>, Play);

/// The cases' set-ups and plays, by name (as the tests above play them).
fn case(name: &str) -> Option<Case> {
    let held = |n: usize, until: fn(&App) -> bool, after: usize| -> Play {
        Box::new(move |a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            a.input(In::Key { code: KeyCode::ArrowUp, down: true, text: None });
            frames(a, d, seen, n, until);
            a.input(In::Key { code: KeyCode::ArrowUp, down: false, text: None });
            frames(a, d, seen, after, |_| false);
        })
    };
    Some(match name {
        "playdungeon" => (Place::Dungeon, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playdown" => (Place::Dungeon, Box::new(|a: &mut App| at_square(a, 11, 10, 0x100)), held(20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0, 2)),
        "playsky" => (Place::Street, Box::new(|a: &mut App| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playnight" => (Place::Street, Box::new(|a: &mut App| night(a, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playstreet" => (Place::Street, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playcrypt" => (Place::Crypt, Box::new(|a: &mut App| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0x100)), held(20, |a| a.engine.m.mem.r8(DS, 0x3C82) != 0, 1)),
        _ => return None,
    })
}

/// The engine's own drawing of the 3D view (`render::draw`, the page it
/// keeps) against the original's page, from gen_worldframes.py --dump's
/// NAME.full and NAME.mask in ARENA_SCREENS: each case's 3D pixels (the
/// mask's 0s) counted alike, the first that differ printed, and
/// NAME-soft.png (the original's, the engine's, white where they
/// differ).
#[test]
fn the_3d_view_drawn_as_the_original_draws_it() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    let only = std::env::var("ARENA_CASE").ok();
    for name in ["playdungeon", "playdown", "playcrypt", "playstreet", "playsky", "playnight"] {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let (Ok(full), Ok(mask)) = (std::fs::read(dump.join(format!("{name}.full"))), std::fs::read(dump.join(format!("{name}.mask")))) else {
            eprintln!("{name}: no dump");
            continue;
        };
        let (place, setup, play) = case(name).unwrap();
        let Some((a, _)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a)
        }, play) else { return };
        let page = a.engine.page.clone().unwrap();
        let (mut n, mut same) = (0, 0);
        let mut shown = 0;
        let mut out = vec![0u8; 960 * 200 * 3];
        let pal = a.assets.dac();
        for i in 0..64000 {
            let three = mask[i] == 0;
            let ok = full[i] == page[i];
            if three {
                n += 1;
                if ok {
                    same += 1;
                } else if shown < 12 {
                    eprintln!("{name}: ({}, {}) original {:02x}, engine {:02x}", i % 320, i / 320, full[i], page[i]);
                    shown += 1;
                }
            }
            let (x, y) = (i % 320, i / 320);
            let diff = if !three { [0x40, 0, 0x40, 0] } else if ok { [0, 0, 0, 0] } else { [0xFF; 4] };
            for (k, c) in [pal[full[i] as usize], pal[page[i] as usize], diff].into_iter().enumerate() {
                let at = (y * 960 + k * 320 + x) * 3;
                out[at..at + 3].copy_from_slice(&c[..3]);
            }
        }
        let bad: Vec<String> = (0..200)
            .filter_map(|y| {
                let k = (0..320).filter(|&x| mask[y * 320 + x] == 0 && full[y * 320 + x] != page[y * 320 + x]).count();
                (k > 0).then(|| format!("{y}:{k}"))
            })
            .collect();
        eprintln!("{name}: rows differing {}", bad.join(" "));
        let _ = std::fs::write(dump.join(format!("{name}.soft")), &page);
        if let Ok(r) = std::env::var("ARENA_ROW") {
            let y: usize = r.parse().unwrap();
            let hx = |v: &[u8]| v.iter().map(|b| format!("{b:02x}")).collect::<String>();
            eprintln!("{name}: row {y} original {}", hx(&full[y * 320..y * 320 + 320]));
            eprintln!("{name}: row {y} engine   {}", hx(&page[y * 320..y * 320 + 320]));
        }
        eprintln!("{name}: {same} of {n} 3D pixels the original's ({:.2}%); not ported: {:?}", 100.0 * same as f64 / n.max(1) as f64, a.engine.page_gaps);
        let file = std::fs::File::create(dump.join(format!("{name}-soft.png"))).expect("create");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), 960, 200);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().expect("png").write_image_data(&out).expect("png");
    }
}

/// The engine's drawing of the case's 3D view (`render::draw` each frame,
/// the level's lights as the original has them: see below) and the 3D
/// pixels of it (where the view's own layer draws nothing): their digest
/// against the original's (`view3d`), and the share of them alike where
/// ARENA_SCREENS has the original's page.
fn soft_view(name: &str) -> Option<(bool, Option<f64>)> {
    let (place, setup, play) = case(name)?;
    let (mut a, _) = played(
        place,
        |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a)
        },
        play,
    )?;
    let page = a.engine.page.take().unwrap();
    let (idx, _) = layer(&mut a);
    let three: Vec<u8> = page.iter().zip(&idx).map(|(&c, &i)| if i >= 0x100 { c } else { 0 }).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == "view3d" && f[1] == name).map(|f| u64::from_str_radix(f[2], 16).unwrap());
    let same = std::env::var("ARENA_SCREENS").ok().and_then(|d| {
        let full = std::fs::read(PathBuf::from(&d).join(format!("{name}.full"))).ok()?;
        let n = idx.iter().filter(|&&i| i >= 0x100).count();
        let k = (0..64000).filter(|&i| idx[i] >= 0x100 && full[i] == page[i]).count();
        Some(100.0 * k as f64 / n.max(1) as f64)
    });
    Some((want == Some(fnv(three.iter().copied())), same))
}

/// The dungeon's first frames' 3D view, and the second level's after the
/// stairs, every pixel as the original's renderer draws them.
#[test]
fn the_dungeons_3d_view_drawn_as_the_original_draws_it() {
    for name in ["playdungeon", "playdown"] {
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
    }
}

/// The street outside the guild by day: the sky's rows, the water's
/// frame under the floor, the floor and the walls unshaded, the trees,
/// every pixel as the original's renderer draws them.
#[test]
fn the_streets_3d_view_drawn_as_the_original_draws_it() {
    let Some((ok, same)) = soft_view("playstreet") else { return };
    assert!(ok, "playstreet: the 3D view differs from the original's ({same:?}% alike)");
}

/// Measured, not yet exact: the crypt (its farthest ceiling row reads
/// past the row table, where the original finds what an earlier area
/// left in its heap; and five rows of its ceiling differ), the street
/// facing open sky (a few of the sky's objects' pixels) and the street
/// at night (the shaded paths by the lamps' lights).
#[test]
fn the_crypts_3d_view_measured() {
    for name in ["playcrypt", "playsky", "playnight"] {
        if let Some((ok, same)) = soft_view(name) {
            eprintln!("{name}: {} ({same:?}% alike)", if ok { "exact" } else { "differs" });
        }
    }
}

