use anyhow::Result;
use glam::{Mat3, Vec3};
use rift_cache::{AnimationClip, Cache, Model, model::hsl_to_rgb};

use crate::champion::ChampionConfig;
use crate::items::ItemConfig;
use crate::scene::{
    CampBuff, HeightGrid, SceneData, SceneVertex, StructureKind, WorldCollision,
    model_vertex_animated,
};

const Q_COOLDOWN: f32 = 5.0;
const PLAYER_RESPAWN: f32 = 5.0;
const MINION_AGGRO: f32 = 520.0;
const MAX_MINIONS_PER_TEAM: usize = 36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnitKind {
    Player,
    Ambient,
    Dummy,
    MeleeMinion,
    RangedMinion,
    Tower,
    Inhibitor,
    Nexus,
    Camp,
}

impl UnitKind {
    fn is_minion(self) -> bool {
        matches!(self, Self::MeleeMinion | Self::RangedMinion)
    }

    pub fn is_structure(self) -> bool {
        matches!(self, Self::Tower | Self::Inhibitor | Self::Nexus)
    }
}

pub struct AnimatedUnit {
    pub model: Model,
    pub idle: AnimationClip,
    pub walk: Option<AnimationClip>,
    pub pos: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub anim_time_ms: f32,
    pub moving: bool,
    pub kind: UnitKind,
    pub team: u8,
    pub health: f32,
    pub max_health: f32,
    pub hit_flash: f32,
    pub dead_timer: f32,
    pub target: Option<Vec3>,
    pub path: Vec<Vec3>,
    pub path_index: usize,
    pub attack_target: Option<usize>,
    pub attack_cooldown: f32,
    pub attack_range: f32,
    pub attack_damage: f32,
    pub attack_interval: f32,
    pub speed: f32,
    pub hit_radius: f32,
    pub bar_height: f32,
    pub lane: Vec<Vec3>,
    pub lane_index: usize,
    pub respawn_time: f32,
    pub camp_buff: Option<CampBuff>,
    pub mana: f32,
    pub max_mana: f32,
    pub level: u32,
    pub xp: f32,
    pub gold: u32,
    pub shield: f32,
    pub shield_timer: f32,
    pub cooldowns: [f32; 4],
    pub visible: bool,
    pub ranged_auto: bool,
    pub q_damage: f32,
    pub r_damage: f32,
    pub e_heal: f32,
    pub e_shield: f32,
    pub w_distance: f32,
    pub ability_power: f32,
    pub structure_lane: Option<usize>,
    pub super_minion: bool,
}

#[derive(Clone, Copy)]
pub struct PlayerStats {
    pub health: f32,
    pub max_health: f32,
    pub mana: f32,
    pub max_mana: f32,
    pub level: u32,
    pub xp: f32,
    pub xp_next: f32,
    pub gold: u32,
    pub cooldowns: [f32; 4],
    pub shield: f32,
    pub shield_timer: f32,
    pub attack_damage: f32,
    pub speed: f32,
    pub ability_power: f32,
}

pub struct Effect {
    pub pos: Vec3,
    pub radius: f32,
    pub remaining: f32,
    pub duration: f32,
    pub color: [u8; 3],
}

#[derive(Debug, Clone, Copy)]
pub struct SceneLight {
    pub pos: Vec3,
    pub color: [f32; 3],
    pub radius: f32,
    pub intensity: f32,
}

pub struct VisibilityGrid {
    pub width: usize,
    pub height: usize,
    pub tiles: Vec<bool>,
}

impl VisibilityGrid {
    pub fn is_visible(&self, tx: i32, ty: i32) -> bool {
        tx >= 0
            && ty >= 0
            && (tx as usize) < self.width
            && (ty as usize) < self.height
            && self.tiles[ty as usize * self.width + tx as usize]
    }
}

#[derive(Clone, Copy)]
pub struct TeamBuff {
    pub remaining: f32,
    pub duration: f32,
    pub damage_mult: f32,
    pub speed_mult: f32,
    pub kind: CampBuff,
}

pub struct Projectile {
    pub pos: Vec3,
    pub dir: Vec3,
    pub speed: f32,
    pub life: f32,
    pub damage: f32,
    pub team: u8,
}

struct MinionTemplate {
    model: Model,
    idle: AnimationClip,
    walk: AnimationClip,
    health: f32,
    range: f32,
    damage: f32,
    interval: f32,
    speed: f32,
    scale: f32,
}

pub struct AnimatedScene {
    pub units: Vec<AnimatedUnit>,
    pub player_index: usize,
    pub vertices: Vec<SceneVertex>,
    pub indices: Vec<u32>,
    pub shadow_vertices: Vec<SceneVertex>,
    pub shadow_indices: Vec<u32>,
    pub marker: Option<(Vec3, f32)>,
    pub projectiles: Vec<Projectile>,
    pub effects: Vec<Effect>,
    pub wave_timer: f32,
    pub wave_interval: f32,
    pub gold_income: f32,
    pub lanes: Vec<Vec<Vec3>>,
    pub nexus_positions: [Vec3; 2],
    pub player_spawn: Vec3,
    pub match_over: Option<u8>,
    pub minimap_bounds: (glam::Vec2, glam::Vec2),
    pub visibility: VisibilityGrid,
    pub brush: Vec<(f32, f32, f32, f32)>,
    pub vision_origin: (i32, i32),
    pub team_buffs: [Option<TeamBuff>; 2],
    pub champion_name: String,
    melee_template: MinionTemplate,
    ranged_template: MinionTemplate,
}

