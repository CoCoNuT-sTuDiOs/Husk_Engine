use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
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
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ],
        }
    }
}

const CUBE_VERTICES: &[Vertex] = &[
    // Front (+z)
    Vertex { position: [-0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0] },
    Vertex { position: [0.5, -0.5, 0.5], normal: [0.0, 0.0, 1.0] },
    Vertex { position: [0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0] },
    Vertex { position: [-0.5, 0.5, 0.5], normal: [0.0, 0.0, 1.0] },
    // Back (-z)
    Vertex { position: [0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
    Vertex { position: [-0.5, -0.5, -0.5], normal: [0.0, 0.0, -1.0] },
    Vertex { position: [-0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0] },
    Vertex { position: [0.5, 0.5, -0.5], normal: [0.0, 0.0, -1.0] },
    // Right (+x)
    Vertex { position: [0.5, -0.5, 0.5], normal: [1.0, 0.0, 0.0] },
    Vertex { position: [0.5, -0.5, -0.5], normal: [1.0, 0.0, 0.0] },
    Vertex { position: [0.5, 0.5, -0.5], normal: [1.0, 0.0, 0.0] },
    Vertex { position: [0.5, 0.5, 0.5], normal: [1.0, 0.0, 0.0] },
    // Left (-x)
    Vertex { position: [-0.5, -0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
    Vertex { position: [-0.5, -0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
    Vertex { position: [-0.5, 0.5, 0.5], normal: [-1.0, 0.0, 0.0] },
    Vertex { position: [-0.5, 0.5, -0.5], normal: [-1.0, 0.0, 0.0] },
    // Top (+y)
    Vertex { position: [-0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
    Vertex { position: [0.5, 0.5, 0.5], normal: [0.0, 1.0, 0.0] },
    Vertex { position: [0.5, 0.5, -0.5], normal: [0.0, 1.0, 0.0] },
    Vertex { position: [-0.5, 0.5, -0.5], normal: [0.0, 1.0, 0.0] },
    // Bottom (-y)
    Vertex { position: [-0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
    Vertex { position: [0.5, -0.5, -0.5], normal: [0.0, -1.0, 0.0] },
    Vertex { position: [0.5, -0.5, 0.5], normal: [0.0, -1.0, 0.0] },
    Vertex { position: [-0.5, -0.5, 0.5], normal: [0.0, -1.0, 0.0] },
];

const CUBE_INDICES: &[u16] = &[
    0, 1, 2, 2, 3, 0, // front
    4, 5, 6, 6, 7, 4, // back
    8, 9, 10, 10, 11, 8, // right
    12, 13, 14, 14, 15, 12, // left
    16, 17, 18, 18, 19, 16, // top
    20, 21, 22, 22, 23, 20, // bottom
];

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneUniform {
    view_proj: [[f32; 4]; 4],
    model: [[f32; 4]; 4],
    light_dir: [f32; 4],
    light_color: [f32; 4],
    base_color: [f32; 4],
    material: [f32; 4],
}

pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    output_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    frame: u32,
    last_frame: Vec<u8>,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    #[allow(dead_code)]
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
}

const BYTES_PER_PIXEL: u32 = 4;

impl Renderer {
    pub fn new(width: u32, height: u32) -> Self {
        pollster::block_on(Self::new_async(width, height))
    }

    async fn new_async(width: u32, height: u32) -> Self {
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

        let output_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("husk_readback_buffer"),
            size: (width * height * BYTES_PER_PIXEL) as u64,
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

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("husk_scene_uniform_buffer"),
            size: std::mem::size_of::<SceneUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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


        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_camera_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("husk_pipeline_layout"),
                bind_group_layouts: &[&bind_group_layout],
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

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_cube_vertex_buffer"),
            contents: bytemuck::cast_slice(CUBE_VERTICES),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_cube_index_buffer"),
            contents: bytemuck::cast_slice(CUBE_INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            device,
            queue,
            texture,
            output_buffer,
            width,
            height,
            frame: 0,
            last_frame: Vec::new(),
            pipeline,
            vertex_buffer,
            index_buffer,
            num_indices: CUBE_INDICES.len() as u32,
            uniform_buffer,
            bind_group,
            depth_texture,
            depth_view,
        }
    }

    pub fn render_frame(&mut self) {
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let angle = (self.frame % 360) as f32 * std::f32::consts::PI / 180.0;
        self.frame = self.frame.wrapping_add(1);

        let aspect = self.width as f32 / self.height as f32;
        let projection =
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);
        let eye = glam::Vec3::new(0.0, 1.5, 3.0);
        let view_matrix = glam::Mat4::look_at_rh(eye, glam::Vec3::ZERO, glam::Vec3::Y);
        let model = glam::Mat4::from_rotation_y(angle);
        let view_proj = projection * view_matrix * model;

        let scene_uniform = SceneUniform {
            view_proj: view_proj.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            light_dir: [-0.4, -1.0, -0.3, 0.0],
            light_color: [1.0, 1.0, 1.0, 1.0],
            base_color: [0.976, 0.451, 0.086, 1.0],
            material: [0.4, 0.2, 0.0, 0.0],
        };
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&[scene_uniform]),
        );


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
                            r: 0.05,
                            g: 0.05,
                            b: 0.08,
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

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }

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
                    bytes_per_row: Some(self.width * BYTES_PER_PIXEL),
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
            self.last_frame.extend_from_slice(&mapped);
        }
        self.output_buffer.unmap();
    }

    pub fn frame_ptr(&self) -> *const u8 {
        self.last_frame.as_ptr()
    }

    pub fn frame_len(&self) -> usize {
        self.last_frame.len()
    }
}