pub struct Renderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture: wgpu::Texture,
    output_buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    frame: u32,
    last_frame: Vec<u8>,
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

        Self {
            device,
            queue,
            texture,
            output_buffer,
            width,
            height,
            frame: 0,
            last_frame: Vec::new(),
        }
    }

    pub fn render_frame(&mut self) {
        let view = self
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let hue = (self.frame % 256) as f64 / 255.0;
        self.frame = self.frame.wrapping_add(1);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("husk_clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: hue,
                            g: 0.2,
                            b: 1.0 - hue,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
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