impl AnimatedScene {
    pub fn new(cache: &Cache, scene: &SceneData, champion: &ChampionConfig) -> Result<Self> {
        let mut units = Vec::new();

        for spawn in &scene.npc_spawns {
            units.push(load_unit(
                cache,
                spawn.model_id,
                Some(spawn.idle_sequence),
                spawn.walk_sequence,
                spawn.pos,
                spawn.scale,
                UnitKind::Ambient,
                2,
                100.0,
                0.0,
                0.0,
                0.0,
                0.0,
                90.0,
                150.0,
                Vec::new(),
            )?);
        }

        let player_index = units.len();
        units.push(load_unit(
            cache,
            champion.model,
            Some(champion.idle),
            Some(champion.walk),
            scene.player_spawn,
            champion.scale,
            UnitKind::Player,
            0,
            champion.health,
            champion.attack_range,
            champion.damage,
            champion.attack_interval,
            champion.speed,
            240.0,
            380.0,
            Vec::new(),
        )?);
        {
            let player = &mut units[player_index];
            player.mana = champion.mana;
            player.max_mana = champion.mana;
            player.level = 1;
            player.ranged_auto = champion.ranged_auto;
            player.q_damage = champion.q_damage;
            player.r_damage = champion.r_damage;
            player.e_heal = champion.e_heal;
            player.e_shield = champion.e_shield;
            player.w_distance = champion.w_distance;
        }

        // A training dummy next to the spawn keeps combat testable without
        // waiting for the first wave to meet in the middle.
        let dummy_world = scene.player_spawn + Vec3::new(4.0 * 128.0, 0.0, 0.0);
        if let Some((tx, ty)) = scene
            .collision
            .nearest_walkable_world(dummy_world.x, dummy_world.z)
        {
            let dummy_pos = scene.collision.world_of_tile(tx, ty, &scene.height);
            units.push(load_unit(
                cache,
                7744,
                Some(222),
                Some(219),
                dummy_pos,
                2.4,
                UnitKind::Dummy,
                1,
                120.0,
                0.0,
                0.0,
                0.0,
                0.0,
                130.0,
                300.0,
                Vec::new(),
            )?);
        }

        for structure in &scene.map.structures {
            tracing::info!(
                "structure: {:?} team {} model {} pos {:?}",
                structure.kind,
                structure.team,
                structure.model_id,
                structure.pos
            );
            let kind = match structure.kind {
                StructureKind::Tower => UnitKind::Tower,
                StructureKind::Inhibitor => UnitKind::Inhibitor,
                StructureKind::Nexus => UnitKind::Nexus,
            };
            let (hit_radius, bar_height) = match kind {
                UnitKind::Tower => (230.0, 500.0),
                UnitKind::Inhibitor => (230.0, 430.0),
                UnitKind::Nexus => (300.0, 400.0),
                _ => (150.0, 200.0),
            };
            let mut unit = load_unit(
                cache,
                structure.model_id,
                None,
                None,
                structure.pos,
                structure.scale,
                kind,
                structure.team,
                structure.health,
                structure.range,
                structure.damage,
                structure.interval,
                0.0,
                hit_radius,
                bar_height,
                Vec::new(),
            )?;
            unit.structure_lane = structure.lane_index;
            units.push(unit);
        }

        for camp in &scene.map.camps {
            tracing::info!(
                "camp: {} model {} pos {:?}",
                camp.id,
                camp.model_id,
                camp.pos
            );
            let mut unit = load_unit(
                cache,
                camp.model_id,
                camp.idle_sequence,
                camp.walk_sequence,
                camp.pos,
                camp.scale,
                UnitKind::Camp,
                2,
                camp.health,
                0.0,
                0.0,
                0.0,
                0.0,
                180.0,
                350.0,
                Vec::new(),
            )?;
            unit.respawn_time = camp.respawn;
            unit.camp_buff = Some(camp.buff);
            units.push(unit);
        }

        let nexus_positions = [
            find_nexus(&units, 0).unwrap_or(scene.player_spawn),
            find_nexus(&units, 1).unwrap_or(scene.player_spawn),
        ];

        let melee_template = MinionTemplate {
            model: load_model(cache, 2944)?,
            idle: AnimationClip::load(cache, 262)?,
            walk: AnimationClip::load(cache, 259)?,
            health: 90.0,
            range: 100.0,
            damage: 7.0,
            interval: 1.0,
            speed: 400.0,
            scale: 1.15,
        };
        let ranged_template = MinionTemplate {
            model: load_model(cache, 21193)?,
            idle: AnimationClip::load(cache, 5483)?,
            walk: AnimationClip::load(cache, 5479)?,
            health: 65.0,
            range: 380.0,
            damage: 9.0,
            interval: 1.3,
            speed: 380.0,
            scale: 1.15,
        };

        let mut min = glam::Vec2::splat(f32::MAX);
        let mut max = glam::Vec2::splat(f32::MIN);
        for lane in &scene.map.lanes {
            for p in &lane.waypoints {
                min = min.min(glam::Vec2::new(p.x, p.z));
                max = max.max(glam::Vec2::new(p.x, p.z));
            }
        }
        for unit in &units {
            if unit.kind.is_structure() || unit.kind == UnitKind::Camp {
                min = min.min(glam::Vec2::new(unit.pos.x, unit.pos.z));
                max = max.max(glam::Vec2::new(unit.pos.x, unit.pos.z));
            }
        }
        let pad = glam::Vec2::splat(3.0 * 128.0);
        let minimap_bounds = (min - pad, max + pad);

        let (vision_origin_x, vision_origin_y) = scene.collision.origin_tile();
        let visibility = VisibilityGrid {
            width: scene.collision.width() as usize,
            height: scene.collision.height() as usize,
            tiles: vec![false; (scene.collision.width() * scene.collision.height()) as usize],
        };

        let mut animated = Self {
            units,
            player_index,
            vertices: Vec::new(),
            indices: Vec::new(),
            shadow_vertices: Vec::new(),
            shadow_indices: Vec::new(),
            marker: None,
            projectiles: Vec::new(),
            effects: Vec::new(),
            wave_timer: 0.0,
            wave_interval: scene.map.wave_interval,
            gold_income: 0.0,
            lanes: scene
                .map
                .lanes
                .iter()
                .map(|lane| lane.waypoints.clone())
                .collect(),
            nexus_positions,
            player_spawn: scene.player_spawn,
            match_over: None,
            minimap_bounds,
            visibility,
            brush: scene.map.brush.clone(),
            vision_origin: (vision_origin_x, vision_origin_y),
            team_buffs: [None, None],
            champion_name: champion.name.clone(),
            melee_template,
            ranged_template,
        };
        // Persistent fountain rings mark the heal/shop area at spawn.
        animated.effects.push(Effect {
            pos: scene.player_spawn,
            radius: 240.0,
            remaining: 1.0e9,
            duration: 1.0e9,
            color: [70, 210, 130],
        });
        animated.effects.push(Effect {
            pos: scene.player_spawn,
            radius: 140.0,
            remaining: 1.0e9,
            duration: 1.0e9,
            color: [120, 255, 170],
        });
        for lane_index in 0..animated.lanes.len() {
            animated.spawn_wave(0, lane_index);
            animated.spawn_wave(1, lane_index);
        }
        animated.wave_timer = animated.wave_interval;
        Ok(animated)
    }

    pub fn champion_name(&self) -> &str {
        &self.champion_name
    }

    pub fn player_pos(&self) -> Vec3 {
        self.units[self.player_index].pos
    }

    pub fn player_stats(&self) -> PlayerStats {
        let p = &self.units[self.player_index];
        PlayerStats {
            health: p.health,
            max_health: p.max_health,
            mana: p.mana,
            max_mana: p.max_mana,
            level: p.level,
            xp: p.xp,
            xp_next: xp_for_level(p.level),
            gold: p.gold,
            cooldowns: p.cooldowns,
            shield: p.shield,
            shield_timer: p.shield_timer,
            attack_damage: p.attack_damage,
            speed: p.speed,
            ability_power: p.ability_power,
        }
    }

    pub fn in_fountain_range(&self) -> bool {
        let player = &self.units[self.player_index];
        player.health > 0.0 && player.pos.distance(self.player_spawn) <= 900.0
    }

    pub fn debug_grant_gold(&mut self, amount: u32) {
        let player = &mut self.units[self.player_index];
        player.gold = player.gold.saturating_add(amount);
    }

    pub fn debug_destroy_inhibitor(&mut self, team: u8, lane_index: usize, respawn: f32) -> bool {
        if let Some(unit) = self.units.iter_mut().find(|u| {
            u.kind == UnitKind::Inhibitor && u.team == team && u.structure_lane == Some(lane_index)
        }) {
            unit.health = 0.0;
            unit.dead_timer = respawn;
            unit.attack_target = None;
            true
        } else {
            false
        }
    }

    pub fn debug_inhibitor_state(&self, team: u8, lane_index: usize) -> Option<(f32, f32, f32)> {
        self.units
            .iter()
            .find(|u| {
                u.kind == UnitKind::Inhibitor
                    && u.team == team
                    && u.structure_lane == Some(lane_index)
            })
            .map(|u| (u.health, u.max_health, u.dead_timer))
    }

    pub fn super_minion_count(&self) -> usize {
        self.units
            .iter()
            .filter(|u| u.super_minion && u.health > 0.0)
            .count()
    }

    pub fn debug_damage_player(&mut self, amount: f32) {
        let player = &mut self.units[self.player_index];
        player.health = (player.health - amount).max(1.0);
    }

