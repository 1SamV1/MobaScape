use std::sync::Arc;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::scene::{HeightGrid, MeshData, SceneData, Vertex};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
    position: [f32; 4],
}

pub struct Camera {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
}

impl Camera {
    pub fn new(target: Vec3) -> Self {
        Self {
            target,
            yaw: 0.72,
            pitch: 0.86,
            distance: 4400.0,
        }
    }

    pub fn eye(&self) -> Vec3 {
        self.target
            + Vec3::new(
                self.distance * self.pitch.cos() * self.yaw.sin(),
                self.distance * self.pitch.sin(),
                self.distance * self.pitch.cos() * self.yaw.cos(),
            )
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        let view = Mat4::look_at_rh(self.eye(), self.target, Vec3::Y);
        let proj = Mat4::perspective_rh(50f32.to_radians(), aspect, 8.0, 60_000.0);
        proj * view
    }
}

struct GpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

impl GpuMesh {
    fn upload(device: &wgpu::Device, mesh: &MeshData, label: &str) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} vertices")),
            contents: bytemuck::cast_slice(&mesh.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{label} indices")),
            contents: bytemuck::cast_slice(&mesh.indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        Self {
            vertex_buffer,
            index_buffer,
            index_count: mesh.indices.len() as u32,
        }
    }
}

pub struct Renderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    camera_buffer: wgpu::Buffer,
    depth_view: wgpu::TextureView,
    terrain: GpuMesh,
    models: GpuMesh,
    dynamic_vertex_buffer: wgpu::Buffer,
    dynamic_index_buffer: wgpu::Buffer,
    dynamic_vertex_capacity: usize,
    dynamic_index_capacity: usize,
    dynamic_index_count: u32,
    _hud_uniform_buffer: wgpu::Buffer,
    hud_bind_group: wgpu::BindGroup,
    hud_vertex_buffer: wgpu::Buffer,
    hud_index_buffer: wgpu::Buffer,
    hud_vertex_capacity: usize,
    hud_index_capacity: usize,
    hud_index_count: u32,
    pub camera: Camera,
    pub size: (u32, u32),
}

