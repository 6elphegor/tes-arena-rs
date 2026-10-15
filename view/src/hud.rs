//! The heads-up layer: the game's texts drawn over the view in the game's
//! own font, on a 320 x 200 screen as the original's (scaled to the
//! window), composed on the CPU and blended over the world.
//!
//! A text is the game's: `\t` and a byte set the colour (a palette
//! index), `\r` ends a line. The font is ARENAFNT.DAT, read from the
//! game's files at run time: its first byte is the height, the next 94
//! the widths of characters 21h..7Eh, then each character's rows, a
//! little-endian word each, the leftmost pixel in the top bit.

pub const W: usize = 320;
pub const H: usize = 200;

/// A font of the game's.
pub struct Font {
    pub h: usize,
    widths: [u8; 94],
    rows: Vec<u16>,
}

impl Font {
    /// The font file `name` from the game's files.
    pub fn load(archive: &crate::archive::Archive, name: &str) -> Option<Font> {
        let d = archive.read(name)?;
        let h = *d.first()? as usize;
        if h == 0 || d.len() < 95 + 94 * h * 2 {
            return None;
        }
        let mut widths = [0u8; 94];
        widths.copy_from_slice(&d[1..95]);
        let rows = d[95..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        Some(Font { h, widths, rows })
    }

    /// The width a space takes (the font has none).
    fn space(&self) -> usize {
        self.h.div_ceil(2)
    }

    /// The width of `c` and its glyph's index (None: a space or a
    /// character the font hasn't).
    fn glyph(&self, c: u8) -> (usize, Option<usize>) {
        match c {
            0x21..=0x7E => {
                let k = (c - 0x21) as usize;
                (self.widths[k] as usize, Some(k))
            }
            _ => (self.space(), None),
        }
    }

    /// The width of a line of plain text.
    pub fn width(&self, s: &str) -> usize {
        s.bytes().map(|c| self.glyph(c).0).sum()
    }
}

/// A run of text in one colour.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub colour: u8,
    pub text: String,
}

/// The game's text at DS:`at` split into lines of coloured spans: `\t`
/// and a byte set the colour, `\t` and three digits a column (the lines
/// of the shops' lists: two spaces here), `\r` (or `\n`) ends a line.
pub fn lines(bytes: &[u8], colour: u8) -> Vec<Vec<Span>> {
    let mut out = vec![Vec::new()];
    let mut colour = colour;
    let mut cur = String::new();
    let flush = |out: &mut Vec<Vec<Span>>, cur: &mut String, colour: u8| {
        if !cur.is_empty() {
            out.last_mut().unwrap().push(Span { colour, text: std::mem::take(cur) });
        }
    };
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\t' if bytes.get(i + 1..i + 4).is_some_and(|d| d.iter().all(u8::is_ascii_digit)) => {
                if !cur.is_empty() {
                    cur.push_str("  ");
                }
                i += 3;
            }
            b'\t' => {
                flush(&mut out, &mut cur, colour);
                if let Some(&c) = bytes.get(i + 1) {
                    colour = c;
                }
                i += 1;
            }
            b'\r' | b'\n' => {
                flush(&mut out, &mut cur, colour);
                out.push(Vec::new());
                // \r\n is one break.
                if bytes[i] == b'\r' && bytes.get(i + 1) == Some(&b'\n') {
                    i += 1;
                }
            }
            c => cur.push(c as char),
        }
        i += 1;
    }
    flush(&mut out, &mut cur, colour);
    while out.len() > 1 && out.last().is_some_and(|l| l.is_empty()) {
        out.pop();
    }
    out
}

/// The plain text of a line.
pub fn plain(line: &[Span]) -> String {
    line.iter().map(|s| s.text.as_str()).collect()
}

/// The layer: RGBA, (0, 0, 0, 0) where nothing is drawn.
pub struct Screen {
    pub px: Vec<[u8; 4]>,
    palette: [[u8; 4]; 256],
}

impl Screen {
    pub fn new(palette: [[u8; 4]; 256]) -> Screen {
        Screen { px: vec![[0; 4]; W * H], palette }
    }

    pub fn clear(&mut self) {
        self.px.fill([0; 4]);
    }

    /// The colours from now on (the character sheet's screens have their
    /// own, CHARSHT.COL).
    pub fn set_palette(&mut self, palette: [[u8; 4]; 256]) {
        self.palette = palette;
    }

    /// A rectangle in palette colour `c`, `alpha` opaque.
    pub fn fill(&mut self, x: i32, y: i32, w: i32, h: i32, c: u8, alpha: u8) {
        let p = self.palette[c as usize];
        for yy in y.max(0)..(y + h).min(H as i32) {
            for xx in x.max(0)..(x + w).min(W as i32) {
                self.px[yy as usize * W + xx as usize] = [p[0], p[1], p[2], alpha];
            }
        }
    }