    /// Buy an item at the fountain. The client applies stats locally for the
    /// prototype; the authoritative server will own this transaction later.
    pub fn buy_item(&mut self, item: &ItemConfig) -> Result<(), &'static str> {
        let pi = self.player_index;
        let player = &mut self.units[pi];
        if player.health <= 0.0 {
            return Err("dead");
        }
        if player.pos.distance(self.player_spawn) > 900.0 {
            return Err("out of fountain range");
        }
        if player.gold < item.cost {
            return Err("not enough gold");
        }
        player.gold -= item.cost;
        player.attack_damage += item.damage;
        if item.health > 0.0 {
            player.max_health += item.health;
            player.health += item.health;
        }
        if item.mana > 0.0 {
            player.max_mana += item.mana;
            player.mana += item.mana;
        }
        player.speed += item.speed;
        player.ability_power += item.ability;
        Ok(())
    }

    pub fn nexus_health(&self, team: u8) -> Option<(f32, f32)> {
        self.units
            .iter()
            .find(|u| u.kind == UnitKind::Nexus && u.team == team)
            .map(|u| (u.health, u.max_health))
    }

    pub fn minion_count(&self) -> usize {
        self.units.iter().filter(|u| u.kind.is_minion()).count()
    }

    pub fn inhibitor_count(&self, team: u8) -> (usize, usize) {
        let all = self
            .units
            .iter()
            .filter(|u| u.kind == UnitKind::Inhibitor && u.team == team)
            .count();
        let alive = self
            .units
            .iter()
            .filter(|u| u.kind == UnitKind::Inhibitor && u.team == team && u.health > 0.0)
            .count();
        (alive, all)
    }

    pub fn nexus_vulnerable(&self, team: u8) -> bool {
        self.units
            .iter()
            .any(|u| u.kind == UnitKind::Inhibitor && u.team == team && u.health <= 0.0)
    }

    pub fn tower_healths(&self, team: u8) -> Vec<(f32, f32)> {
        self.units
            .iter()
            .filter(|u| u.kind == UnitKind::Tower && u.team == team)
            .map(|u| (u.health, u.max_health))
            .collect()
    }

    pub fn set_player_target(
        &mut self,
        target: Vec3,
        collision: &WorldCollision,
        height: &HeightGrid,
    ) -> usize {
        let start = self.player_pos();
        let path = collision.path_world(start, target, height);
        let unit = &mut self.units[self.player_index];
        unit.path = path;
        unit.path_index = 0;
        unit.target = Some(target);
        unit.attack_target = None;
        self.marker = Some((target, 1.1));
        unit.path.len()
    }

    pub fn set_attack_target(
        &mut self,
        index: usize,
        collision: &WorldCollision,
        height: &HeightGrid,
    ) -> bool {
        if index >= self.units.len() {
            return false;
        }
        let target = &self.units[index];
        let targetable = target.team == 1 || (target.team == 2 && target.kind == UnitKind::Camp);
        if !targetable || target.health <= 0.0 || !target.visible {
            return false;
        }
        if target.kind == UnitKind::Nexus && !self.nexus_vulnerable(target.team) {
            return false;
        }
        let target_pos = target.pos;
        let player = &mut self.units[self.player_index];
        player.attack_target = Some(index);
        player.target = Some(target_pos);
        let distance = player.pos.distance(target_pos);
        if distance > player.attack_range {
            player.path = collision.path_world(player.pos, target_pos, height);
            player.path_index = 0;
        } else {
            player.path.clear();
            player.path_index = 0;
        }
        self.marker = Some((target_pos, 1.1));
        true
    }

    pub fn steer_player(
        &mut self,
        direction: Vec3,
        dt: f32,
        height: &HeightGrid,
        collision: &WorldCollision,
    ) {
        let speed_mult = self.team_speed_mult(self.units[self.player_index].team);
        let unit = &mut self.units[self.player_index];
        unit.path.clear();
        unit.path_index = 0;
        unit.target = None;
        unit.attack_target = None;
        let flat = Vec3::new(direction.x, 0.0, direction.z);
        if flat.length_squared() < 1e-6 {
            unit.moving = false;
            return;
        }
        let dir = flat.normalize();
        let next = collision.slide_move(unit.pos, dir * unit.speed * speed_mult * dt, height);
        let moved = next.distance(unit.pos) > 0.01;
        unit.pos = next;
        unit.yaw = dir.x.atan2(dir.z);
        unit.moving = moved;
    }

    pub fn cast_q(&mut self, target: Vec3) -> bool {
        let pi = self.player_index;
        let player = &mut self.units[pi];
        if player.health <= 0.0 || player.cooldowns[0] > 0.0 || player.mana < 25.0 {
            return false;
        }
        let flat = Vec3::new(target.x - player.pos.x, 0.0, target.z - player.pos.z);
        if flat.length_squared() < 1.0 {
            return false;
        }
        let dir = flat.normalize();
        let pos = player.pos + Vec3::Y * (player.scale * 80.0) + dir * 40.0;
        let team = player.team;
        player.mana -= 25.0;
        player.cooldowns[0] = Q_COOLDOWN;
        let damage = player.q_damage * (1.0 + player.ability_power / 100.0);
        self.projectiles.push(Projectile {
            pos,
            dir,
            speed: 950.0,
            life: 1.4,
            damage,
            team,
        });
        self.effects.push(Effect {
            pos,
            radius: 110.0,
            remaining: 0.22,
            duration: 0.22,
            color: [255, 120, 40],
        });
        true
    }

    pub fn cast_w(
        &mut self,
        target: Vec3,
        collision: &WorldCollision,
        height: &HeightGrid,
    ) -> bool {
        let pi = self.player_index;
        let player = &mut self.units[pi];
        if player.health <= 0.0 || player.cooldowns[1] > 0.0 || player.mana < 30.0 {
            return false;
        }
        let flat = Vec3::new(target.x - player.pos.x, 0.0, target.z - player.pos.z);
        if flat.length_squared() < 1.0 {
            return false;
        }
        let dir = flat.normalize();
        let start = player.pos;
        let mut destination = start;
        let steps = (player.w_distance / 64.0).ceil().max(1.0) as i32;
        for step in (1..=steps).rev() {
            let p = start + dir * (step as f32 * 64.0);
            if collision.is_walkable_world(p.x, p.z) {
                destination = Vec3::new(p.x, height.height_at(p.x, p.z), p.z);
                break;
            }
        }
        if destination.distance(start) < 8.0 {
            return false;
        }
        player.pos = destination;
        player.mana -= 30.0;
        player.cooldowns[1] = 8.0;
        player.path.clear();
        player.path_index = 0;
        self.effects.push(Effect {
            pos: start,
            radius: 140.0,
            remaining: 0.35,
            duration: 0.35,
            color: [90, 220, 255],
        });
        true
    }

    pub fn cast_e(&mut self) -> bool {
        let pi = self.player_index;
        let player = &mut self.units[pi];
        if player.health <= 0.0 || player.cooldowns[2] > 0.0 || player.mana < 40.0 {
            return false;
        }
        player.mana -= 40.0;
        player.cooldowns[2] = 12.0;
        let ability_mult = 1.0 + player.ability_power / 100.0;
        player.health = (player.health + player.e_heal * ability_mult).min(player.max_health);
        player.shield = player.e_shield * ability_mult;
        player.shield_timer = 4.0;
        let pos = player.pos;
        self.effects.push(Effect {
            pos,
            radius: 160.0,
            remaining: 0.5,
            duration: 0.5,
            color: [90, 255, 140],
        });
        true
    }

    pub fn cast_r(&mut self, target: Vec3) -> bool {
        let pi = self.player_index;
        let player = &mut self.units[pi];
        if player.health <= 0.0 || player.cooldowns[3] > 0.0 || player.mana < 80.0 {
            return false;
        }
        player.mana -= 80.0;
        player.cooldowns[3] = 45.0;
        self.effects.push(Effect {
            pos: target,
            radius: 384.0,
            remaining: 0.6,
            duration: 0.6,
            color: [255, 110, 50],
        });
        let radius = 384.0;
        let damage = player.r_damage * (1.0 + player.ability_power / 100.0);
        for i in 0..self.units.len() {
            let unit = &self.units[i];
            if unit.health <= 0.0 || unit.team == 0 || unit.team == 2 {
                continue;
            }
            if unit.pos.distance(target) <= radius + unit.hit_radius {
                self.damage_unit(i, damage, 0);
            }
        }
        true
    }

    pub fn pick_unit(&self, origin: Vec3, dir: Vec3) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, unit) in self.units.iter().enumerate() {
            if !(unit.team == 1 || (unit.team == 2 && unit.kind == UnitKind::Camp))
                || unit.health <= 0.0
                || !unit.visible
            {
                continue;
            }
            if unit.kind == UnitKind::Nexus && !self.nexus_vulnerable(unit.team) {
                continue;
            }
            let center = unit.pos + Vec3::Y * (unit.bar_height * 0.45);
            let radius = unit.hit_radius;
            let oc = center - origin;
            let t = oc.dot(dir);
            if t < 0.0 {
                continue;
            }
            let closest = origin + dir * t;
            if closest.distance(center) <= radius && best.is_none_or(|(_, best_t)| t < best_t) {
                best = Some((i, t));
            }
        }
        best.map(|(i, _)| i)
    }

    pub fn nearest_enemy(&self, pos: Vec3) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, unit) in self.units.iter().enumerate() {
            if unit.team != 1 || unit.health <= 0.0 {
                continue;
            }
            let d = unit.pos.distance_squared(pos);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    pub fn nearest_camp(&self, pos: Vec3) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, unit) in self.units.iter().enumerate() {
            if unit.kind != UnitKind::Camp || unit.health <= 0.0 {
                continue;
            }
            let d = unit.pos.distance_squared(pos);
            if best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    pub fn teleport_player(&mut self, pos: Vec3) {
        let unit = &mut self.units[self.player_index];
        unit.pos = pos;
        unit.path.clear();
        unit.path_index = 0;
        unit.moving = false;
    }

    pub fn unit_pos(&self, index: usize) -> Option<Vec3> {
        self.units.get(index).map(|u| u.pos)
    }

    pub fn unit_health(&self, index: usize) -> Option<f32> {
        self.units.get(index).map(|u| u.health)
    }

    pub fn update(
        &mut self,
        dt: f32,
        height: &HeightGrid,
        collision: &WorldCollision,
        camera_right: Vec3,
        camera_up: Vec3,
        camera_forward: Vec3,
    ) {
        let pi = self.player_index;
        let player_spawn = self.player_spawn;
        {
            let player = &mut self.units[pi];
            for cd in &mut player.cooldowns {
                *cd = (*cd - dt).max(0.0);
            }
            player.mana = (player.mana + 8.0 * dt).min(player.max_mana);
            if player.health > 0.0 && player.pos.distance(player_spawn) <= 900.0 {
                player.health =
                    (player.health + player.max_health * 0.06 * dt).min(player.max_health);
                player.mana = (player.mana + player.max_mana * 0.06 * dt).min(player.max_mana);
            }
            if player.shield_timer > 0.0 {
                player.shield_timer -= dt;
                if player.shield_timer <= 0.0 {
                    player.shield = 0.0;
                }
            }
        }
        self.gold_income += 8.0 * dt;
        if self.gold_income >= 1.0 {
            let amount = self.gold_income.floor() as u32;
            self.gold_income -= amount as f32;
            let player = &mut self.units[pi];
            if player.health > 0.0 {
                player.gold = player.gold.saturating_add(amount);
            }
        }
        for effect in &mut self.effects {
            effect.remaining -= dt;
        }
        self.effects.retain(|effect| effect.remaining > 0.0);
        if let Some((_, ref mut remaining)) = self.marker {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.marker = None;
            }
        }

        if self.match_over.is_none() {
            self.wave_timer -= dt;
            if self.wave_timer <= 0.0 {
                for lane_index in 0..self.lanes.len() {
                    self.spawn_wave(0, lane_index);
                    self.spawn_wave(1, lane_index);
                }
                self.wave_timer = self.wave_interval;
            }
        }

        for buff in &mut self.team_buffs {
            if let Some(b) = buff {
                b.remaining -= dt;
                if b.remaining <= 0.0 {
                    *buff = None;
                }
            }
        }

        for unit in &mut self.units {
            unit.hit_flash = (unit.hit_flash - dt).max(0.0);
            unit.attack_cooldown = (unit.attack_cooldown - dt).max(0.0);
            unit.anim_time_ms += dt * 1000.0;
            if unit.health <= 0.0
                && matches!(
                    unit.kind,
                    UnitKind::Dummy | UnitKind::Camp | UnitKind::Inhibitor
                )
                && unit.dead_timer > 0.0
            {
                unit.dead_timer -= dt;
                if unit.dead_timer <= 0.0 {
                    unit.health = unit.max_health;
                    unit.dead_timer = 0.0;
                    unit.attack_target = None;
                    unit.path.clear();
                    unit.path_index = 0;
                }
            }
        }

        self.update_player(dt, height, collision);

        let unit_count = self.units.len();
        for i in 0..unit_count {
            let kind = self.units[i].kind;
            if self.units[i].health <= 0.0 {
                continue;
            }
            if kind.is_minion() {
                self.update_minion(i, dt, height);
            } else if kind == UnitKind::Tower {
                self.update_tower(i);
            }
        }

        self.update_projectiles(dt, height);
        self.update_visibility();
        self.rebuild_shadows(height);
        self.rebuild_mesh(camera_right, camera_up, camera_forward);
    }

    fn update_player(&mut self, dt: f32, height: &HeightGrid, collision: &WorldCollision) {
        let pi = self.player_index;
        let speed_mult = self.team_speed_mult(0);
        if self.units[pi].health <= 0.0 {
            self.units[pi].dead_timer -= dt;
            if self.units[pi].dead_timer <= 0.0 {
                let spawn = self.player_spawn;
                let unit = &mut self.units[pi];
                unit.pos = spawn;
                unit.health = unit.max_health;
                unit.dead_timer = 0.0;
                unit.attack_target = None;
                unit.path.clear();
                unit.path_index = 0;
                unit.moving = false;
            }
            return;
        }

        let target_index = self.units[pi].attack_target;
        if let Some(ti) = target_index {
            let target_alive = self.units.get(ti).is_some_and(|u| {
                u.health > 0.0 && (u.team == 1 || (u.team == 2 && u.kind == UnitKind::Camp))
            });
            if target_alive {
                let target_pos = self.units[ti].pos;
                let player_pos = self.units[pi].pos;
                let distance = player_pos.distance(target_pos);
                if distance > self.units[pi].attack_range {
                    let unit = &mut self.units[pi];
                    if unit.path.is_empty()
                        || unit
                            .path
                            .last()
                            .is_none_or(|p| p.distance(target_pos) > 48.0)
                    {
                        unit.path = collision.path_world(player_pos, target_pos, height);
                        unit.path_index = 0;
                    }
                    follow_path(unit, dt, height, speed_mult);
                } else {
                    let unit = &mut self.units[pi];
                    unit.path.clear();
                    unit.path_index = 0;
                    unit.moving = false;
                    let dir = target_pos - player_pos;
                    if dir.length_squared() > 1e-6 {
                        unit.yaw = dir.x.atan2(dir.z);
                    }
                    if unit.attack_cooldown <= 0.0 {
                        unit.attack_cooldown = unit.attack_interval;
                        let damage = unit.attack_damage;
                        let ranged = unit.ranged_auto;
                        let from = unit.pos + Vec3::Y * (unit.scale * 80.0);
                        let team = unit.team;
                        if ranged {
                            let dir = (target_pos - from).normalize_or_zero();
                            self.projectiles.push(Projectile {
                                pos: from,
                                dir,
                                speed: 900.0,
                                life: 1.4,
                                damage,
                                team,
                            });
                        } else {
                            self.damage_unit(ti, damage, 0);
                            if self.units[ti].health <= 0.0 {
                                self.units[pi].attack_target = None;
                            }
                        }
                    }
                }
                return;
            } else {
                self.units[pi].attack_target = None;
            }
        }

        let unit = &mut self.units[pi];
        unit.moving = false;
        if !unit.path.is_empty() {
            follow_path(unit, dt, height, speed_mult);
        }
    }

    fn update_minion(&mut self, index: usize, dt: f32, height: &HeightGrid) {
        let team = self.units[index].team;
        let speed_mult = self.team_speed_mult(team);
        let pos = self.units[index].pos;
        let target = self.find_target(index, team, MINION_AGGRO);

        if let Some(target) = target {
            let target_pos = self.units[target].pos;
            let distance = pos.distance(target_pos);
            if distance <= self.units[index].attack_range {
                let unit = &mut self.units[index];
                unit.moving = false;
                unit.attack_target = Some(target);
                let dir = target_pos - unit.pos;
                if dir.length_squared() > 1e-6 {
                    unit.yaw = dir.x.atan2(dir.z);
                }
                if unit.attack_cooldown <= 0.0 {
                    unit.attack_cooldown = unit.attack_interval;
                    let damage = unit.attack_damage;
                    if unit.kind == UnitKind::RangedMinion {
                        let dir = (target_pos - unit.pos).normalize_or_zero();
                        self.projectiles.push(Projectile {
                            pos: unit.pos + Vec3::Y * 80.0 + dir * 20.0,
                            dir,
                            speed: 650.0,
                            life: 1.4,
                            damage,
                            team: unit.team,
                        });
                    } else {
                        self.damage_unit(target, damage, team);
                    }
                }
            } else {
                self.units[index].attack_target = Some(target);
                self.move_unit_toward(index, target_pos, dt, height, speed_mult);
            }
        } else {
            self.units[index].attack_target = None;
            self.follow_lane(index, dt, height, speed_mult);
        }
    }

    fn update_tower(&mut self, index: usize) {
        let team = self.units[index].team;
        let pos = self.units[index].pos;
        let range = self.units[index].attack_range;
        let target = self.find_target(index, team, range);
        if let Some(target) = target {
            if self.units[index].attack_cooldown <= 0.0 {
                self.units[index].attack_cooldown = self.units[index].attack_interval;
                let target_pos = self.units[target].pos;
                let dir = (target_pos - pos).normalize_or_zero();
                let damage = self.units[index].attack_damage;
                self.projectiles.push(Projectile {
                    pos: pos + Vec3::Y * 380.0,
                    dir,
                    speed: 700.0,
                    life: 1.6,
                    damage,
                    team,
                });
            }
        }
    }

    fn find_target(&self, index: usize, team: u8, range: f32) -> Option<usize> {
        let pos = self.units[index].pos;
        let mut best: Option<(usize, f32)> = None;
        for (i, unit) in self.units.iter().enumerate() {
            if i == index || unit.team == team || unit.team == 2 || unit.health <= 0.0 {
                continue;
            }
            if unit.kind == UnitKind::Nexus && !self.nexus_vulnerable(unit.team) {
                continue;
            }
            let d = pos.distance_squared(unit.pos);
            if d <= range * range && best.is_none_or(|(_, bd)| d < bd) {
                best = Some((i, d));
            }
        }
        best.map(|(i, _)| i)
    }

    fn move_unit_toward(
        &mut self,
        index: usize,
        target: Vec3,
        dt: f32,
        height: &HeightGrid,
        speed_mult: f32,
    ) {
        let unit = &mut self.units[index];
        let delta = target - unit.pos;
        let distance = delta.length();
        if distance < 4.0 {
            unit.moving = false;
            return;
        }
        let dir = delta / distance;
        let step = (unit.speed * speed_mult * dt).min(distance);
        unit.pos += dir * step;
        unit.pos.y = height.height_at(unit.pos.x, unit.pos.z);
        unit.yaw = dir.x.atan2(dir.z);
        unit.moving = true;
    }

    fn follow_lane(&mut self, index: usize, dt: f32, height: &HeightGrid, speed_mult: f32) {
        let unit = &mut self.units[index];
        if unit.lane.is_empty() {
            unit.moving = false;
            return;
        }
        if unit.lane_index >= unit.lane.len() {
            let target = unit.lane.last().copied().unwrap_or(unit.pos);
            let dir = target - unit.pos;
            if dir.length_squared() > 1e-6 {
                unit.yaw = dir.x.atan2(dir.z);
            }
            unit.moving = false;
            return;
        }
        let waypoint = unit.lane[unit.lane_index];
        let delta = waypoint - unit.pos;
        let distance = delta.length();
        if distance < 10.0 {
            unit.lane_index += 1;
            return;
        }
        let dir = delta / distance;
        let step = (unit.speed * speed_mult * dt).min(distance);
        unit.pos += dir * step;
        unit.pos.y = height.height_at(unit.pos.x, unit.pos.z);
        unit.yaw = dir.x.atan2(dir.z);
        unit.moving = true;
    }

    fn damage_unit(&mut self, index: usize, amount: f32, attacker_team: u8) {
        if index >= self.units.len() {
            return;
        }
        if self.units[index].kind == UnitKind::Nexus
            && !self.nexus_vulnerable(self.units[index].team)
        {
            return;
        }
        let amount = amount * self.team_damage_mult(attacker_team);
        let (target_kind, camp_buff, respawn_time, killed) = {
            let unit = &mut self.units[index];
            if unit.health <= 0.0 {
                return;
            }
            let mut amount = amount;
            if unit.kind == UnitKind::Player && unit.shield > 0.0 {
                let absorbed = unit.shield.min(amount);
                unit.shield -= absorbed;
                amount -= absorbed;
            }
            unit.health = (unit.health - amount).max(0.0);
            unit.hit_flash = 0.2;
            (
                unit.kind,
                unit.camp_buff,
                unit.respawn_time,
                unit.health <= 0.0,
            )
        };
        if !killed {
            return;
        }
        if attacker_team == 0 {
            self.grant_reward(target_kind);
        }
        match target_kind {
            UnitKind::Nexus => {
                self.match_over = Some(attacker_team);
            }
            UnitKind::Player => {
                let unit = &mut self.units[index];
                unit.dead_timer = PLAYER_RESPAWN;
                unit.attack_target = None;
                unit.path.clear();
                unit.path_index = 0;
            }
            UnitKind::Tower => {
                self.units[index].dead_timer = 0.0;
            }
            UnitKind::Inhibitor => {
                self.units[index].dead_timer = 120.0;
            }
            UnitKind::Dummy => {
                self.units[index].dead_timer = 4.0;
            }
            UnitKind::Camp => {
                self.units[index].dead_timer = respawn_time;
                if let Some(kind) = camp_buff {
                    let (damage_mult, speed_mult) = match kind {
                        CampBuff::Might => (1.25, 1.0),
                        CampBuff::Swift => (1.0, 1.2),
                    };
                    if attacker_team < 2 {
                        self.team_buffs[attacker_team as usize] = Some(TeamBuff {
                            remaining: 60.0,
                            duration: 60.0,
                            damage_mult,
                            speed_mult,
                            kind,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    fn grant_reward(&mut self, kind: UnitKind) {
        let (xp, gold) = match kind {
            UnitKind::MeleeMinion | UnitKind::RangedMinion => (20.0, 10),
            UnitKind::Tower => (100.0, 100),
            UnitKind::Inhibitor => (150.0, 150),
            UnitKind::Camp => (80.0, 50),
            UnitKind::Dummy => (50.0, 20),
            _ => (0.0, 0),
        };
        if xp <= 0.0 {
            return;
        }
        let pi = self.player_index;
        let player = &mut self.units[pi];
        player.gold = player.gold.saturating_add(gold);
        player.xp += xp;
        while player.level < 11 && player.xp >= xp_for_level(player.level) {
            player.xp -= xp_for_level(player.level);
            player.level += 1;
            player.max_health += 40.0;
            player.health = (player.health + 40.0).min(player.max_health);
            player.max_mana += 20.0;
            player.mana = (player.mana + 20.0).min(player.max_mana);
            player.attack_damage += 2.0;
        }
    }

    fn update_visibility(&mut self) {
        self.visibility.tiles.fill(false);
        for unit in &mut self.units {
            unit.visible = unit.team == 0;
        }

        let sources = self
            .units
            .iter()
            .filter(|u| u.team == 0 && u.health > 0.0)
            .map(|u| {
                let radius: f32 = match u.kind {
                    UnitKind::Player => 1500.0,
                    UnitKind::MeleeMinion | UnitKind::RangedMinion => 700.0,
                    _ => 1000.0,
                };
                (u.pos, radius)
            })
            .collect::<Vec<_>>();

        let (ox, oy) = self.vision_origin;
        for (pos, radius) in &sources {
            let rt = (*radius / 128.0).ceil() as i32;
            let ctx = (pos.x / 128.0).floor() as i32 - ox;
            let cty = (pos.z / 128.0).floor() as i32 - oy;
            for dy in -rt..=rt {
                for dx in -rt..=rt {
                    if dx * dx + dy * dy > rt * rt {
                        continue;
                    }
                    let tx = ctx + dx;
                    let ty = cty + dy;
                    if tx >= 0
                        && ty >= 0
                        && (tx as usize) < self.visibility.width
                        && (ty as usize) < self.visibility.height
                    {
                        self.visibility.tiles[ty as usize * self.visibility.width + tx as usize] =
                            true;
                    }
                }
            }
        }

        let player_pos = self.player_pos();
        let brush = self.brush.clone();
        for unit in &mut self.units {
            if unit.team == 0 {
                continue;
            }
            let tx = (unit.pos.x / 128.0).floor() as i32 - ox;
            let ty = (unit.pos.z / 128.0).floor() as i32 - oy;
            if !self.visibility.is_visible(tx, ty) {
                continue;
            }
            if point_in_brush(&brush, unit.pos) && player_pos.distance(unit.pos) > 500.0 {
                continue;
            }
            unit.visible = true;
        }
    }

    pub fn visible_enemy_count(&self) -> usize {
        self.units
            .iter()
            .filter(|u| u.team != 0 && u.health > 0.0 && u.visible)
            .count()
    }

    fn team_damage_mult(&self, team: u8) -> f32 {
        self.team_buffs
            .get(team as usize)
            .and_then(|b| *b)
            .map(|b| b.damage_mult)
            .unwrap_or(1.0)
    }

    fn team_speed_mult(&self, team: u8) -> f32 {
        self.team_buffs
            .get(team as usize)
            .and_then(|b| *b)
            .map(|b| b.speed_mult)
            .unwrap_or(1.0)
    }

    pub fn team_buff(&self, team: u8) -> Option<(f32, f32, CampBuff)> {
        self.team_buffs
            .get(team as usize)
            .and_then(|b| *b)
            .map(|b| (b.remaining, b.duration, b.kind))
    }

    fn update_projectiles(&mut self, dt: f32, height: &HeightGrid) {
        let projectiles = std::mem::take(&mut self.projectiles);
        let mut survivors = Vec::with_capacity(projectiles.len());

        for mut projectile in projectiles {
            projectile.life -= dt;
            projectile.pos += projectile.dir * projectile.speed * dt;

            if projectile.life <= 0.0
                || projectile.pos.y <= height.height_at(projectile.pos.x, projectile.pos.z)
            {
                continue;
            }

            let mut hit = None;
            for (i, unit) in self.units.iter().enumerate() {
                if unit.team == projectile.team
                    || unit.health <= 0.0
                    || (unit.team == 2 && unit.kind != UnitKind::Camp)
                {
                    continue;
                }
                let radius = unit.hit_radius.max(70.0);
                let center = unit.pos + Vec3::Y * (unit.bar_height * 0.35);
                if projectile.pos.distance(center) <= radius {
                    hit = Some(i);
                    break;
                }
            }
            if let Some(index) = hit {
                self.damage_unit(index, projectile.damage, projectile.team);
                self.effects.push(Effect {
                    pos: projectile.pos,
                    radius: 180.0,
                    remaining: 0.7,
                    duration: 0.7,
                    color: [255, 120, 40],
                });
            } else {
                survivors.push(projectile);
            }
        }
        self.projectiles = survivors;
    }

    fn spawn_wave(&mut self, team: u8, lane_index: usize) {
        if self.match_over.is_some() {
            return;
        }
        if self.units.iter().filter(|u| u.is_minion_of(team)).count() >= MAX_MINIONS_PER_TEAM {
            return;
        }
        let Some(lane_waypoints) = self.lanes.get(lane_index).cloned() else {
            return;
        };

        let nexus = self.nexus_positions[team as usize];
        let lane = if team == 0 {
            lane_waypoints
        } else {
            let mut reversed = lane_waypoints;
            reversed.reverse();
            reversed
        };
        let enemy_team = 1 - team;
        let super_lane = self.units.iter().any(|u| {
            u.kind == UnitKind::Inhibitor
                && u.team == enemy_team
                && u.structure_lane == Some(lane_index)
                && u.health <= 0.0
        });
        let mut offsets = vec![
            (Vec3::new(70.0, 0.0, 0.0), false, false),
            (Vec3::new(-70.0, 0.0, 0.0), false, false),
            (Vec3::new(0.0, 0.0, 70.0), true, false),
        ];
        if super_lane {
            offsets.push((Vec3::new(0.0, 0.0, -70.0), false, true));
        }

        for (offset, ranged, super_minion) in offsets {
            let template = if ranged {
                &self.ranged_template
            } else {
                &self.melee_template
            };
            let health = if super_minion {
                template.health * 2.5
            } else {
                template.health
            };
            let damage = if super_minion {
                template.damage * 2.0
            } else {
                template.damage
            };
            let scale = if super_minion {
                template.scale * 1.6
            } else {
                template.scale
            };
            let speed = if super_minion {
                template.speed * 0.9
            } else {
                template.speed
            };
            let mut pos = nexus + offset;
            pos.y = self
                .units
                .iter()
                .find(|u| u.kind == UnitKind::Nexus && u.team == team)
                .map(|u| u.pos.y)
                .unwrap_or(0.0);
            let unit = AnimatedUnit {
                model: template.model.clone(),
                idle: template.idle.clone(),
                walk: Some(template.walk.clone()),
                pos,
                yaw: if team == 0 { 0.0 } else { std::f32::consts::PI },
                scale,
                anim_time_ms: 0.0,
                moving: false,
                kind: if ranged {
                    UnitKind::RangedMinion
                } else {
                    UnitKind::MeleeMinion
                },
                team,
                health,
                max_health: health,
                hit_flash: 0.0,
                dead_timer: 0.0,
                target: None,
                path: Vec::new(),
                path_index: 0,
                attack_target: None,
                attack_cooldown: 0.0,
                attack_range: template.range,
                attack_damage: damage,
                attack_interval: template.interval,
                speed,
                hit_radius: if super_minion { 120.0 } else { 90.0 },
                bar_height: if super_minion { 220.0 } else { 170.0 },
                lane: lane.clone(),
                lane_index: 1.min(lane.len()),
                respawn_time: 0.0,
                camp_buff: None,
                mana: 0.0,
                max_mana: 0.0,
                level: 1,
                xp: 0.0,
                gold: 0,
                shield: 0.0,
                shield_timer: 0.0,
                cooldowns: [0.0; 4],
                visible: true,
                ranged_auto: false,
                q_damage: 0.0,
                r_damage: 0.0,
                e_heal: 0.0,
                e_shield: 0.0,
                w_distance: 0.0,
                ability_power: 0.0,
                structure_lane: None,
                super_minion,
            };
            self.units.push(unit);
        }
    }

    fn rebuild_shadows(&mut self, height: &HeightGrid) {
        self.shadow_vertices.clear();
        self.shadow_indices.clear();
        for unit in &self.units {
            if unit.health <= 0.0 || !unit.visible {
                continue;
            }
            let size = (unit.hit_radius * 1.15).clamp(55.0, 480.0);
            let y = height.height_at(unit.pos.x, unit.pos.z) + 4.0;
            let base = self.shadow_vertices.len() as u32;
            let p = |x: f32, z: f32, uv: [f32; 2]| SceneVertex::blob(Vec3::new(x, y, z), 0.38, uv);
            self.shadow_vertices
                .push(p(unit.pos.x - size, unit.pos.z - size, [0.0, 0.0]));
            self.shadow_vertices
                .push(p(unit.pos.x + size, unit.pos.z - size, [1.0, 0.0]));
            self.shadow_vertices
                .push(p(unit.pos.x - size, unit.pos.z + size, [0.0, 1.0]));
            self.shadow_vertices
                .push(p(unit.pos.x + size, unit.pos.z + size, [1.0, 1.0]));
            self.shadow_indices.extend_from_slice(&[
                base,
                base + 1,
                base + 2,
                base + 2,
                base + 1,
                base + 3,
            ]);
        }
    }

    pub fn lights(&self) -> Vec<SceneLight> {
        let mut lights = Vec::new();
        lights.push(SceneLight {
            pos: self.player_spawn + Vec3::Y * 60.0,
            color: [0.25, 1.0, 0.45],
            radius: 800.0,
            intensity: 0.35,
        });
        for effect in &self.effects {
            let color = [
                effect.color[0] as f32 / 255.0,
                effect.color[1] as f32 / 255.0,
                effect.color[2] as f32 / 255.0,
            ];
            let fade = (effect.remaining / effect.duration.max(0.01)).clamp(0.0, 1.0);
            lights.push(SceneLight {
                pos: effect.pos + Vec3::Y * 40.0,
                color,
                radius: (effect.radius * 3.0).max(220.0),
                intensity: 1.2 * fade,
            });
        }
        for projectile in &self.projectiles {
            lights.push(SceneLight {
                pos: projectile.pos + Vec3::Y * 30.0,
                color: [1.0, 0.45, 0.12],
                radius: 520.0,
                intensity: 1.6,
            });
        }
        lights
    }

    fn rebuild_mesh(&mut self, camera_right: Vec3, camera_up: Vec3, camera_forward: Vec3) {
        self.vertices.clear();
        self.indices.clear();

        let units = std::mem::take(&mut self.units);
        for unit in &units {
            if unit.health > 0.0 && unit.visible {
                let clip = if unit.moving {
                    unit.walk.as_ref().unwrap_or(&unit.idle)
                } else {
                    &unit.idle
                };
                let positions = unit.model.animated_positions(clip, unit.anim_time_ms);
                append_unit(self, unit, &positions);
            }
        }
        self.units = units;

        let bars = self
            .units
            .iter()
            .filter(|unit| unit.health > 0.0 && (unit.team != 2 || unit.kind == UnitKind::Camp))
            .map(|unit| {
                let width = match unit.kind {
                    UnitKind::Nexus => 190.0,
                    UnitKind::Tower => 150.0,
                    UnitKind::Player => 150.0,
                    _ => 55.0,
                } * (unit.scale / 2.8).clamp(0.7, 1.5);
                (
                    unit.pos + Vec3::Y * unit.bar_height,
                    width,
                    (unit.health / unit.max_health).clamp(0.0, 1.0),
                    if unit.team == 0 {
                        [80u8, 150, 255]
                    } else if unit.team == 2 {
                        [220u8, 180, 70]
                    } else {
                        [220u8, 65, 60]
                    },
                )
            })
            .collect::<Vec<_>>();
        for (center, width, fill, color) in bars {
            append_bar(
                self,
                center,
                width,
                14.0,
                fill,
                color,
                camera_right,
                camera_up,
                camera_forward,
            );
        }

        let projectile_positions = self
            .projectiles
            .iter()
            .map(|projectile| projectile.pos)
            .collect::<Vec<_>>();
        for pos in projectile_positions {
            append_billboard(self, pos, 22.0, [255, 150, 40], camera_right, camera_up);
        }

        let player = &self.units[self.player_index];
        if player.health > 0.0 {
            append_ring(self, player.pos, player.pos.y + 3.0, 95.0, [90, 180, 255]);
        }

        let structure_rings = self
            .units
            .iter()
            .filter(|u| {
                u.visible
                    && matches!(u.kind, UnitKind::Tower | UnitKind::Inhibitor)
                    && u.health > 0.0
            })
            .map(|u| (u.pos, u.team))
            .collect::<Vec<_>>();
        for (pos, team) in structure_rings {
            append_ring(
                self,
                pos,
                pos.y + 4.0,
                140.0,
                if team == 0 {
                    [90, 160, 255]
                } else {
                    [255, 90, 80]
                },
            );
        }

        let camp_rings = self
            .units
            .iter()
            .filter(|u| u.visible && u.kind == UnitKind::Camp && u.health > 0.0)
            .map(|u| u.pos)
            .collect::<Vec<_>>();
        for pos in camp_rings {
            append_ring(self, pos, pos.y + 4.0, 120.0, [230, 180, 70]);
        }

        let effects = self
            .effects
            .iter()
            .map(|e| {
                let t = 1.0 - e.remaining / e.duration.max(0.01);
                (e.pos, e.radius * (0.35 + t * 0.65), e.color)
            })
            .collect::<Vec<_>>();
        for (pos, radius, color) in effects {
            append_ring(self, pos, pos.y + 6.0, radius, color);
        }

        let shield_pos = self.units[self.player_index].pos;
        let shield = self.units[self.player_index].shield;
        if shield > 0.0 {
            append_ring(self, shield_pos, shield_pos.y + 5.0, 120.0, [90, 255, 140]);
        }

        if let Some((marker, remaining)) = self.marker {
            let pulse = (remaining * 6.0).sin().abs() * 0.25 + 0.75;
            append_ring(self, marker, marker.y + 5.0, 55.0 * pulse, [120, 255, 120]);
        }
    }
}

impl AnimatedUnit {
    fn is_minion_of(&self, team: u8) -> bool {
        self.kind.is_minion() && self.team == team
    }
}

fn point_in_brush(brush: &[(f32, f32, f32, f32)], pos: Vec3) -> bool {
    brush.iter().any(|&(x0, z0, x1, z1)| {
        pos.x >= x0.min(x1) && pos.x <= x0.max(x1) && pos.z >= z0.min(z1) && pos.z <= z0.max(z1)
    })
}

fn xp_for_level(level: u32) -> f32 {
    100.0 + (level.saturating_sub(1) as f32) * 80.0
}

fn find_nexus(units: &[AnimatedUnit], team: u8) -> Option<Vec3> {
    units
        .iter()
        .find(|u| u.kind == UnitKind::Nexus && u.team == team)
        .map(|u| u.pos)
}

fn load_model(cache: &Cache, model_id: u32) -> Result<Model> {
    let group = cache.group(7, model_id)?;
    Ok(Model::load(model_id, &group.data)?)
}

#[allow(clippy::too_many_arguments)]
fn load_unit(
    cache: &Cache,
    model_id: u32,
    idle_sequence: Option<u32>,
    walk_sequence: Option<u32>,
    pos: Vec3,
    scale: f32,
    kind: UnitKind,
    team: u8,
    health: f32,
    attack_range: f32,
    attack_damage: f32,
    attack_interval: f32,
    speed: f32,
    hit_radius: f32,
    bar_height: f32,
    lane: Vec<Vec3>,
) -> Result<AnimatedUnit> {
    let model = load_model(cache, model_id)?;
    let idle = match idle_sequence {
        Some(seq) => AnimationClip::load(cache, seq)?,
        None => AnimationClip::default(),
    };
    let walk = match walk_sequence {
        Some(seq) => Some(AnimationClip::load(cache, seq)?),
        None => None,
    };
    Ok(AnimatedUnit {
        model,
        idle,
        walk,
        pos,
        yaw: 0.0,
        scale,
        anim_time_ms: 0.0,
        moving: false,
        kind,
        team,
        health,
        max_health: health,
        hit_flash: 0.0,
        dead_timer: 0.0,
        target: None,
        path: Vec::new(),
        path_index: 0,
        attack_target: None,
        attack_cooldown: 0.0,
        attack_range,
        attack_damage,
        attack_interval,
        speed,
        hit_radius,
        bar_height,
        lane,
        lane_index: 0,
        respawn_time: 0.0,
        camp_buff: None,
        mana: 0.0,
        max_mana: 0.0,
        level: 1,
        xp: 0.0,
        gold: 0,
        shield: 0.0,
        shield_timer: 0.0,
        cooldowns: [0.0; 4],
        visible: true,
        ranged_auto: false,
        q_damage: 0.0,
        r_damage: 0.0,
        e_heal: 0.0,
        e_shield: 0.0,
        w_distance: 0.0,
        ability_power: 0.0,
        structure_lane: None,
        super_minion: false,
    })
}

fn follow_path(unit: &mut AnimatedUnit, dt: f32, height: &HeightGrid, speed_mult: f32) {
    let mut budget = unit.speed * speed_mult * dt;
    unit.moving = false;
    while budget > 0.0 && unit.path_index < unit.path.len() {
        let waypoint = unit.path[unit.path_index];
        let delta = waypoint - unit.pos;
        let distance = delta.length();
        if distance <= budget.max(4.0) {
            unit.pos = waypoint;
            unit.path_index += 1;
            budget = (budget - distance).max(0.0);
            unit.moving = true;
            if unit.path_index >= unit.path.len() {
                unit.path.clear();
                unit.path_index = 0;
                unit.target = None;
                unit.moving = false;
                break;
            }
        } else {
            let dir = delta / distance;
            unit.pos += dir * budget;
            unit.pos.y = height.height_at(unit.pos.x, unit.pos.z);
            unit.yaw = dir.x.atan2(dir.z);
            unit.moving = true;
            budget = 0.0;
        }
    }
}

fn append_unit(scene: &mut AnimatedScene, unit: &AnimatedUnit, positions: &[[f32; 3]]) {
    let rot = Mat3::from_rotation_y(unit.yaw);

    for (face_idx, &[a, b, c]) in unit.model.faces.iter().enumerate() {
        let pa = model_vertex_animated(positions, a as usize, unit.pos, rot, unit.scale);
        let pb = model_vertex_animated(positions, b as usize, unit.pos, rot, unit.scale);
        let pc = model_vertex_animated(positions, c as usize, unit.pos, rot, unit.scale);

        let mut normal = (pb - pa).cross(pc - pa).normalize_or_zero();
        if normal.length_squared() < 1e-8 {
            normal = rot
                * Vec3::from_array(
                    unit.model
                        .face_normals
                        .get(face_idx)
                        .copied()
                        .unwrap_or([0.0, 1.0, 0.0]),
                );
        }

        let textured = unit
            .model
            .face_textures
            .as_ref()
            .and_then(|t| t.get(face_idx))
            .is_some_and(|&t| t >= 0);
        let mut rgb = if unit.kind == UnitKind::Tower {
            [125, 117, 102]
        } else if textured || unit.model.face_colors[face_idx] == 127 {
            [48, 95, 42]
        } else {
            hsl_to_rgb(unit.model.face_colors[face_idx], 1.0)
        };
        if matches!(unit.kind, UnitKind::Nexus | UnitKind::Inhibitor) && unit.team == 1 {
            rgb[0] = rgb[0].saturating_add(90);
            rgb[1] = (rgb[1] as f32 * 0.5) as u8;
            rgb[2] = (rgb[2] as f32 * 0.8) as u8;
        }
        if unit.hit_flash > 0.0 {
            rgb[0] = 255;
            rgb[1] = (rgb[1] as f32 * 0.55) as u8;
            rgb[2] = (rgb[2] as f32 * 0.55) as u8;
        }

        let mut va = SceneVertex::new(pa, normal, rgb);
        let mut vb = SceneVertex::new(pb, normal, rgb);
        let mut vc = SceneVertex::new(pc, normal, rgb);
        va.material = 3;
        vb.material = 3;
        vc.material = 3;
        let base = scene.vertices.len() as u32;
        scene.vertices.push(va);
        scene.vertices.push(vb);
        scene.vertices.push(vc);
        scene.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

#[allow(clippy::too_many_arguments)]
fn append_bar(
    scene: &mut AnimatedScene,
    center: Vec3,
    width: f32,
    height: f32,
    fill: f32,
    color: [u8; 3],
    right: Vec3,
    up: Vec3,
    forward: Vec3,
) {
    let bg = [18u8, 18, 20];
    append_quad(
        scene,
        center,
        right * (width * 0.5),
        up * (height * 0.5),
        bg,
    );
    let fill_center = center + forward * 1.5 + right * (-width * 0.5 + width * fill * 0.5);
    append_quad(
        scene,
        fill_center,
        right * (width * fill * 0.5),
        up * (height * 0.45),
        color,
    );
}

fn append_billboard(
    scene: &mut AnimatedScene,
    center: Vec3,
    size: f32,
    color: [u8; 3],
    right: Vec3,
    up: Vec3,
) {
    append_quad(
        scene,
        center,
        right * (size * 0.5),
        up * (size * 0.5),
        color,
    );
}

fn append_ring(scene: &mut AnimatedScene, center: Vec3, y: f32, radius: f32, color: [u8; 3]) {
    let segments = 32u32;
    let base = scene.vertices.len() as u32;
    for i in 0..segments {
        let a = i as f32 / segments as f32 * std::f32::consts::TAU;
        let b = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
        let inner = 0.78;
        let p0 = Vec3::new(center.x + a.cos() * radius, y, center.z + a.sin() * radius);
        let p1 = Vec3::new(center.x + b.cos() * radius, y, center.z + b.sin() * radius);
        let p2 = Vec3::new(
            center.x + a.cos() * radius * inner,
            y,
            center.z + a.sin() * radius * inner,
        );
        let p3 = Vec3::new(
            center.x + b.cos() * radius * inner,
            y,
            center.z + b.sin() * radius * inner,
        );
        scene.vertices.push(SceneVertex::unlit(p0, color));
        scene.vertices.push(SceneVertex::unlit(p1, color));
        scene.vertices.push(SceneVertex::unlit(p2, color));
        scene.vertices.push(SceneVertex::unlit(p3, color));
        let o = base + i * 4;
        scene
            .indices
            .extend_from_slice(&[o, o + 1, o + 2, o + 2, o + 1, o + 3]);
    }
}

fn append_quad(scene: &mut AnimatedScene, center: Vec3, right: Vec3, up: Vec3, color: [u8; 3]) {
    let base = scene.vertices.len() as u32;
    scene
        .vertices
        .push(SceneVertex::unlit(center - right - up, color));
    scene
        .vertices
        .push(SceneVertex::unlit(center + right - up, color));
    scene
        .vertices
        .push(SceneVertex::unlit(center - right + up, color));
    scene
        .vertices
        .push(SceneVertex::unlit(center + right + up, color));
    scene
        .indices
        .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
}
