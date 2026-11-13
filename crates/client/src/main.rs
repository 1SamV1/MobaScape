mod animated;
mod hud;
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
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::animated::AnimatedScene;
use crate::hud::Hud;
use crate::renderer::{Camera, Renderer};
use crate::scene::{CampBuff, HeightGrid, SceneData, WorldCollision};

#[derive(Parser, Debug)]
#[command(name = "rift-client", about = "Rift native client (M1 vertical slice)")]
struct Cli {
    #[arg(long, default_value = ".cache/osrs-235/cache")]
    cache: PathBuf,
    #[arg(long, default_value_t = 44)]
    region_x: u32,
    #[arg(long, default_value_t = 47)]
    region_y: u32,
    /// Radius in 64-tile regions (1 = 3x3 block).
    #[arg(long, default_value_t = 1)]
    radius: u32,
    /// Initial camera distance in world units.
    #[arg(long, default_value_t = 4400.0)]
    distance: f32,
    /// Map design data.
    #[arg(long, default_value = "data/maps/karamja_rift/map.json")]
    map: PathBuf,
    /// Demo: walk to this tile coordinate after one second (x y).
    #[arg(long, num_args = 2, value_names = ["X", "Y"])]
    demo_target: Option<Vec<f32>>,
    /// Demo: auto-target a dummy and fire Q to verify combat.
    #[arg(long)]
    demo_combat: bool,
    /// Demo: teleport to a jungle camp, attack it, and verify the buff.
    #[arg(long)]
    demo_camp: bool,
    /// Simulation speed multiplier for verification.
    #[arg(long, default_value_t = 1.0)]
    timescale: f32,
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
        "building {0}x{0} region block around {1},{2}",
        cli.radius * 2 + 1,
        cli.region_x,
        cli.region_y
    );
    let scene = scene::build(&cache, cli.region_x, cli.region_y, cli.radius, &cli.map)?;
    let animated = AnimatedScene::new(&cache, &scene)?;
    let height = scene.height.clone();
    let collision = scene.collision.clone();
    tracing::info!(
        "scene ready: {} placements, {} animated units",
        scene.placements,
        animated.units.len()
    );

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);

    let demo_target = cli.demo_target.as_ref().and_then(|v| {
        if v.len() == 2 {
            Some((v[0], v[1]))
        } else {
            None
        }
    });

    let mut app = App {
        scene: Some(scene),
        animated: Some(animated),
        height: Some(height),
        collision: Some(collision),
        window: None,
        renderer: None,
        held: HashSet::new(),
        focused: false,
        follow: true,
        mouse_pos: (640.0, 360.0),
        lmb_down: false,
        lmb_press: None,
        initial_distance: cli.distance,
        demo_target,
        demo_timer: 0.0,
        demo_combat: cli.demo_combat,
        demo_camp: cli.demo_camp,
        demo_camp_timer: 0.0,
        demo_camp_target: None,
        demo_camp_cast: false,
        demo_camp_done: false,
        timescale: cli.timescale,
        demo_combat_timer: 0.0,
        demo_combat_targeted: false,
        demo_combat_cast: false,
        demo_combat_target: None,
        hud: Hud::new(),
        lane_log_timer: 0.0,
        last_frame: Instant::now(),
        title_timer: 0.0,
        title_frames: 0,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    scene: Option<SceneData>,
    animated: Option<AnimatedScene>,
    height: Option<HeightGrid>,
    collision: Option<WorldCollision>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    held: HashSet<KeyCode>,
    focused: bool,
    follow: bool,
    mouse_pos: (f64, f64),
    lmb_down: bool,
    lmb_press: Option<Instant>,
    initial_distance: f32,
    demo_target: Option<(f32, f32)>,
    demo_timer: f32,
    demo_combat: bool,
    demo_camp: bool,
    demo_camp_timer: f32,
    demo_camp_target: Option<usize>,
    demo_camp_cast: bool,
    demo_camp_done: bool,
    timescale: f32,
    demo_combat_timer: f32,
    demo_combat_targeted: bool,
    demo_combat_cast: bool,
    demo_combat_target: Option<usize>,
    hud: Hud,
    lane_log_timer: f32,
    last_frame: Instant,
    title_timer: f32,
    title_frames: u32,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attributes = Window::default_attributes()
            .with_title("Rift - Karamja")
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
        let animated = self.animated.take().expect("animated scene missing");
        let camera = Camera::new(animated.player_pos() + Vec3::Y * 140.0);
        let mut renderer = match pollster::block_on(Renderer::new(window.clone(), &scene, camera)) {
            Ok(renderer) => renderer,
            Err(err) => {
                tracing::error!("failed to create renderer: {err:#}");
                event_loop.exit();
                return;
            }
        };

        renderer.camera.distance = self.initial_distance;
        renderer.update_camera();
        tracing::info!(
            "camera target {:?}, distance {:.0}; click the ground to move",
            renderer.camera.target,
            renderer.camera.distance
        );
        self.window = Some(window);
        self.renderer = Some(renderer);
        self.animated = Some(animated);
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
            WindowEvent::Focused(focused) => self.focused = focused,
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = (position.x, position.y);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if button != MouseButton::Left || self.demo_camp || self.demo_combat {
                    return;
                }
                match state {
                    ElementState::Pressed => {
                        self.lmb_down = true;
                        self.lmb_press = Some(Instant::now());
                    }
                    ElementState::Released => {
                        let held = self
                            .lmb_press
                            .take()
                            .map(|t| t.elapsed().as_secs_f32())
                            .unwrap_or(0.0);
                        self.lmb_down = false;
                        if held < 0.18 {
                            self.pick_and_move();
                        }
                    }
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
                        self.follow = true;
                        if let Some(renderer) = self.renderer.as_mut() {
                            renderer.reset_camera();
                            print_camera(renderer);
                        }
                    } else if code == KeyCode::KeyF {
                        self.follow = !self.follow;
                        println!("follow={}", self.follow);
                    } else if code == KeyCode::KeyQ {
                        self.cast_q();
                    }
                } else {
                    self.held.remove(&code);
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !self.focused
                    || !(self.held.contains(&KeyCode::ControlLeft)
                        || self.held.contains(&KeyCode::ControlRight))
                {
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
                    (renderer.camera.distance * (1.0 - amount * 0.12)).clamp(1200.0, 24_000.0);
                renderer.update_camera();
                print_camera(renderer);
            }
            WindowEvent::RedrawRequested => {
                self.tick();
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
    fn tick(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last_frame).as_secs_f32().min(0.1);
        self.last_frame = now;

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let steering = self.lmb_down
            && !self.demo_camp
            && !self.demo_combat
            && self
                .lmb_press
                .map(|t| t.elapsed().as_secs_f32() >= 0.18)
                .unwrap_or(false);

        let eye = renderer.camera.eye();
        let camera_forward = (eye - renderer.camera.target).normalize_or_zero();
        let camera_right = camera_forward.cross(Vec3::Y).normalize_or_zero();
        let camera_up = camera_right.cross(camera_forward).normalize_or_zero();

        if let (Some(animated), Some(height), Some(collision)) = (
            self.animated.as_mut(),
            self.height.as_ref(),
            self.collision.as_ref(),
        ) {
            if self.demo_combat {
                self.demo_combat_timer += dt;
                if !self.demo_combat_targeted && self.demo_combat_timer >= 1.0 {
                    if let Some(index) = animated.nearest_enemy(animated.player_pos()) {
                        let ok = animated.set_attack_target(index, collision, height);
                        tracing::info!("demo combat: target {index}, attack={ok}");
                        self.demo_combat_target = Some(index);
                    }
                    self.demo_combat_targeted = true;
                }
                if !self.demo_combat_cast && self.demo_combat_timer >= 2.6 {
                    if let Some(index) = self.demo_combat_target {
                        if let Some(pos) = animated.unit_pos(index) {
                            let ok = animated.cast_q(pos);
                            tracing::info!("demo combat: Q at {index}, cast={ok}");
                        }
                    }
                    self.demo_combat_cast = true;
                }
                if self.demo_combat_timer >= 3.5 {
                    if let Some(index) = self.demo_combat_target {
                        if let Some(health) = animated.unit_health(index) {
                            tracing::info!("demo combat: target {index} health {health:.0}");
                        }
                    }
                    self.demo_combat = false;
                }
            }

            if self.demo_camp && !self.demo_camp_done {
                self.demo_camp_timer += dt;
                if self.demo_camp_timer >= 1.0 && self.demo_camp_target.is_none() {
                    if let Some(index) = animated.nearest_camp(animated.player_pos()) {
                        if let Some(camp_pos) = animated.unit_pos(index) {
                            let spawn = camp_pos + Vec3::new(200.0, 0.0, 0.0);
                            animated.teleport_player(spawn);
                            let ok = animated.set_attack_target(index, collision, height);
                            tracing::info!("demo camp: target {index}, attack={ok}");
                            self.demo_camp_target = Some(index);
                        }
                    }
                }
                if !self.demo_camp_cast && self.demo_camp_timer >= 2.6 {
                    if let Some(index) = self.demo_camp_target {
                        if let Some(pos) = animated.unit_pos(index) {
                            let ok = animated.cast_q(pos);
                            tracing::info!("demo camp: Q cast={ok}");
                        }
                    }
                    self.demo_camp_cast = true;
                }
                if self.demo_camp_timer >= 10.0 {
                    if let Some(index) = self.demo_camp_target {
                        let health = animated.unit_health(index).unwrap_or(-1.0);
                        let buff = animated.team_buff(0);
                        tracing::info!("demo camp: health {health:.0}, buff {buff:?}");
                    }
                    self.demo_camp_done = true;
                }
            }
            if steering {
                if let Some(point) =
                    renderer.pick_ground(self.mouse_pos.0, self.mouse_pos.1, height)
                {
                    let dir = point - animated.player_pos();
                    animated.steer_player(dir, dt, height, collision);
                }
            }
            if let Some((tx, ty)) = self.demo_target {
                self.demo_timer += dt;
                if self.demo_timer >= 1.0 {
                    let x = tx * 128.0 + 64.0;
                    let z = ty * 128.0 + 64.0;
                    let point = Vec3::new(x, height.height_at(x, z), z);
                    let waypoints = animated.set_player_target(point, collision, height);
                    tracing::info!(
                        "demo move target tile {:.0},{:.0} -> {waypoints} waypoints",
                        tx,
                        ty
                    );
                    self.demo_target = None;
                }
            }
            let steps = self.timescale.round().clamp(1.0, 20.0) as u32;
            for _ in 0..steps {
                animated.update(
                    dt,
                    height,
                    collision,
                    camera_right,
                    camera_up,
                    camera_forward,
                );
            }
            renderer.set_dynamic_mesh(&animated.vertices, &animated.indices);

            let (hp, max_hp) = animated.player_health();
            let nexus0 = animated.nexus_health(0).unwrap_or((0.0, 1.0));
            let nexus1 = animated.nexus_health(1).unwrap_or((0.0, 1.0));
            self.lane_log_timer += dt;
            if self.lane_log_timer >= 5.0 {
                self.lane_log_timer = 0.0;
                tracing::info!(
                    "lane: minions={} towers0={:?} towers1={:?} inhib0={:?} inhib1={:?} nexus0={:?} nexus1={:?} vulnerable0={} vulnerable1={}",
                    animated.minion_count(),
                    animated.tower_healths(0),
                    animated.tower_healths(1),
                    animated.inhibitor_count(0),
                    animated.inhibitor_count(1),
                    animated.nexus_health(0),
                    animated.nexus_health(1),
                    animated.nexus_vulnerable(0),
                    animated.nexus_vulnerable(1)
                );
            }
            let buff = animated.team_buff(0).map(|(remaining, duration, kind)| {
                let color = match kind {
                    CampBuff::Might => [255, 160, 60],
                    CampBuff::Swift => [90, 220, 255],
                };
                (remaining, duration, color)
            });
            self.hud.build(
                renderer.size.0 as f32,
                renderer.size.1 as f32,
                hp,
                max_hp,
                1.0,
                1.0,
                animated.q_cooldown,
                5.0,
                nexus0,
                nexus1,
                animated.wave_timer,
                animated.wave_interval,
                animated.match_over,
                buff,
                self.mouse_pos,
            );
            self.hud
                .build_minimap(renderer.size.0 as f32, renderer.size.1 as f32, animated);
            renderer.set_hud_mesh(&self.hud.vertices, &self.hud.indices);
            if self.follow {
                renderer.camera.target = animated.player_pos() + Vec3::Y * 140.0;
                renderer.update_camera();
            }

            self.title_timer += dt;
            self.title_frames += 1;
            if self.title_timer >= 0.5 {
                let fps = self.title_frames as f32 / self.title_timer;
                let pos = animated.player_pos();
                let (hp, max_hp) = animated.player_health();
                let q = animated.q_cooldown;
                let nexus0 = animated.nexus_health(0).unwrap_or((0.0, 1.0));
                let nexus1 = animated.nexus_health(1).unwrap_or((0.0, 1.0));
                let result = match animated.match_over {
                    Some(0) => " | VICTORY",
                    Some(_) => " | DEFEAT",
                    None => "",
                };
                let inhib0 = animated.inhibitor_count(0);
                let inhib1 = animated.inhibitor_count(1);
                let shield0 = if animated.nexus_vulnerable(0) {
                    "open"
                } else {
                    "shielded"
                };
                let shield1 = if animated.nexus_vulnerable(1) {
                    "open"
                } else {
                    "shielded"
                };
                let buff_text = animated
                    .team_buff(0)
                    .map(|(remaining, _, kind)| format!("{:?} {remaining:.0}s", kind))
                    .unwrap_or_else(|| "-".to_string());
                if let Some(window) = self.window.as_ref() {
                    window.set_title(&format!(
                        "Rift - Karamja | tile {:.0},{:.0} | {:.0} fps | HP {:.0}/{:.0} | Q {:.1}s | nexus {:.0}/{:.0} ({}) - {:.0}/{:.0} ({}) | inhib {}/{} - {}/{} | wave {:.0}s | minions {} | buff {}{}",
                        pos.x / 128.0,
                        pos.z / 128.0,
                        fps,
                        hp,
                        max_hp,
                        q,
                        nexus0.0,
                        nexus0.1,
                        shield0,
                        nexus1.0,
                        nexus1.1,
                        shield1,
                        inhib0.0,
                        inhib0.1,
                        inhib1.0,
                        inhib1.1,
                        animated.wave_timer,
                        animated.minion_count(),
                        buff_text,
                        result
                    ));
                }
                self.title_timer = 0.0;
                self.title_frames = 0;
            }
        }

        self.update_camera(dt);
    }

    fn pick_and_move(&mut self) {
        let (Some(renderer), Some(height), Some(collision), Some(animated)) = (
            self.renderer.as_ref(),
            self.height.as_ref(),
            self.collision.as_ref(),
            self.animated.as_mut(),
        ) else {
            return;
        };
        if let Some((origin, dir)) = renderer.screen_ray(self.mouse_pos.0, self.mouse_pos.1) {
            if let Some(index) = animated.pick_unit(origin, dir) {
                let ok = animated.set_attack_target(index, collision, height);
                println!("ATTACK target={index} ok={ok}");
                return;
            }
        }
        if let Some(point) = renderer.pick_ground(self.mouse_pos.0, self.mouse_pos.1, height) {
            let waypoints = animated.set_player_target(point, collision, height);
            println!(
                "MOVE target={:.0},{:.0},{:.0} path={} waypoints",
                point.x, point.y, point.z, waypoints
            );
        }
    }

    fn cast_q(&mut self) {
        let (Some(renderer), Some(height), Some(animated)) = (
            self.renderer.as_ref(),
            self.height.as_ref(),
            self.animated.as_mut(),
        ) else {
            return;
        };
        if let Some(point) = renderer.pick_ground(self.mouse_pos.0, self.mouse_pos.1, height) {
            let cast = animated.cast_q(point);
            println!("Q cast={cast}");
        }
    }

    fn update_camera(&mut self, dt: f32) {
        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

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
            self.follow = false;
            let speed = 2600.0 * dt;
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
