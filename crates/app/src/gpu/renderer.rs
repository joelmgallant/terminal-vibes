use anyhow::Result;
use std::sync::Arc;
use winit::window::Window;

use super::audio_texture::AudioTextureData;
use super::uniforms::Uniforms;

const FEEDBACK_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub struct GpuRenderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    // Viz pipeline
    pipeline: Option<wgpu::RenderPipeline>,
    bind_groups: Option<[wgpu::BindGroup; 2]>,
    bind_group_layout: Option<wgpu::BindGroupLayout>,
    audio_texture: Option<wgpu::Texture>,
    uniform_buffer: Option<wgpu::Buffer>,
    // Feedback buffer (ping-pong)
    feedback_textures: Option<[wgpu::Texture; 2]>,
    feedback_views: Option<[wgpu::TextureView; 2]>,
    feedback_index: usize,
    feedback_sampler: Option<wgpu::Sampler>,
    // Blit pipeline
    blit_pipeline: Option<wgpu::RenderPipeline>,
    blit_bind_group_layout: Option<wgpu::BindGroupLayout>,
    blit_bind_groups: Option<[wgpu::BindGroup; 2]>,
}

impl GpuRenderer {
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapter found"))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("terminal-vibes"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    ..Default::default()
                },
                None,
            )
            .await?;

        let size = window.inner_size();
        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            pipeline: None,
            bind_groups: None,
            bind_group_layout: None,
            audio_texture: None,
            uniform_buffer: None,
            feedback_textures: None,
            feedback_views: None,
            feedback_index: 0,
            feedback_sampler: None,
            blit_pipeline: None,
            blit_bind_group_layout: None,
            blit_bind_groups: None,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width > 0 && height > 0 {
            self.surface_config.width = width;
            self.surface_config.height = height;
            self.surface.configure(&self.device, &self.surface_config);
        }
    }

    pub fn init_pipeline(&mut self, fragment_wgsl: &str) -> Result<()> {
        let vertex_shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fullscreen_vert"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/fullscreen_vert.wgsl").into(),
                ),
            });

        let fragment_shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("fragment"),
                source: wgpu::ShaderSource::Wgsl(fragment_wgsl.into()),
            });

        let blit_shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("blit_frag"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blit.wgsl").into()),
            });

        // Audio texture (256x2)
        let audio_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("audio_data"),
            size: wgpu::Extent3d {
                width: 256,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let audio_texture_view = audio_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let audio_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("audio_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Feedback textures (screen-sized, ping-pong)
        let fb_size = wgpu::Extent3d {
            width: self.surface_config.width.max(1),
            height: self.surface_config.height.max(1),
            depth_or_array_layers: 1,
        };
        let fb_usage =
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING;
        let fb_desc = |label: &'static str| wgpu::TextureDescriptor {
            label: Some(label),
            size: fb_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FEEDBACK_FORMAT,
            usage: fb_usage,
            view_formats: &[],
        };
        let feedback_a = self.device.create_texture(&fb_desc("feedback_a"));
        let feedback_b = self.device.create_texture(&fb_desc("feedback_b"));
        let fb_view_a = feedback_a.create_view(&wgpu::TextureViewDescriptor::default());
        let fb_view_b = feedback_b.create_view(&wgpu::TextureViewDescriptor::default());
        let feedback_sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("feedback_sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Uniform buffer
        let uniform_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Viz bind group layout (bindings 0-4)
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("viz_bind_group_layout"),
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
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        // Two viz bind groups: [0] reads fb_b as prev, [1] reads fb_a as prev
        let bind_group_0 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz_bind_group_0"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&audio_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&audio_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&fb_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&feedback_sampler),
                },
            ],
        });
        let bind_group_1 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("viz_bind_group_1"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&audio_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&audio_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&fb_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&feedback_sampler),
                },
            ],
        });

        // Viz pipeline (renders to FEEDBACK_FORMAT offscreen texture)
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("viz_pipeline_layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("viz_pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &fragment_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: FEEDBACK_FORMAT,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Blit pipeline (copies feedback texture to swapchain)
        let blit_bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("blit_bind_group_layout"),
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
        let blit_pipeline_layout =
            self.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("blit_pipeline_layout"),
                    bind_group_layouts: &[&blit_bind_group_layout],
                    push_constant_ranges: &[],
                });
        let blit_pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("blit_pipeline"),
                layout: Some(&blit_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &vertex_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &blit_shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.surface_config.format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        // Blit bind groups: [0] reads fb_a, [1] reads fb_b
        let blit_bg_0 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bind_group_0"),
            layout: &blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&fb_view_a),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&feedback_sampler),
                },
            ],
        });
        let blit_bg_1 = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bind_group_1"),
            layout: &blit_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&fb_view_b),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&feedback_sampler),
                },
            ],
        });

        self.pipeline = Some(pipeline);
        self.bind_groups = Some([bind_group_0, bind_group_1]);
        self.bind_group_layout = Some(bind_group_layout);
        self.audio_texture = Some(audio_texture);
        self.uniform_buffer = Some(uniform_buffer);
        self.feedback_textures = Some([feedback_a, feedback_b]);
        self.feedback_views = Some([fb_view_a, fb_view_b]);
        self.feedback_index = 0;
        self.feedback_sampler = Some(feedback_sampler);
        self.blit_pipeline = Some(blit_pipeline);
        self.blit_bind_group_layout = Some(blit_bind_group_layout);
        self.blit_bind_groups = Some([blit_bg_0, blit_bg_1]);

        Ok(())
    }

    pub fn update_audio_texture(&self, tex_data: &AudioTextureData) {
        if let Some(texture) = &self.audio_texture {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &tex_data.data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(tex_data.width * 4),
                    rows_per_image: Some(tex_data.height),
                },
                wgpu::Extent3d {
                    width: tex_data.width,
                    height: tex_data.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    pub fn update_uniforms(&self, uniforms: &Uniforms) {
        if let Some(buffer) = &self.uniform_buffer {
            self.queue
                .write_buffer(buffer, 0, bytemuck::bytes_of(uniforms));
        }
    }

    /// Render viz shader to current feedback texture, then blit to swapchain.
    /// Call `flip_feedback()` after this to advance the ping-pong.
    pub fn render_shader(&self) -> Result<()> {
        let pipeline = self
            .pipeline
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No pipeline"))?;
        let bind_groups = self
            .bind_groups
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No bind groups"))?;
        let feedback_views = self
            .feedback_views
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No feedback views"))?;
        let blit_pipeline = self
            .blit_pipeline
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No blit pipeline"))?;
        let blit_bind_groups = self
            .blit_bind_groups
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("No blit bind groups"))?;

        let output = self.surface.get_current_texture()?;
        let swapchain_view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("render_encoder"),
            });

        // Pass 1: Render viz to current feedback texture (reads prev via bind group)
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("viz_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &feedback_views[self.feedback_index],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &bind_groups[self.feedback_index], &[]);
            pass.draw(0..3, 0..1);
        }

        // Pass 2: Blit current feedback texture to swapchain
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &swapchain_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(blit_pipeline);
            pass.set_bind_group(0, &blit_bind_groups[self.feedback_index], &[]);
            pass.draw(0..3, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    /// Advance the ping-pong index. Call after render_shader().
    pub fn flip_feedback(&mut self) {
        self.feedback_index = 1 - self.feedback_index;
    }

    pub fn render_clear(&self, r: f64, g: f64, b: f64) -> Result<()> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear_encoder"),
            });

        {
            let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r, g, b, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}
