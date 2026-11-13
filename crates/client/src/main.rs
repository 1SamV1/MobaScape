mod renderer;
mod scene;

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use glam::Vec3;
use rift_cache::Cache;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::renderer::{Camera, Renderer};
use crate::scene::SceneData;

#[derive(Parser, Debug)]
#[command(name = "rift-client", about = "Rift native client (M0 cache viewer)")]
struct Cli {
    #[arg(long, default_value = ".cache/osrs-235/cache")]
    cache: PathBuf,
    #[arg(long, default_value_t = 44)]
    region_x: u32,
    #[arg(long, default_value_t = 47)]
    region_y: u32,
    #[arg(long, default_value_t = 320)]
    model_limit: usize,
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let cache = Cache::open_with_keys(&cli.cache, None::<&PathBuf>)
        .with_context(|| format!("opening cache at {}", cli.cache.display()))?;

    tracing::info!(
        "building scene for region {},{}",
        cli.region_x,
        cli.region_y
    );
    let scene = scene::build(&cache, cli.region_x, cli.region_y, cli.model_limit)?;
    tracing::info!("scene ready: {} placements", scene.placements);

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let mut app = App {
        scene: Some(scene),
        window: None,
        renderer: None,
        held: HashSet::new(),
        focused: false,
        last_frame: Instant::now(),
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    scene: Option<SceneData>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    held: HashSet<KeyCode>,
    focused: bool,
    last_frame: Instant,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Rift — Karamja")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(err) => {
                tracing::error!("failed to create window: {err}");
                event_loop.exit();
                return;
            }
        };

        let scene = self.scene.take().expect("scene missing");
        let camera = Camera::new(scene.center);
        let renderer = match pollster::block_on(Renderer::new(window.clone(), &scene, camera)) {
            Ok(renderer) => renderer,
            Err(err) => {
                tracing::error!("failed to create renderer: {err:#}");
                event_loop.exit();
                return;
            }
        };

        tracing::info!(
            "camera target {:?}, distance {:.0}",
            renderer.camera.target,
            renderer.camera.distance
        );
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.last_frame = Instant::now();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = self.renderer.as_mut() {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };

                if event.state == ElementState::Pressed {
                    self.held.insert(code);
                    if code == KeyCode::Escape {
                        event_loop.exit();
                    } else if code == KeyCode::KeyR {
                        if let Some(renderer) = self.renderer.as_mut() {
                            renderer.reset_camera();
                            print_camera(renderer);
                        }
                    }
                } else {
                    self.held.remove(&code);
                }
            }
            WindowEvent::Focused(focused) => {
                self.focused = focused;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !self.focused {
                    return;
                }
                let Some(renderer) = self.renderer.as_mut() else {
                    return;
                };
                let amount = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 120.0,
                };
                renderer.camera.distance =
                    (renderer.camera.distance * (1.0 - amount * 0.12)).clamp(1200.0, 18_000.0);
                renderer.update_camera();
                print_camera(renderer);
            }
            WindowEvent::RedrawRequested => {
                self.update_camera();
                if let Some(renderer) = self.renderer.as_mut() {
                    if let Err(err) = renderer.render() {
                        tracing::error!("render error: {err:#}");
                        event_loop.exit();
                    }
                }
                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.as_ref() {
            window.request_redraw();
        }
    }
}

impl App {
    fn update_camera(&mut self) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        let yaw = renderer.camera.yaw;
        let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());

        let mut movement = Vec3::ZERO;
        if self.held.contains(&KeyCode::KeyW) || self.held.contains(&KeyCode::ArrowUp) {
            movement += forward;
        }
        if self.held.contains(&KeyCode::KeyS) || self.held.contains(&KeyCode::ArrowDown) {
            movement -= forward;
        }
        if self.held.contains(&KeyCode::KeyD) || self.held.contains(&KeyCode::ArrowRight) {
            movement += right;
        }
        if self.held.contains(&KeyCode::KeyA) || self.held.contains(&KeyCode::ArrowLeft) {
            movement -= right;
        }

        let mut changed = false;
        if movement.length_squared() > 0.0 {
            let speed = 2400.0 * dt;
            renderer.camera.target += movement.normalize() * speed;
            changed = true;
        }
        if self.held.contains(&KeyCode::KeyQ) {
            renderer.camera.yaw += 1.2 * dt;
            changed = true;
        }
        if self.held.contains(&KeyCode::KeyE) {
            renderer.camera.yaw -= 1.2 * dt;
            changed = true;
        }

        if changed {
            renderer.update_camera();
        }
    }
}

fn print_camera(renderer: &Renderer) {
    let target = renderer.camera.target;
    println!(
        "CAM target={:.0},{:.0},{:.0} yaw={:.2} pitch={:.2} dist={:.0}",
        target.x,
        target.y,
        target.z,
        renderer.camera.yaw,
        renderer.camera.pitch,
        renderer.camera.distance
    );
}
