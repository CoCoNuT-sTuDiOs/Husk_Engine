use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

use crate::scene::light::MAX_LIGHTS;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    joint_indices: [u32; 4],
    weights: [f32; 4],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 12,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: 24,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32x4,
                },
                wgpu::VertexAttribute {
                    offset: 40,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

const MAX_BONES: usize = 128;

/// Base colours for the first few objects so they can be told apart:
/// Husk orange, tool blue, green and red from the palette.
pub(crate) const OBJECT_COLORS: [[f32; 4]; 4] = [
    [0.976, 0.451, 0.086, 1.0],
    [0.231, 0.510, 0.965, 1.0],
    [0.133, 0.773, 0.369, 1.0],
    [0.937, 0.267, 0.267, 1.0],
];

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    normal_matrix: [[f32; 4]; 4],
    light_dir: [f32; 4],
    light_color: [f32; 4],
    base_color: [f32; 4],
    material: [f32; 4],
    camera_pos: [f32; 4],
}

/// One light as the shader reads it. `position.w` is 0 for a sun and 1
/// for a point light. `direction` is the way the light's rays travel (a
/// sun's only direction). `color` is the light's colour times its
/// intensity.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuLight {
    position: [f32; 4],
    direction: [f32; 4],
    color: [f32; 4],
}

/// Every light the shader shines: how many are in use (in `count[0]`),
/// then the lights. With none in use the shader falls back to the light
/// that follows the camera.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct LightsUniform {
    count: [u32; 4],
    items: [GpuLight; MAX_LIGHTS],
}

/// Packs the scene's lights for the shader. Lights past `MAX_LIGHTS` are
/// left out.
fn lights_uniform(
    lights: &[(crate::scene::transform::Transform, crate::scene::light::Light)],
) -> LightsUniform {
    let mut uniform: LightsUniform = bytemuck::Zeroable::zeroed();
    for (transform, light) in lights.iter().take(MAX_LIGHTS) {
        let slot = uniform.count[0] as usize;
        let position = transform.translation;
        let direction = crate::scene::light::shine_direction(transform);
        let kind = match light.kind {
            crate::scene::light::LightKind::Sun => 0.0,
            crate::scene::light::LightKind::Point => 1.0,
        };
        uniform.items[slot] = GpuLight {
            position: [position.x, position.y, position.z, kind],
            direction: [direction.x, direction.y, direction.z, 0.0],
            color: [
                light.color[0] * light.intensity,
                light.color[1] * light.intensity,
                light.color[2] * light.intensity,
                0.0,
            ],
        };
        uniform.count[0] += 1;
    }
    uniform
}


/// A point inside the view of a camera at `eye` looking at `target`, with a
/// vertical field of view of `fov` radians, in a frame `aspect` (width over
/// height) wide. It is `depth_fraction` of the way from the eye to the
/// target, then moved right and up by `right_fraction` and `up_fraction` of
/// what the frame shows at that depth: 0 is the middle, 1 the edge of the
/// frame, and a negative number goes left or down.
fn spot_in_view(
    eye: glam::Vec3,
    target: glam::Vec3,
    fov: f32,
    aspect: f32,
    depth_fraction: f32,
    right_fraction: f32,
    up_fraction: f32,
) -> glam::Vec3 {
    let to_target = target - eye;
    let distance = to_target.length();
    let forward = if distance > 1e-6 {
        to_target / distance
    } else {
        -glam::Vec3::Z
    };
    // Looking straight up or down has no sideways direction of its own.
    let mut right = forward.cross(glam::Vec3::Y);
    if right.length_squared() < 1e-8 {
        right = glam::Vec3::X;
    }
    let right = right.normalize();
    let up = right.cross(forward);
    let depth = distance * depth_fraction;
    let half_height = depth * (fov * 0.5).tan();
    let half_width = half_height * aspect;
    eye + forward * depth + right * (half_width * right_fraction) + up * (half_height * up_fraction)
}

/// What the ground grid's shader reads: the camera, the camera's distance
/// from what it looks at, and the colour of the plain grid lines.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct GridUniform {
    view_proj: [[f32; 4]; 4],
    inverse_view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    params: [f32; 4],
    line_color: [f32; 4],
}

/// How bright the ground grid's lines are: light on a dark background and
/// dark on a light one, so they show on either.
fn grid_line_brightness(red: f64, green: f64, blue: f64) -> f32 {
    let brightness = 0.2126 * red + 0.7152 * green + 0.0722 * blue;
    if brightness > 0.5 {
        0.1
    } else {
        0.8
    }
}

/// What the background gradient's shader reads: the colour at the top, the
/// colour at the bottom, and where the gradient starts and how tall it is
/// (in pixel rows).
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct BackgroundUniform {
    top: [f32; 4],
    bottom: [f32; 4],
    span: [f32; 4],
}

/// Packs the background gradient for the shader. `frame_top` and
/// `frame_height` are the pixel rows the gradient runs over.
fn background_uniform(
    top: [f32; 3],
    bottom: [f32; 3],
    frame_top: f32,
    frame_height: f32,
) -> BackgroundUniform {
    BackgroundUniform {
        top: [top[0], top[1], top[2], 1.0],
        bottom: [bottom[0], bottom[1], bottom[2], 1.0],
        span: [frame_top, frame_height, 0.0, 0.0],
    }
}

