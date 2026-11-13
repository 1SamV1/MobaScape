mod animated;
mod champion;
mod font;
mod hud;
mod items;
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
use crate::champion::ChampionConfig;
use crate::hud::{Hud, PlayerHud, minimap_world_at};
use crate::items::{ItemShop, shop_row_at, shop_row_rect};
use crate::renderer::{Camera, Renderer};
use crate::scene::{CampBuff, HeightGrid, SceneData, SceneVertex, WorldCollision};

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
    /// Optional initial camera pitch in radians (dev screenshots).
    #[arg(long)]
    pitch: Option<f32>,
    /// Map design data.
    #[arg(long, default_value = "data/maps/karamja_rift/map.json")]
    map: PathBuf,
    /// Champion config name (monkey, jad, kree).
    #[arg(long, default_value = "monkey")]
    champion: String,
    /// Demo: walk to this tile coordinate after one second (x y).
    #[arg(long, num_args = 2, value_names = ["X", "Y"])]
    demo_target: Option<Vec<f32>>,
    /// Demo: auto-target a dummy and fire Q to verify combat.
    #[arg(long)]
    demo_combat: bool,
    /// Demo: teleport to a jungle camp, attack it, and verify the buff.
    #[arg(long)]
    demo_camp: bool,
    /// Demo: cast W/E/R and print the resulting player stats.
    #[arg(long)]
    demo_kit: bool,
    /// Demo: grant gold, open the shop, and buy one of every item.
    #[arg(long)]
    demo_shop: bool,
    /// Demo: damage the player and verify fountain healing plus passive gold.
    #[arg(long)]
    demo_base: bool,
    /// Demo: destroy an inhibitor and verify super minions plus respawn.
    #[arg(long)]
    demo_inhibitor: bool,
    /// Graphics preset: 117hd or classic.
    #[arg(long, default_value = "117hd")]
    graphics: String,
    /// Debug view: lit, albedo, normal, lighting, fog.
    #[arg(long, default_value = "lit")]
    debug_view: String,
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
    let champion = ChampionConfig::load(&cli.champion)?;
    let shop = ItemShop::load()?;
    let scene = scene::build(&cache, cli.region_x, cli.region_y, cli.radius, &cli.map)?;
    let animated = AnimatedScene::new(&cache, &scene, &champion)?;
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
        initial_pitch: cli.pitch,
        demo_target,
        demo_timer: 0.0,
        demo_combat: cli.demo_combat,
        demo_camp: cli.demo_camp,
        demo_kit: cli.demo_kit,
        demo_kit_timer: 0.0,
        demo_kit_stage: 0,
        shop,
        demo_shop: cli.demo_shop,
        demo_shop_timer: 0.0,
        demo_shop_stage: 0,
        demo_base: cli.demo_base,
        demo_base_timer: 0.0,
        demo_base_stage: 0,
        demo_inhibitor: cli.demo_inhibitor,
        demo_inhibitor_timer: 0.0,
        demo_inhibitor_stage: 0,
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
        graphics_classic: cli.graphics.eq_ignore_ascii_case("classic"),
        debug_view: parse_debug_view(&cli.debug_view),
        app_time: 0.0,
        fog_timer: 0.0,
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
    initial_pitch: Option<f32>,
    demo_target: Option<(f32, f32)>,
    demo_timer: f32,
    demo_combat: bool,
    demo_camp: bool,
    demo_kit: bool,
    demo_kit_timer: f32,
    demo_kit_stage: u8,
    shop: ItemShop,
    demo_shop: bool,
    demo_shop_timer: f32,
    demo_shop_stage: u8,
    demo_base: bool,
    demo_base_timer: f32,
    demo_base_stage: u8,
    demo_inhibitor: bool,
    demo_inhibitor_timer: f32,
    demo_inhibitor_stage: u8,
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
    graphics_classic: bool,
    debug_view: u32,
    app_time: f32,
    fog_timer: f32,
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
        if let Some(pitch) = self.initial_pitch {
            renderer.camera.pitch = pitch;
        }
        renderer.set_graphics_mode(!self.graphics_classic);
        renderer.set_debug_view(self.debug_view);
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
                if button != MouseButton::Left
                    || self.demo_camp
                    || self.demo_combat
                    || self.demo_kit
                    || self.demo_shop
                    || self.demo_base
                    || self.demo_inhibitor
                {
                    return;
                }
                if state == ElementState::Pressed && self.shop.open {
                    let (w, h) = self
                        .renderer
                        .as_ref()
                        .map(|r| (r.size.0 as f32, r.size.1 as f32))
                        .unwrap_or((1280.0, 720.0));
                    if let Some(row) = shop_row_at(w, h, self.mouse_pos) {
                        self.purchase_item(row);
                        return;
                    }
                    self.shop.open = false;
                    return;
                }
                match state {
                    ElementState::Pressed => {
                        self.lmb_down = true;
                        self.lmb_press = Some(Instant::now());
                    }
                    ElementState::Released => {
                        let Some(press) = self.lmb_press.take() else {
                            return;
                        };
                        let held = press.elapsed().as_secs_f32();
                        self.lmb_down = false;
                        if held < 0.18 && !self.click_minimap() {
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
                        if self.shop.open {
                            self.shop.open = false;
                        } else {
                            event_loop.exit();
                        }
                    } else if code == KeyCode::KeyF {
                        self.follow = !self.follow;
                        println!("follow={}", self.follow);
                    } else if code == KeyCode::KeyQ {
                        self.cast_q();
                    } else if code == KeyCode::KeyW {
                        self.cast_w();
                    } else if code == KeyCode::KeyE {
                        self.cast_e();
                    } else if code == KeyCode::KeyR {
                        self.cast_r();
                    } else if code == KeyCode::KeyB {
                        self.shop.open = !self.shop.open;
                        println!("shop open={}", self.shop.open);
                    } else if let Some(index) = digit_index(code) {
                        if self.shop.open {
                            self.purchase_item(index);
                        }
                    } else if code == KeyCode::Space {
                        self.follow = true;
                        if let (Some(renderer), Some(animated)) =
                            (self.renderer.as_mut(), self.animated.as_ref())
                        {
                            renderer.camera.target = animated.player_pos() + Vec3::Y * 140.0;
                            renderer.update_camera();
                            print_camera(renderer);
                        }
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
        self.app_time += dt;

        if self.demo_shop {
            self.demo_shop_timer += dt;
            if self.demo_shop_stage == 0 && self.demo_shop_timer >= 0.5 {
                if let Some(animated) = self.animated.as_mut() {
                    animated.debug_grant_gold(4000);
                }
                self.shop.open = true;
                tracing::info!("demo shop: granted 4000 gold, shop open");
                self.demo_shop_stage = 1;
            }
            if (1..=6).contains(&self.demo_shop_stage) {
                let due = self.demo_shop_stage as f32 * 0.5;
                if self.demo_shop_timer >= due {
                    let index = (self.demo_shop_stage - 1) as usize;
                    self.purchase_item(index);
                    self.demo_shop_stage += 1;
                }
            }
            if self.shop.open {
                // Keep the demo cursor over the next unowned row so the
                // hovered-item tooltip is part of the captured verification.
                let (w, h) = self
                    .renderer
                    .as_ref()
                    .map(|r| (r.size.0 as f32, r.size.1 as f32))
                    .unwrap_or((1280.0, 720.0));
                let row = self
                    .shop
                    .owned
                    .iter()
                    .position(Option::is_none)
                    .unwrap_or(5);
                let (x, y, rw, rh) = shop_row_rect(w, h, row);
                self.mouse_pos = ((x + rw * 0.5) as f64, (y + rh * 0.5) as f64);
            }
            if self.demo_shop_stage == 7 && self.demo_shop_timer >= 4.5 {
                if let Some(animated) = self.animated.as_ref() {
                    let s = animated.player_stats();
                    tracing::info!(
                        "demo shop: items {}/6 dmg {:.0} hp {:.0}/{:.0} mp {:.0}/{:.0} speed {:.0} ap {:.0}% gold {}",
                        self.shop.item_count(),
                        s.attack_damage,
                        s.health,
                        s.max_health,
                        s.mana,
                        s.max_mana,
                        s.speed,
                        s.ability_power,
                        s.gold
                    );
                }
                self.demo_shop = false;
            }
        }

        if self.demo_base {
            self.demo_base_timer += dt;
            if self.demo_base_stage == 0 && self.demo_base_timer >= 0.5 {
                if let Some(animated) = self.animated.as_mut() {
                    animated.debug_damage_player(300.0);
                    let s = animated.player_stats();
                    tracing::info!(
                        "demo base: damaged to {:.0}/{:.0} hp, {:.0}/{:.0} mp, gold {}",
                        s.health,
                        s.max_health,
                        s.mana,
                        s.max_mana,
                        s.gold
                    );
                }
                self.demo_base_stage = 1;
            }
            if self.demo_base_stage == 1 && self.demo_base_timer >= 4.0 {
                if let Some(animated) = self.animated.as_ref() {
                    let s = animated.player_stats();
                    tracing::info!(
                        "demo base: fountain healed to {:.0}/{:.0} hp, {:.0}/{:.0} mp, passive gold {}",
                        s.health,
                        s.max_health,
                        s.mana,
                        s.max_mana,
                        s.gold
                    );
                }
                self.demo_base = false;
            }
        }

        if self.demo_inhibitor {
            self.demo_inhibitor_timer += dt;
            if self.demo_inhibitor_stage == 0 && self.demo_inhibitor_timer >= 0.5 {
                if let Some(animated) = self.animated.as_mut() {
                    let ok = animated.debug_destroy_inhibitor(1, 0, 30.0);
                    tracing::info!(
                        "demo inhibitor: destroyed={ok} state={:?} inhib1={:?} vulnerable1={} supers={}",
                        animated.debug_inhibitor_state(1, 0),
                        animated.inhibitor_count(1),
                        animated.nexus_vulnerable(1),
                        animated.super_minion_count()
                    );
                }
                self.demo_inhibitor_stage = 1;
            }
            if self.demo_inhibitor_stage == 1 && self.demo_inhibitor_timer >= 2.0 {
                if let Some(animated) = self.animated.as_ref() {
                    tracing::info!(
                        "demo inhibitor: after wave state={:?} inhib1={:?} vulnerable1={} supers={}",
                        animated.debug_inhibitor_state(1, 0),
                        animated.inhibitor_count(1),
                        animated.nexus_vulnerable(1),
                        animated.super_minion_count()
                    );
                }
                self.demo_inhibitor_stage = 2;
            }
            if self.demo_inhibitor_stage == 2 && self.demo_inhibitor_timer >= 5.5 {
                if let Some(animated) = self.animated.as_ref() {
                    tracing::info!(
                        "demo inhibitor: after respawn state={:?} inhib1={:?} vulnerable1={} supers={}",
                        animated.debug_inhibitor_state(1, 0),
                        animated.inhibitor_count(1),
                        animated.nexus_vulnerable(1),
                        animated.super_minion_count()
                    );
                }
                self.demo_inhibitor = false;
            }
        }

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let over_minimap = self.animated.as_ref().is_some_and(|animated| {
            minimap_world_at(
                renderer.size.0 as f32,
                renderer.size.1 as f32,
                self.mouse_pos,
                animated.minimap_bounds,
            )
            .is_some()
        });
        let steering = self.lmb_down
            && !over_minimap
            && !self.demo_camp
            && !self.demo_combat
            && !self.demo_kit
            && !self.demo_shop
            && !self.demo_base
            && !self.demo_inhibitor
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
            if self.demo_kit {
                self.demo_kit_timer += dt;
                if self.demo_kit_stage == 0 && self.demo_kit_timer >= 1.0 {
                    let dest = animated.player_pos() + Vec3::new(500.0, 0.0, 300.0);
                    let ok = animated.cast_w(dest, collision, height);
                    tracing::info!("demo kit: W cast={ok}");
                    self.demo_kit_stage = 1;
                }
                if self.demo_kit_stage == 1 && self.demo_kit_timer >= 2.0 {
                    let ok = animated.cast_e();
                    tracing::info!("demo kit: E cast={ok}");
                    self.demo_kit_stage = 2;
                }
                if self.demo_kit_stage == 2 && self.demo_kit_timer >= 3.0 {
                    if let Some(index) = animated.nearest_enemy(animated.player_pos()) {
                        if let Some(pos) = animated.unit_pos(index) {
                            let ok = animated.cast_r(pos);
                            tracing::info!("demo kit: R cast={ok}");
                        }
                    }
                    self.demo_kit_stage = 3;
                }
                if self.demo_kit_stage == 3 && self.demo_kit_timer >= 4.0 {
                    let s = animated.player_stats();
                    tracing::info!(
                        "demo kit: level {} xp {:.0}/{:.0} gold {} mana {:.0}/{:.0} cds {:?}",
                        s.level,
                        s.xp,
                        s.xp_next,
                        s.gold,
                        s.mana,
                        s.max_mana,
                        s.cooldowns
                    );
                    self.demo_kit = false;
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
            renderer.set_shadow_mesh(&animated.shadow_vertices, &animated.shadow_indices);
            renderer.set_lights(&animated.lights());
            renderer.set_time(self.app_time);
            self.fog_timer += dt;
            if self.fog_timer >= 0.5 {
                self.fog_timer = 0.0;
                rebuild_fog(renderer, animated, height, collision);
            }

            let nexus0 = animated.nexus_health(0).unwrap_or((0.0, 1.0));
            let nexus1 = animated.nexus_health(1).unwrap_or((0.0, 1.0));
            self.lane_log_timer += dt;
            if self.lane_log_timer >= 5.0 {
                self.lane_log_timer = 0.0;
                tracing::info!(
                    "lane: minions={} supers={} visible_enemies={} towers0={:?} towers1={:?} inhib0={:?} inhib1={:?} nexus0={:?} nexus1={:?} vulnerable0={} vulnerable1={}",
                    animated.minion_count(),
                    animated.super_minion_count(),
                    animated.visible_enemy_count(),
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
            let buff_data = animated.team_buff(0).map(|(remaining, duration, kind)| {
                let (color, label) = match kind {
                    CampBuff::Might => ([255, 160, 60], "MIGHT"),
                    CampBuff::Swift => ([90, 220, 255], "SWIFT"),
                };
                (remaining, duration, color, label)
            });
            let buff =
                buff_data.map(|(remaining, duration, color, _)| (remaining, duration, color));
            let buff_label = buff_data.map(|(_, _, _, label)| label);
            let stats = animated.player_stats();
            let (hud_w, hud_h) = (renderer.size.0 as f32, renderer.size.1 as f32);
            let shop_hover = if self.shop.open {
                shop_row_at(hud_w, hud_h, self.mouse_pos)
            } else {
                None
            };
            let player_hud = PlayerHud {
                health: stats.health,
                max_health: stats.max_health,
                mana: stats.mana,
                max_mana: stats.max_mana,
                xp: stats.xp,
                xp_next: stats.xp_next,
                cooldowns: stats.cooldowns,
                shield: stats.shield,
                shield_timer: stats.shield_timer,
                buff,
                buff_label,
                level: stats.level,
                gold: stats.gold,
                items: self.shop.owned_colors(),
                shop_open: self.shop.open,
                shop_hover,
                shop_in_range: animated.in_fountain_range(),
            };
            self.hud.build(
                renderer.size.0 as f32,
                renderer.size.1 as f32,
                &player_hud,
                nexus0,
                nexus1,
                animated.wave_timer,
                animated.wave_interval,
                animated.match_over,
                self.mouse_pos,
            );
            self.hud.build_shop(hud_w, hud_h, &player_hud, &self.shop);
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
                let stats = animated.player_stats();
                let (hp, max_hp) = (stats.health, stats.max_health);
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
                let shop_note = if self.shop.open {
                    match shop_row_at(
                        renderer.size.0 as f32,
                        renderer.size.1 as f32,
                        self.mouse_pos,
                    ) {
                        Some(i) => format!(
                            " | SHOP {} {}g{}",
                            self.shop.items[i].name,
                            self.shop.items[i].cost,
                            if animated.in_fountain_range() {
                                ""
                            } else {
                                " (return to fountain)"
                            }
                        ),
                        None => " | SHOP open (B)".to_string(),
                    }
                } else {
                    String::new()
                };
                if let Some(window) = self.window.as_ref() {
                    window.set_title(&format!(
                        "Rift - Karamja | {} | tile {:.0},{:.0} | {:.0} fps | L{} {:.0}/{:.0} XP | HP {:.0}/{:.0} | MP {:.0}/{:.0} | gold {} | dmg {:.0} spd {:.0} | items {}/6{} | Q/W/E/R {:.1}/{:.1}/{:.1}/{:.1}s | nexus {:.0}/{:.0} ({}) - {:.0}/{:.0} ({}) | inhib {}/{} - {}/{} | wave {:.0}s | minions {} | buff {}{}",
                        animated.champion_name(),
                        pos.x / 128.0,
                        pos.z / 128.0,
                        fps,
                        stats.level,
                        stats.xp,
                        stats.xp_next,
                        hp,
                        max_hp,
                        stats.mana,
                        stats.max_mana,
                        stats.gold,
                        stats.attack_damage,
                        stats.speed,
                        self.shop.item_count(),
                        shop_note,
                        stats.cooldowns[0],
                        stats.cooldowns[1],
                        stats.cooldowns[2],
                        stats.cooldowns[3],
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

    fn click_minimap(&mut self) -> bool {
        let Some(renderer) = self.renderer.as_mut() else {
            return false;
        };
        let Some(animated) = self.animated.as_ref() else {
            return false;
        };
        let Some(height) = self.height.as_ref() else {
            return false;
        };
        let Some((wx, wz)) = minimap_world_at(
            renderer.size.0 as f32,
            renderer.size.1 as f32,
            self.mouse_pos,
            animated.minimap_bounds,
        ) else {
            return false;
        };
        let wy = height.height_at(wx, wz) + 140.0;
        renderer.camera.target = Vec3::new(wx, wy, wz);
        self.follow = false;
        renderer.update_camera();
        tracing::info!("minimap pan to tile {:.0},{:.0}", wx / 128.0, wz / 128.0);
        true
    }

    fn purchase_item(&mut self, index: usize) {
        let Some(item) = self.shop.items.get(index).cloned() else {
            return;
        };
        if !self.shop.has_free_slot() {
            tracing::info!("shop: inventory full");
            return;
        }
        let Some(animated) = self.animated.as_mut() else {
            return;
        };
        match animated.buy_item(&item) {
            Ok(()) => {
                self.shop.add_owned(index);
                tracing::info!(
                    "shop: bought {} ({}) for {}g ({} slots used)",
                    item.name,
                    item.id,
                    item.cost,
                    self.shop.item_count()
                );
            }
            Err(err) => tracing::info!("shop: {} failed: {err}", item.name),
        }
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

    fn cast_w(&mut self) {
        let (Some(renderer), Some(height), Some(collision), Some(animated)) = (
            self.renderer.as_ref(),
            self.height.as_ref(),
            self.collision.as_ref(),
            self.animated.as_mut(),
        ) else {
            return;
        };
        if let Some(point) = renderer.pick_ground(self.mouse_pos.0, self.mouse_pos.1, height) {
            let cast = animated.cast_w(point, collision, height);
            println!("W cast={cast}");
        }
    }

    fn cast_e(&mut self) {
        let Some(animated) = self.animated.as_mut() else {
            return;
        };
        let cast = animated.cast_e();
        println!("E cast={cast}");
    }

    fn cast_r(&mut self) {
        let (Some(renderer), Some(height), Some(animated)) = (
            self.renderer.as_ref(),
            self.height.as_ref(),
            self.animated.as_mut(),
        ) else {
            return;
        };
        if let Some(point) = renderer.pick_ground(self.mouse_pos.0, self.mouse_pos.1, height) {
            let cast = animated.cast_r(point);
            println!("R cast={cast}");
        }
    }

    fn update_camera(&mut self, dt: f32) {
        if self.demo_base {
            self.demo_base_timer += dt;
            if self.demo_base_stage == 0 && self.demo_base_timer >= 0.5 {
                if let Some(animated) = self.animated.as_mut() {
                    animated.debug_damage_player(300.0);
                    let s = animated.player_stats();
                    tracing::info!(
                        "demo base: damaged to {:.0}/{:.0} hp, {:.0}/{:.0} mp, gold {}",
                        s.health,
                        s.max_health,
                        s.mana,
                        s.max_mana,
                        s.gold
                    );
                }
                self.demo_base_stage = 1;
            }
            if self.demo_base_stage == 1 && self.demo_base_timer >= 4.0 {
                if let Some(animated) = self.animated.as_ref() {
                    let s = animated.player_stats();
                    tracing::info!(
                        "demo base: fountain healed to {:.0}/{:.0} hp, {:.0}/{:.0} mp, passive gold {}",
                        s.health,
                        s.max_health,
                        s.mana,
                        s.max_mana,
                        s.gold
                    );
                }
                self.demo_base = false;
            }
        }

        if self.demo_inhibitor {
            self.demo_inhibitor_timer += dt;
            if self.demo_inhibitor_stage == 0 && self.demo_inhibitor_timer >= 0.5 {
                if let Some(animated) = self.animated.as_mut() {
                    let ok = animated.debug_destroy_inhibitor(1, 0, 30.0);
                    tracing::info!(
                        "demo inhibitor: destroyed={ok} state={:?} inhib1={:?} vulnerable1={} supers={}",
                        animated.debug_inhibitor_state(1, 0),
                        animated.inhibitor_count(1),
                        animated.nexus_vulnerable(1),
                        animated.super_minion_count()
                    );
                }
                self.demo_inhibitor_stage = 1;
            }
            if self.demo_inhibitor_stage == 1 && self.demo_inhibitor_timer >= 2.0 {
                if let Some(animated) = self.animated.as_ref() {
                    tracing::info!(
                        "demo inhibitor: after wave state={:?} inhib1={:?} vulnerable1={} supers={}",
                        animated.debug_inhibitor_state(1, 0),
                        animated.inhibitor_count(1),
                        animated.nexus_vulnerable(1),
                        animated.super_minion_count()
                    );
                }
                self.demo_inhibitor_stage = 2;
            }
            if self.demo_inhibitor_stage == 2 && self.demo_inhibitor_timer >= 5.5 {
                if let Some(animated) = self.animated.as_ref() {
                    tracing::info!(
                        "demo inhibitor: after respawn state={:?} inhib1={:?} vulnerable1={} supers={}",
                        animated.debug_inhibitor_state(1, 0),
                        animated.inhibitor_count(1),
                        animated.nexus_vulnerable(1),
                        animated.super_minion_count()
                    );
                }
                self.demo_inhibitor = false;
            }
        }

        let Some(renderer) = self.renderer.as_mut() else {
            return;
        };

        let yaw = renderer.camera.yaw;
        let forward = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
        let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());

        let mut movement = Vec3::ZERO;
        if self.held.contains(&KeyCode::ArrowUp) {
            movement += forward;
        }
        if self.held.contains(&KeyCode::ArrowDown) {
            movement -= forward;
        }
        if self.held.contains(&KeyCode::ArrowRight) {
            movement += right;
        }
        if self.held.contains(&KeyCode::ArrowLeft) {
            movement -= right;
        }

        let mut changed = false;
        if movement.length_squared() > 0.0 {
            self.follow = false;
            let speed = 2600.0 * dt;
            renderer.camera.target += movement.normalize() * speed;
            changed = true;
        }
        if self.held.contains(&KeyCode::KeyZ) {
            renderer.camera.yaw += 1.2 * dt;
            changed = true;
        }
        if self.held.contains(&KeyCode::KeyX) {
            renderer.camera.yaw -= 1.2 * dt;
            changed = true;
        }

        if changed {
            renderer.update_camera();
        }
    }
}

fn rebuild_fog(
    renderer: &mut Renderer,
    animated: &AnimatedScene,
    height: &HeightGrid,
    collision: &WorldCollision,
) {
    let (ox, oy) = collision.origin_tile();
    let width = collision.width();
    let height_tiles = collision.height();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let color = [14u8, 20, 28];

    for gy in 0..height_tiles {
        for gx in 0..width {
            if animated.visibility.is_visible(gx, gy) {
                continue;
            }
            let x0 = (ox + gx) as f32 * 128.0;
            let z0 = (oy + gy) as f32 * 128.0;
            let x1 = x0 + 128.0;
            let z1 = z0 + 128.0;
            let y = height.height_at(x0 + 64.0, z0 + 64.0) + 120.0;
            let base = vertices.len() as u32;
            vertices.push(SceneVertex::unlit(Vec3::new(x0, y, z0), color));
            vertices.push(SceneVertex::unlit(Vec3::new(x1, y, z0), color));
            vertices.push(SceneVertex::unlit(Vec3::new(x0, y, z1), color));
            vertices.push(SceneVertex::unlit(Vec3::new(x1, y, z1), color));
            indices.extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
        }
    }
    renderer.set_fog_mesh(&vertices, &indices);
}

fn parse_debug_view(name: &str) -> u32 {
    match name.to_ascii_lowercase().as_str() {
        "albedo" => 1,
        "normal" => 2,
        "lighting" => 3,
        "fog" => 4,
        "lights" => 5,
        _ => 0,
    }
}

fn digit_index(code: KeyCode) -> Option<usize> {
    match code {
        KeyCode::Digit1 => Some(0),
        KeyCode::Digit2 => Some(1),
        KeyCode::Digit3 => Some(2),
        KeyCode::Digit4 => Some(3),
        KeyCode::Digit5 => Some(4),
        KeyCode::Digit6 => Some(5),
        _ => None,
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
