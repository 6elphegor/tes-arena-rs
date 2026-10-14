// The world: textures are palette indices (an array of 64 x 64 layers),
// shaded by distance through the original's shading table (16 rows of
// 256 indices, darkest last), then coloured by the palette.

struct Globals {
    view_proj: mat4x4<f32>,
    eye: vec4<f32>,
    // x: distance per shading row, y: rows (darkest kept), z: light radius.
    shade: vec4<f32>,
    // x: outdoors (a sky), y: afternoon (the second dither), z, w: the
    // target's size.
    sky: vec4<f32>,
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
    let dist = distance(i.world.xz, g.eye.xz);
    var row = i32(dist / g.shade.x + (1.0 - i.light) * 4.0);
    row = clamp(row, 0, i32(g.shade.y));
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
    let row = clamp(i32(pos.y / (g.sky.w * 0.5) * 20.0), 0, 19) + select(0, 20, g.sky.y != 0.0);
    let col = i32(pos.x * 320.0 / g.sky.z) % 40;
    let index = textureLoad(dither, vec2<i32>(col, row), 0).r;
    return g.palette[index];
}
