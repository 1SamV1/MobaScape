mod animated;
mod champion;
mod font;
mod hero_select;
mod hud;
mod items;
mod minimap;
mod model_textures;
mod portraits;
mod renderer;
mod scene;
mod terrain_textures;
mod ui_assets;

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

use crate::animated::{AnimatedScene, BuyOutcome, UnitKind};
use crate::champion::ChampionConfig;
use crate::hero_select::HeroSelect;
use crate::hud::{HeroMarker, HoverInfo, Hud, PlayerHud, minimap_world_at};
use crate::items::{
    ItemShop, delivery_button_at, point_in_rect, shop_button_at, shop_buy_at, shop_card_at,
    shop_card_rect, shop_category_at, shop_close_at, shop_sell_at, shop_window_rect,
};
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
    /// Radius in 64-tile regions (2 = 5x5 block, required by the full map).
    #[arg(long, default_value_t = 2)]
    radius: u32,
    /// Initial camera distance in world units.
    #[arg(long, default_value_t = 2750.0)]
    distance: f32,
    /// Optional initial camera pitch in radians (dev screenshots).
    #[arg(long)]
    pitch: Option<f32>,
    /// Map design data.
    #[arg(long, default_value = "data/maps/karamja_rift/map.json")]
    map: PathBuf,
    /// Champion config name (monkey, jad, kree). Skips the hero-select screen.
    #[arg(long)]
    champion: Option<String>,
    /// Start directly in the match with the default champion.
    #[arg(long)]
    skip_hero_select: bool,
    /// Demo: walk to this tile coordinate after one second (x y).
    #[arg(long, num_args = 2, value_names = ["X", "Y"])]
    demo_target: Option<Vec<f32>>,
    /// Debug: spawn the hero at this tile coordinate (x y).
    #[arg(long, num_args = 2, value_names = ["X", "Y"])]
    spawn: Option<Vec<f32>>,
    /// Demo: auto-target a dummy and fire Q to verify combat.
    #[arg(long)]
    demo_combat: bool,
    /// Demo: teleport to a jungle camp, attack it, and verify the buff.
    #[arg(long)]
    demo_camp: bool,
    /// Demo: verify watchtower aggro, lock range and target reset.
    #[arg(long)]
    demo_watchtower: bool,
    /// Demo: buy away from base, call the guard and verify delivery.
    #[arg(long)]
    demo_delivery: bool,
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
    /// Debug view: lit, albedo, normal, lighting, fog, lights, shadow, ao.
    #[arg(long, default_value = "lit")]
    debug_view: String,
    /// Shadows: off, blob, or map.
    #[arg(long, default_value = "map")]
    shadows: String,
    /// Present mode: fifo (vsync), immediate (uncapped), or mailbox.
    #[arg(long, default_value = "fifo")]
    present_mode: String,
    /// Scene MSAA: 1 (off) or 4 (default).
    #[arg(long, default_value_t = 4)]
    msaa: u32,
    /// Run a fixed-duration frame-time benchmark, then exit (seconds).
    #[arg(long)]
    benchmark: Option<f32>,
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
    let champions = ChampionConfig::load_all()?;
    let selected_champion = match cli.champion.as_deref() {
        Some(name) => champions
            .iter()
            .position(|champion| {
                champion.id.eq_ignore_ascii_case(name) || champion.name.eq_ignore_ascii_case(name)
            })
            .with_context(|| format!("unknown champion '{name}'"))?,
        None => champions
            .iter()
            .position(|champion| champion.id == "monkey")
            .unwrap_or(0),
    };
    let champion = champions[selected_champion].clone();
    let hero_select_open = cli.champion.is_none() && !cli.skip_hero_select && !has_demo_flags(&cli);
    let shop = ItemShop::load()?;
    let scene = scene::build(&cache, cli.region_x, cli.region_y, cli.radius, &cli.map)?;
    let animated = AnimatedScene::new(&cache, &scene, &champion)?;
    let height = scene.height.clone();
    let collision = scene.collision.clone();
    let mut animated = animated;
    if let Some(spawn) = cli.spawn.as_ref().filter(|v| v.len() == 2) {
        let x = (spawn[0] + 0.5) * 128.0;
        let z = (spawn[1] + 0.5) * 128.0;
        let point = Vec3::new(x, height.height_at(x, z), z);
        animated.teleport_player(point);
        tracing::info!("spawned hero at tile {},{}", spawn[0], spawn[1]);
    }
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

    let mut ui_atlas = ui_assets::UiAtlas::load()?;
    if hero_select_open {
        for champion in &champions {
            match portraits::bake_portrait(&cache, champion.model, champion.idle) {
                Ok(image) => ui_atlas.add_hero_portrait(&champion.id, &image),
                Err(err) => tracing::warn!("hero portrait {}: {err:#}", champion.id),
            }
        }
    }
    // Bake a real terrain map for the minimap from the scene colours.
    let minimap_image = minimap::render(&scene, animated.minimap_bounds, minimap::MINIMAP_SIZE);
    ui_atlas.add_minimap(&minimap_image);
    let ui_atlas = Arc::new(ui_atlas);
    let mut app = App {
        cache,
        scene: Some(scene),
        animated: Some(animated),
        height: Some(height),
        collision: Some(collision),
        hero_select: HeroSelect::new(champions, selected_champion, hero_select_open),
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
        demo_watchtower: cli.demo_watchtower,
        demo_watchtower_timer: 0.0,
        demo_watchtower_stage: 0,
        demo_watchtower_target: None,
        demo_delivery: cli.demo_delivery,
        demo_delivery_timer: 0.0,
        demo_delivery_stage: 0,
        demo_kit: cli.demo_kit,
        demo_kit_timer: 0.0,
        demo_kit_stage: 0,
        shop,
        event_log: vec![
            "FOUNTAIN HEALS".to_string(),
            "PRESS B FOR SHOP".to_string(),
            "WAVE INBOUND".to_string(),
        ],
        last_level: 1,
        last_buff_label: None,
        last_inhib0: (3, 3),
        last_inhib1: (3, 3),
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
        hud: Hud::new(ui_atlas),
        graphics_classic: cli.graphics.eq_ignore_ascii_case("classic"),
        debug_view: parse_debug_view(&cli.debug_view),
        shadow_mode: cli.shadows.clone(),
        present_mode: cli.present_mode.clone(),
        msaa: cli.msaa,
        benchmark_remaining: cli.benchmark,
        benchmark_samples: Vec::new(),
        benchmark_done: false,
        app_time: 0.0,
        lane_log_timer: 0.0,
        last_frame: Instant::now(),
        title_timer: 0.0,
        title_frames: 0,
    };
    event_loop.run_app(&mut app)?;
    Ok(())
}

