use wgpu::util::DeviceExt;


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
    mesh_data: crate::asset_import::MeshData,
    last_view_proj: glam::Mat4,
    bone_matrices_buffer: wgpu::Buffer,
    bone_bind_group: wgpu::BindGroup,
    orbit_yaw: f32,
    orbit_pitch: f32,
    orbit_distance: f32,
    orbit_target: glam::Vec3,
}
const BYTES_PER_PIXEL: u32 = 4;


impl Renderer {
    pub fn new(width: u32, height: u32, mesh: crate::asset_import::MeshData) -> Self {
        pollster::block_on(Self::new_async(width, height, mesh))
    }

    async fn new_async(width: u32, height: u32, mesh: crate::asset_import::MeshData) -> Self {        
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
        let padded_bytes_per_row =
            (unpadded_bytes_per_row + align - 1) / align * align;

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

        let identity_bone_matrices: Vec<[[f32; 4]; 4]> =
            vec![glam::Mat4::IDENTITY.to_cols_array_2d(); MAX_BONES];

        let bone_matrices_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_bone_matrices_buffer"),
            contents: bytemuck::cast_slice(&identity_bone_matrices),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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

        let bone_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("husk_bone_bind_group"),
            layout: &bone_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: bone_matrices_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("husk_pipeline_layout"),
                bind_group_layouts: &[&bind_group_layout, &bone_bind_group_layout],
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

        let mesh_vertices: Vec<Vertex> = mesh
            .positions
            .iter()
            .zip(mesh.normals.iter())
            .map(|(position, normal)| Vertex {
                position: *position,
                normal: *normal,
                // Defaults to fully-weighted-to-bone-0 with an identity
                // matrix there a mathematical no-op until real skin
                joint_indices: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            })
            .collect();



        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_mesh_vertex_buffer"),
            contents: bytemuck::cast_slice(&mesh_vertices),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("husk_mesh_index_buffer"),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let num_indices = mesh.indices.len() as u32;

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

        Self {
            device,
            queue,
            texture,
            output_buffer,
            width,
            height,
           // frame: 0,
            last_frame: Vec::new(),
            pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
            uniform_buffer,
            bind_group,
            depth_texture,
            depth_view,
            mesh_data: mesh,
            last_view_proj: glam::Mat4::IDENTITY,
            bone_matrices_buffer,
            bone_bind_group,
            // Matches the original fixed camera exactly (eye (0, 1.5, 3),
            // looking at the origin) just expressed as orbit parameters
            // now instead of a hardcoded eye position.
            orbit_yaw: 0.0,
            orbit_pitch: 0.463_65,
            orbit_distance: 3.354_102,
            orbit_target,
        }
    }

    /// Adjusts the camera's orbit angles by the given deltas (radians).
    /// Pitch is clamped to avoid flipping over at the poles.
    pub fn orbit(&mut self, delta_yaw: f32, delta_pitch: f32) {
        self.orbit_yaw += delta_yaw;
        self.orbit_pitch = (self.orbit_pitch + delta_pitch).clamp(-1.5, 1.5);
    }

    /// Moves the camera closer (a factor below 1) or farther away (above 1),
    /// within sensible limits.
    pub fn zoom(&mut self, factor: f32) {
        self.orbit_distance = (self.orbit_distance * factor).clamp(0.3, 500.0);
    }

    pub fn render_frame(&mut self) {
    let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let aspect = self.width as f32 / self.height as f32;
        let projection =
            glam::Mat4::perspective_rh(std::f32::consts::FRAC_PI_4, aspect, 0.1, 100.0);

        let target = self.orbit_target;
        let eye = target
            + self.orbit_distance
                * glam::Vec3::new(
                    self.orbit_pitch.cos() * self.orbit_yaw.sin(),
                    self.orbit_pitch.sin(),
                    self.orbit_pitch.cos() * self.orbit_yaw.cos(),
                );
        let view_matrix = glam::Mat4::look_at_rh(eye, target, glam::Vec3::Y);

        let model = glam::Mat4::IDENTITY;
        let view_proj = projection * view_matrix * model;        
        self.last_view_proj = view_proj;

        let scene_uniform = SceneUniform {
            view_proj: view_proj.to_cols_array_2d(),
            model: model.to_cols_array_2d(),
            light_dir: {
                // A light that travels with the camera: it shines from the
                // viewer's side, a little from above and from the left, so
                // whatever faces the camera is lit however you orbit.
                let shine = ((target - eye).normalize() + glam::Vec3::new(0.3, -0.5, 0.0))
                    .normalize();
                [shine.x, shine.y, shine.z, 0.0]
            },
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
            pass.set_bind_group(1, &self.bone_bind_group, &[]);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);           
            pass.draw_indexed(0..self.num_indices, 0, 0..1);
        }