/// One drawable thing in the scene: its own geometry, its own bone
/// matrices, and its own placement and colour.
struct RenderObject {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bone_matrices_buffer: wgpu::Buffer,
    bone_bind_group: wgpu::BindGroup,
    mesh_data: crate::geometry::mesh::MeshData,
    model: glam::Mat4,
    base_color: [f32; 4],
    // Roughness, metalness and two unused values, as the shader reads them.
    material: [f32; 4],
}
/// A scene camera as the renderer needs it: where it sits and faces (no
/// scale), its vertical field of view, and the shape (width / height) of
/// the output frame it is framed for.
#[derive(Clone, Copy, Debug)]
pub struct CameraView {
    pub world: glam::Mat4,
    pub fov_degrees: f32,
    pub frame_aspect: f32,
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    output_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    last_frame: Vec<u8>,
    retired_frames: Vec<Vec<u8>>,
    pipeline: wgpu::RenderPipeline,
    objects: Vec<RenderObject>,
    active_object: usize,
    bind_group_layout: wgpu::BindGroupLayout,
    bone_bind_group_layout: wgpu::BindGroupLayout,
    lights_buffer: wgpu::Buffer,
    lights_bind_group: wgpu::BindGroup,
    world_buffer: wgpu::Buffer,
    world_bind_group: wgpu::BindGroup,
    clear_color: wgpu::Color,
    frame_time_total: Duration,
    frame_time_count: u32,
    #[allow(dead_code)]
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    last_view_proj: glam::Mat4,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_distance: f32,
    orbit_target: glam::Vec3,
    scene_camera: Option<CameraView>,
        camera_view_on: bool,
    grid_pipeline: wgpu::RenderPipeline,
    grid_buffer: wgpu::Buffer,
    grid_bind_group: wgpu::BindGroup,
    /// Whether the ground grid is drawn (it never is in the shape tray's
    /// previews, and not while looking through the scene camera).
    show_grid: bool,
    background_pipeline: wgpu::RenderPipeline,
    background_buffer: wgpu::Buffer,
    background_bind_group: wgpu::BindGroup,
    /// The colour at the top of the background gradient. The bottom colour
    /// is `clear_color`.
    background_top: [f32; 3],
    /// Whether the background is a gradient (never in the shape tray's
    /// previews, which need a plain background to cut out).
    gradient_on: bool,
}

const BYTES_PER_PIXEL: u32 = 4;

impl Renderer {
    pub fn new(width: u32, height: u32, mesh: crate::geometry::mesh::MeshData) -> Self {
        pollster::block_on(Self::new_async(width, height, mesh))
    }