struct App {
    cache: Cache,
    scene: Option<SceneData>,
    animated: Option<AnimatedScene>,
    height: Option<HeightGrid>,
    collision: Option<WorldCollision>,
    hero_select: HeroSelect,
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
    demo_watchtower: bool,
    demo_watchtower_timer: f32,
    demo_watchtower_stage: u8,
    demo_watchtower_target: Option<usize>,
    demo_delivery: bool,
    demo_delivery_timer: f32,
    demo_delivery_stage: u8,
    demo_kit: bool,
    demo_kit_timer: f32,
    demo_kit_stage: u8,
    shop: ItemShop,
    event_log: Vec<String>,
    last_level: u32,
    last_buff_label: Option<&'static str>,
    last_inhib0: (usize, usize),
    last_inhib1: (usize, usize),
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
    shadow_mode: String,
    present_mode: String,
    msaa: u32,
    benchmark_remaining: Option<f32>,
    benchmark_samples: Vec<f32>,
    benchmark_done: bool,
    app_time: f32,
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
        let model_textures = match crate::model_textures::ModelTextures::load(&self.cache) {
            Ok(textures) => textures,
            Err(err) => {
                tracing::error!("failed to load model textures: {err:#}");
                event_loop.exit();
                return;
            }
        };
        let mut renderer = match pollster::block_on(Renderer::new(
            window.clone(),
            &scene,
            camera,
            parse_present_mode(&self.present_mode),
            self.msaa,
            model_textures,
            self.hud.atlas.clone(),
        )) {
            Ok(renderer) => renderer,
            Err(err) => {
                tracing::error!("failed to create renderer: {err:#}");
                event_loop.exit();
                return;
            }
        };

