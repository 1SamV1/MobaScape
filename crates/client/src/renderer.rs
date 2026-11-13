use std::sync::Arc;

use anyhow::{Context, Result};
use glam::{Mat4, Vec3, Vec4};
use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::animated::SceneLight;
use crate::scene::{
    Atmosphere, HeightGrid, MeshData, SceneData, SceneVertex, Vertex, srgb_to_linear,
};

#[repr(C)]
#[derive(Copy, Clone, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct GpuLight {
    position: [f32; 4],
    color: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct SceneGlobals {
    view_proj: [[f32; 4]; 4],
    camera_pos: [f32; 4],
    sun_dir: [f32; 4],
    sun_color: [f32; 4],
    ambient_sky: [f32; 4],
    ambient_ground: [f32; 4],
    fog_color: [f32; 4],
    fog_params: [f32; 4],
    sky_top: [f32; 4],
    sky_horizon: [f32; 4],
    sun_screen: [f32; 4],
    tuning: [f32; 4],
    lights: [GpuLight; 16],
    light_count: [u32; 4],
}

impl SceneGlobals {
    fn from_atmosphere(atmosphere: &Atmosphere) -> Self {
        let color = |v: [f32; 3]| {
            [
                srgb_to_linear(v[0]),
                srgb_to_linear(v[1]),
                srgb_to_linear(v[2]),
                1.0,
            ]
        };
        let sun = Vec3::from_array(atmosphere.sun_dir).normalize_or_zero();
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0; 4],
            sun_dir: [sun.x, sun.y, sun.z, 0.0],
            sun_color: color(atmosphere.sun_color),
            ambient_sky: color(atmosphere.ambient_sky),
            ambient_ground: color(atmosphere.ambient_ground),
            fog_color: color(atmosphere.fog_color),
            fog_params: [
                atmosphere.fog_density,
                1.0 / atmosphere.fog_height.max(1.0),
                atmosphere.fog_height,
                atmosphere.ground_fog,
            ],
            sky_top: color(atmosphere.sky_top),
            sky_horizon: color(atmosphere.sky_horizon),
            sun_screen: [0.0; 4],
            tuning: [1.0, 4.0, 0.0, 0.0],
            lights: [GpuLight::default(); 16],
            light_count: [0; 4],
        }
    }
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
    scene_pipeline: wgpu::RenderPipeline,
    hud_pipeline: wgpu::RenderPipeline,
    sky_pipeline: wgpu::RenderPipeline,
    blob_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    scene_buffer: wgpu::Buffer,
    scene_globals: SceneGlobals,
    depth_view: wgpu::TextureView,
    terrain: GpuMesh,
    models: GpuMesh,
    static_shadows: GpuMesh,
    dynamic_vertex_buffer: wgpu::Buffer,
    dynamic_index_buffer: wgpu::Buffer,
    dynamic_vertex_capacity: usize,
    dynamic_index_capacity: usize,
    dynamic_index_count: u32,
    shadow_vertex_buffer: wgpu::Buffer,
    shadow_index_buffer: wgpu::Buffer,
    shadow_vertex_capacity: usize,
    shadow_index_capacity: usize,
    shadow_index_count: u32,
    hud_vertex_buffer: wgpu::Buffer,
    hud_index_buffer: wgpu::Buffer,
    hud_vertex_capacity: usize,
    hud_index_capacity: usize,
    hud_index_count: u32,
    fog_vertex_buffer: wgpu::Buffer,
    fog_index_buffer: wgpu::Buffer,
    fog_vertex_capacity: usize,
    fog_index_capacity: usize,
    fog_index_count: u32,
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

        let scene_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rift scene shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("scene.wgsl").into()),
        });
        let hud_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rift hud shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("hud.wgsl").into()),
        });
        let sky_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rift sky shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sky.wgsl").into()),
        });
        let blob_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rift blob shadow shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("blob.wgsl").into()),
        });

        let scene_globals = SceneGlobals::from_atmosphere(&scene.atmosphere);
        let scene_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene globals uniform"),
            size: std::mem::size_of::<SceneGlobals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&scene_buffer, 0, bytemuck::bytes_of(&scene_globals));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene globals layout"),
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
            label: Some("scene globals bind group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: scene_buffer.as_entire_binding(),
            }],
        });

        let scene_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("scene pipeline layout"),
                bind_group_layouts: &[Some(&bind_group_layout)],
                immediate_size: 0,
            });
        let hud_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hud pipeline layout"),
            bind_group_layouts: &[],
            immediate_size: 0,
        });

        const SCENE_ATTRS: [wgpu::VertexAttribute; 6] = wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
            2 => Float32x4,
            3 => Float32x2,
            4 => Float32,
            5 => Uint32
        ];
        const HUD_ATTRS: [wgpu::VertexAttribute; 2] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

        let scene_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("scene pipeline"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &scene_shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SceneVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SCENE_ATTRS,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_shader,
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
            layout: Some(&hud_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &hud_shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &HUD_ATTRS,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_shader,
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

        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky pipeline"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sky_shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_shader,
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
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let blob_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blob shadow pipeline"),
            layout: Some(&scene_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &blob_shader,
                entry_point: Some("vs"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<SceneVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &SCENE_ATTRS,
                })],
            },
            fragment: Some(wgpu::FragmentState {
                module: &blob_shader,
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
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState {
                    constant: -2,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let depth_view = create_depth_view(&device, width, height);
        let terrain = GpuMesh::upload(&device, &scene.terrain, "terrain");
        let models = GpuMesh::upload(&device, &scene.models, "models");
        let static_shadows = GpuMesh::upload(&device, &scene.shadows, "static shadows");

        let dynamic_vertex_capacity = 1024;
        let dynamic_index_capacity = 3072;
        let dynamic_vertex_buffer = create_dynamic_buffer(
            &device,
            "dynamic vertices",
            dynamic_vertex_capacity * std::mem::size_of::<SceneVertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let dynamic_index_buffer = create_dynamic_buffer(
            &device,
            "dynamic indices",
            dynamic_index_capacity * std::mem::size_of::<u32>(),
            wgpu::BufferUsages::INDEX,
        );

        let shadow_vertex_capacity = 1024;
        let shadow_index_capacity = 1536;
        let shadow_vertex_buffer = create_dynamic_buffer(
            &device,
            "shadow vertices",
            shadow_vertex_capacity * std::mem::size_of::<SceneVertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let shadow_index_buffer = create_dynamic_buffer(
            &device,
            "shadow indices",
            shadow_index_capacity * std::mem::size_of::<u32>(),
            wgpu::BufferUsages::INDEX,
        );

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

        let fog_vertex_capacity = 1024;
        let fog_index_capacity = 1536;
        let fog_vertex_buffer = create_dynamic_buffer(
            &device,
            "fog vertices",
            fog_vertex_capacity * std::mem::size_of::<SceneVertex>(),
            wgpu::BufferUsages::VERTEX,
        );
        let fog_index_buffer = create_dynamic_buffer(
            &device,
            "fog indices",
            fog_index_capacity * std::mem::size_of::<u32>(),
            wgpu::BufferUsages::INDEX,
        );

        let mut renderer = Self {
            surface,
            device,
            queue,
            config,
            scene_pipeline,
            hud_pipeline,
            sky_pipeline,
            blob_pipeline,
            bind_group,
            scene_buffer,
            scene_globals,
            depth_view,
            terrain,
            models,
            static_shadows,
            dynamic_vertex_buffer,
            dynamic_index_buffer,
            dynamic_vertex_capacity,
            dynamic_index_capacity,
            dynamic_index_count: 0,
            shadow_vertex_buffer,
            shadow_index_buffer,
            shadow_vertex_capacity,
            shadow_index_capacity,
            shadow_index_count: 0,
            hud_vertex_buffer,
            hud_index_buffer,
            hud_vertex_capacity,
            hud_index_capacity,
            hud_index_count: 0,
            fog_vertex_buffer,
            fog_index_buffer,
            fog_vertex_capacity,
            fog_index_capacity,
            fog_index_count: 0,
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
        self.scene_globals.view_proj = view_proj.to_cols_array_2d();
        self.scene_globals.camera_pos = [eye.x, eye.y, eye.z, 1.0];

        let sun_dir = Vec3::new(
            self.scene_globals.sun_dir[0],
            self.scene_globals.sun_dir[1],
            self.scene_globals.sun_dir[2],
        )
        .normalize_or_zero();
        let sun_world = eye + sun_dir * 10_000.0;
        let clip = view_proj * Vec4::new(sun_world.x, sun_world.y, sun_world.z, 1.0);
        if clip.w > 0.0 {
            let ndc = Vec3::new(clip.x, clip.y, clip.z) / clip.w;
            self.scene_globals.sun_screen = [ndc.x, ndc.y, 1.0, 0.0];
        } else {
            self.scene_globals.sun_screen = [0.0, 0.0, 0.0, 0.0];
        }
        self.upload_globals();
    }

    fn upload_globals(&mut self) {
        self.queue.write_buffer(
            &self.scene_buffer,
            0,
            bytemuck::bytes_of(&self.scene_globals),
        );
    }

    pub fn set_graphics_mode(&mut self, enhanced: bool) {
        let debug = self.scene_globals.tuning[3] as u32 & 15;
        self.scene_globals.tuning[3] = if enhanced {
            debug as f32
        } else {
            (debug | 256) as f32
        };
        if !enhanced {
            self.scene_globals.light_count = [0; 4];
        }
        self.upload_globals();
    }

    pub fn set_debug_view(&mut self, view: u32) {
        let classic = self.scene_globals.tuning[3] as u32 & 256;
        self.scene_globals.tuning[3] = (classic | (view & 15)) as f32;
        self.upload_globals();
    }

    pub fn set_time(&mut self, time: f32) {
        self.scene_globals.tuning[2] = time;
        self.upload_globals();
    }

    pub fn set_lights(&mut self, lights: &[SceneLight]) {
        self.scene_globals.lights = [GpuLight::default(); 16];
        let count = lights.len().min(16);
        for (index, light) in lights.iter().take(count).enumerate() {
            self.scene_globals.lights[index] = GpuLight {
                position: [light.pos.x, light.pos.y, light.pos.z, 1.0],
                color: [
                    light.color[0],
                    light.color[1],
                    light.color[2],
                    light.intensity,
                ],
                params: [light.radius, 0.0, 0.0, 0.0],
            };
        }
        self.scene_globals.light_count = [count as u32, 0, 0, 0];
        self.upload_globals();
    }

    pub fn set_dynamic_mesh(&mut self, vertices: &[SceneVertex], indices: &[u32]) {
        if vertices.len() > self.dynamic_vertex_capacity {
            let capacity = vertices.len().next_power_of_two();
            self.dynamic_vertex_buffer = create_dynamic_buffer(
                &self.device,
                "dynamic vertices",
                capacity * std::mem::size_of::<SceneVertex>(),
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

    pub fn set_shadow_mesh(&mut self, vertices: &[SceneVertex], indices: &[u32]) {
        if vertices.len() > self.shadow_vertex_capacity {
            let capacity = vertices.len().next_power_of_two();
            self.shadow_vertex_buffer = create_dynamic_buffer(
                &self.device,
                "shadow vertices",
                capacity * std::mem::size_of::<SceneVertex>(),
                wgpu::BufferUsages::VERTEX,
            );
            self.shadow_vertex_capacity = capacity;
        }
        if indices.len() > self.shadow_index_capacity {
            let capacity = indices.len().next_power_of_two();
            self.shadow_index_buffer = create_dynamic_buffer(
                &self.device,
                "shadow indices",
                capacity * std::mem::size_of::<u32>(),
                wgpu::BufferUsages::INDEX,
            );
            self.shadow_index_capacity = capacity;
        }
        if !vertices.is_empty() {
            self.queue.write_buffer(
                &self.shadow_vertex_buffer,
                0,
                bytemuck::cast_slice(vertices),
            );
        }
        if !indices.is_empty() {
            self.queue
                .write_buffer(&self.shadow_index_buffer, 0, bytemuck::cast_slice(indices));
        }
        self.shadow_index_count = indices.len() as u32;
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

    pub fn set_fog_mesh(&mut self, vertices: &[SceneVertex], indices: &[u32]) {
        if vertices.len() > self.fog_vertex_capacity {
            let capacity = vertices.len().next_power_of_two();
            self.fog_vertex_buffer = create_dynamic_buffer(
                &self.device,
                "fog vertices",
                capacity * std::mem::size_of::<SceneVertex>(),
                wgpu::BufferUsages::VERTEX,
            );
            self.fog_vertex_capacity = capacity;
        }
        if indices.len() > self.fog_index_capacity {
            let capacity = indices.len().next_power_of_two();
            self.fog_index_buffer = create_dynamic_buffer(
                &self.device,
                "fog indices",
                capacity * std::mem::size_of::<u32>(),
                wgpu::BufferUsages::INDEX,
            );
            self.fog_index_capacity = capacity;
        }
        if !vertices.is_empty() {
            self.queue
                .write_buffer(&self.fog_vertex_buffer, 0, bytemuck::cast_slice(vertices));
        }
        if !indices.is_empty() {
            self.queue
                .write_buffer(&self.fog_index_buffer, 0, bytemuck::cast_slice(indices));
        }
        self.fog_index_count = indices.len() as u32;
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

            pass.set_pipeline(&self.sky_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);

            pass.set_pipeline(&self.scene_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            for mesh in [&self.terrain, &self.models] {
                if mesh.index_count == 0 {
                    continue;
                }
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
            if self.fog_index_count > 0 {
                pass.set_vertex_buffer(0, self.fog_vertex_buffer.slice(..));
                pass.set_index_buffer(self.fog_index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.fog_index_count, 0, 0..1);
            }
            if self.dynamic_index_count > 0 {
                pass.set_vertex_buffer(0, self.dynamic_vertex_buffer.slice(..));
                pass.set_index_buffer(
                    self.dynamic_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.dynamic_index_count, 0, 0..1);
            }
            if self.static_shadows.index_count > 0 || self.shadow_index_count > 0 {
                pass.set_pipeline(&self.blob_pipeline);
                pass.set_bind_group(0, &self.bind_group, &[]);
            }
            if self.static_shadows.index_count > 0 {
                pass.set_vertex_buffer(0, self.static_shadows.vertex_buffer.slice(..));
                pass.set_index_buffer(
                    self.static_shadows.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.static_shadows.index_count, 0, 0..1);
            }
            if self.shadow_index_count > 0 {
                pass.set_vertex_buffer(0, self.shadow_vertex_buffer.slice(..));
                pass.set_index_buffer(
                    self.shadow_index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..self.shadow_index_count, 0, 0..1);
            }
            if self.hud_index_count > 0 {
                pass.set_pipeline(&self.hud_pipeline);
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