    async fn new_async(width: u32, height: u32, mesh: crate::geometry::mesh::MeshData) -> Self {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find a suitable GPU adapter");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .expect("Failed to create wgpu device");

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("husk_offscreen_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        // GPU texture-to-buffer copies require each row's byte stride to be
        // a multiple of COPY_BYTES_PER_ROW_ALIGNMENT (256) most
        // resolutions don't naturally land on that boundary, so the
        // readback buffer is sized to the *padded* row width, and the
        // padding gets stripped back out afterward (see render_frame).
        let unpadded_bytes_per_row = width * BYTES_PER_PIXEL;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("husk_readback_buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("husk_cube_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("husk_depth_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("husk_scene_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let bone_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("husk_bone_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let lights_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("husk_lights_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // No lights yet: the shader uses the light that follows the camera.
        let lights_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_lights_buffer"),
            contents: bytemuck::bytes_of(&lights_uniform(&[])),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let lights_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_lights_bind_group"),
            layout: &lights_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: lights_buffer.as_entire_binding(),
            }],
        });

        let world_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("husk_world_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let world_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_world_buffer"),
            contents: bytemuck::bytes_of(&crate::scene::world::World::default().shader_values()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let world_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_world_bind_group"),
            layout: &world_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: world_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("husk_pipeline_layout"),
            bind_group_layouts: &[
                &bind_group_layout,
                &bone_bind_group_layout,
                &lights_bind_group_layout,
                &world_bind_group_layout,
            ],
                push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("husk_cube_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

                // The ground grid: drawn after the objects, tested against their
        // depth but not writing any, and blended over them.
        let grid_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("husk_grid_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("grid.wgsl").into()),
        });
        let grid_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("husk_grid_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let grid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("husk_grid_buffer"),
            size: std::mem::size_of::<GridUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grid_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_grid_bind_group"),
            layout: &grid_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: grid_buffer.as_entire_binding(),
            }],
        });
        let grid_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("husk_grid_pipeline_layout"),
            bind_group_layouts: &[&grid_bind_group_layout],
            push_constant_ranges: &[],
        });
        let grid_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("husk_grid_pipeline"),
            layout: Some(&grid_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &grid_shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &grid_shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

                // The background gradient: one full-screen pass drawn before the
        // objects, ignoring depth.
        let background_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("husk_background_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("background.wgsl").into()),
        });
        let background_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("husk_background_bind_group_layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let background_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("husk_background_buffer"),
            size: std::mem::size_of::<BackgroundUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let background_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_background_bind_group"),
            layout: &background_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: background_buffer.as_entire_binding(),
            }],
        });
        let background_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("husk_background_pipeline_layout"),
                bind_group_layouts: &[&background_bind_group_layout],
                push_constant_ranges: &[],
            });
        let background_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("husk_background_pipeline"),
            layout: Some(&background_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &background_shader,
                entry_point: "vs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &background_shader,
                entry_point: "fs_main",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let mut orbit_target = glam::Vec3::ZERO;
        let mut orbit_distance = 3.354_102_f32;
        if !mesh.positions.is_empty() {
            let mut bounds_min = glam::Vec3::splat(f32::MAX);
            let mut bounds_max = glam::Vec3::splat(f32::MIN);
            for position in &mesh.positions {
                let p = glam::Vec3::from(*position);
                bounds_min = bounds_min.min(p);
                bounds_max = bounds_max.max(p);
            }
            orbit_target = (bounds_min + bounds_max) * 0.5;
            let extent = bounds_max - bounds_min;
            let longest_side = extent.x.max(extent.y).max(0.1);
            orbit_distance =
                (longest_side * 0.5 / (std::f32::consts::FRAC_PI_4 * 0.5).tan()) * 1.25;
        }

        let first_object = Self::create_object(
            &device,
            &bind_group_layout,
            &bone_bind_group_layout,
            mesh,
            glam::Mat4::IDENTITY,
            OBJECT_COLORS[0],
        );

        Self {
            device,
            queue,
            texture,
            output_buffer,
            width,
            height,
            last_frame: Vec::new(),
            retired_frames: Vec::new(),
            pipeline,
            objects: vec![first_object],
            active_object: 0,
            bind_group_layout,
            bone_bind_group_layout,
            lights_buffer,
            lights_bind_group,
            world_buffer,
            world_bind_group,
            clear_color: wgpu::Color {
                r: 0.05,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            },
            frame_time_total: Duration::ZERO,
            frame_time_count: 0,
            depth_texture,
            depth_view,
            last_view_proj: glam::Mat4::IDENTITY,
            orbit_yaw: 0.0,
            orbit_pitch: 0.463_65,
            orbit_distance: 3.354_102,
            orbit_target,
            scene_camera: None,
            camera_view_on: false,
            grid_pipeline,
            grid_buffer,
            grid_bind_group,
            show_grid: true,
            background_pipeline,
            background_buffer,
            background_bind_group,
            background_top: crate::scene::world::World::default().background_top,
            gradient_on: false,
        }
        }

    /// Builds the GPU resources for one drawable object: its own geometry,
    /// its own uniform (carrying its placement and colour) and its own
    /// bone matrices, starting at the plain no-skin defaults.
    fn create_object(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        bone_bind_group_layout: &wgpu::BindGroupLayout,
        mesh: crate::geometry::mesh::MeshData,
        model: glam::Mat4,
        base_color: [f32; 4],
    ) -> RenderObject {
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("husk_scene_uniform_buffer"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_object_bind_group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let identity_bone_matrices: Vec<[[f32; 4]; 4]> =
            vec![glam::Mat4::IDENTITY.to_cols_array_2d(); MAX_BONES];

        let bone_matrices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_bone_matrices_buffer"),
            contents: bytemuck::cast_slice(&identity_bone_matrices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        let bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_bone_bind_group"),
            layout: bone_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_matrices_buffer.as_entire_binding(),
            }],
        });

        // Fully weighted to bone 0, which holds an identity matrix: a
        // mathematical no-op until real skin weights arrive.
        let vertices: Vec<Vertex> = mesh
            .positions
            .iter()
            .zip(mesh.normals.iter())
            .map(|(position, normal)| Vertex {
                position: *position,
                normal: *normal,
                joint_indices: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            })
            .collect();

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_mesh_vertex_buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_mesh_index_buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = mesh.indices.len() as u32;

        RenderObject {
            vertex_buffer,
            index_buffer,
            num_indices,
            uniform_buffer,
            bind_group,
            bone_matrices_buffer,
            bone_bind_group,
            mesh_data: mesh,
            model,
            base_color,
            material: [0.4, 0.2, 0.0, 0.0],
        }
        }

    /// Draws one mesh on its own, fitted to the frame, at `size` x `size`
    /// pixels, and returns the RGBA pixels with the background made
    /// transparent. The shape tray uses it for its previews. It builds a
    /// throwaway renderer (its own GPU device), so call it off the UI
    /// thread and never once per frame.
    pub fn render_preview(mesh: crate::geometry::mesh::MeshData, size: u32) -> Vec<u8> {
        // A light neutral grey-blue, so the shape reads on the dark panel.
        const PREVIEW_COLOR: [f32; 4] = [0.70, 0.72, 0.76, 1.0];
        let mut renderer = Renderer::new(size, size, mesh);
        renderer.show_grid = false;
        renderer.objects[0].base_color = PREVIEW_COLOR;
        // Fit the whole shape (its bounding sphere) in the frame, so every
        // preview fills about the same share of its tile.
        let centre = renderer.orbit_target;
        let radius = renderer.objects[0]
            .mesh_data
            .positions
            .iter()
            .map(|p| (glam::Vec3::from(*p) - centre).length())
            .fold(0.0_f32, f32::max)
            .max(0.01);
        renderer.orbit_distance = radius / (std::f32::consts::FRAC_PI_4 * 0.5).sin() * 1.1;
        // A three-quarter view from slightly above.
        renderer.orbit(0.6, 0.15);
        renderer.render_frame();
        let mut pixels = renderer.last_frame.clone();
        // The corner pixel is always background (the camera frames the
        // shape with a margin), and the renderer does not blend edges, so
        // every pixel of exactly that colour is background: make it
        // transparent.
        if pixels.len() >= 4 {
            let background = [pixels[0], pixels[1], pixels[2], pixels[3]];
            for pixel in pixels.chunks_exact_mut(4) {
                if pixel[..] == background[..] {
                    pixel.copy_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        pixels
    }
    /// The scene camera the viewport is looking through right now, if it is.
    fn camera_through(&self) -> Option<CameraView> {
        if self.camera_view_on {
            self.scene_camera
        } else {
            None
        }
    }

    /// Tells the renderer which camera the scene has (None if it has none),
    /// where it is and what it sees. With no camera, camera view switches
    /// itself off.
    pub fn set_scene_camera(&mut self, camera: Option<CameraView>) {
        self.scene_camera = camera;
        if camera.is_none() {
            self.camera_view_on = false;
        }
    }

    /// Starts or stops looking through the scene camera. Returns whether
    /// the viewport is looking through it now: false when asked to start
    /// but the scene has no camera.
    pub fn set_camera_view(&mut self, on: bool) -> bool {
        self.camera_view_on = on && self.scene_camera.is_some();
        self.camera_view_on
    }

    /// Whether the viewport is looking through the scene camera.
    pub fn camera_view_active(&self) -> bool {
        self.camera_view_on
    }

    /// While looking through the scene camera: where the output frame sits
    /// in the viewport, as (x, y, width, height) in pixels. None otherwise.
    pub fn camera_frame_rect(&self) -> Option<(f32, f32, f32, f32)> {
        let camera = self.camera_through()?;
        Some(crate::scene::camera::frame_rect(
            self.width as f32,
            self.height as f32,
            camera.frame_aspect,
        ))
    }

    /// Where the viewport camera is in the scene.
    fn eye(&self) -> glam::Vec3 {
        self.orbit_target
            + self.orbit_distance
                * glam::Vec3::new(
                    self.orbit_pitch.cos() * self.orbit_yaw.sin(),
                    self.orbit_pitch.sin(),
                    self.orbit_pitch.cos() * self.orbit_yaw.cos(),
                )
    }

    /// Adds a scene camera, drawn as `mesh` (a small camera shape), placed
    /// where the viewport camera is and looking the way it looks, as
    /// Blender's "camera to view" does. The shape is sized to the viewing
    /// distance so it stays visible. Returns its index, its placement matrix
    /// and its colour. The active object does not change.
    pub fn add_camera_at_view(
        &mut self,
        mesh: crate::geometry::mesh::MeshData,
    ) -> (usize, glam::Mat4, [f32; 4]) {
        const CAMERA_COLOR: [f32; 4] = [0.75, 0.75, 0.78, 1.0];
        // Off to the side and above, inside the view, aimed at what the
        // viewport is looking at: seen from outside, like a cameraman.
        let position = spot_in_view(
            self.eye(),
            self.orbit_target,
            std::f32::consts::FRAC_PI_4,
            self.width as f32 / self.height as f32,
            0.55,
            0.55,
            0.35,
        );
        let placed =
            glam::Mat4::look_at_rh(position, self.orbit_target, glam::Vec3::Y).inverse();
        let model = placed * glam::Mat4::from_scale(glam::Vec3::splat(self.orbit_distance * 0.06));
        let index = self.add_object(mesh, model, CAMERA_COLOR);
        (index, model, CAMERA_COLOR)
    }

    pub fn add_light_at_view(
        &mut self,
        mesh: crate::geometry::mesh::MeshData,
    ) -> (usize, glam::Mat4, [f32; 4]) {
        const LIGHT_COLOR: [f32; 4] = [0.98, 0.84, 0.35, 1.0];
        // Upper left, inside the view, so a new light is always on screen.
        let position = spot_in_view(
            self.eye(),
            self.orbit_target,
            std::f32::consts::FRAC_PI_4,
            self.width as f32 / self.height as f32,
            0.7,
            -0.6,
            0.6,
        );
        let placed =
            glam::Mat4::look_at_rh(position, self.orbit_target, glam::Vec3::Y).inverse();
        let model = placed * glam::Mat4::from_scale(glam::Vec3::splat(self.orbit_distance * 0.06));
        let index = self.add_object(mesh, model, LIGHT_COLOR);
        (index, model, LIGHT_COLOR)
    }

    /// Adds another object to the scene and returns its index. Section 5c
    /// will use this for the add-model action.
    pub fn add_object(
        &mut self,
        mesh: crate::geometry::mesh::MeshData,
        model: glam::Mat4,
        base_color: [f32; 4],
    ) -> usize {
        let object = Self::create_object(
            &self.device,
            &self.bind_group_layout,
            &self.bone_bind_group_layout,
            mesh,
            model,
            base_color,
        );
        self.objects.push(object);
        self.objects.len() - 1
    }

    /// Adds a built-in shape to the scene and returns its index in the
    /// object list (the scene's own list must grow in step, see
    /// `scene::add_object`). The shape is sized to a quarter of the first
    /// object's longest side and centred where the ray through the screen
    /// pixel crosses the camera-facing plane through the orbit target.
    /// The active object does not change. Returns the index together with
    /// the placement matrix the shape was given, which the caller stores
    /// as the scene object's transform. None if that ray cannot reach the
    /// plane.
    pub fn add_shape_at_screen(
        &mut self,
        mesh: crate::geometry::mesh::MeshData,
        screen_x: f32,
        screen_y: f32,
        ) -> Option<(usize, glam::Mat4, [f32; 4])> {
        let centre = self.screen_to_view_plane(screen_x, screen_y, self.orbit_target)?;
        let positions = &self.objects[0].mesh_data.positions;
        let mut size = 1.0_f32;
        if !positions.is_empty() {
            let mut bounds_min = glam::Vec3::splat(f32::MAX);
            let mut bounds_max = glam::Vec3::splat(f32::MIN);
            for position in positions {
                let p = glam::Vec3::from(*position);
                bounds_min = bounds_min.min(p);
                bounds_max = bounds_max.max(p);
            }
            size = (bounds_max - bounds_min).max_element() * 0.25;
        }
        let model = glam::Mat4::from_translation(centre)
            * glam::Mat4::from_scale(glam::Vec3::splat(size));
        let colour = OBJECT_COLORS[self.objects.len() % OBJECT_COLORS.len()];
        let index = self.add_object(mesh, model, colour);
        Some((index, model, colour))
    }


    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.orbit_yaw += delta_yaw;
        self.orbit_pitch = (self.orbit_pitch + delta_pitch).clamp(-1.5, 1.5);
    }

    /// Moves the camera closer (a factor below 1) or farther away (above 1),
    /// within sensible limits.
    pub fn zoom(&mut self, factor: f32) {
        self.orbit_distance = (self.orbit_distance * factor).clamp(0.3, 500.0);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.clamp(16, 4096);
        let height = height.clamp(16, 4096);
        if width == self.width && height == self.height {
            return;
        }

        self.texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("husk_offscreen_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        self.depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("husk_depth_texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        self.depth_view = self
            .depth_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let unpadded_bytes_per_row = width * BYTES_PER_PIXEL;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;
        self.output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("husk_readback_buffer"),
            size: (padded_bytes_per_row * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        self.width = width;
        self.height = height;

        let fresh = vec![0u8; (width * height * BYTES_PER_PIXEL) as usize];
        let old = std::mem::replace(&mut self.last_frame, fresh);
        self.retired_frames.push(old);
        if self.retired_frames.len() > 3 {
            self.retired_frames.remove(0);
        }
    }

    pub fn render_frame(&mut self) {
    let frame_start = Instant::now();

        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let aspect = self.width as f32 / self.height as f32;
        let mut projection =
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);

        let mut target = self.orbit_target;
        let mut eye = self.eye();
        let mut view_matrix = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);

        // Looking through the scene camera instead of the viewport's own.
        if let Some(camera) = self.camera_through() {
            eye = camera.world.transform_point3(glam::Vec3::ZERO);
            target = eye + camera.world.transform_vector3(-glam::Vec3::Z);
            view_matrix = camera.world.inverse();
            // The output frame sits inside the viewport. Widen the view so
            // that the frame sees exactly the camera's field of view.
            let (_, _, _, frame_height) = crate::scene::camera::frame_rect(
                self.width as f32,
                self.height as f32,
                camera.frame_aspect,
            );
            let fov = crate::scene::camera::viewport_fov_degrees(
                camera.fov_degrees,
                self.height as f32,
                frame_height,
            );
            projection = glam::Mat4::perspective_rh(fov.to_radians(), aspect, 0.1, 100.0);
        }
            // Each object's own placement is applied in the shader (scene.model),
        // so this is just the camera.
        let view_proj = projection * view_matrix;
        self.last_view_proj = view_proj;

        // A light that travels with the camera: it shines from the
        // viewer's side, a little from above and from the left, so
        // whatever faces the camera is lit however you orbit.
        let shine = ((target - eye).normalize() + glam::Vec3::new(0.3, -0.5, 0.0)).normalize();
        let light_dir = [shine.x, shine.y, shine.z, 0.0];

        for object in &self.objects {
            let scene_uniform = SceneUniform {
                view_proj: view_proj.to_cols_array_2d(),
                 model: object.model.to_cols_array_2d(),
                // The inverse transpose keeps normals correct under
                // non-uniform scale.
                normal_matrix: object.model.inverse().transpose().to_cols_array_2d(),
                light_dir,
                light_color: [1.0, 1.0, 1.0, 1.0],
                base_color: object.base_color,
                material: object.material,
                camera_pos: [eye.x, eye.y, eye.z, 1.0],
            };
                self.queue.write_buffer(
                &object.uniform_buffer,
                0,
                bytemuck::cast_slice(&[scene_uniform]),
            );
        }

                // The ground grid reads the camera from its own uniform.
        let grid_line = grid_line_brightness(
            self.clear_color.r,
            self.clear_color.g,
            self.clear_color.b,
        );
        // With a gradient, the lines are light or dark for the middle of it.
        let grid_line = if self.gradient_on {
            grid_line_brightness(
                (self.clear_color.r + self.background_top[0] as f64) * 0.5,
                (self.clear_color.g + self.background_top[1] as f64) * 0.5,
                (self.clear_color.b + self.background_top[2] as f64) * 0.5,
            )
        } else {
            grid_line
        };
        let grid_uniform = GridUniform {
        view_proj: view_proj.to_cols_array_2d(),
            inverse_view_proj: view_proj.inverse().to_cols_array_2d(),
            camera_pos: [eye.x, eye.y, eye.z, 1.0],
            params: [self.orbit_distance, 0.0, 0.0, 0.0],
            line_color: [grid_line, grid_line, grid_line, 1.0],
        };
        self.queue
            .write_buffer(&self.grid_buffer, 0, bytemuck::bytes_of(&grid_uniform));

        // The background gradient runs over the output frame when looking
        // through the scene camera, and over the whole viewport otherwise.
        let (frame_top, frame_height) = match self.camera_frame_rect() {
            Some((_, y, _, height)) => (y, height),
            None => (0.0, self.height as f32),
        };
        let background = background_uniform(
            self.background_top,
            [
                self.clear_color.r as f32,
                self.clear_color.g as f32,
                self.clear_color.b as f32,
            ],
            frame_top,
            frame_height,
        );
        self.queue
            .write_buffer(&self.background_buffer, 0, bytemuck::bytes_of(&background));
        let mut encoder = self
        .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("husk_cube_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: self.clear_color.r,
                            g: self.clear_color.g,
                            b: self.clear_color.b,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // The background gradient goes first, behind everything.
            if self.gradient_on {
                pass.set_pipeline(&self.background_pipeline);
                pass.set_bind_group(0, &self.background_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(2, &self.lights_bind_group, &[]);
            pass.set_bind_group(3, &self.world_bind_group, &[]);
            for object in &self.objects {
                pass.set_bind_group(0, &object.bind_group, &[]);
                pass.set_bind_group(1, &object.bone_bind_group, &[]);
                pass.set_vertex_buffer(0, object.vertex_buffer.slice(..));
                pass.set_index_buffer(object.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                                pass.draw_indexed(0..object.num_indices, 0, 0..1);
            }

            // The ground grid goes last, so objects hide it where they are
            // in front and it shows through where they cross the ground.
            // Looking through the scene camera, it stays off: that view is
            // what the video will show.
            if self.show_grid && self.camera_through().is_none() {
                pass.set_pipeline(&self.grid_pipeline);
                pass.set_bind_group(0, &self.grid_bind_group, &[]);
                pass.draw(0..3, 0..1);
            }

            }

        let unpadded_bytes_per_row = self.width * BYTES_PER_PIXEL;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) / align * align;

        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.output_buffer,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(Some(encoder.finish()));

        let buffer_slice = self.output_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).ok();
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().expect("Failed to map buffer");

        {
            let mapped = buffer_slice.get_mapped_range();
            self.last_frame.clear();

            if unpadded_bytes_per_row == padded_bytes_per_row {
                // No padding needed — common case at aligned resolutions.
                self.last_frame.extend_from_slice(&mapped);
            } else {
                for row in 0..self.height as usize {
                    let start = row * padded_bytes_per_row as usize;
                    let end = start + unpadded_bytes_per_row as usize;
                    self.last_frame.extend_from_slice(&mapped[start..end]);
                }
            }
        }
        self.output_buffer.unmap();

        // Average frame time over every 120 frames, including the GPU
        // readback, so the number reflects what a real frame costs.
        self.frame_time_total += frame_start.elapsed();
        self.frame_time_count += 1;
        if self.frame_time_count == 120 {
            eprintln!(
                "Husk: {} object(s), average frame time {:.1} ms over {} frames",
                self.objects.len(),
                self.frame_time_total.as_secs_f64() * 1000.0 / self.frame_time_count as f64,
                self.frame_time_count
            );
            self.frame_time_total = Duration::ZERO;
            self.frame_time_count = 0;
        }
    }

    pub fn frame_ptr(&self) -> *const u8 {
        self.last_frame.as_ptr()
    }

    pub fn frame_len(&self) -> usize {
        self.last_frame.len()
    }

    pub fn mesh_positions(&self) -> &[[f32; 3]] {
        &self.objects[self.active_object].mesh_data.positions
    }

    /// The active mesh's triangle indices (three per triangle), needed to
    /// voxelize the mesh for geodesic skin weighting.
    pub fn mesh_indices(&self) -> &[u32] {
        &self.objects[self.active_object].mesh_data.indices
    }

    pub fn update_vertex_weights(&self, weights: &[crate::rig::skinning::VertexWeights]) {
        let object = &self.objects[self.active_object];
        let updated: Vec<Vertex> = object
            .mesh_data
            .positions
            .iter()
            .zip(object.mesh_data.normals.iter())
            .zip(weights.iter())
            .map(|((position, normal), w)| Vertex {
                position: *position,
                normal: *normal,
                joint_indices: w.joint_indices,
                weights: w.weights,
            })
            .collect();

        self.queue
            .write_buffer(&object.vertex_buffer, 0, bytemuck::cast_slice(&updated));
    }

    pub fn set_object_bone_matrices(&self, index: usize, matrices: &[glam::Mat4]) {
        let Some(object) = self.objects.get(index) else {
            return;
        };
        let mut data: Vec<[[f32; 4]; 4]> = matrices.iter().map(|m| m.to_cols_array_2d()).collect();
        data.resize(MAX_BONES, glam::Mat4::IDENTITY.to_cols_array_2d());
        self.queue
            .write_buffer(&object.bone_matrices_buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn set_bone_matrices(&self, matrices: &[glam::Mat4]) {
    let mut data: Vec<[[f32; 4]; 4]> = matrices.iter().map(|m| m.to_cols_array_2d()).collect();
        data.resize(MAX_BONES, glam::Mat4::IDENTITY.to_cols_array_2d());
        self.queue.write_buffer(
            &self.objects[self.active_object].bone_matrices_buffer,
            0,
            bytemuck::cast_slice(&data),
        );
    }

    /// Tells the renderer what the world looks like: the colour behind the
    /// objects and how strong the studio light is. The scene is the source
    /// of truth, so this is called after anything that changes it.
    pub fn set_world(&mut self, world: &crate::scene::world::World) {
        self.background_top = world.background_top;
        self.gradient_on = world.gradient;
        self.clear_color = wgpu::Color {
        r: world.background[0] as f64,
            g: world.background[1] as f64,
            b: world.background[2] as f64,
            a: 1.0,
        };
        self.queue.write_buffer(
            &self.world_buffer,
            0,
            bytemuck::bytes_of(&world.shader_values()),
        );
    }

    /// Tells the renderer which lights the scene has (at most
    /// `MAX_LIGHTS` are used). The scene is the source of truth, so this
    /// is called after anything that changes them. With no lights the
    /// shader uses the light that follows the camera.
    pub fn set_lights(
        &self,
        lights: &[(crate::scene::transform::Transform, crate::scene::light::Light)],
    ) {
        self.queue.write_buffer(
            &self.lights_buffer,
            0,
            bytemuck::bytes_of(&lights_uniform(lights)),
        );
    }

    pub fn project_to_screen(&self, world_point: glam::Vec3) -> Option<(f32, f32)> {
    crate::render::picking::project_to_screen(
            self.last_view_proj,
            self.width as f32,
            self.height as f32,
            world_point,
        )

    }

    /// The camera's right-hand direction on screen, as of the last rendered frame.
    pub fn view_right(&self) -> glam::Vec3 {
        let inverse = self.last_view_proj.inverse();
        let middle = inverse.project_point3(glam::Vec3::new(0.0, 0.0, 0.5));
        let right = inverse.project_point3(glam::Vec3::new(1.0, 0.0, 0.5));
        (right - middle).normalize()
    }

    /// The direction the camera is looking, as of the last rendered frame.
    pub fn view_forward(&self) -> glam::Vec3 {
        let inverse = self.last_view_proj.inverse();
        let near = inverse.project_point3(glam::Vec3::new(0.0, 0.0, 0.0));
        let far = inverse.project_point3(glam::Vec3::new(0.0, 0.0, 1.0));
        (far - near).normalize()
    }

    /// Where the ray through screen pixel (x, y) crosses the plane that
    /// passes through `plane_point` and faces the camera. Dragging on it
    /// moves a point in any direction the camera can see, at constant depth.
    pub fn screen_to_view_plane(
        &self,
        screen_x: f32,
        screen_y: f32,
        plane_point: glam::Vec3,
    ) -> Option<glam::Vec3> {
        let ndc_x = screen_x / self.width as f32 * 2.0 - 1.0;
        let ndc_y = 1.0 - screen_y / self.height as f32 * 2.0;
        let inverse = self.last_view_proj.inverse();
        let near = inverse.project_point3(glam::Vec3::new(ndc_x, ndc_y, 0.0));
        let far = inverse.project_point3(glam::Vec3::new(ndc_x, ndc_y, 1.0));
        let ray_direction = far - near;

        // The camera looks along the ray through the middle of the screen.
        let middle_near = inverse.project_point3(glam::Vec3::new(0.0, 0.0, 0.0));
        let middle_far = inverse.project_point3(glam::Vec3::new(0.0, 0.0, 1.0));
        let normal = (middle_far - middle_near).normalize();

        let denominator = normal.dot(ray_direction);
        if denominator.abs() < 1e-6 {
            return None;
        }
        let t = normal.dot(plane_point - near) / denominator;
        Some(near + ray_direction * t)
    }

    /// Unprojects a screen pixel into a world-space ray using the last
    /// frame's camera, and returns where that ray crosses the plane
    /// z = `plane_z`. None if the ray runs parallel to the plane.
    pub fn screen_to_plane_z(&self, screen_x: f32, screen_y: f32, plane_z: f32) -> Option<glam::Vec3> {
        let ndc_x = screen_x / self.width as f32 * 2.0 - 1.0;
        let ndc_y = 1.0 - screen_y / self.height as f32 * 2.0;
        let inverse = self.last_view_proj.inverse();
        let near = inverse.project_point3(glam::Vec3::new(ndc_x, ndc_y, 0.0));
        let far = inverse.project_point3(glam::Vec3::new(ndc_x, ndc_y, 1.0));
        let direction = far - near;
        if direction.z.abs() < 1e-6 {
            return None;
        }
        let t = (plane_z - near.z) / direction.z;
        Some(near + direction * t)
    }

    /// Where the cursor at screen pixel (x, y) falls along an axis line
    /// through `axis_origin` (see `picking::closest_param_on_axis`), using
    /// the camera from the last rendered frame.
    pub fn axis_drag_param(
        &self,
        screen_x: f32,
        screen_y: f32,
        axis_origin: glam::Vec3,
        axis_dir: glam::Vec3,
    ) -> Option<f32> {
        let ray = crate::render::picking::Ray::from_screen(
            screen_x,
            screen_y,
            self.width as f32,
            self.height as f32,
            self.last_view_proj,
        );
        crate::render::picking::closest_param_on_axis(&ray, axis_origin, axis_dir)
    }

    /// The screen points of a gizmo ring (see `picking::ring_screen_points`),
    /// using the camera from the last rendered frame.
    pub fn gizmo_ring(
        &self,
        ring_origin: glam::Vec3,
        ring_axis: glam::Vec3,
        pixels: f32,
        segments: usize,
    ) -> Option<Vec<(f32, f32)>> {
        crate::render::picking::ring_screen_points(
            self.last_view_proj,
            self.width as f32,
            self.height as f32,
            ring_origin,
            ring_axis,
            pixels,
            segments,
        )
    }

    /// The angle of the cursor at screen pixel (x, y) around the axis
    /// through `ring_origin` (see `picking::angle_around_axis`), using the
    /// camera from the last rendered frame.
    pub fn ring_drag_angle(
        &self,
        screen_x: f32,
        screen_y: f32,
        ring_origin: glam::Vec3,
        ring_axis: glam::Vec3,
    ) -> Option<f32> {
        let ray = crate::render::picking::Ray::from_screen(
            screen_x,
            screen_y,
            self.width as f32,
            self.height as f32,
            self.last_view_proj,
        );
        crate::render::picking::angle_around_axis(&ray, ring_origin, ring_axis)
    }

    /// The screen pixels to draw one gizmo axis between (see
    /// `picking::axis_screen_ends`), using the camera from the last
    /// rendered frame.
    pub fn gizmo_axis_ends(
        &self,
        axis_origin: glam::Vec3,
        axis_dir: glam::Vec3,
        pixels: f32,
    ) -> Option<((f32, f32), (f32, f32))> {
        crate::render::picking::axis_screen_ends(
            self.last_view_proj,
            self.width as f32,
            self.height as f32,
            axis_origin,
            axis_dir,
            pixels,
        )
    }

    /// The index of the object under screen pixel (x, y), using the camera

    /// from the last rendered frame and each object's own placement. The
    /// nearest object along the ray wins; None if the pixel shows empty
    /// space. Tests the objects' rest shapes, not their posed ones.
    pub fn pick_object(&self, screen_x: f32, screen_y: f32) -> Option<usize> {
        self.pick_object_except(screen_x, screen_y, &[])
    }

    /// Like `pick_object`, but an object whose entry in `locked` is true
    /// cannot be hit: the ray goes through it to whatever is behind.
    pub fn pick_object_except(
        &self,
        screen_x: f32,
        screen_y: f32,
        locked: &[bool],
    ) -> Option<usize> {
    let ray = crate::render::picking::Ray::from_screen(
            screen_x,
            screen_y,
            self.width as f32,
            self.height as f32,
            self.last_view_proj,
        );
                // A locked object is given an empty mesh, which no ray can hit, so
        // every object keeps its place in the list and its index.
        let no_positions: &[[f32; 3]] = &[];
        let no_indices: &[u32] = &[];
        let placed: Vec<(glam::Mat4, &[[f32; 3]], &[u32])> = self
            .objects
            .iter()
            .enumerate()
            .map(|(index, object)| {
                if locked.get(index).copied().unwrap_or(false) {
                    (object.model, no_positions, no_indices)
                } else {
                    (
                        object.model,
                        object.mesh_data.positions.as_slice(),
                        object.mesh_data.indices.as_slice(),
                    )
                }
            })
            .collect();
        crate::render::picking::pick_nearest(&ray, &placed)
    }

    /// Makes `index` the object the tools act on. False, changing
    /// nothing, if there is no such object.
    pub fn set_active_object(&mut self, index: usize) -> bool {
        if index >= self.objects.len() {
            return false;
        }
        self.active_object = index;
        true
    }

    /// Removes the object at `index` and frees its GPU buffers. Follows
    /// the same rules as `Scene::remove_object`: object 0 and unknown
    /// indices are refused (false, nothing changes).
    pub fn remove_object(&mut self, index: usize) -> bool {
        if index == 0 || index >= self.objects.len() {
            return false;
        }
        self.objects.remove(index);
        self.active_object = crate::scene::active_after_removal(self.active_object, index);
        true
    }

    /// Sets where the object at `index` is drawn. The scene holds the
    /// real transform; this is its GPU-side copy. False, changing
    /// nothing, if there is no such object.
    pub fn set_object_model(&mut self, index: usize, model: glam::Mat4) -> bool {
        match self.objects.get_mut(index) {
            Some(object) => {
                object.model = model;
                true
            }
            None => false,
        }
    }

    /// Sets how the object at `index` looks. The scene holds the real
    /// material; this is its GPU-side copy. False, changing nothing, if
    /// there is no such object.
    pub fn set_object_material(
        &mut self,
        index: usize,
        material: &crate::scene::material::Material,
    ) -> bool {
        match self.objects.get_mut(index) {
            Some(object) => {
                let c = material.base_color;
                object.base_color = [c[0], c[1], c[2], 1.0];
                object.material = material.shader_values();
                true
            }
            None => false,
        }
    }
        pub fn pick_inside(&self, screen_x: f32, screen_y: f32) -> Option<glam::Vec3> {
        let ray = crate::render::picking::Ray::from_screen(
            screen_x,
            screen_y,
            self.width as f32,
            self.height as f32,
            self.last_view_proj,
        );
        let object = &self.objects[self.active_object];
        let mesh = &object.mesh_data;
        let local = ray.to_local(object.model);
        crate::render::picking::ray_mesh_midpoint(&local, &mesh.positions, &mesh.indices)
    }

    /// Casts a ray from a screen-space point (in pixels, matching this
    /// renderer's own width/height) using the camera matrix from the most
    /// recently rendered frame, and returns the world-space point where it
    /// hits the active object's mesh, if any, wherever that object is
    /// placed.
    pub fn pick(&self, screen_x: f32, screen_y: f32) -> Option<glam::Vec3> {
        let ray = crate::render::picking::Ray::from_screen(
            screen_x,
            screen_y,
            self.width as f32,
            self.height as f32,
            self.last_view_proj,
        );
        let object = &self.objects[self.active_object];
        let mesh = &object.mesh_data;
        // Test in the object's own space. A distance t along the local ray
        // is the same t along the world ray, so the scene point comes
        // straight from the world ray.
        let local = ray.to_local(object.model);
        crate::render::picking::ray_mesh_intersection(&local, &mesh.positions, &mesh.indices)
            .map(|hit| ray.origin + ray.direction * hit.distance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::light::{Light, LightKind};
    use crate::scene::transform::Transform;

    #[test]
    fn the_lights_uniform_matches_the_layout_the_shader_expects() {
        // 16 bytes of count, then eight lights of three vec4s each.
        assert_eq!(std::mem::size_of::<GpuLight>(), 48);
        assert_eq!(std::mem::size_of::<LightsUniform>(), 16 + 8 * 48);
    }

    #[test]
    fn a_spot_in_view_lands_exactly_where_the_fractions_say_on_screen() {
        let eye = glam::Vec3::new(0.0, 2.0, 5.0);
        let target = glam::Vec3::new(0.0, 1.0, 0.0);
        let fov = std::f32::consts::FRAC_PI_4;
        let aspect = 16.0 / 9.0;
        let view_proj = glam::Mat4::perspective_rh(fov, aspect, 0.1, 100.0)
            * glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);
        for (depth, right, up) in [(0.55, 0.55, 0.35), (0.7, -0.6, 0.6), (0.7, 0.6, -0.6)] {
            let spot = spot_in_view(eye, target, fov, aspect, depth, right, up);
            let clip = view_proj * spot.extend(1.0);
            assert!(clip.w > 0.0, "the spot was behind the camera");
            // The fractions are exactly where it lands, in screen units
            // that run from -1 (left, bottom) to 1 (right, top).
            assert!((clip.x / clip.w - right).abs() < 1e-3);
            assert!((clip.y / clip.w - up).abs() < 1e-3);
        }
    }

    #[test]
    fn a_spot_in_view_works_when_looking_straight_down_or_at_the_eye() {
        let fov = std::f32::consts::FRAC_PI_4;
        let down = spot_in_view(
            glam::Vec3::new(0.0, 5.0, 0.0),
            glam::Vec3::ZERO,
            fov,
            1.5,
            0.7,
            -0.6,
            0.6,
        );
        assert!(down.is_finite());
        let at_eye = spot_in_view(glam::Vec3::ONE, glam::Vec3::ONE, fov, 1.5, 0.7, 0.5, 0.5);
        assert!(at_eye.is_finite());
    }

    #[test]
    fn the_grid_uniform_matches_the_layout_the_shader_expects() {
    // Two matrices and three vec4s, all 16-byte aligned.
        assert_eq!(std::mem::size_of::<GridUniform>(), 2 * 64 + 3 * 16);
    }

    #[test]
    fn grid_lines_are_light_on_a_dark_background_and_dark_on_a_light_one() {
        assert_eq!(grid_line_brightness(0.05, 0.05, 0.08), 0.8);
        assert_eq!(grid_line_brightness(1.0, 1.0, 1.0), 0.1);
        assert_eq!(grid_line_brightness(0.53, 0.71, 0.91), 0.1);
    }

    #[test]
    fn the_background_uniform_matches_the_layout_the_shader_expects() {
        // Three vec4s: top colour, bottom colour, and the span.
        assert_eq!(std::mem::size_of::<BackgroundUniform>(), 3 * 16);
    }

    #[test]
    fn the_background_packs_its_colours_and_the_rows_it_runs_over() {
        let packed = background_uniform([0.1, 0.2, 0.3], [0.4, 0.5, 0.6], 10.0, 200.0);
        assert_eq!(packed.top, [0.1, 0.2, 0.3, 1.0]);
        assert_eq!(packed.bottom, [0.4, 0.5, 0.6, 1.0]);
        assert_eq!(packed.span, [10.0, 200.0, 0.0, 0.0]);
    }

    #[test]
    fn no_lights_packs_to_a_count_of_zero() {
    let uniform = lights_uniform(&[]);
        assert_eq!(uniform.count, [0, 0, 0, 0]);
    }

    #[test]
    fn a_sun_packs_its_direction_and_scaled_colour() {
        let mut transform = Transform::default();
        transform.rotation = glam::Quat::from_rotation_y(90f32.to_radians());
        let sun = Light {
            kind: LightKind::Sun,
            color: [1.0, 0.5, 0.25],
            intensity: 2.0,
        };
        let uniform = lights_uniform(&[(transform, sun)]);
        assert_eq!(uniform.count[0], 1);
        let packed = uniform.items[0];
        assert_eq!(packed.position[3], 0.0);
        // Rays travel along the object's -Z, which a quarter turn about Y
        // sends to -X.
        assert!((packed.direction[0] + 1.0).abs() < 1e-5);
        assert!(packed.direction[1].abs() < 1e-5 && packed.direction[2].abs() < 1e-5);
        assert_eq!(packed.color, [2.0, 1.0, 0.5, 0.0]);
    }

    #[test]
    fn a_point_light_packs_its_position_and_kind() {
        let mut transform = Transform::default();
        transform.translation = glam::Vec3::new(1.0, 2.0, 3.0);
        let lamp = Light {
            kind: LightKind::Point,
            color: [1.0, 1.0, 1.0],
            intensity: 40.0,
        };
        let uniform = lights_uniform(&[(transform, lamp)]);
        assert_eq!(uniform.count[0], 1);
        assert_eq!(uniform.items[0].position, [1.0, 2.0, 3.0, 1.0]);
        assert_eq!(uniform.items[0].color, [40.0, 40.0, 40.0, 0.0]);
    }

    #[test]
    fn lights_past_the_cap_are_left_out() {
        let many: Vec<_> = (0..12)
            .map(|_| (Transform::default(), Light::point()))
            .collect();
        let uniform = lights_uniform(&many);
        assert_eq!(uniform.count[0] as usize, MAX_LIGHTS);
    }
}