impl Renderer {
    pub async fn new(
        window: Arc<Window>,
        scene: &SceneData,
        initial_camera: Camera,
    ) -> Result<Self> {
        let size = window.inner_size();
        let width = size.width.max(1);
        let height = size.height.max(1);

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .context("creating wgpu surface")?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
                apply_limit_buckets: false,
            })
            .await
            .context("requesting adapter")?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("rift device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .context("requesting device")?;

        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rift scene shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });

        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("camera uniform"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera layout"),
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
            label: Some("camera bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene pipeline layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        const ATTRS: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRS,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let hud_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &ATTRS,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let depth_view = create_depth_view(&device, width, height);
        let terrain = GpuMesh::upload(&device, &scene.terrain, "terrain");
        let models = GpuMesh::upload(&device, &scene.models, "models");

        let dynamic_vertex_capacity = 1024;
        let dynamic_index_capacity = 3072;
        let dynamic_vertex_buffer = create_dynamic_buffer(
            &device,
            "dynamic vertices",
            dynamic_vertex_capacity * std::mem::size_of::<Vertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let dynamic_index_buffer = create_dynamic_buffer(
            &device,
            "dynamic indices",
            dynamic_index_capacity * std::mem::size_of::<u32>(),
            wgpu::BufferUsages::INDEX,
        );

        let hud_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud camera uniform"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_uniform = CameraUniform {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            position: [0.0; 4],
        };
        queue.write_buffer(&hud_uniform_buffer, 0, bytemuck::bytes_of(&hud_uniform));
        let hud_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud camera bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: hud_uniform_buffer.as_entire_binding(),
            }],
        });

        let hud_vertex_capacity = 1024;
        let hud_index_capacity = 3072;
        let hud_vertex_buffer = create_dynamic_buffer(
            &device,
            "hud vertices",
            hud_vertex_capacity * std::mem::size_of::<Vertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let hud_index_buffer = create_dynamic_buffer(
            &device,
            "hud indices",
            hud_index_capacity * std::mem::size_of::<u32>(),
            wgpu::BufferUsages::INDEX,
        );

        let mut renderer = Self {
            surface,
            device,
            queue,
            config,
            pipeline,
            hud_pipeline,
            bind_group,
            camera_buffer,
            depth_view,
            terrain,
            models,
            dynamic_vertex_buffer,
            dynamic_index_buffer,
            dynamic_vertex_capacity,
            dynamic_index_capacity,
            dynamic_index_count: 0,
            _hud_uniform_buffer: hud_uniform_buffer,
            hud_bind_group,
            hud_vertex_buffer,
            hud_index_buffer,
            hud_vertex_capacity,
            hud_index_capacity,
            hud_index_count: 0,
            camera: initial_camera,
            size: (width, height),
        };
        renderer.update_camera();

        Ok(renderer)
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }
        self.size = (width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.depth_view = create_depth_view(&self.device, width, height);
    }

    pub fn update_camera(&mut self) {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let view_proj = self.camera.view_proj(aspect);
        let eye = self.camera.eye();
        let uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
            position: [eye.x, eye.y, eye.z, 1.0],
        };
        self.queue
            .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    pub fn set_dynamic_mesh(&mut self, vertices: &[Vertex], indices: &[u32]) {
        if vertices.len() > self.dynamic_vertex_capacity {
            let capacity = vertices.len().next_power_of_two();
            self.dynamic_vertex_buffer = create_dynamic_buffer(
                &self.device,
                "dynamic vertices",
                capacity * std::mem::size_of::<Vertex>(),
                wgpu::BufferUsages::VERTEX,
            );
            self.dynamic_vertex_capacity = capacity;
        }
        if indices.len() > self.dynamic_index_capacity {
            let capacity = indices.len().next_power_of_two();
            self.dynamic_index_buffer = create_dynamic_buffer(
                &self.device,
                "dynamic indices",
                capacity * std::mem::size_of::<u32>(),
                wgpu::BufferUsages::INDEX,
            );
            self.dynamic_index_capacity = capacity;
        }
        if !vertices.is_empty() {
            self.queue.write_buffer(
                &self.dynamic_vertex_buffer,
                0,
                bytemuck::cast_slice(vertices),
            );
        }
        if !indices.is_empty() {
            self.queue
                .write_buffer(&self.dynamic_index_buffer, 0, bytemuck::cast_slice(indices));
        }
        self.dynamic_index_count = indices.len() as u32;
    }

    pub fn screen_ray(&self, cursor_x: f64, cursor_y: f64) -> Option<(Vec3, Vec3)> {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let inv = self.camera.view_proj(aspect).inverse();
        let ndc_x = 2.0 * cursor_x as f32 / self.config.width as f32 - 1.0;
        let ndc_y = 1.0 - 2.0 * cursor_y as f32 / self.config.height as f32;
        let near = inv * Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
        let far = inv * Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
        if near.w.abs() < 1e-6 || far.w.abs() < 1e-6 {
            return None;
        }
        let origin = Vec3::new(near.x / near.w, near.y / near.w, near.z / near.w);
        let far = Vec3::new(far.x / far.w, far.y / far.w, far.z / far.w);
        let dir = (far - origin).normalize_or_zero();
        if dir.length_squared() < 1e-6 {
            return None;
        }
        Some((origin, dir))
    }

    pub fn set_hud_mesh(&mut self, vertices: &[Vertex], indices: &[u32]) {
        if vertices.len() > self.hud_vertex_capacity {
            let capacity = vertices.len().next_power_of_two();
            self.hud_vertex_buffer = create_dynamic_buffer(
                &self.device,
                "hud vertices",
                capacity * std::mem::size_of::<Vertex>(),
                wgpu::BufferUsages::VERTEX,
            );
            self.hud_vertex_capacity = capacity;
        }
        if indices.len() > self.hud_index_capacity {
            let capacity = indices.len().next_power_of_two();
            self.hud_index_buffer = create_dynamic_buffer(
                &self.device,
                "hud indices",
                capacity * std::mem::size_of::<u32>(),
                wgpu::BufferUsages::INDEX,
            );
            self.hud_index_capacity = capacity;
        }
        if !vertices.is_empty() {
            self.queue
                .write_buffer(&self.hud_vertex_buffer, 0, bytemuck::cast_slice(vertices));
        }
        if !indices.is_empty() {
            self.queue
                .write_buffer(&self.hud_index_buffer, 0, bytemuck::cast_slice(indices));
        }
        self.hud_index_count = indices.len() as u32;
    }

    pub fn pick_ground(&self, cursor_x: f64, cursor_y: f64, height: &HeightGrid) -> Option<Vec3> {
        let (origin, dir) = self.screen_ray(cursor_x, cursor_y)?;

        let mut t = 0.0f32;
        let step = 24.0f32;
        let mut last = origin;
        while t < 80_000.0 {
            let p = origin + dir * t;
            if height.contains(p.x, p.z) && p.y <= height.height_at(p.x, p.z) {
                let mut lo = (t - step).max(0.0);
                let mut hi = t;
                for _ in 0..24 {
                    let mid = (lo + hi) * 0.5;
                    let q = origin + dir * mid;
                    if q.y <= height.height_at(q.x, q.z) {
                        hi = mid;
                    } else {
                        lo = mid;
                    }
                }
                let q = origin + dir * hi;
                return Some(Vec3::new(q.x, height.height_at(q.x, q.z), q.z));
            }
            last = p;
            t += step;
        }
        let _ = last;
        None
    }

    pub fn render(&mut self) -> Result<()> {
        let output = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Suboptimal(texture) => {
                self.surface.configure(&self.device, &self.config);
                texture
            }
            wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            wgpu::CurrentSurfaceTexture::Timeout
            | wgpu::CurrentSurfaceTexture::Occluded
            | wgpu::CurrentSurfaceTexture::Validation => return Ok(()),
        };

        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("rift frame"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.58,
                            g: 0.72,
                            b: 0.82,
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
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for mesh in [&self.terrain, &self.models] {
                if mesh.index_count == 0 {
                    continue;
                }
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
            if self.dynamic_index_count > 0 {
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(..));
                pass.set_index_buffer(
                    self.dynamic_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
            }
            if self.hud_index_count > 0 {
                pass.set_pipeline(&self.hud_pipeline);
                pass.set_bind_group(0, &self.hud_bind_group, &[]);
                pass.set_vertex_buffer(0, self.hud_vertex_buffer.slice(..));
                pass.set_index_buffer(self.hud_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.hud_index_count, 0, 0..1);
            }
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        self.queue.present(output);
        Ok(())
    }
}

fn create_dynamic_buffer(
    device: &wgpu::Device,
    label: &str,
    size: usize,
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size: size.max(4) as u64,
        usage: usage | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    })
}

fn create_depth_view(device: &wgpu::Device, width: u32, height: u32) -> wgpu::TextureView {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("depth"),
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
    texture.create_view(&wgpu::TextureViewDescriptor::default())
}
