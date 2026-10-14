//! Drawing the world with wgpu: one pipeline, the level's mesh, its
//! textures as a 2D array of palette indices, the shading table, and a
//! uniform block with the camera and the palette.

use crate::world::Vertex;
use glam::{Mat4, Vec3};
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    eye: [f32; 4],
    shade: [f32; 4],
    sky: [f32; 4],
    light: [f32; 4],
    // The view's window: the screen quads' row scale and shift (x, y).
    window: [f32; 4],
    lights: [[f32; 4]; 64],
    palette: [[f32; 4]; 256],
}

/// Where the world is drawn: the top `rows` of the target (a fraction:
/// the original's view is 147 rows of 200, the interface below it), its
/// eye line `horizon` of the way down that view (the original's C3D8 + 1
/// rows of its A91A + 1), and the sky objects' screen quads, made for
/// 100 rows a half view, scaled to it.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub rows: f32,
    pub horizon: f32,
}

impl Default for Window {
    fn default() -> Self {
        Window { rows: 1.0, horizon: 0.5 }
    }
}

/// The lights this frame (0000:B6EB's lists): none in full daylight,
/// else up to 64 (x, y, radius), the rows C8E7, C8E9, C8EB, and the rows
/// a light behind the wall drawn loses (BB02, which the wall's drawing
/// sets: 8, outdoors 2, 0000:AC1A).
#[derive(Clone, Debug, Default)]
pub struct Lights {
    pub lights: Vec<[u16; 3]>,
    pub rows: [u16; 3],
    pub behind: u16,
}

/// The sky this frame: outdoors or not, the afternoon's dither or the
/// morning's, and palette entries 1..8 (the window of the day's colours).
#[derive(Clone, Copy, Debug, Default)]
pub struct Sky {
    pub outdoors: bool,
    pub afternoon: bool,
    pub colours: [[u8; 3]; 8],
    /// The row of the shading table everything is remapped through
    /// first (the light level's high byte; 0 by day).
    pub dark: u32,
    /// Palette entry 71h, the lamps' glow, once the game set it.
    pub lamp: Option<[u8; 3]>,
    /// In fog (2294:0B01), the colour filling the view for the sky.
    pub fog: Option<u8>,
}

/// Where the player stands and looks (map units; heading 0..511 a turn,
/// 0 facing +y).
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub x: f32,
    pub y: f32,
    pub height: f32,
    pub heading: f32,
}

pub const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    sky: Sky,
    lights: Lights,
    globals: wgpu::Buffer,
    bind: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    count: u32,
    sprites: Option<(wgpu::Buffer, u64)>,
    palette: [[f32; 4]; 256],
    depth: Option<(wgpu::TextureView, u32, u32)>,
    window: Window,
}

pub struct Scene<'a> {
    pub palette: [[u8; 4]; 256],
    pub textures: &'a [[u8; 4096]],
    pub shading: &'a [u8],
    pub mesh: &'a [Vertex],
    pub sprites: &'a crate::world::SpriteSheet,
    /// DITHER.IMG then DITHER2.IMG (800 bytes each; zeros if missing).
    pub dither: &'a [u8],
}

