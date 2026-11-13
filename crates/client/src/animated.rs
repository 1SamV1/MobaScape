use anyhow::Result;
use glam::{Mat3, Vec3};
use rift_cache::{AnimationClip, Cache, Model, model::hsl_to_rgb};

use crate::scene::{
    HeightGrid, NpcSpawn, SceneData, Vertex, WorldCollision, model_vertex_animated,
};

const PLAYER_SPEED: f32 = 420.0;
const ATTACK_RANGE: f32 = 330.0;
const ATTACK_INTERVAL: f32 = 1.15;
const ATTACK_DAMAGE: f32 = 12.0;
const Q_COOLDOWN: f32 = 5.0;
const Q_DAMAGE: f32 = 38.0;
const RESPAWN_TIME: f32 = 4.0;

pub struct AnimatedUnit {
    pub model: Model,
    pub idle: AnimationClip,
    pub walk: Option<AnimationClip>,
    pub pos: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub anim_time_ms: f32,
    pub moving: bool,
    pub is_player: bool,
    pub is_dummy: bool,
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
}

pub struct Projectile {
    pub pos: Vec3,
    pub dir: Vec3,
    pub speed: f32,
    pub life: f32,
    pub damage: f32,
    pub team: u8,
}

pub struct AnimatedScene {
    pub units: Vec<AnimatedUnit>,
    pub player_index: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub marker: Option<(Vec3, f32)>,
    pub projectiles: Vec<Projectile>,
    pub q_cooldown: f32,
}

impl AnimatedScene {
    pub fn new(cache: &Cache, scene: &SceneData) -> Result<Self> {
        let mut units = Vec::new();

        for spawn in &scene.npc_spawns {
            units.push(load_unit(cache, spawn, false, false, 2, 100.0)?);
        }

        let player_spawn = NpcSpawn {
            model_id: 7744,
            idle_sequence: 222,
            walk_sequence: Some(219),
            pos: scene.player_spawn,
            scale: 2.8,
        };
        let player_index = units.len();
        units.push(load_unit(cache, &player_spawn, true, false, 0, 500.0)?);

        // Three target dummies near the player. Snap each to a walkable tile.
        for (dx, dy) in [(4.0f32, 0.0f32), (-3.0, 3.0), (0.0, -4.0)] {
            let wx = scene.player_spawn.x + dx * 128.0;
            let wz = scene.player_spawn.z + dy * 128.0;
            if let Some((tx, ty)) = scene.collision.nearest_walkable_world(wx, wz) {
                let pos = scene.collision.world_of_tile(tx, ty, &scene.height);
                let spawn = NpcSpawn {
                    model_id: 7744,
                    idle_sequence: 222,
                    walk_sequence: Some(219),
                    pos,
                    scale: 2.4,
                };
                units.push(load_unit(cache, &spawn, false, true, 1, 120.0)?);
            }
        }

        Ok(Self {
            units,
            player_index,
            vertices: Vec::new(),
            indices: Vec::new(),
            marker: None,
            projectiles: Vec::new(),
            q_cooldown: 0.0,
        })
    }

    pub fn player_pos(&self) -> Vec3 {
        self.units[self.player_index].pos
    }