    /// Palette pixels (`w` x `h`, row-major) with their top left at
    /// (x, y); colour 0 is see-through, rows below `bottom` are cut.
    pub fn blit(&mut self, x: i32, y: i32, w: usize, h: usize, px: &[u8], bottom: i32) {
        for r in 0..h as i32 {
            let yy = y + r;
            if yy < 0 || yy > bottom || yy >= H as i32 {
                continue;
            }
            for c in 0..w as i32 {
                let xx = x + c;
                if !(0..W as i32).contains(&xx) {
                    continue;
                }
                let v = *px.get((r * w as i32 + c) as usize).unwrap_or(&0);
                if v != 0 {
                    let p = self.palette[v as usize];
                    self.px[yy as usize * W + xx as usize] = [p[0], p[1], p[2], 0xFF];
                }
            }
        }
    }

    /// Text at (x, y) in colour `c`, without a shadow.
    pub fn text_plain(&mut self, font: &Font, x: i32, y: i32, s: &str, c: u8) {
        self.text_raw(font, x, y, s, c);
    }

    /// A box: a dark fill with a one-pixel frame in colour `frame`.
    pub fn panel(&mut self, x: i32, y: i32, w: i32, h: i32, frame: u8) {
        self.fill(x, y, w, h, 0, 0xE0);
        self.fill(x, y, w, 1, frame, 0xFF);
        self.fill(x, y + h - 1, w, 1, frame, 0xFF);
        self.fill(x, y, 1, h, frame, 0xFF);
        self.fill(x + w - 1, y, 1, h, frame, 0xFF);
    }

    /// Plain text at (x, y), its top left, in colour `c`, with a shadow
    /// one pixel down and right in colour 0.
    pub fn text(&mut self, font: &Font, x: i32, y: i32, s: &str, c: u8) {
        self.text_raw(font, x + 1, y + 1, s, 0);
        self.text_raw(font, x, y, s, c);
    }

    fn text_raw(&mut self, font: &Font, mut x: i32, y: i32, s: &str, c: u8) {
        let p = self.palette[c as usize];
        for ch in s.bytes() {
            let (w, g) = font.glyph(ch);
            if let Some(k) = g {
                for r in 0..font.h {
                    let bits = font.rows.get(k * font.h + r).copied().unwrap_or(0);
                    for i in 0..16 {
                        if bits & (0x8000 >> i) == 0 {
                            continue;
                        }
                        let (xx, yy) = (x + i, y + r as i32);
                        if (0..W as i32).contains(&xx) && (0..H as i32).contains(&yy) {
                            self.px[yy as usize * W + xx as usize] = [p[0], p[1], p[2], 0xFF];
                        }
                    }
                }
            }
            x += w as i32;
        }
    }

    /// A line of spans at (x, y).
    pub fn spans(&mut self, font: &Font, mut x: i32, y: i32, line: &[Span]) {
        for s in line {
            self.text(font, x, y, &s.text, s.colour);
            x += font.width(&s.text) as i32;
        }
    }
}