        let unpadded_bytes_per_row = self.width * BYTES_PER_PIXEL;
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row =
            (unpadded_bytes_per_row + align - 1) / align * align;

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
                // Strip each row's padding, keeping only the real pixel
                // bytes, so everything downstream (the Flutter pixel
                // buffer, in particular) still sees a tightly-packed
                // image at exactly width * height * 4 bytes.
                for row in 0..self.height as usize {
                    let start = row * padded_bytes_per_row as usize;
                    let end = start + unpadded_bytes_per_row as usize;
                    self.last_frame.extend_from_slice(&mapped[start..end]);
                }
            }
        }
        self.output_buffer.unmap();

    }

    pub fn frame_ptr(&self) -> *const u8 {
        self.last_frame.as_ptr()
    }

    pub fn frame_len(&self) -> usize {
        self.last_frame.len()
    }

    /// The active mesh's raw vertex positions, in the same order used
    /// everywhere else (skinning, picking) needed so weight computation
    /// can be done against whatever mesh is actually loaded right now.
    pub fn mesh_positions(&self) -> &[[f32; 3]] {
        &self.mesh_data.positions
    }

    /// The active mesh's triangle indices (three per triangle), needed to
    /// voxelize the mesh for geodesic skin weighting.
    pub fn mesh_indices(&self) -> &[u32] {
        &self.mesh_data.indices
    }
    pub fn update_vertex_weights(&self, weights: &[crate::skinning::VertexWeights]) {
        let updated: Vec<Vertex> = self
            .mesh_data
            .positions
            .iter()
            .zip(self.mesh_data.normals.iter())
            .zip(weights.iter())
            .map(|((position, normal), w)| Vertex {
                position: *position,
                normal: *normal,
                joint_indices: w.joint_indices,
                weights: w.weights,
            })
            .collect();

        self.queue
            .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&updated));
    }

    pub fn set_bone_matrices(&self, matrices: &[glam::Mat4]) {
        let mut data: Vec<[[f32; 4]; 4]> = matrices.iter().map(|m| m.to_cols_array_2d()).collect();
        data.resize(MAX_BONES, glam::Mat4::IDENTITY.to_cols_array_2d());
        self.queue
            .write_buffer(&self.bone_matrices_buffer, 0, bytemuck::cast_slice(&data));
    }

    pub fn project_to_screen(&self, world_point: glam::Vec3) -> Option<(f32, f32)> {
        let clip = self.last_view_proj * world_point.extend(1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc_x = clip.x / clip.w;
        let ndc_y = clip.y / clip.w;
        Some((
            (ndc_x * 0.5 + 0.5) * self.width as f32,
            (0.5 - ndc_y * 0.5) * self.height as f32,
        ))
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

    /// Like `pick`, but returns the point halfway through the mesh along
    /// the ray (between where it enters and where it last exits) instead
    /// of the front surface, so bones land inside a limb, not on its skin.
    pub fn pick_inside(&self, screen_x: f32, screen_y: f32) -> Option<glam::Vec3> {
        let ray = crate::picking::Ray::from_screen(
            screen_x,
            screen_y,
            self.width as f32,
            self.height as f32,
            self.last_view_proj,
        );
        crate::picking::ray_mesh_midpoint(&ray, &self.mesh_data.positions, &self.mesh_data.indices)
    }

    /// Casts a ray from a screen-space point (in pixels, matching this
    /// renderer's own width/height) using the camera matrix from the most
    /// recently rendered frame, and returns the world-space point where it
    /// hits the mesh, if any.
    pub fn pick(&self, screen_x: f32, screen_y: f32) -> Option<glam::Vec3> {
        let ray = crate::picking::Ray::from_screen(
            screen_x,
            screen_y,
            self.width as f32,
            self.height as f32,
            self.last_view_proj,
        );
        crate::picking::ray_mesh_intersection(&ray, &self.mesh_data.positions, &self.mesh_data.indices)
            .map(|hit| hit.point)
    }
}