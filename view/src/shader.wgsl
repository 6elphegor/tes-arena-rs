// The world: textures are palette indices (an array of 64 x 64 layers),
// shaded by distance through the original's shading table (16 rows of
// 256 indices, darkest last), then coloured by the palette.

struct Globals {
    view_proj: mat4x4<f32>,
    // xyz: the eye; w: the rows a light behind the wall drawn loses
    // (BB02).
    eye: vec4<f32>,
    // x: distance per shading row, y: rows (darkest kept), z: the row
    // every index is remapped through first (night outdoors; 0 none), w:
    // the fog's colour plus one (0 none), filling the view for the sky.
    shade: vec4<f32>,
    // x: outdoors (a sky), y: afternoon (the second dither), z, w: the
    // target's size.
    sky: vec4<f32>,
    // The lights (x, y, radius; 0000:B6EB): x the count (0: full daylight,
    // no shading), y the limit row, z the row past it, w the row beyond a
    // light (C8E7, C8E9, C8EB).
    light: vec4<f32>,
    lights: array<vec4<f32>, 64>,
    palette: array<vec4<f32>, 256>,
};

@group(0) @binding(0) var<uniform> g: Globals;
@group(0) @binding(1) var textures: texture_2d_array<u32>;
@group(0) @binding(2) var shading: texture_2d<u32>;
@group(0) @binding(3) var sprites: texture_2d<u32>;
// DITHER.IMG (rows 0..19) and DITHER2.IMG (20..39): 40 x 20 indices 1..8.
@group(0) @binding(4) var dither: texture_2d<u32>;

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(2) world: vec3<f32>,
    @location(3) light: f32,
    @location(4) @interpolate(flat) flags: u32,
};

@vertex
fn vs_main(
    @location(0) pos: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) layer: u32,
    @location(3) light: f32,
    @location(4) flags: u32,
) -> VsOut {
    var o: VsOut;
    o.clip = g.view_proj * vec4<f32>(pos, 1.0);
    if (flags & 16u) != 0u {
        // The sky's objects: already on the screen, behind everything.
        o.clip = vec4<f32>(pos.xy, 0.999999, 1.0);
    }
    o.uv = uv;
    o.layer = layer;
    o.world = pos;
    o.light = light;
    o.flags = flags;
    return o;
}

@fragment
fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
    var index: u32;
    if (i.flags & 1u) != 0u {
        // A flat: texels in the atlas, index 0 see-through.
        index = textureLoad(sprites, vec2<i32>(floor(i.uv)), 0).r;
        if index == 0u {
            discard;
        }
    } else {
        let t = vec2<i32>(floor(i.uv)) & vec2<i32>(63, 63);
        index = textureLoad(textures, t, i32(i.layer), 0).r;
    }
    if g.shade.z > 0.0 {
        index = textureLoad(shading, vec2<i32>(i32(index), i32(g.shade.z)), 0).r;
    }
    var row = 0;
    if (i.flags & 16u) == 0u {
        row = light_row(vec2<i32>(floor(i.world.xz)), (i.flags >> 1u) & 7u, vec2<i32>(round(i.world.xz)));
    }
    let shaded = textureLoad(shading, vec2<i32>(i32(index), row), 0).r;
    return g.palette[shaded];
}

// The sky: palette entries 1..8 through the dither, its 20 rows from the
// top of the view to the horizon (the middle), 40 columns repeating
// across a view 320 wide; below the horizon its last row. Black indoors.
@vertex
fn vs_sky(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    return vec4<f32>(p * 2.0 - 1.0, 1.0, 1.0);
}

@fragment
fn fs_sky(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    if g.sky.x == 0.0 {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    if g.shade.w > 0.0 {
        return g.palette[u32(g.shade.w) - 1u];
    }
    let row = clamp(i32(pos.y / (g.sky.w * 0.5) * 20.0), 0, 19) + select(0, 20, g.sky.y != 0.0);
    let col = i32(pos.x * 320.0 / g.sky.z) % 40;
    let index = textureLoad(dither, vec2<i32>(col, row), 0).r;
    return g.palette[index];
}

// 0000:B6EB (src/light.rs `light_row`): the brightest row any light gives
// the point, in the game's integer arithmetic: the distance is the larger
// axis plus a quarter of the smaller; within half the radius row 0, then
// one more every (radius / 2) / 13 units; beyond the radius, or at the
// limit, the far row; the result at the limit or past it the dark row.
// On a wall face (`side` its side plus one, 0000:B7A9; `plane` the
// fragment rounded, its plane's coordinate on its axis) a light behind
// it is BB02 rows darker, the dark row at the limit or past it. (The
// original's near and moving lists, kept apart there, are one here.)
fn light_row(p: vec2<i32>, side: u32, plane: vec2<i32>) -> i32 {
    let n = i32(g.light.x);
    if n == 0 {
        return 0;
    }
    let limit = i32(g.light.y);
    let far = i32(g.light.w);
    var darkest = 14;
    for (var k = 0; k < n; k++) {
        let l = g.lights[k];
        let a = abs(p.x - i32(l.x));
        let b = abs(p.y - i32(l.y));
        let d = (min(a, b) >> 2u) + max(a, b);
        let r = i32(l.z);
        var row = far;
        if d <= r {
            let half = r >> 1u;
            if d < half {
                row = 0;
            } else {
                let step = max(half / 13, 1);
                row = (d - half) / step + 1;
                if row >= limit {
                    row = far;
                }
            }
        }
        if side != 0u {
            let lx = i32(l.x);
            let ly = i32(l.y);
            var behind = false;
            switch side {
                case 1u: { behind = ly >= plane.y; }
                case 2u: { behind = lx < plane.x; }
                case 3u: { behind = ly < plane.y; }
                default: { behind = lx >= plane.x; }
            }
            if behind {
                row = row + i32(g.eye.w);
                if row >= limit {
                    row = i32(g.light.z);
                }
            }
        }
        darkest = min(darkest, row);
    }
    if darkest >= limit {
        darkest = i32(g.light.z);
    }
    return clamp(darkest, 0, 15);
}