    pub fn player_health(&self) -> (f32, f32) {
        let p = &self.units[self.player_index];
        (p.health, p.max_health)
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
        if target.team != 1 || target.health <= 0.0 {
            return false;
        }
        let target_pos = target.pos;
        let player = &mut self.units[self.player_index];
        player.attack_target = Some(index);
        player.target = Some(target_pos);
        let distance = player.pos.distance(target_pos);
        if distance > ATTACK_RANGE {
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
        let next = collision.slide_move(unit.pos, dir * PLAYER_SPEED * dt, height);
        let moved = next.distance(unit.pos) > 0.01;
        unit.pos = next;
        unit.yaw = dir.x.atan2(dir.z);
        unit.moving = moved;
    }

    pub fn cast_q(&mut self, target: Vec3) -> bool {
        if self.q_cooldown > 0.0 {
            return false;
        }
        let player = &self.units[self.player_index];
        let flat = Vec3::new(target.x - player.pos.x, 0.0, target.z - player.pos.z);
        if flat.length_squared() < 1.0 {
            return false;
        }
        let dir = flat.normalize();
        let pos = player.pos + Vec3::Y * (player.scale * 80.0) + dir * 40.0;
        self.projectiles.push(Projectile {
            pos,
            dir,
            speed: 950.0,
            life: 1.4,
            damage: Q_DAMAGE,
            team: player.team,
        });
        self.q_cooldown = Q_COOLDOWN;
        true
    }

    pub fn pick_unit(&self, origin: Vec3, dir: Vec3) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for (i, unit) in self.units.iter().enumerate() {
            if unit.team != 1 || unit.health <= 0.0 {
                continue;
            }
            let center = unit.pos + Vec3::Y * (unit.scale * 95.0);
            let radius = unit.scale * 90.0;
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
        self.q_cooldown = (self.q_cooldown - dt).max(0.0);
        if let Some((_, ref mut remaining)) = self.marker {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.marker = None;
            }
        }

        for unit in &mut self.units {
            unit.hit_flash = (unit.hit_flash - dt).max(0.0);
            unit.attack_cooldown = (unit.attack_cooldown - dt).max(0.0);
            if unit.health <= 0.0 && !unit.is_player {
                unit.dead_timer -= dt;
                if unit.dead_timer <= 0.0 {
                    unit.health = unit.max_health;
                    unit.dead_timer = 0.0;
                    unit.attack_target = None;
                    unit.path.clear();
                    unit.path_index = 0;
                }
            }
            unit.anim_time_ms += dt * 1000.0;
        }

        self.update_player(dt, height, collision);
        self.update_projectiles(dt, height);

        self.vertices.clear();
        self.indices.clear();

        let units = std::mem::take(&mut self.units);
        for unit in &units {
            if unit.health > 0.0 {
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

        // Health bars for the player and target dummies.
        let bars = self
            .units
            .iter()
            .filter(|unit| unit.health > 0.0 && (unit.is_player || unit.is_dummy))
            .map(|unit| {
                (
                    unit.pos + Vec3::Y * (unit.scale * 120.0 + 40.0),
                    150.0 * (unit.scale / 2.8).clamp(0.8, 1.4),
                    (unit.health / unit.max_health).clamp(0.0, 1.0),
                    if unit.is_player {
                        [80u8, 150, 255]
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
                16.0,
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

        // Selection ring under the player.
        let player = &self.units[self.player_index];
        let base_y = player.pos.y + 3.0;
        let radius = 95.0 * player.scale / 2.8;
        let segments = 32u32;
        let ring_base = self.vertices.len() as u32;
        for i in 0..segments {
            let a = i as f32 / segments as f32 * std::f32::consts::TAU;
            let b = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
            let inner = 0.78;
            let color = [90u8, 180, 255];
            let p0 = Vec3::new(
                player.pos.x + a.cos() * radius,
                base_y,
                player.pos.z + a.sin() * radius,
            );
            let p1 = Vec3::new(
                player.pos.x + b.cos() * radius,
                base_y,
                player.pos.z + b.sin() * radius,
            );
            let p2 = Vec3::new(
                player.pos.x + a.cos() * radius * inner,
                base_y,
                player.pos.z + a.sin() * radius * inner,
            );
            let p3 = Vec3::new(
                player.pos.x + b.cos() * radius * inner,
                base_y,
                player.pos.z + b.sin() * radius * inner,
            );
            self.vertices.push(Vertex::new(p0, color));
            self.vertices.push(Vertex::new(p1, color));
            self.vertices.push(Vertex::new(p2, color));
            self.vertices.push(Vertex::new(p3, color));
            let o = ring_base + i * 4;
            self.indices
                .extend_from_slice(&[o, o + 1, o + 2, o + 2, o + 1, o + 3]);
        }

        if let Some((marker, remaining)) = self.marker {
            let pulse = (remaining * 6.0).sin().abs() * 0.25 + 0.75;
            let radius = 55.0 * pulse;
            let segments = 20u32;
            let base_y = marker.y + 5.0;
            let ring_base = self.vertices.len() as u32;
            for i in 0..segments {
                let a = i as f32 / segments as f32 * std::f32::consts::TAU;
                let b = (i + 1) as f32 / segments as f32 * std::f32::consts::TAU;
                let color = [120u8, 255, 120];
                let p0 = Vec3::new(
                    marker.x + a.cos() * radius,
                    base_y,
                    marker.z + a.sin() * radius,
                );
                let p1 = Vec3::new(
                    marker.x + b.cos() * radius,
                    base_y,
                    marker.z + b.sin() * radius,
                );
                let p2 = Vec3::new(
                    marker.x + a.cos() * radius * 0.6,
                    base_y,
                    marker.z + a.sin() * radius * 0.6,
                );
                let p3 = Vec3::new(
                    marker.x + b.cos() * radius * 0.6,
                    base_y,
                    marker.z + b.sin() * radius * 0.6,
                );
                self.vertices.push(Vertex::new(p0, color));
                self.vertices.push(Vertex::new(p1, color));
                self.vertices.push(Vertex::new(p2, color));
                self.vertices.push(Vertex::new(p3, color));
                let o = ring_base + i * 4;
                self.indices
                    .extend_from_slice(&[o, o + 1, o + 2, o + 2, o + 1, o + 3]);
            }
        }
    }

    fn update_player(&mut self, dt: f32, height: &HeightGrid, collision: &WorldCollision) {
        let pi = self.player_index;
        let target_index = self.units[pi].attack_target;

        if let Some(ti) = target_index {
            let target_alive = self
                .units
                .get(ti)
                .is_some_and(|u| u.health > 0.0 && u.team == 1);
            if target_alive {
                let target_pos = self.units[ti].pos;
                let player_pos = self.units[pi].pos;
                let distance = player_pos.distance(target_pos);
                if distance > ATTACK_RANGE {
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
                    follow_path(unit, dt, height);
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
                        unit.attack_cooldown = ATTACK_INTERVAL;
                        let target = &mut self.units[ti];
                        target.health = (target.health - ATTACK_DAMAGE).max(0.0);
                        target.hit_flash = 0.18;
                        if target.health <= 0.0 {
                            target.dead_timer = RESPAWN_TIME;
                        }
                        if self.units[ti].health <= 0.0 {
                            self.units[pi].attack_target = None;
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
            follow_path(unit, dt, height);
        }
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

            let mut hit = false;
            for unit in &mut self.units {
                if unit.team == projectile.team || unit.health <= 0.0 {
                    continue;
                }
                let radius = unit.scale * 90.0;
                if projectile
                    .pos
                    .distance(unit.pos + Vec3::Y * (unit.scale * 70.0))
                    <= radius
                {
                    unit.health = (unit.health - projectile.damage).max(0.0);
                    unit.hit_flash = 0.2;
                    if unit.health <= 0.0 {
                        unit.dead_timer = RESPAWN_TIME;
                    }
                    hit = true;
                    break;
                }
            }
            if !hit {
                survivors.push(projectile);
            }
        }
        self.projectiles = survivors;
    }
}

fn follow_path(unit: &mut AnimatedUnit, dt: f32, height: &HeightGrid) {
    let mut budget = PLAYER_SPEED * dt;
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

fn load_unit(
    cache: &Cache,
    spawn: &NpcSpawn,
    is_player: bool,
    is_dummy: bool,
    team: u8,
    health: f32,
) -> Result<AnimatedUnit> {
    let group = cache.group(7, spawn.model_id)?;
    let model = Model::load(spawn.model_id, &group.data)?;
    let idle = AnimationClip::load(cache, spawn.idle_sequence)?;
    let walk = match spawn.walk_sequence {
        Some(seq) => Some(AnimationClip::load(cache, seq)?),
        None => None,
    };
    Ok(AnimatedUnit {
        model,
        idle,
        walk,
        pos: spawn.pos,
        yaw: 0.0,
        scale: spawn.scale,
        anim_time_ms: 0.0,
        moving: false,
        is_player,
        is_dummy,
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
    })
}

fn append_unit(scene: &mut AnimatedScene, unit: &AnimatedUnit, positions: &[[f32; 3]]) {
    let rot = Mat3::from_rotation_y(unit.yaw);
    let light = Vec3::new(0.35, 0.85, 0.4).normalize();

    for (face_idx, &[a, b, c]) in unit.model.faces.iter().enumerate() {
        let pa = model_vertex_animated(positions, a as usize, unit.pos, rot, unit.scale);
        let pb = model_vertex_animated(positions, b as usize, unit.pos, rot, unit.scale);
        let pc = model_vertex_animated(positions, c as usize, unit.pos, rot, unit.scale);

        let normal = (pb - pa).cross(pc - pa).normalize_or_zero();
        let ndotl = normal.dot(light).clamp(0.0, 1.0);
        let brightness = 0.62 + 0.22 * (1.0 - ndotl);

        let textured = unit
            .model
            .face_textures
            .as_ref()
            .and_then(|t| t.get(face_idx))
            .is_some_and(|&t| t >= 0);
        let mut rgb = if textured || unit.model.face_colors[face_idx] == 127 {
            [
                (48.0 * brightness) as u8,
                (95.0 * brightness) as u8,
                (42.0 * brightness) as u8,
            ]
        } else {
            hsl_to_rgb(unit.model.face_colors[face_idx], brightness)
        };
        if unit.hit_flash > 0.0 {
            rgb[0] = 255;
            rgb[1] = (rgb[1] as f32 * 0.55) as u8;
            rgb[2] = (rgb[2] as f32 * 0.55) as u8;
        }

        let base = scene.vertices.len() as u32;
        scene.vertices.push(Vertex::new(pa, rgb));
        scene.vertices.push(Vertex::new(pb, rgb));
        scene.vertices.push(Vertex::new(pc, rgb));
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

fn append_quad(scene: &mut AnimatedScene, center: Vec3, right: Vec3, up: Vec3, color: [u8; 3]) {
    let base = scene.vertices.len() as u32;
    scene.vertices.push(Vertex::new(center - right - up, color));
    scene.vertices.push(Vertex::new(center + right - up, color));
    scene.vertices.push(Vertex::new(center - right + up, color));
    scene.vertices.push(Vertex::new(center + right + up, color));
    scene
        .indices
        .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
}