/// What the layer shows this frame.
#[derive(Clone, Debug, Default)]
pub struct Hud {
    /// The game's message line (061A), while it counts down.
    pub message: Option<Vec<Vec<Span>>>,
    /// A message box waiting to be read, its lines.
    pub boxed: Option<Vec<Vec<Span>>>,
    /// A menu waiting for a pick: its title's lines, its entries, the
    /// entry highlighted.
    pub menu: Option<Menu>,
    /// Hours to type: the prompt and what has been typed.
    pub typing: Option<(String, String)>,
    /// A status line at the bottom (the rest's hours).
    pub status: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct Menu {
    pub title: Vec<Vec<Span>>,
    pub entries: Vec<Vec<Span>>,
    pub keys: Vec<u8>,
    pub selected: usize,
}

/// Where a menu's entries are drawn: (x, y of the first, line height,
/// width), for the mouse to pick them.
pub fn menu_layout(font: &Font, menu: &Menu) -> (i32, i32, i32, i32) {
    let lh = font.h as i32 + 2;
    let w = menu
        .title
        .iter()
        .chain(menu.entries.iter())
        .map(|l| font.width(&plain(l)) as i32)
        .max()
        .unwrap_or(0)
        + 16;
    let w = w.clamp(80, W as i32 - 8);
    let h = (menu.title.len() + menu.entries.len()) as i32 * lh + 10;
    let x = (W as i32 - w) / 2;
    let y = ((H as i32 - h) / 2).max(4);
    (x + 8, y + 5 + menu.title.len() as i32 * lh, lh, w - 16)
}

/// `hud`'s texts drawn over what the layer has (the interface, drawn
/// after the layer is cleared).
pub fn compose_over(screen: &mut Screen, font: &Font, hud: &Hud) {
    let lh = font.h as i32 + 2;
    if let Some(lines) = &hud.message {
        let mut y = 4;
        for l in lines {
            let w = font.width(&plain(l)) as i32;
            screen.spans(font, (W as i32 - w) / 2, y, l);
            y += lh;
        }
    }
    if let Some(s) = &hud.status {
        let w = font.width(s) as i32;
        screen.panel((W as i32 - w) / 2 - 6, H as i32 - lh - 8, w + 12, lh + 4, 0x0F);
        screen.text(font, (W as i32 - w) / 2, H as i32 - lh - 6, s, 0x0F);
    }
    let mut boxed_bottom = None;
    if let Some(lines) = &hud.boxed {
        let w = lines.iter().map(|l| font.width(&plain(l)) as i32).max().unwrap_or(0) + 16;
        let w = w.clamp(60, W as i32 - 8);
        let h = lines.len() as i32 * lh + 10;
        let x = (W as i32 - w) / 2;
        // Above a menu shown with it, else in the middle.
        let y = if hud.menu.is_some() { 8 } else { ((H as i32 - h) / 2).max(4) };
        screen.panel(x, y, w, h, 0x0F);
        for (i, l) in lines.iter().enumerate() {
            screen.spans(font, x + 8, y + 5 + i as i32 * lh, l);
        }
        boxed_bottom = Some(y + h);
    }
    if let Some(menu) = &hud.menu {
        let (ex, ey, lh, ew) = menu_layout(font, menu);
        let (mut x, mut y) = (ex - 8, ey - 5 - menu.title.len() as i32 * lh);
        if let Some(b) = boxed_bottom {
            y = y.max(b + 4);
        }
        let dy = y - (ey - 5 - menu.title.len() as i32 * lh);
        let h = (menu.title.len() + menu.entries.len()) as i32 * lh + 10;
        x = x.max(4);
        screen.panel(x, y, ew + 16, h, 0x0F);
        for (i, l) in menu.title.iter().enumerate() {
            screen.spans(font, ex, y + 5 + i as i32 * lh, l);
        }
        for (i, l) in menu.entries.iter().enumerate() {
            let ly = ey + dy + i as i32 * lh;
            if i == menu.selected {
                screen.fill(ex - 3, ly - 1, ew + 6, lh, 0x08, 0xC0);
            }
            screen.spans(font, ex, ly, l);
        }
    }
    if let Some((prompt, typed)) = &hud.typing {
        let line = format!("{prompt} {typed}_");
        let w = (font.width(&line) as i32 + 16).clamp(80, W as i32 - 8);
        let x = (W as i32 - w) / 2;
        let y = H as i32 / 2 - lh;
        screen.panel(x, y, w, lh + 10, 0x0F);
        screen.text(font, x + 8, y + 5, &line, 0x0F);
    }
}

/// The layer's pipeline: a 320 x 200 RGBA texture, nearest-sampled over
/// the whole target, alpha-blended.
pub struct Overlay {
    pipeline: wgpu::RenderPipeline,
    bind: wgpu::BindGroup,
    texture: wgpu::Texture,
}

const SHADER: &str = r#"
@group(0) @binding(0) var layer: texture_2d<f32>;
@group(0) @binding(1) var near: sampler;

struct Out {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> Out {
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return textureSample(layer, near, i.uv);
}
"#;

impl Overlay {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Overlay {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("hud"), source: wgpu::ShaderSource::Wgsl(SHADER.into()) });
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hud"),
            size: wgpu::Extent3d { width: W as u32, height: H as u32, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hud"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let view = texture.create_view(&Default::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("hud"), bind_group_layouts: &[&layout], push_constant_ranges: &[] });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud"),
            layout: Some(&pl),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: Some(wgpu::BlendState::ALPHA_BLENDING), write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Overlay { pipeline, bind, texture }
    }

    /// Draws the layer `screen` over `target`.
    pub fn draw(&self, device: &wgpu::Device, queue: &wgpu::Queue, target: &wgpu::TextureView, screen: &Screen) {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &self.texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            bytemuck::cast_slice(&screen.px),
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(W as u32 * 4), rows_per_image: Some(H as u32) },
            wgpu::Extent3d { width: W as u32, height: H as u32, depth_or_array_layers: 1 },
        );
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Load, store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.draw(0..3, 0..1);
        }
        queue.submit([enc.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_split_colours_and_breaks() {
        let l = lines(b"\t\xc0 CAMP\r\t\xc0C\t\xd4amp\r", 0x0F);
        assert_eq!(l.len(), 2);
        assert_eq!(l[0], vec![Span { colour: 0xC0, text: " CAMP".into() }]);
        assert_eq!(plain(&l[1]), "Camp");
        assert_eq!(l[1][1].colour, 0xD4);
    }
}
