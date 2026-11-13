use anyhow::Result;
use glam::{Mat3, Vec3};
use rift_cache::{AnimationClip, Cache, Model, model::hsl_to_rgb};

use crate::scene::{HeightGrid, NpcSpawn, SceneData, Vertex, model_vertex_animated};

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
    pub target: Option<Vec3>,
}

pub struct AnimatedScene {
    pub units: Vec<AnimatedUnit>,
    pub player_index: usize,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub marker: Option<(Vec3, f32)>,
}

impl AnimatedScene {
    pub fn new(cache: &Cache, scene: &SceneData) -> Result<Self> {
        let mut units = Vec::new();

        for spawn in &scene.npc_spawns {
            units.push(load_unit(cache, spawn, false)?);
        }

        let player_spawn = NpcSpawn {
            model_id: 7744,
            idle_sequence: 222,
            walk_sequence: Some(219),
            pos: scene.player_spawn,
            scale: 2.8,
        };
        let player_index = units.len();
        units.push(load_unit(cache, &player_spawn, true)?);

        Ok(Self {
            units,
            player_index,
            vertices: Vec::new(),
            indices: Vec::new(),
            marker: None,
        })
    }

    pub fn player_pos(&self) -> Vec3 {
        self.units[self.player_index].pos
    }

    pub fn set_player_target(&mut self, target: Vec3) {
        self.units[self.player_index].target = Some(target);
        self.marker = Some((target, 1.1));
    }

    pub fn update(&mut self, dt: f32, height: &HeightGrid) {
        if let Some((_, ref mut remaining)) = self.marker {
            *remaining -= dt;
            if *remaining <= 0.0 {
                self.marker = None;
            }
        }
        for unit in &mut self.units {
            if unit.is_player {
                if let Some(target) = unit.target {
                    let delta = target - unit.pos;
                    let distance = delta.length();
                    if distance > 6.0 {
                        let dir = delta / distance;
                        let speed = 420.0;
                        let step = (speed * dt).min(distance);
                        unit.pos += dir * step;
                        unit.pos.y = height.height_at(unit.pos.x, unit.pos.z);
                        unit.yaw = dir.x.atan2(dir.z);
                        unit.moving = true;
                    } else {
                        unit.target = None;
                        unit.moving = false;
                    }
                } else {
                    unit.moving = false;
                }
            }

            unit.anim_time_ms += dt * 1000.0;
        }

        self.vertices.clear();
        self.indices.clear();

        let units = std::mem::take(&mut self.units);
        for unit in &units {
            let clip = if unit.moving {
                unit.walk.as_ref().unwrap_or(&unit.idle)
            } else {
                &unit.idle
            };
            let positions = unit.model.animated_positions(clip, unit.anim_time_ms);
            append_unit(self, unit, &positions);
        }
        self.units = units;

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
    }
}

fn load_unit(cache: &Cache, spawn: &NpcSpawn, is_player: bool) -> Result<AnimatedUnit> {
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
        target: None,
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
        let rgb = if textured || unit.model.face_colors[face_idx] == 127 {
            [
                (48.0 * brightness) as u8,
                (95.0 * brightness) as u8,
                (42.0 * brightness) as u8,
            ]
        } else {
            hsl_to_rgb(unit.model.face_colors[face_idx], brightness)
        };

        let base = scene.vertices.len() as u32;
        scene.vertices.push(Vertex::new(pa, rgb));
        scene.vertices.push(Vertex::new(pb, rgb));
        scene.vertices.push(Vertex::new(pc, rgb));
        scene.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
}