impl Renderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat, scene: &Scene) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("world"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let layers = scene.textures.len() as u32;
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("textures"),
            size: wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: layers },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let flat: Vec<u8> = scene.textures.iter().flat_map(|t| t.iter().copied()).collect();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &flat,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(64), rows_per_image: Some(64) },
            wgpu::Extent3d { width: 64, height: 64, depth_or_array_layers: layers },
        );
        let shade = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("shading"),
            size: wgpu::Extent3d { width: 256, height: 16, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &shade, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &scene.shading[..0x1000],
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256), rows_per_image: Some(16) },
            wgpu::Extent3d { width: 256, height: 16, depth_or_array_layers: 1 },
        );
        let sheet = scene.sprites;
        let atlas = wgpu::Extent3d { width: crate::world::ATLAS, height: sheet.h, depth_or_array_layers: 1 };
        let sprite_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("sprites"),
            size: atlas,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &sprite_tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &sheet.pixels,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(crate::world::ATLAS), rows_per_image: Some(sheet.h) },
            atlas,
        );
        let dither = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dither"),
            size: wgpu::Extent3d { width: 40, height: 40, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let mut d = scene.dither.to_vec();
        d.resize(1600, 0);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo { texture: &dither, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &d,
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(40), rows_per_image: Some(40) },
            wgpu::Extent3d { width: 40, height: 40, depth_or_array_layers: 1 },
        );
        let mut palette = [[0f32; 4]; 256];
        for (p, c) in palette.iter_mut().zip(scene.palette.iter()) {
            *p = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0];
        }
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("world"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let tex_view = tex.create_view(&wgpu::TextureViewDescriptor { dimension: Some(wgpu::TextureViewDimension::D2Array), ..Default::default() });
        let shade_view = shade.create_view(&wgpu::TextureViewDescriptor::default());
        let sprite_view = sprite_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let dither_view = dither.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("world"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: globals.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&tex_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(&shade_view) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::TextureView(&sprite_view) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&dither_view) },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("world"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("world"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32, 3 => Float32, 4 => Uint32],
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState { module: &module, entry_point: Some("vs_sky"), compilation_options: Default::default(), buffers: &[] },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_sky"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState { format, blend: None, write_mask: wgpu::ColorWrites::ALL })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("mesh"),
            contents: bytemuck::cast_slice(scene.mesh),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Self { pipeline, sky_pipeline, sky: Sky::default(), lights: Lights::default(), globals, bind, vertices, count: scene.mesh.len() as u32, sprites: None, palette, depth: None, window: Window::default() }
    }

    /// Where the world is drawn from now on.
    pub fn set_window(&mut self, window: Window) {
        self.window = window;
    }

    /// The lights from now on.
    pub fn set_lights(&mut self, lights: Lights) {
        self.lights = lights;
    }

    /// The sky from now on: palette entries 1..8 become its colours.
    pub fn set_sky(&mut self, sky: Sky) {
        self.sky = sky;
        if sky.outdoors {
            for (i, c) in sky.colours.iter().enumerate() {
                self.palette[1 + i] = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0];
            }
            if let Some(c) = sky.lamp {
                self.palette[0x71] = [c[0] as f32 / 255.0, c[1] as f32 / 255.0, c[2] as f32 / 255.0, 1.0];
            }
        }
    }

    /// Draws the world seen from `cam` into `target` (`width` x `height`),
    /// with this frame's billboards.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, target: &wgpu::TextureView, width: u32, height: u32, cam: Camera, sprites: &[Vertex]) {
        let bytes: &[u8] = bytemuck::cast_slice(sprites);
        if !matches!(self.sprites, Some((_, cap)) if cap >= bytes.len() as u64) {
            let cap = (bytes.len() as u64).next_power_of_two().max(4096);
            let buf = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("sprites"),
                size: cap,
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.sprites = Some((buf, cap));
        }
        let sprite_buf = &self.sprites.as_ref().unwrap().0;
        queue.write_buffer(sprite_buf, 0, bytes);
        if !matches!(self.depth, Some((_, w, h)) if w == width && h == height) {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth"),
                size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: DEPTH,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            self.depth = Some((tex.create_view(&Default::default()), width, height));
        }
        let a = cam.heading / 512.0 * std::f32::consts::TAU;
        let eye = Vec3::new(cam.x, cam.height, cam.y);
        let forward = Vec3::new(a.sin(), 0.0, a.cos());
        let view = Mat4::look_at_lh(eye, eye + forward, Vec3::Y);
        // The world in the window's rows, at the same scale a row as over
        // the whole target, its eye line moved down to the window's.
        let rows = self.window.rows.clamp(0.05, 1.0);
        let view_h = ((height as f32 * rows).round() as u32).clamp(1, height);
        let fov = 2.0 * ((30f32).to_radians().tan() * view_h as f32 / height as f32).atan();
        let shift = 1.0 - 2.0 * self.window.horizon;
        let proj = Mat4::from_translation(Vec3::new(0.0, shift, 0.0)) * Mat4::perspective_lh(fov, width as f32 / view_h as f32, 4.0, 8192.0);
        // The screen quads: rows from the eye line at 100 a half of the
        // original's 200, over this window's half.
        let quad_scale = height as f32 / view_h as f32;
        let globals = Globals {
            view_proj: (proj * view).to_cols_array_2d(),
            eye: [cam.x, cam.height, cam.y, self.lights.behind as f32],
            shade: [96.0, 12.0, self.sky.dark as f32, self.sky.fog.map_or(0.0, |c| c as f32 + 1.0)],
            // The dither spans the view's top to its eye line.
            sky: [self.sky.outdoors as u32 as f32, self.sky.afternoon as u32 as f32, width as f32, 2.0 * view_h as f32 * self.window.horizon],
            light: {
                let r = self.lights.rows;
                [self.lights.lights.len().min(64) as f32, r[0] as f32, r[1] as f32, r[2] as f32]
            },
            window: [quad_scale, shift, 0.0, 0.0],
            lights: {
                let mut a = [[0f32; 4]; 64];
                for (d, l) in a.iter_mut().zip(&self.lights.lights) {
                    *d = [l[0] as f32, l[1] as f32, l[2] as f32, 0.0];
                }
                a
            },
            palette: self.palette,
        };
        queue.write_buffer(&self.globals, 0, bytemuck::bytes_of(&globals));
        let mut enc = device.create_command_encoder(&Default::default());
        {
            let depth = &self.depth.as_ref().unwrap().0;
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("world"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_viewport(0.0, 0.0, width as f32, view_h as f32, 0.0, 1.0);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_pipeline(&self.sky_pipeline);
            pass.draw(0..3, 0..1);
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..self.count, 0..1);
            if !sprites.is_empty() {
                pass.set_vertex_buffer(0, sprite_buf.slice(..bytes.len() as u64));
                pass.draw(0..sprites.len() as u32, 0..1);
            }
        }
        queue.submit([enc.finish()]);
    }
}
