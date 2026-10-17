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
    // (In u32: 22 x 60 shifted left by 10 is past a u16.)
    m.mem.w16(DS, SKY_TIME, (((22 * 60) << 10) / 1440u32) as u16);
}

/// `night(heading)` in a thunderstorm with a bolt forced at the circle's
/// `angle`, as gen_worldframes.py's `storm` sets it: the weather rain
/// (BB09 1), 5C8 bit 1, the first bolt's image (D8AE) at its frame 0
/// (43C6), its place 2710h times the sine table's words (B03C, B13C),
/// the high word doubled.
fn storm(a: &mut App, heading: u16, angle: u16) {
    night(a, heading);
    let m = &mut a.engine.m;
    m.mem.w8(DS, 0xBB09, 1);
    let v = m.mem.r16(DS, 0x05C8);
    m.mem.w16(DS, 0x05C8, v | 1);
    let image = m.mem.r16(DS, 0xD8AE);
    m.mem.w16(DS, 0x43C8, image);
    m.mem.w16(DS, 0x43C6, 0);
    for (at, table) in [(0x43C2u16, 0xB03Cu16), (0x43C4, 0xB13C)] {
        let v = m.mem.r16(DS, table + 2 * angle) as i16 as i32;
        m.mem.w16(DS, at, ((((0x2710 * v) >> 16) * 2) & 0xFFFF) as u16);
    }
}

/// Facing `heading`, the location moved to province 3 at (84h + `dx`,
/// 34h + `dy`) and the sky's objects made again (the counts cleared
/// first, as 0000:027A clears them), as gen_worldframes.py's `volcanic`
/// sets it.
fn volcanic(a: &mut App, heading: u16, dx: u16, dy: u16) {
    use arena::skyobj::var::{CLOUD_COUNT, OBJECT_COUNT};
    let m = &mut a.engine.m;
    m.mem.w8(DS, 0x0A46, 3);
    m.mem.w16(DS, 0x0A48, 0x84 + dx);
    m.mem.w16(DS, 0x0A4A, 0x34 + dy);
    m.mem.w8(DS, OBJECT_COUNT, 0);
    m.mem.w8(DS, CLOUD_COUNT, 0);
    arena::light::LightHooks::sky_objects(&mut a.assets.light, &mut a.engine.m);
    a.engine.m.mem.w16(DS, arena::movement::var::HEADING, heading);
}

/// Where a case is played: the dungeon `into_dungeon` makes, or the main
/// quest's Crypt of Hearts (`into_crypt`).
#[derive(Clone, Copy)]
enum Place {
    Dungeon,
    Crypt,
    Street,
    /// The street with its zone's weather rain (`wet_street`), or snow.
    WetStreet,
    SnowStreet,
    Wild,
    /// The main quest dungeon 39699021.MIF, whose start sees a square's
    /// cap.
    Capped,
    /// The main quest dungeon 40516921.MIF, with fog.
    Fogged,
}

/// From the guild to the street, then into the level `name` the way the
/// outer loop enters a named level (0000:0993): the next area 2, no
/// interior, its .MIF named at 0A36.
fn into_named(a: &mut App, name: &[u8]) {
    area_left(a);
    let m = &mut a.engine.m;
    m.mem.w8(DS, arena::trigger::var::AREA, 2);
    m.mem.w16(DS, arena::transit::var::INTERIOR, 0);
    for (i, &c) in name.iter().chain(b"\0").enumerate() {
        m.mem.w8(DS, 0x0A36 + i as u16, c);
    }
    area_left(a);
}

/// From the guild to the street, then out of the city's gate the way
/// its trigger leaves (0000:43A0): the city written as the wilderness's
/// blocks 1..4 (217B:027A, `wild_leave_city`), the next area 1.
fn into_wild(a: &mut App) {
    area_left(a);
    let e = &mut a.engine;
    e.m.wild_leave_city(&mut e.files, &mut arena::wild::PortWild);
    e.m.mem.w8(DS, arena::trigger::var::AREA, 1);
    area_left(a);
}