        self.scene = Some(scene);
        renderer.camera.distance = self.initial_distance;
        if let Some(pitch) = self.initial_pitch {
            renderer.camera.pitch = pitch;
        }
        if self.hero_select.open {
            window.set_title("Rift - Choose Your Hero");
        }
        renderer.set_graphics_mode(!self.graphics_classic);
        renderer.set_shadow_mode(parse_shadow_mode(&self.shadow_mode));
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
                if button != MouseButton::Left {
                    return;
                }
                let (w, h) = self
                    .renderer
                    .as_ref()
                    .map(|r| (r.size.0 as f32, r.size.1 as f32))
                    .unwrap_or((1280.0, 720.0));
                if self.hero_select.open {
                    if state == ElementState::Pressed {
                        if let Some(index) = hero_select::card_at(w, h, self.mouse_pos) {
                            self.hero_select.selected = index;
                        } else if hero_select::confirm_at(w, h, self.mouse_pos) {
                            self.start_match();
                        }
                    }
                    return;
                }
                if self.demo_camp
                    || self.demo_combat
                    || self.demo_kit
                    || self.demo_shop
                    || self.demo_base
                    || self.demo_inhibitor
                {
                    return;
                }
                if state == ElementState::Pressed && self.shop.open {
                    if shop_close_at(w, h, self.mouse_pos) {
                        self.shop.open = false;
                        return;
                    }
                    if let Some(category) = shop_category_at(w, h, self.mouse_pos) {
                        self.shop.category = category;
                        self.shop.selected = self.shop.filtered_items().first().copied();
                        return;
                    }
                    if let Some(item) = shop_card_at(w, h, self.mouse_pos, &self.shop) {
                        self.shop.selected = Some(item);
                        return;
                    }
                    if shop_buy_at(w, h, self.mouse_pos) {
                        if let Some(item) = self.shop.selected {
                            self.purchase_item(item);
                        }
                        return;
                    }
                    if shop_sell_at(w, h, self.mouse_pos) {
                        if let Some(item) = self.shop.selected {
                            self.sell_item(item);
                        }
                        return;
                    }
                    if point_in_rect(shop_window_rect(w, h), self.mouse_pos) {
                        return;
                    }
                    self.shop.open = false;
                    return;
                }
                if state == ElementState::Pressed && delivery_button_at(w, h, self.mouse_pos) {
                    if let Some(animated) = self.animated.as_mut() {
                        if animated.call_delivery() {
                            log_event(
                                &mut self.event_log,
                                "ARMADYLEAN GUARD DISPATCHED".to_string(),
                            );
                            tracing::info!("delivery: guard dispatched");
                        }
                    }
                    return;
                }
                if state == ElementState::Pressed
                    && !self.shop.open
                    && shop_button_at(w, h, self.mouse_pos)
                {
                    self.shop.open = true;
                    self.shop.selected = self.shop.filtered_items().first().copied();
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

                if self.hero_select.open {
                    if event.state == ElementState::Pressed {
                        match code {
                            KeyCode::Escape => event_loop.exit(),
                            KeyCode::ArrowLeft | KeyCode::KeyA => self.hero_select.cycle(-1),
                            KeyCode::ArrowRight | KeyCode::KeyD => self.hero_select.cycle(1),
                            KeyCode::Digit1 => self.hero_select.selected = 0,
                            KeyCode::Digit2 => self.hero_select.selected = 1,
                            KeyCode::Digit3 => self.hero_select.selected = 2,
                            KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Space => {
                                self.start_match()
                            }
                            _ => {}
                        }
                    }
                    return;
                }

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
                        if self.shop.open {
                            self.shop.selected = self.shop.filtered_items().first().copied();
                        }
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
                    (renderer.camera.distance * (1.0 - amount * 0.12)).clamp(900.0, 12_000.0);
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

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.benchmark_done {
            event_loop.exit();
            return;
        }
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

        if let Some(remaining) = self.benchmark_remaining.as_mut() {
            *remaining -= dt;
            // Ignore the first second of shader/present warm-up.
            if self.app_time > 1.0 {
                self.benchmark_samples.push(dt);
            }
            if *remaining <= 0.0 {
                let mut samples = std::mem::take(&mut self.benchmark_samples);
                samples.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                let percentile = |p: f32| {
                    if samples.is_empty() {
                        0.0
                    } else {
                        samples[((samples.len() - 1) as f32 * p) as usize] * 1000.0
                    }
                };
                let avg = if samples.is_empty() {
                    0.0
                } else {
                    samples.iter().sum::<f32>() / samples.len() as f32 * 1000.0
                };
                tracing::info!(
                    "benchmark: frames={} avg={:.2}ms p50={:.2}ms p95={:.2}ms p99={:.2}ms fps={:.1}",
                    samples.len(),
                    avg,
                    percentile(0.50),
                    percentile(0.95),
                    percentile(0.99),
                    1000.0 / avg.max(0.001)
                );
                self.benchmark_remaining = None;
                self.benchmark_done = true;
            }
        }

        if self.hero_select.open {
            if self.renderer.is_some() {
                self.tick_hero_select(dt);
            }
            return;
        }

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
                let filtered = self.shop.filtered_items();
                let slot = filtered
                    .iter()
                    .position(|index| !self.shop.is_owned(*index))
                    .unwrap_or(0);
                let (x, y, rw, rh) = shop_card_rect(w, h, slot);
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
                            animated.refresh_visibility(1.0, collision);
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
                        let camp_pos = animated.unit_pos(index).unwrap_or(Vec3::ZERO);
                        let home = animated.unit_home(index).unwrap_or(Vec3::ZERO);
                        let aggro = animated.unit_camp_aggro(index).unwrap_or(false);
                        let player_health = animated.player_stats().health;
                        tracing::info!(
                            "demo camp: health {health:.0}, player {player_health:.0}, aggro {aggro}, camp {camp_pos:?}, home {home:?}, drift {:.0}, buff {buff:?}",
                            camp_pos.distance(home)
                        );
                    }
                    self.demo_camp_done = true;
                }
            }
            if self.demo_delivery {
                self.demo_delivery_timer += dt;
                if self.demo_delivery_stage == 0 && self.demo_delivery_timer >= 1.0 {
                    let away = animated.player_pos() + Vec3::new(26.0 * 128.0, 0.0, 0.0);
                    animated.teleport_player(away);
                    animated.debug_grant_gold(4000);
                    let first = self.shop.items[0].clone();
                    let second = self.shop.items[1].clone();
                    let _ = animated.buy_item(0, &first);
                    let _ = animated.buy_item(1, &second);
                    tracing::info!(
                        "demo delivery: pending {}, shop items {}",
                        animated.delivery_count(),
                        self.shop.item_count()
                    );
                    self.demo_delivery_stage = 1;
                }
                if self.demo_delivery_stage == 1 && self.demo_delivery_timer >= 2.0 {
                    let ok = animated.call_delivery();
                    tracing::info!("demo delivery: guard dispatched={ok}");
                    self.demo_delivery_stage = 2;
                }
                if self.demo_delivery_stage == 2 && self.demo_delivery_timer >= 9.0 {
                    let stats = animated.player_stats();
                    tracing::info!(
                        "demo delivery: pending {}, shop items {}, damage {:.0}, hp {:.0}/{:.0}, state {:?}",
                        animated.delivery_count(),
                        self.shop.item_count(),
                        stats.attack_damage,
                        stats.health,
                        stats.max_health,
                        animated.delivery_state
                    );
                    self.demo_delivery = false;
                }
            }
            if self.demo_watchtower {
                self.demo_watchtower_timer += dt;
                if self.demo_watchtower_stage == 0 && self.demo_watchtower_timer >= 1.0 {
                    if let Some(index) = animated.nearest_watchtower(1, animated.player_pos()) {
                        if let Some(pos) = animated.unit_pos(index) {
                            let spawn = pos + Vec3::new(0.0, 0.0, 4.0 * 128.0);
                            animated.teleport_player(spawn);
                            self.demo_watchtower_target = Some(index);
                            tracing::info!(
                                "demo watchtower: target {index}, player inside aggro at 4 tiles"
                            );
                        }
                    }
                    self.demo_watchtower_stage = 1;
                }
                if self.demo_watchtower_stage == 1 && self.demo_watchtower_timer >= 4.0 {
                    let hp = animated.player_stats().health;
                    let target = self
                        .demo_watchtower_target
                        .and_then(|i| animated.unit_attack_target(i));
                    tracing::info!(
                        "demo watchtower: after aggro, player hp {hp:.0}, tower target {target:?}"
                    );
                    if let Some(index) = self.demo_watchtower_target {
                        if let Some(pos) = animated.unit_pos(index) {
                            // 8 tiles away: outside the 620 aggro radius but
                            // inside the 1200 lock range.
                            animated.teleport_player(pos + Vec3::new(0.0, 0.0, 8.0 * 128.0));
                        }
                    }
                    self.demo_watchtower_stage = 2;
                }
                if self.demo_watchtower_stage == 2 && self.demo_watchtower_timer >= 8.0 {
                    let hp = animated.player_stats().health;
                    let target = self
                        .demo_watchtower_target
                        .and_then(|i| animated.unit_attack_target(i));
                    tracing::info!(
                        "demo watchtower: at 8 tiles (outside aggro, inside lock) hp {hp:.0}, tower target {target:?}"
                    );
                    if let Some(index) = self.demo_watchtower_target {
                        if let Some(pos) = animated.unit_pos(index) {
                            // 24 tiles away: outside the lock range.
                            animated.teleport_player(pos + Vec3::new(0.0, 0.0, 24.0 * 128.0));
                        }
                    }
                    self.demo_watchtower_stage = 3;
                }
                if self.demo_watchtower_stage == 3 && self.demo_watchtower_timer >= 12.0 {
                    let hp = animated.player_stats().health;
                    let target = self
                        .demo_watchtower_target
                        .and_then(|i| animated.unit_attack_target(i));
                    tracing::info!(
                        "demo watchtower: at 24 tiles (outside lock) hp {hp:.0}, tower target {target:?}"
                    );
                    self.demo_watchtower = false;
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
                animated.update(dt, height, collision);
            }
            animated.refresh_visibility(dt, collision);
            animated.prepare_render(height, camera_right, camera_up, camera_forward);
            renderer.set_dynamic_mesh(&animated.vertices, &animated.indices);
            renderer.set_shadow_mesh(&animated.shadow_vertices, &animated.shadow_indices);
            renderer.set_caster_mesh(&animated.caster_vertices, &animated.caster_indices);
            renderer.set_lights(&animated.lights());
            renderer.set_time(self.app_time);
            // Fog of war is a shader-side circle around the hero, so it has
            // no geometry to fight with the terrain's z levels.
            renderer.set_vision(animated.player_pos(), 1500.0);
            animated.visibility_dirty = false;

            let nexus0 = animated.nexus_health(0).unwrap_or((0.0, 1.0));
            let nexus1 = animated.nexus_health(1).unwrap_or((0.0, 1.0));
            self.lane_log_timer += dt;
            if self.lane_log_timer >= 5.0 {
                self.lane_log_timer = 0.0;
                tracing::debug!("minion states: {:?}", animated.debug_minion_states());
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
            if stats.level > self.last_level {
                self.last_level = stats.level;
                log_event(&mut self.event_log, format!("LEVEL UP! L{}", stats.level));
            }
            if buff_label != self.last_buff_label {
                self.last_buff_label = buff_label;
                if let Some(label) = buff_label {
                    log_event(&mut self.event_log, format!("BUFF {label}"));
                }
            }
            let i0 = animated.inhibitor_count(0);
            if i0.0 < self.last_inhib0.0 {
                log_event(&mut self.event_log, "INHIBITOR DOWN".to_string());
            } else if i0.0 > self.last_inhib0.0 {
                log_event(&mut self.event_log, "INHIBITOR RESPAWN".to_string());
            }
            self.last_inhib0 = i0;
            let i1 = animated.inhibitor_count(1);
            if i1.0 < self.last_inhib1.0 {
                log_event(&mut self.event_log, "ENEMY INHIBITOR DOWN".to_string());
            } else if i1.0 > self.last_inhib1.0 {
                log_event(&mut self.event_log, "ENEMY INHIBITOR UP".to_string());
            }
            self.last_inhib1 = i1;
            let (hud_w, hud_h) = (renderer.size.0 as f32, renderer.size.1 as f32);
            let shop_hover = if self.shop.open {
                shop_card_at(hud_w, hud_h, self.mouse_pos, &self.shop)
            } else {
                None
            };
            let towers = {
                let healths = animated.tower_healths(0);
                let alive = healths.iter().filter(|(hp, _)| *hp > 0.0).count();
                (alive, healths.len())
            };
            for index in animated.take_delivered() {
                self.shop.add_owned(index);
                if let Some(item) = self.shop.items.get(index) {
                    log_event(&mut self.event_log, format!("DELIVERED {}", item.name));
                    tracing::info!("delivery: handed over {}", item.name);
                }
            }
            let scores = animated.team_scores();
            let heroes: Vec<HeroMarker> = animated
                .units
                .iter()
                .filter(|unit| unit.kind == UnitKind::Player && unit.team == 0)
                .map(|unit| HeroMarker {
                    name: animated.champion_name().to_string(),
                    team: unit.team,
                    pos: (unit.pos.x, unit.pos.z),
                    alive: unit.health > 0.0,
                    health: unit.health,
                    max_health: unit.max_health,
                    mana: unit.mana,
                    max_mana: unit.max_mana,
                    level: unit.level,
                })
                .collect();
            let hover = {
                let mut hover = None;
                if let Some((origin, dir)) = renderer.screen_ray(self.mouse_pos.0, self.mouse_pos.1)
                {
                    if let Some(index) = animated.pick_unit(origin, dir) {
                        if let Some((name, level, description)) = animated.unit_info(index) {
                            let (health, max_health, team, kind) = animated.unit_status(index);
                            hover = Some(HoverInfo {
                                name,
                                level,
                                health,
                                max_health,
                                team,
                                kind,
                                description,
                            });
                        }
                    }
                }
                hover
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
                shop_open: self.shop.open,
                shop_hover,
                shop_selected: self.shop.selected,
                champion_name: animated.champion_name().to_string(),
                minions: animated.minion_count(),
                visible_enemies: animated.visible_enemy_count(),
                towers,
                inhibitors: animated.inhibitor_count(0),
                items_owned: self.shop.item_count(),
                events: self.event_log.clone(),
                kills: (scores.0.0, scores.1.0),
                deaths: (scores.0.1, scores.1.1),
                match_time: self.app_time,
                hover,
                biome: animated.current_biome(),
                heroes: heroes.clone(),
                delivery_pending: animated.delivery_count(),
                delivery_state: match animated.delivery_state {
                    crate::animated::DeliveryState::Idle => 0,
                    crate::animated::DeliveryState::FlyingToHero => 1,
                    crate::animated::DeliveryState::Returning => 2,
                },
                pending_ids: animated
                    .delivery_pending
                    .iter()
                    .map(|(_, item)| item.id.clone())
                    .collect(),
            };
            self.hud.build(
                renderer.size.0 as f32,
                renderer.size.1 as f32,
                &player_hud,
                &self.shop,
                nexus0,
                nexus1,
                animated.wave_timer,
                animated.wave_interval,
                animated.match_over,
                self.mouse_pos,
            );
            self.hud.build_shop(hud_w, hud_h, &player_hud, &self.shop);
            self.hud.build_minimap(
                renderer.size.0 as f32,
                renderer.size.1 as f32,
                animated,
                (renderer.camera.target.x, renderer.camera.target.z),
                &heroes,
            );
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
                    match shop_card_at(
                        renderer.size.0 as f32,
                        renderer.size.1 as f32,
                        self.mouse_pos,
                        &self.shop,
                    ) {
                        Some(i) => format!(
                            " | SHOP {} {}g",
                            self.shop.items[i].name, self.shop.items[i].cost
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

    /// Render the paused world behind the champion picker.
    fn tick_hero_select(&mut self, dt: f32) {
        self.hero_select.time += dt;
        let (w, h) = self
            .renderer
            .as_ref()
            .map(|r| (r.size.0 as f32, r.size.1 as f32))
            .unwrap_or((1280.0, 720.0));
        self.hero_select.hover = hero_select::card_at(w, h, self.mouse_pos);

        if let (Some(renderer), Some(animated), Some(height), Some(collision)) = (
            self.renderer.as_mut(),
            self.animated.as_mut(),
            self.height.as_ref(),
            self.collision.as_ref(),
        ) {
            let eye = renderer.camera.eye();
            let forward = (eye - renderer.camera.target).normalize_or_zero();
            let right = forward.cross(Vec3::Y).normalize_or_zero();
            let up = right.cross(forward).normalize_or_zero();
            animated.refresh_visibility(dt.min(0.05), collision);
            animated.prepare_render(height, right, up, forward);
            renderer.set_dynamic_mesh(&animated.vertices, &animated.indices);
            renderer.set_shadow_mesh(&animated.shadow_vertices, &animated.shadow_indices);
            renderer.set_caster_mesh(&animated.caster_vertices, &animated.caster_indices);
            renderer.set_lights(&animated.lights());
            renderer.set_time(self.app_time);
            renderer.set_vision(animated.player_pos(), 1500.0);
            animated.visibility_dirty = false;
            if self.follow {
                renderer.camera.target = animated.player_pos() + Vec3::Y * 140.0;
                renderer.update_camera();
            }
        }

        self.hud
            .build_hero_select(w, h, &self.hero_select, self.mouse_pos);
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.set_hud_mesh(&self.hud.vertices, &self.hud.indices);
        }
    }

    /// Lock in the highlighted champion and start a clean match.
    fn start_match(&mut self) {
        let index = self.hero_select.selected;
        let Some(champion) = self.hero_select.champions.get(index).cloned() else {
            return;
        };
        let Some(scene) = self.scene.as_ref() else {
            return;
        };
        let animated = match AnimatedScene::new(&self.cache, scene, &champion) {
            Ok(animated) => animated,
            Err(err) => {
                tracing::error!("failed to start match as {}: {err:#}", champion.name);
                return;
            }
        };

        self.animated = Some(animated);
        self.hero_select.open = false;
        self.hero_select.hover = None;
        self.shop.reset();
        self.event_log = vec![
            format!("{} READY", champion.name.to_uppercase()),
            "PRESS B FOR SHOP".to_string(),
            "WAVE INBOUND".to_string(),
        ];
        self.last_level = 1;
        self.last_buff_label = None;
        self.last_inhib0 = (3, 3);
        self.last_inhib1 = (3, 3);
        self.lane_log_timer = 0.0;
        self.title_timer = 0.0;
        self.title_frames = 0;
        self.app_time = 0.0;
        self.lmb_down = false;
        self.lmb_press = None;
        self.held.clear();
        self.follow = true;
        self.last_frame = Instant::now();

        if let (Some(renderer), Some(animated)) = (self.renderer.as_mut(), self.animated.as_ref()) {
            renderer.camera.target = animated.player_pos() + Vec3::Y * 140.0;
            renderer.camera.distance = self.initial_distance;
            renderer.update_camera();
        }
        if let Some(window) = self.window.as_ref() {
            window.set_title(&format!("Rift - Karamja | {}", champion.name));
        }
        tracing::info!(
            "hero select: locked in {} ({}) at tile {:.0},{:.0}",
            champion.name,
            champion.id,
            self.animated
                .as_ref()
                .map(|animated| animated.player_pos().x / 128.0)
                .unwrap_or(0.0),
            self.animated
                .as_ref()
                .map(|animated| animated.player_pos().z / 128.0)
                .unwrap_or(0.0)
        );
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
        let pending = self
            .animated
            .as_ref()
            .map(|animated| animated.delivery_count())
            .unwrap_or(0);
        if !self.shop.has_free_slot() || self.shop.item_count() + pending >= 6 {
            tracing::info!("shop: inventory full (including items in transit)");
            log_event(&mut self.event_log, "INVENTORY FULL".to_string());
            return;
        }
        let Some(animated) = self.animated.as_mut() else {
            return;
        };
        match animated.buy_item(index, &item) {
            Ok(BuyOutcome::Delivered) => {
                self.shop.add_owned(index);
                tracing::info!(
                    "shop: bought {} ({}) for {}g ({} slots used)",
                    item.name,
                    item.id,
                    item.cost,
                    self.shop.item_count()
                );
                log_event(&mut self.event_log, format!("BOUGHT {}", item.name));
            }
            Ok(BuyOutcome::Stored) => {
                tracing::info!("shop: {} stored in the chest for delivery", item.name);
                log_event(&mut self.event_log, format!("STORED {}", item.name));
            }
            Err(err) => tracing::info!("shop: {} failed: {err}", item.name),
        }
    }

    fn sell_item(&mut self, index: usize) {
        let Some(item) = self.shop.items.get(index).cloned() else {
            return;
        };
        if !self.shop.is_owned(index) {
            return;
        }
        let Some(animated) = self.animated.as_mut() else {
            return;
        };
        match animated.sell_item(&item) {
            Ok(()) => {
                self.shop.remove_owned(index);
                tracing::info!(
                    "shop: sold {} for {}g ({} slots used)",
                    item.name,
                    item.cost / 2,
                    self.shop.item_count()
                );
                log_event(&mut self.event_log, format!("SOLD {}", item.name));
            }
            Err(err) => tracing::info!("shop: sell {} failed: {err}", item.name),
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

/// Dev demos and explicit spawn requests boot straight into the match instead
/// of stopping at the champion picker.
fn has_demo_flags(cli: &Cli) -> bool {
    cli.demo_target.is_some()
        || cli.spawn.is_some()
        || cli.demo_combat
        || cli.demo_camp
        || cli.demo_watchtower
        || cli.demo_delivery
        || cli.demo_kit
        || cli.demo_shop
        || cli.demo_base
        || cli.demo_inhibitor
}

fn log_event(events: &mut Vec<String>, message: String) {
    events.insert(0, message);
    events.truncate(5);
}

fn parse_present_mode(name: &str) -> wgpu::PresentMode {
    match name.to_ascii_lowercase().as_str() {
        "immediate" => wgpu::PresentMode::Immediate,
        "mailbox" => wgpu::PresentMode::Mailbox,
        _ => wgpu::PresentMode::Fifo,
    }
}

fn parse_shadow_mode(name: &str) -> u32 {
    match name.to_ascii_lowercase().as_str() {
        "off" => 0,
        "blob" => 1,
        "map" => 2,
        _ => 2,
    }
}

fn parse_debug_view(name: &str) -> u32 {
    match name.to_ascii_lowercase().as_str() {
        "albedo" => 1,
        "normal" => 2,
        "lighting" => 3,
        "fog" => 4,
        "lights" => 5,
        "shadow" => 6,
        "ao" => 7,
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