/// The random seed of gen_worldframes.py's game after its load from the
/// save (the harness's BIOS ticks at 21EE:0059, 0000:6FC2's seed, run on
/// through the title and the load).
const HARNESS_SEED: u32 = 0x19B7_6141;

thread_local! {
    /// The state as `played`'s load left it, before the place's entry
    /// (for `the_state_measured_against_the_originals`).
    static BASE: std::cell::RefCell<Option<(Vec<u8>, Vec<u8>)>> = const { std::cell::RefCell::new(None) };
}

/// DGROUP and the EMS banks 0..1Fh.
fn grab(a: &App) -> (Vec<u8>, Vec<u8>) {
    // The random seed as the game's memory would hold it (the engine
    // keeps it in `rng` between its stores).
    // DGROUP with the drawing's writes on its copy as the original's,
    // drawing on the machine, has them (`Engine::page_state`).
    let mut e = arena::engine::Engine::new(a.engine.m.clone(), a.engine.files.clone());
    e.m.store_rng();
    e.page_wrote = a.engine.page_wrote.clone();
    (e.page_state(), (0..0x20).flat_map(|b| e.m.ems_bank(b)).collect())
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
    // The random seed as the harness's game has it after its load: the
    // original seeds it from the BIOS ticks (21EE:0059 -> 0000:6FC2), so
    // a game run in the harness has the seed its clock gives.
    a.engine.m.rng.seed = HARNESS_SEED;
    a.engine.m.store_rng();
    let mut d = Driver::new(&mut a);
    clear(&mut d, &mut a);
    BASE.with(|b| *b.borrow_mut() = Some(grab(&a)));
    match place {
        Place::Dungeon => into_dungeon(&mut a),
        Place::Crypt => into_named(&mut a, b"41649375.MIF"),
        Place::Capped => into_named(&mut a, b"39699021.MIF"),
        Place::Fogged => into_named(&mut a, b"40516921.MIF"),
        Place::Street => area_left(&mut a),
        Place::WetStreet | Place::SnowStreet => {
            // gen_worldframes.py's `weathered`: the zone's weather (A5A
            // + the zone at A51) rain (2) or snow (3) as the guild is left.
            let w = if matches!(place, Place::WetStreet) { 2 } else { 3 };
            let zone = a.engine.m.mem.r8(DS, arena::weather::var::ZONE) as u16;
            a.engine.m.mem.w8(DS, arena::weather::var::ZONE_WEATHER + zone, w);
            area_left(&mut a)
        }
        Place::Wild => into_wild(&mut a),
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

/// Inside the main quest's dungeon 39699021.MIF, its first frames,
/// nothing held.
#[test]
fn play_in_a_main_quest_dungeon_as_the_original_draws_it() {
    let (place, setup, play) = case("playcap").unwrap();
    world_case("playcap", place, setup, play);
}

/// Inside the main quest's dungeon 40516921.MIF, with fog, its first
/// frames, nothing held.
#[test]
fn play_in_fog_as_the_original_draws_it() {
    let (place, setup, play) = case("playfog").unwrap();
    world_case("playfog", place, setup, play);
}

/// Out of the city's gate into the wilderness, its first frames, nothing
/// held.
#[test]
fn play_in_the_wilderness_as_the_original_draws_it() {
    let (place, setup, play) = case("playwild").unwrap();
    world_case("playwild", place, setup, play);
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

/// gen_worldframes.py's `ghost`: a ghost (type Ch, remapped: drawn
/// through the light table, 0000:953E) brought by a quest's encounter
/// (1194:89A7, one at level 1), put 100h north of the start and faced.
fn ghost(a: &mut App) {
    use arena::encounter::var::CREATURE_SLOTS;
    let e = &mut a.engine;
    e.m.quest_encounter(&mut e.files, &mut arena::encounter::NoFightHooks, 1, 0xC, 1).expect("a ghost brought");
    let (x, z) = (e.m.mem.r16(DS, 0xC3D0), e.m.mem.r16(DS, 0xC3D4));
    e.m.mem.w16(DS, CREATURE_SLOTS + 4, z.wrapping_add(0x100));
    e.m.mem.w16(DS, CREATURE_SLOTS, x);
    e.m.mem.w16(DS, arena::movement::var::HEADING, 0);
}

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
        "playghost" => (Place::Dungeon, Box::new(ghost), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
        "playghostclick" => (Place::Dungeon, Box::new(ghost), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| {
            // The harness's click is read by the next frame's controls
            // (its input step, after that frame's drawing): the window's
            // turn takes it before a frame, as that step's.
            frames(a, d, seen, 2, |_| false);
            // The harness's mouse at (160, 100): the click 4 on (CB60).
            a.input(In::Cursor(164.0 * 3.0 + 1.0, 104.0 * 3.0 + 1.0));
            a.input(In::Button { button: MouseButton::Left, down: true });
            frames(a, d, seen, 1, |_| false);
            a.input(In::Button { button: MouseButton::Left, down: false });
            frames(a, d, seen, 10, |a| a.engine.m.mem.r16(DS, 0x061E) != 0);
            frames(a, d, seen, 1, |_| false);
        })),
        "playdown" => (Place::Dungeon, Box::new(|a: &mut App| at_square(a, 11, 10, 0x100)), held(20, |a| a.engine.m.mem.r8(DS, arena::movement::var::LEVEL) != 0, 2)),
        "playsky" => (Place::Street, Box::new(|a: &mut App| a.engine.m.mem.w16(DS, arena::movement::var::HEADING, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playnight" => (Place::Street, Box::new(|a: &mut App| night(a, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playbolt" => (Place::Street, Box::new(|a: &mut App| storm(a, 0, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
        "playfog" => (Place::Fogged, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playfogfirst" => (Place::Fogged, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 0, |_| false))),
        "playcap" => (Place::Capped, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playrain" => (Place::WetStreet, Box::new(|a: &mut App| at_square(a, 89, 41, 0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playsnow" => (Place::SnowStreet, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playwild" => (Place::Wild, Box::new(|_: &mut App| {}), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 3, |_| false))),
        "playvolcano" => (Place::Wild, Box::new(|a: &mut App| volcanic(a, 0x100, 0, 0x20)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
        "playvolcanofar" => (Place::Wild, Box::new(|a: &mut App| volcanic(a, 0x100, 0, 0xA0)), Box::new(|a: &mut App, d: &mut Driver, seen: &mut Vec<[u16; 5]>| frames(a, d, seen, 1, |_| false))),
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
    for name in ["playdungeon", "playdown", "playcrypt", "playstreet", "playsky", "playnight", "playwild", "playcap", "playfog", "playghost"] {
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
    soft_view_from(name, 0)
}

/// `soft_view` from row `top` on (`view3d1`: from row 1, the rows above
/// left 0).
fn soft_view_from(name: &str, top: usize) -> Option<(bool, Option<f64>)> {
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
    let three: Vec<u8> = page.iter().zip(&idx).enumerate().map(|(k, (&c, &i))| if i >= 0x100 && k >= top * 320 { c } else { 0 }).collect();
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let kind = if top == 0 { "view3d" } else { "view3d1" };
    let want = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == kind && f[1] == name).map(|f| u64::from_str_radix(f[2], 16).unwrap());
    let same = std::env::var("ARENA_SCREENS").ok().and_then(|d| {
        let full = std::fs::read(PathBuf::from(&d).join(format!("{name}.full"))).ok()?;
        let n = idx.iter().filter(|&&i| i >= 0x100).count();
        let k = (0..64000).filter(|&i| idx[i] >= 0x100 && full[i] == page[i]).count();
        Some(100.0 * k as f64 / n.max(1) as f64)
    });
    Some((want == Some(fnv(three.iter().copied())), same))
}

/// The dungeon's first frames' 3D view, the second level's after the
/// stairs, and two of the main quest's dungeons' (one with fog: the view
/// filled to the horizon with its colour, 2294:0B4D, the fog's light
/// table's last rows the fill colour, 0C36), every pixel as the
/// original's renderer draws them.
#[test]
fn the_dungeons_3d_view_drawn_as_the_original_draws_it() {
    for name in ["playdungeon", "playdown", "playcap", "playfog", "playfogfirst", "playghost"] {
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
    }
}

/// The street outside the guild by day (the sky's rows, the water's
/// frame under the floor, the floor and the walls unshaded, the trees,
/// the townspeople in their colours), turned to the open sky (a cloud,
/// a lamp post, people far off) and at ten at night (the afternoon's
/// dither, the textures relit by the night's row of the light table),
/// every pixel as the original's renderer draws them.
#[test]
fn the_streets_3d_view_drawn_as_the_original_draws_it() {
    for name in ["playstreet", "playsky", "playnight", "playbolt", "playrain", "playsnow"] {
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
    }
}

/// The wilderness out of the city's gate (its fields, the sky), every
/// pixel as the original's renderer draws them.
#[test]
fn the_wildernesss_3d_view_drawn_as_the_original_draws_it() {
    for name in ["playwild", "playvolcano", "playvolcanofar"] {
        let Some((ok, same)) = soft_view(name) else { return };
        assert!(ok, "{name}: the 3D view differs from the original's ({same:?}% alike)");
    }
}

/// The crypt (the ceiling's dark runs stepped by the EMS frame's segment,
/// CCCE: 0000:D865's EDX bug, the frame at F000h as the harness's), every
/// pixel as the original's renderer draws them from row 1 on. Its topmost
/// ceiling row reads one entry past the row table (DB78's), the next heap
/// block's header in the original, whose words are its heap's segments
/// (7A33h, 78FFh there): no other layout has them, so row 0 is left out
/// (`view3d1`).
#[test]
fn the_crypts_3d_view_drawn_as_the_original_draws_it() {
    let Some((ok, same)) = soft_view_from("playcrypt", 1) else { return };
    assert!(ok, "playcrypt: the 3D view from row 1 differs from the original's ({same:?}% alike)");
}


/// Byte ranges where `a` and `b` differ, runs closer than `gap` joined.
fn differing(a: &[u8], b: &[u8], gap: usize) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = Vec::new();
    for i in (0..a.len().min(b.len())).filter(|&i| a[i] != b[i]) {
        match out.last_mut() {
            Some(r) if i - r.1 <= gap => r.1 = i + 1,
            _ => out.push((i, i + 1)),
        }
    }
    out
}

/// The game's state as each case leaves it, the engine's against the
/// original's (gen_worldframes.py --dump's NAME.ds and NAME.ems in
/// ARENA_SCREENS): DGROUP and the EMS banks, the ranges that differ
/// printed. A report: what a frame stage the view skips leaves behind.
#[test]
fn the_state_measured_against_the_originals() {
    let Some(dump) = std::env::var("ARENA_SCREENS").ok().map(PathBuf::from) else { return };
    let only = std::env::var("ARENA_CASE").ok();
    for name in ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playdown", "playcrypt", "playfog", "playghostclick"] {
        if only.as_deref().is_some_and(|o| o != name) {
            continue;
        }
        let (Ok(ds), Ok(ems)) = (std::fs::read(dump.join(format!("{name}.ds"))), std::fs::read(dump.join(format!("{name}.ems")))) else {
            eprintln!("{name}: no dump");
            continue;
        };
        // From the load (ARENA_FROM=load: the places' entries counted
        // too) or from the case's start.
        let from_load = std::env::var("ARENA_FROM").is_ok_and(|v| v == "load");
        let (f_ds, f_ems) = if from_load { ("base.ds".to_string(), "base.ems".to_string()) } else { (format!("{name}.ds0"), format!("{name}.ems0")) };
        let (Ok(ds0), Ok(ems0)) = (std::fs::read(dump.join(f_ds)), std::fs::read(dump.join(f_ems))) else {
            eprintln!("{name}: no dump of the start");
            continue;
        };
        let (place, setup, play) = case(name).unwrap();
        let pre = std::cell::RefCell::new(None);
        let Some((a, _)) = played(place, |a| {
            a.engine.page = Some(vec![0; 64000]);
            setup(a);
            *pre.borrow_mut() = Some(grab(a));
        }, play) else { return };
        let (mine0, banks0) = if from_load { BASE.with(|b| b.borrow_mut().take()).unwrap() } else { pre.into_inner().unwrap() };
        let (mine, banks) = grab(&a);
        // (ARENA_MINE: the engine's DGROUP as the case leaves it, and as
        // the load left it, beside the original's dumps.)
        if std::env::var("ARENA_MINE").is_ok() {
            let _ = std::fs::write(dump.join(format!("{name}.mine")), &mine);
            let _ = std::fs::write(dump.join(format!("{name}.mine0")), &mine0);
        }
        // What each side's frames changed: a byte the original's changed
        // and the engine's didn't (or the other way), or both changed it
        // to different values where they started alike.
        let drawn: Vec<u8> = a.engine.page_wrote.iter().map(|w| u8::from(w.is_some())).collect();
        let report = |what: &str, o0: &[u8], o1: &[u8], m0: &[u8], m1: &[u8], gap: usize, unit: usize| {
            let flag: Vec<u8> = (0..o0.len()).map(|i| {
                // Where they end differently and one side's frames
                // changed it and the other's didn't, or both started alike.
                let (oc, mc) = (o0[i] != o1[i], m0[i] != m1[i]);
                u8::from(o1[i] != m1[i] && (oc || mc) && (oc != mc || o0[i] == m0[i]))
            }).collect();
            let zero = vec![0u8; flag.len()];
            let r = differing(&flag, &zero, gap);
            eprintln!("{name}: {what} {} ranges", r.len());
            let hx = |v: &[u8]| v.iter().take(12).map(|b| format!("{b:02x}")).collect::<String>();
            for &(s, e) in r.iter().take(80) {
                // The drawing's copy wrote there (DGROUP only).
                let tag = if unit == 0x10000 && what == "DGROUP" && (s..e).all(|i| flag[i] == 0 || drawn[i] != 0) { " (the drawing's)" } else { "" };
                eprintln!("  {:02x}:{:04x}..{:04x} orig {}->{} mine {}->{}{tag}", s / unit, s % unit, s % unit + (e - s), hx(&o0[s..e]), hx(&o1[s..e]), hx(&m0[s..e]), hx(&m1[s..e]));
            }
        };
        report("DGROUP", &ds0, &ds, &mine0, &mine, 4, 0x10000);
        if std::env::var("ARENA_STRICT").is_ok() {
            // The changes as sets: changed on one side only, or on both to
            // different values (the drawing's copy's writes left out).
            let flag: Vec<u8> = (0..0x10000).map(|i| {
                let (oc, mc) = (ds0[i] != ds[i], mine0[i] != mine[i]);
                u8::from(drawn[i] == 0 && (oc != mc || (oc && ds[i] != mine[i])))
            }).collect();
            for &(s, e) in differing(&flag, &vec![0u8; 0x10000], 4).iter().take(80) {
                let hx = |v: &[u8]| v.iter().take(12).map(|b| format!("{b:02x}")).collect::<String>();
                eprintln!("  strict {s:04x}..{e:04x} orig {}->{} mine {}->{}", hx(&ds0[s..e]), hx(&ds[s..e]), hx(&mine0[s..e]), hx(&mine[s..e]));
            }
        }
        report("EMS", &ems0, &ems, &banks0, &banks, 32, 0x10000);
    }
}

/// tests/vectors/stateexclude.txt's DGROUP offsets (`state` lines leave
/// them out).
fn state_excluded() -> Vec<bool> {
    let mut out = vec![false; 0x10000];
    excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/stateexclude.txt"), &mut out);
    out
}

/// stateexclude.txt's and stateloadexclude.txt's DGROUP offsets
/// (`stateload` lines leave them out).
fn state_load_excluded() -> Vec<bool> {
    let mut out = state_excluded();
    excluded_in(concat!(env!("CARGO_MANIFEST_DIR"), "/../tests/vectors/stateloadexclude.txt"), &mut out);
    out
}

/// The ranges a state exclusion file names, marked in `out`.
fn excluded_in(path: &str, out: &mut [bool]) {
    for line in std::fs::read_to_string(path).expect(path).lines() {
        let f: Vec<&str> = line.split('#').next().unwrap_or("").split_whitespace().collect();
        if f.len() >= 2 {
            let (s, e) = (usize::from_str_radix(f[0], 16).unwrap(), usize::from_str_radix(f[1], 16).unwrap());
            out[s..e].iter_mut().for_each(|x| *x = true);
        }
    }
}

/// The case's frames' changes to DGROUP against the original's (`state`
/// lines: the offsets it changed, the digest of their bytes after), the
/// drawing's writes on its copy counted as the original's drawing makes
/// them on the machine (`Engine::page_state`): the same offsets changed,
/// to the same bytes. Where a stage the view skips leaves the game's
/// state other than the original's, this fails.
fn state_case(name: &str) {
    state_compared(name, false);
}

/// `state_case` from the load (`stateload` lines): the place's way in
/// counted too, against the state `played`'s load left (`BASE`).
fn state_load_case(name: &str) {
    state_compared(name, true);
}

fn state_compared(name: &str, from_load: bool) {
    let kind = if from_load { "stateload" } else { "state" };
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let Some(f) = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 4 && f[0] == kind && f[1] == name) else {
        panic!("{name}: no {kind} line");
    };
    let mut want = vec![false; 0x10000];
    for run in f[2].split_whitespace() {
        let (s, n) = run.split_once('+').unwrap();
        let (s, n) = (usize::from_str_radix(s, 16).unwrap(), usize::from_str_radix(n, 16).unwrap());
        want[s..s + n].iter_mut().for_each(|x| *x = true);
    }
    let digest = u64::from_str_radix(f[3], 16).unwrap();
    let skip = if from_load { state_load_excluded() } else { state_excluded() };
    let (place, setup, play) = case(name).unwrap();
    let pre = std::cell::RefCell::new(None);
    let Some((a, _)) = played(place, |a| {
        a.engine.page = Some(vec![0; 64000]);
        setup(a);
        *pre.borrow_mut() = Some(grab(a).0);
    }, play) else { return };
    let before = if from_load { BASE.with(|b| b.borrow_mut().take()).unwrap().0 } else { pre.into_inner().unwrap() };
    let after = grab(&a).0;
    // (The drawing's own setting up, the original's done before the case
    // starts, the engine's copy's at its first page: not counted.)
    let extra: Vec<usize> = (0..0x10000).filter(|&i| before[i] != after[i] && !skip[i] && !want[i] && a.engine.page_wrote[i].is_none()).collect();
    let got = fnv((0..0x10000).filter(|&i| want[i]).map(|i| after[i]));
    let hx = |v: &[usize]| v.iter().take(24).map(|i| format!("{i:04x}")).collect::<Vec<_>>().join(" ");
    // From the load, the engine's load and the original's already part
    // (some 1100 bytes: the program's start, the heap, scratch), so a byte
    // the engine changes and the original doesn't may be the engine's
    // setting it as the original's load had it: those are checked against
    // a bit of each of the original's bytes as the case leaves them
    // (`stateloadbits`; its bytes are the save's, never in the vectors).
    let extra = if from_load {
        let Some(bits) = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 3 && f[0] == "stateloadbits" && f[1] == name).map(|f| f[2].to_string()) else {
            panic!("{name}: no stateloadbits line");
        };
        let bit = |i: usize| (u8::from_str_radix(&bits[(i >> 3) * 2..(i >> 3) * 2 + 2], 16).unwrap() >> (i & 7)) & 1;
        extra.into_iter().filter(|&i| (fnv([after[i], i as u8, (i >> 8) as u8]) >> 63) as u8 != bit(i)).collect()
    } else {
        extra
    };
    assert!(extra.is_empty(), "{name}: the frames changed what the original's don't: {}", hx(&extra));
    let unchanged: Vec<usize> = (0..0x10000).filter(|&i| want[i] && before[i] == after[i]).collect();
    assert_eq!(got, digest, "{name}: the state differs from the original's (of its changes, unchanged here: {})", hx(&unchanged));
}

/// Each case's frames change the EMS banks 0..1Fh as the original's do
/// (`stateems` lines: the same bytes changed, to the same values).
fn state_ems_case(name: &str) {
    let text = std::fs::read_to_string(VECTORS).expect(VECTORS);
    let Some(f) = text.lines().map(|l| l.split(" | ").collect::<Vec<_>>()).find(|f| f.len() == 4 && f[0] == "stateems" && f[1] == name) else {
        panic!("{name}: no stateems line");
    };
    let (count, offs) = f[2].split_once(' ').unwrap();
    let want = (usize::from_str_radix(count, 16).unwrap(), u64::from_str_radix(offs, 16).unwrap(), u64::from_str_radix(f[3], 16).unwrap());
    let (place, setup, play) = case(name).unwrap();
    let pre = std::cell::RefCell::new(None);
    let Some((a, _)) = played(place, |a| {
        a.engine.page = Some(vec![0; 64000]);
        setup(a);
        *pre.borrow_mut() = Some(grab(a).1);
    }, play) else { return };
    let before = pre.into_inner().unwrap();
    let after = grab(&a).1;
    let changed: Vec<usize> = (0..after.len()).filter(|&i| before[i] != after[i]).collect();
    let got = (changed.len(), fnv(changed.iter().flat_map(|&i| (i as u32).to_le_bytes())), fnv(changed.iter().map(|&i| after[i])));
    assert_eq!(got, want, "{name}: the EMS banks' changes differ from the original's ({} bytes changed here)", changed.len());
}

/// Each case's frames change the EMS banks as the original's do
/// (`state_ems_case`). Not the stairs' (playdown): three bytes the
/// original's frames change already hold their value as the engine's
/// case starts (no byte ends otherwise, the report finds).
#[test]
fn the_ems_banks_as_the_original_leaves_them() {
    for name in ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playcrypt", "playfog", "playcap", "playsky"] {
        state_ems_case(name);
    }
}

/// From the load, the way into each case's place and its frames leave
/// the game's state as the original's do (`state_load_case`).
#[test]
fn the_state_from_the_load_as_the_original_leaves_it() {
    for name in ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playdown", "playcrypt", "playfog", "playcap", "playsky"] {
        state_load_case(name);
    }
}

/// Each case's frames leave the game's state as the original's do
/// (`state_case`): the street by day, at night, in rain and snow, the
/// wilderness, a dungeon and its stairs, the crypt, fog.
#[test]
fn the_state_as_the_original_leaves_it() {
    for name in ["playstreet", "playnight", "playrain", "playsnow", "playwild", "playdungeon", "playdown", "playcrypt", "playfog", "playcap", "playsky", "playghostclick"] {
        state_case(name);
    }
}
