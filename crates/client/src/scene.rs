use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use glam::{Mat3, Vec2, Vec3};
use rift_cache::{
    Cache, Locations, Model, ObjectDefinition, OverlayDefinition, Terrain, UnderlayDefinition,
    model::hsl_to_rgb,
};
use rift_sim::{Grid, find_path, simplify_path};
use serde::Deserialize;

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl Vertex {
    pub fn new(position: Vec3, color: [u8; 3]) -> Self {
        Self {
            position: position.to_array(),
            color: [
                srgb_to_linear(color[0] as f32 / 255.0),
                srgb_to_linear(color[1] as f32 / 255.0),
                srgb_to_linear(color[2] as f32 / 255.0),
                1.0,
            ],
        }
    }
}

/// Convert an sRGB channel to linear light. The surface is an sRGB target, so
/// shader output must be linear or the palette is double-gamma corrected.
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SceneVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
    pub uv: [f32; 2],
    pub ao: f32,
    pub material: u32,
}

impl SceneVertex {
    pub fn new(position: Vec3, normal: Vec3, color: [u8; 3]) -> Self {
        Self {
            position: position.to_array(),
            normal: normal.normalize_or_zero().to_array(),
            color: [
                srgb_to_linear(color[0] as f32 / 255.0),
                srgb_to_linear(color[1] as f32 / 255.0),
                srgb_to_linear(color[2] as f32 / 255.0),
                1.0,
            ],
            uv: [0.0, 0.0],
            ao: 1.0,
            material: 0,
        }
    }

    pub fn unlit(position: Vec3, color: [u8; 3]) -> Self {
        let mut vertex = Self::new(position, Vec3::Y, color);
        vertex.material = 1;
        vertex
    }

    pub fn blob(position: Vec3, alpha: f32, uv: [f32; 2]) -> Self {
        Self {
            position: position.to_array(),
            normal: Vec3::Y.to_array(),
            color: [0.0, 0.0, 0.0, alpha],
            uv,
            ao: 1.0,
            material: 2,
        }
    }
}

pub struct MeshData {
    pub vertices: Vec<SceneVertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn push_triangle(&mut self, a: SceneVertex, b: SceneVertex, c: SceneVertex) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[a, b, c]);
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

/// Height grid covering the whole loaded block, used for camera picking and
/// keeping units on the ground.
#[derive(Clone)]
pub struct HeightGrid {
    origin_tile_x: i32,
    origin_tile_y: i32,
    size: usize,
    heights: Vec<f32>,
}

impl HeightGrid {
    pub fn height_at(&self, world_x: f32, world_z: f32) -> f32 {
        let gx = world_x / 128.0 - self.origin_tile_x as f32;
        let gy = world_z / 128.0 - self.origin_tile_y as f32;
        if gx < 0.0 || gy < 0.0 || gx >= (self.size - 1) as f32 || gy >= (self.size - 1) as f32 {
            return 0.0;
        }
        let x0 = gx.floor() as usize;
        let y0 = gy.floor() as usize;
        let fx = gx - x0 as f32;
        let fy = gy - y0 as f32;
        let h00 = self.heights[y0 * self.size + x0];
        let h10 = self.heights[y0 * self.size + x0 + 1];
        let h01 = self.heights[(y0 + 1) * self.size + x0];
        let h11 = self.heights[(y0 + 1) * self.size + x0 + 1];
        let a = h00 + (h10 - h00) * fx;
        let b = h01 + (h11 - h01) * fx;
        a + (b - a) * fy
    }

    pub fn contains(&self, world_x: f32, world_z: f32) -> bool {
        let gx = world_x / 128.0 - self.origin_tile_x as f32;
        let gy = world_z / 128.0 - self.origin_tile_y as f32;
        gx >= 0.0 && gy >= 0.0 && gx < (self.size - 1) as f32 && gy < (self.size - 1) as f32
    }
}

#[derive(Clone)]
pub struct WorldCollision {
    grid: Grid,
    origin_tile_x: i32,
    origin_tile_y: i32,
}

impl WorldCollision {
    pub fn new(grid: Grid, origin_tile_x: i32, origin_tile_y: i32) -> Self {
        Self {
            grid,
            origin_tile_x,
            origin_tile_y,
        }
    }

    pub fn width(&self) -> i32 {
        self.grid.width()
    }

    pub fn height(&self) -> i32 {
        self.grid.height()
    }

    pub fn origin_tile(&self) -> (i32, i32) {
        (self.origin_tile_x, self.origin_tile_y)
    }

    pub fn tile_of(&self, world_x: f32, world_z: f32) -> (i32, i32) {
        (
            (world_x / 128.0).floor() as i32 - self.origin_tile_x,
            (world_z / 128.0).floor() as i32 - self.origin_tile_y,
        )
    }

    pub fn world_of_tile(&self, tx: i32, ty: i32, height: &HeightGrid) -> Vec3 {
        let x = (self.origin_tile_x + tx) as f32 * 128.0 + 64.0;
        let z = (self.origin_tile_y + ty) as f32 * 128.0 + 64.0;
        Vec3::new(x, height.height_at(x, z), z)
    }

    pub fn is_walkable_world(&self, world_x: f32, world_z: f32) -> bool {
        let (tx, ty) = self.tile_of(world_x, world_z);
        self.grid.is_walkable(tx, ty)
    }

    pub fn nearest_walkable_world(&self, world_x: f32, world_z: f32) -> Option<(i32, i32)> {
        let (tx, ty) = self.tile_of(world_x, world_z);
        self.grid.nearest_walkable(tx, ty, 12)
    }

    pub fn path_world(&self, start: Vec3, goal: Vec3, height: &HeightGrid) -> Vec<Vec3> {
        let start_tile = self.tile_of(start.x, start.z);
        let goal_tile = self.tile_of(goal.x, goal.z);
        let Some(path) = find_path(&self.grid, start_tile, goal_tile) else {
            return Vec::new();
        };
        let simplified = simplify_path(&self.grid, &path);
        simplified
            .into_iter()
            .skip(1)
            .map(|(tx, ty)| self.world_of_tile(tx, ty, height))
            .collect()
    }

    /// Move with axis-separated collision sliding. Used by hold-to-steer.
    pub fn slide_move(&self, pos: Vec3, delta: Vec3, height: &HeightGrid) -> Vec3 {
        let mut next = pos + delta;
        if self.is_walkable_world(next.x, next.z) {
            next.y = height.height_at(next.x, next.z);
            return next;
        }
        next = pos + Vec3::new(delta.x, 0.0, 0.0);
        if self.is_walkable_world(next.x, next.z) {
            next.y = height.height_at(next.x, next.z);
            return next;
        }
        next = pos + Vec3::new(0.0, 0.0, delta.z);
        if self.is_walkable_world(next.x, next.z) {
            next.y = height.height_at(next.x, next.z);
            return next;
        }
        pos
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum StructureKind {
    Tower,
    Inhibitor,
    Nexus,
}

#[derive(Debug, Clone)]
pub struct StructureSpawn {
    pub team: u8,
    pub kind: StructureKind,
    pub lane_index: Option<usize>,
    pub tile: [f32; 2],
    pub model_id: u32,
    pub pos: Vec3,
    pub health: f32,
    pub range: f32,
    pub damage: f32,
    pub interval: f32,
    pub scale: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct StructureJson {
    team: u8,
    kind: StructureKind,
    tile: [f32; 2],
    model: u32,
    health: f32,
    range: f32,
    damage: f32,
    interval: f32,
    scale: f32,
}

#[derive(Debug, Clone, Deserialize)]
struct LaneJson {
    id: String,
    waypoints: Vec<[f32; 2]>,
    towers: Vec<StructureJson>,
    inhibitors: Vec<StructureJson>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Atmosphere {
    pub sun_dir: [f32; 3],
    pub sun_color: [f32; 3],
    pub ambient_sky: [f32; 3],
    pub ambient_ground: [f32; 3],
    pub fog_color: [f32; 3],
    pub fog_density: f32,
    pub fog_height: f32,
    pub sky_top: [f32; 3],
    pub sky_horizon: [f32; 3],
    pub ground_fog: f32,
}

impl Default for Atmosphere {
    fn default() -> Self {
        Self {
            sun_dir: [-0.45, 0.78, 0.35],
            sun_color: [0.95, 0.82, 0.62],
            ambient_sky: [0.35, 0.45, 0.60],
            ambient_ground: [0.20, 0.22, 0.16],
            fog_color: [0.52, 0.66, 0.78],
            fog_density: 0.000012,
            fog_height: 120.0,
            sky_top: [0.24, 0.42, 0.72],
            sky_horizon: [0.52, 0.66, 0.78],
            ground_fog: 0.04,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct MapJson {
    #[serde(default)]
    name: String,
    wave_interval: f32,
    nexus: Vec<StructureJson>,
    lanes: Vec<LaneJson>,
    #[serde(default)]
    camps: Vec<CampJson>,
    #[serde(default)]
    brush: Vec<[f32; 4]>,
    #[serde(default)]
    atmosphere: Atmosphere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CampBuff {
    Might,
    Swift,
}

#[derive(Debug, Clone, Deserialize)]
struct CampJson {
    id: String,
    tile: [f32; 2],
    model: u32,
    idle: Option<u32>,
    walk: Option<u32>,
    health: f32,
    respawn: f32,
    buff: String,
    scale: f32,
}

#[derive(Debug, Clone)]
pub struct CampSpawn {
    pub id: String,
    pub pos: Vec3,
    pub model_id: u32,
    pub idle_sequence: Option<u32>,
    pub walk_sequence: Option<u32>,
    pub health: f32,
    pub respawn: f32,
    pub buff: CampBuff,
    pub scale: f32,
}

#[derive(Debug, Clone)]
pub struct LaneSpawn {
    pub id: String,
    pub waypoints: Vec<Vec3>,
}

#[derive(Debug, Clone)]
pub struct MapLayout {
    pub name: String,
    pub lanes: Vec<LaneSpawn>,
    pub structures: Vec<StructureSpawn>,
    pub camps: Vec<CampSpawn>,
    pub brush: Vec<(f32, f32, f32, f32)>,
    pub wave_interval: f32,
    pub atmosphere: Atmosphere,
}

impl MapLayout {
    pub fn load(path: &Path, height: &HeightGrid) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("reading map {}", path.display()))?;
        let map: MapJson = serde_json::from_str(&data)?;
        if map.lanes.is_empty() {
            anyhow::bail!("map {} needs at least one lane", path.display());
        }

        let world_of = |tile: [f32; 2]| {
            let x = (tile[0] + 0.5) * 128.0;
            let z = (tile[1] + 0.5) * 128.0;
            Vec3::new(x, height.height_at(x, z), z)
        };
        let to_spawn = |s: &StructureJson| StructureSpawn {
            team: s.team,
            kind: s.kind,
            lane_index: None,
            tile: s.tile,
            model_id: s.model,
            pos: world_of(s.tile),
            health: s.health,
            range: s.range,
            damage: s.damage,
            interval: s.interval,
            scale: s.scale,
        };

        let mut lanes = Vec::with_capacity(map.lanes.len());
        let mut structures = Vec::new();
        for s in &map.nexus {
            structures.push(to_spawn(s));
        }
        for (lane_index, lane) in map.lanes.iter().enumerate() {
            if lane.waypoints.len() < 2 {
                anyhow::bail!("lane {} needs at least two waypoints", lane.id);
            }
            lanes.push(LaneSpawn {
                id: lane.id.clone(),
                waypoints: lane.waypoints.iter().map(|&tile| world_of(tile)).collect(),
            });
            for s in &lane.towers {
                let mut spawn = to_spawn(s);
                spawn.lane_index = Some(lane_index);
                structures.push(spawn);
            }
            for s in &lane.inhibitors {
                let mut spawn = to_spawn(s);
                spawn.lane_index = Some(lane_index);
                structures.push(spawn);
            }
        }

        let camps = map
            .camps
            .iter()
            .map(|camp| CampSpawn {
                id: camp.id.clone(),
                pos: world_of(camp.tile),
                model_id: camp.model,
                idle_sequence: camp.idle,
                walk_sequence: camp.walk,
                health: camp.health,
                respawn: camp.respawn,
                buff: if camp.buff.eq_ignore_ascii_case("swift") {
                    CampBuff::Swift
                } else {
                    CampBuff::Might
                },
                scale: camp.scale,
            })
            .collect();

        let brush = map
            .brush
            .iter()
            .map(|b| {
                (
                    (b[0] + 0.5) * 128.0,
                    (b[1] + 0.5) * 128.0,
                    (b[2] + 0.5) * 128.0,
                    (b[3] + 0.5) * 128.0,
                )
            })
            .collect();

        Ok(Self {
            name: map.name,
            lanes,
            structures,
            camps,
            brush,
            wave_interval: map.wave_interval,
            atmosphere: map.atmosphere,
        })
    }

    pub fn distance_to_lane(&self, x: f32, z: f32) -> f32 {
        let p = Vec2::new(x, z);
        let mut best = f32::MAX;
        for lane in &self.lanes {
            for pair in lane.waypoints.windows(2) {
                let a = Vec2::new(pair[0].x, pair[0].z);
                let b = Vec2::new(pair[1].x, pair[1].z);
                best = best.min(point_segment_distance(p, a, b));
            }
        }
        best
    }

    pub fn clear_collision(
        &self,
        grid: &mut Grid,
        origin_tile_x: i32,
        origin_tile_y: i32,
        radius_tiles: f32,
    ) {
        for lane in &self.lanes {
            for pair in lane.waypoints.windows(2) {
                let a = Vec2::new(pair[0].x, pair[0].z);
                let b = Vec2::new(pair[1].x, pair[1].z);
                let steps = ((a.distance(b) / 24.0).ceil() as i32).max(1);
                for i in 0..=steps {
                    let p = a.lerp(b, i as f32 / steps as f32);
                    let tx = (p.x / 128.0).floor() as i32 - origin_tile_x;
                    let ty = (p.y / 128.0).floor() as i32 - origin_tile_y;
                    let r = radius_tiles.ceil() as i32;
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if (dx * dx + dy * dy) as f32 <= radius_tiles * radius_tiles {
                                grid.set_blocked(tx + dx, ty + dy, false);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn block_structures(&self, grid: &mut Grid, origin_tile_x: i32, origin_tile_y: i32) {
        for structure in &self.structures {
            let tx = structure.tile[0].floor() as i32 - origin_tile_x;
            let ty = structure.tile[1].floor() as i32 - origin_tile_y;
            grid.block_rect(tx - 1, ty - 1, 3, 3);
        }
    }

    pub fn build_mesh(&self, mesh: &mut MeshData, height: &HeightGrid, width: f32) {
        for lane in &self.lanes {
            for pair in lane.waypoints.windows(2) {
                let a = Vec2::new(pair[0].x, pair[0].z);
                let b = Vec2::new(pair[1].x, pair[1].z);
                let len = a.distance(b);
                if len < 1.0 {
                    continue;
                }
                let steps = ((len / 64.0).ceil() as usize).max(1);
                let dir = (b - a) / len;
                let perp = Vec2::new(-dir.y, dir.x) * (width * 0.5);
                let color = [96, 91, 80];

                for i in 0..steps {
                    let t0 = i as f32 / steps as f32;
                    let t1 = (i + 1) as f32 / steps as f32;
                    let p0 = a.lerp(b, t0);
                    let p1 = a.lerp(b, t1);
                    let corners = [
                        Vec2::new(p0.x + perp.x, p0.y + perp.y),
                        Vec2::new(p1.x + perp.x, p1.y + perp.y),
                        Vec2::new(p0.x - perp.x, p0.y - perp.y),
                        Vec2::new(p1.x - perp.x, p1.y - perp.y),
                    ];
                    let ys = corners.map(|p| height.height_at(p.x, p.y) + 40.0);
                    let v0 = Vec3::new(corners[0].x, ys[0], corners[0].y);
                    let v1 = Vec3::new(corners[1].x, ys[1], corners[1].y);
                    let v2 = Vec3::new(corners[2].x, ys[2], corners[2].y);
                    let v3 = Vec3::new(corners[3].x, ys[3], corners[3].y);
                    mesh.push_triangle(
                        SceneVertex::new(v0, Vec3::Y, color),
                        SceneVertex::new(v1, Vec3::Y, color),
                        SceneVertex::new(v2, Vec3::Y, color),
                    );
                    mesh.push_triangle(
                        SceneVertex::new(v2, Vec3::Y, color),
                        SceneVertex::new(v1, Vec3::Y, color),
                        SceneVertex::new(v3, Vec3::Y, color),
                    );
                }
            }
        }
    }
}

fn point_segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    if len2 <= 1e-6 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

#[derive(Debug, Clone)]
pub struct NpcSpawn {
    pub model_id: u32,
    pub idle_sequence: u32,
    pub walk_sequence: Option<u32>,
    pub pos: Vec3,
    pub scale: f32,
}

pub struct SceneData {
    pub terrain: MeshData,
    pub models: MeshData,
    pub shadows: MeshData,
    pub placements: usize,
    pub height: HeightGrid,
    pub collision: WorldCollision,
    pub map: MapLayout,
    pub npc_spawns: Vec<NpcSpawn>,
    pub player_spawn: Vec3,
    pub atmosphere: Atmosphere,
}

pub fn build(
    cache: &Cache,
    center_x: u32,
    center_y: u32,
    radius: u32,
    map_path: &Path,
) -> Result<SceneData> {
    let keys = cache
        .keys()
        .context("cache has no keys.json; cannot resolve map groups")?;

    let min_x = center_x.saturating_sub(radius);
    let max_x = center_x + radius;
    let min_y = center_y.saturating_sub(radius);
    let max_y = center_y + radius;

    let mut terrains: HashMap<(u32, u32), Terrain> = HashMap::new();
    let mut all_locations = Vec::new();

    for rx in min_x..=max_x {
        for ry in min_y..=max_y {
            let Some(locations_group) = keys.find_locations_group(rx, ry) else {
                continue;
            };
            let terrain_group = locations_group.saturating_sub(1);
            let terrain = Terrain::load(rx, ry, &cache.group(5, terrain_group)?.data)?;
            let locations = Locations::load(rx, ry, &cache.group(5, locations_group)?.data)?;
            all_locations.push((rx, ry, locations));
            terrains.insert((rx, ry), terrain);
        }
    }

    let underlays = load_underlays(cache)?;
    let overlays = load_overlays(cache)?;
    let object_files = cache
        .archive_files_with_ids(2, 6)
        .context("loading object definitions")?
        .into_iter()
        .collect::<HashMap<u32, Vec<u8>>>();

    let origin_tile_x = min_x as i32 * 64;
    let origin_tile_y = min_y as i32 * 64;
    let size = ((max_x - min_x + 1) * 64 + 1) as usize;

    let mut heights = vec![0.0f32; size * size];
    let mut colors = vec![[60u8, 90, 50]; size * size];

    for gy in 0..size {
        for gx in 0..size {
            let mut sum = 0.0f32;
            let mut color = [0u32; 3];
            let mut count = 0.0f32;

            for dy in -1..=0 {
                for dx in -1..=0 {
                    let tx = origin_tile_x + gx as i32 + dx;
                    let ty = origin_tile_y + gy as i32 + dy;
                    let rx = (tx >> 6) as u32;
                    let ry = (ty >> 6) as u32;
                    let lx = (tx & 63) as usize;
                    let ly = (ty & 63) as usize;
                    let Some(terrain) = terrains.get(&(rx, ry)) else {
                        continue;
                    };
                    let tile = terrain.tile(0, lx, ly);
                    sum += tile.height as f32 * 8.0;
                    let c = tile_color(terrain, &underlays, &overlays, lx, ly);
                    color[0] += c[0] as u32;
                    color[1] += c[1] as u32;
                    color[2] += c[2] as u32;
                    count += 1.0;
                }
            }

            let idx = gy * size + gx;
            heights[idx] = if count > 0.0 { sum / count } else { 0.0 };
            colors[idx] = if count > 0.0 {
                [
                    (color[0] as f32 / count) as u8,
                    (color[1] as f32 / count) as u8,
                    (color[2] as f32 / count) as u8,
                ]
            } else {
                [60, 90, 50]
            };
        }
    }

    let mut terrain_mesh = MeshData::new();
    for gy in 0..size - 1 {
        for gx in 0..size - 1 {
            let idx = gy * size + gx;
            let x0 = (origin_tile_x + gx as i32) as f32 * 128.0;
            let z0 = (origin_tile_y + gy as i32) as f32 * 128.0;
            let x1 = x0 + 128.0;
            let z1 = z0 + 128.0;
            let p00 = Vec3::new(x0, heights[idx], z0);
            let p10 = Vec3::new(x1, heights[idx + 1], z0);
            let p01 = Vec3::new(x0, heights[idx + size], z1);
            let p11 = Vec3::new(x1, heights[idx + size + 1], z1);

            let mut c = [0u32; 3];
            for (color, weight) in [
                (colors[idx], 4u32),
                (colors[idx + 1], 3),
                (colors[idx + size], 3),
                (colors[idx + size + 1], 2),
            ] {
                c[0] += color[0] as u32 * weight;
                c[1] += color[1] as u32 * weight;
                c[2] += color[2] as u32 * weight;
            }
            let color = [(c[0] / 12) as u8, (c[1] / 12) as u8, (c[2] / 12) as u8];

            let normal_at = |vx: usize, vy: usize| {
                let i = vy * size + vx;
                let h = heights[i];
                let hl = if vx > 0 { heights[i - 1] } else { h };
                let hr = if vx + 1 < size { heights[i + 1] } else { h };
                let hd = if vy > 0 { heights[i - size] } else { h };
                let hu = if vy + 1 < size { heights[i + size] } else { h };
                Vec3::new(hl - hr, 2.0 * 128.0, hd - hu).normalize_or_zero()
            };
            let n00 = normal_at(gx, gy);
            let n10 = normal_at(gx + 1, gy);
            let n01 = normal_at(gx, gy + 1);
            let n11 = normal_at(gx + 1, gy + 1);

            terrain_mesh.push_triangle(
                SceneVertex::new(p00, n00, color),
                SceneVertex::new(p01, n01, color),
                SceneVertex::new(p10, n10, color),
            );
            terrain_mesh.push_triangle(
                SceneVertex::new(p10, n10, color),
                SceneVertex::new(p01, n01, color),
                SceneVertex::new(p11, n11, color),
            );
        }
    }

    let height = HeightGrid {
        origin_tile_x,
        origin_tile_y,
        size,
        heights,
    };

    // Collision: water and blocking scenery footprints. This is deliberately
    // conservative; the editor will replace it with authored collision later.
    let tiles_x = ((max_x - min_x + 1) * 64) as usize;
    let tiles_y = ((max_y - min_y + 1) * 64) as usize;
    let mut collision_grid = Grid::new(tiles_x, tiles_y);
    for ((rx, ry), terrain) in &terrains {
        for ty in 0..64 {
            for tx in 0..64 {
                let tile = terrain.tile(0, tx, ty);
                if tile.underlay_id == 0 {
                    let gx = (*rx as i32 * 64 + tx as i32) - origin_tile_x;
                    let gy = (*ry as i32 * 64 + ty as i32) - origin_tile_y;
                    collision_grid.set_blocked(gx, gy, true);
                }
            }
        }
    }

    let mut def_cache: HashMap<u32, ObjectDefinition> = HashMap::new();
    for (rx, ry, locations) in &all_locations {
        for loc in &locations.locations {
            if let std::collections::hash_map::Entry::Vacant(entry) = def_cache.entry(loc.id) {
                if let Some(data) = object_files.get(&loc.id) {
                    if let Ok(def) = ObjectDefinition::parse(loc.id, data) {
                        entry.insert(def);
                    }
                }
            }
            let Some(def) = def_cache.get(&loc.id) else {
                continue;
            };
            if !is_blocking_object(def) {
                continue;
            }
            let mut w = def.size_x.max(1) as i32;
            let mut h = def.size_y.max(1) as i32;
            if loc.rotation % 2 == 1 {
                std::mem::swap(&mut w, &mut h);
            }
            let gx = (*rx as i32 * 64 + loc.x as i32) - origin_tile_x;
            let gy = (*ry as i32 * 64 + loc.y as i32) - origin_tile_y;
            collision_grid.block_rect(gx, gy, w, h);
        }
    }
    let map = MapLayout::load(map_path, &height)?;
    map.clear_collision(&mut collision_grid, origin_tile_x, origin_tile_y, 4.5);
    map.block_structures(&mut collision_grid, origin_tile_x, origin_tile_y);
    let collision = WorldCollision::new(collision_grid, origin_tile_x, origin_tile_y);

    let center = Vec3::new(
        (center_x as f32 * 64.0 + 32.0) * 128.0,
        height.height_at(
            (center_x as f32 * 64.0 + 32.0) * 128.0,
            (center_y as f32 * 64.0 + 32.0) * 128.0,
        ),
        (center_y as f32 * 64.0 + 32.0) * 128.0,
    );

    // Sort scenery by distance to the player so the near field looks dense and
    // the 900-model budget is spent where it is visible.
    let mut candidates = Vec::new();
    for (rx, ry, locations) in &all_locations {
        for loc in &locations.locations {
            let wx = ((*rx as f32 * 64.0) + loc.x as f32 + 0.5) * 128.0;
            let wz = ((*ry as f32 * 64.0) + loc.y as f32 + 0.5) * 128.0;
            if map.distance_to_lane(wx, wz) < 5.0 * 128.0 {
                continue;
            }
            let dx = wx - center.x;
            let dz = wz - center.z;
            candidates.push((dx * dx + dz * dz, *rx, *ry, *loc));
        }
    }
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut model_mesh = MeshData::new();
    let mut shadow_mesh = MeshData::new();
    let mut model_cache: HashMap<u32, Model> = HashMap::new();
    let mut placements = 0usize;
    const MODEL_LIMIT: usize = 900;

    for (_, rx, ry, loc) in candidates {
        if placements >= MODEL_LIMIT {
            break;
        }
        let Some(terrain) = terrains.get(&(rx, ry)) else {
            continue;
        };
        let Some(data) = object_files.get(&loc.id) else {
            continue;
        };
        let Ok(def) = ObjectDefinition::parse(loc.id, data) else {
            continue;
        };
        if !is_scenery(&def) {
            continue;
        }
        let Some(model_id) = def.first_model() else {
            continue;
        };
        let model = if let Some(model) = model_cache.get(&model_id) {
            model
        } else {
            let group = cache.group(7, model_id)?;
            let model = Model::load(model_id, &group.data)?;
            model_cache.insert(model_id, model);
            model_cache.get(&model_id).unwrap()
        };

        let base = Vec3::new(
            ((rx as f32 * 64.0) + loc.x as f32 + 0.5) * 128.0,
            terrain.height(loc.z as usize, loc.x as usize, loc.y as usize) as f32 * 8.0,
            ((ry as f32 * 64.0) + loc.y as f32 + 0.5) * 128.0,
        );
        let angle = loc.rotation as f32 * std::f32::consts::FRAC_PI_2;
        append_model(&mut model_mesh, model, base, angle);
        let shadow_size = (def.size_x.max(def.size_y) as f32 * 128.0 * 0.55).clamp(55.0, 360.0);
        push_blob_quad(&mut shadow_mesh, base, shadow_size, 0.24);
        placements += 1;
    }

    map.build_mesh(&mut model_mesh, &height, 3.0 * 128.0);

    let spawn_hint = map
        .lanes
        .first()
        .and_then(|lane| lane.waypoints.get(1))
        .copied()
        .unwrap_or(center);
    let player_spawn = find_walkable_spawn(&height, &collision, spawn_hint, &terrains)?;
    let npc_spawns = build_npc_spawns(&height, &collision, center, player_spawn, &terrains);

    tracing::info!(
        "scene: {} regions, {} terrain verts, {} model verts, {} placements, {} npcs, map {:?} lanes {:?}, center {:?}",
        terrains.len(),
        terrain_mesh.vertices.len(),
        model_mesh.vertices.len(),
        placements,
        npc_spawns.len(),
        map.name,
        map.lanes
            .iter()
            .map(|lane| lane.id.as_str())
            .collect::<Vec<_>>(),
        center
    );

    // A distant ground plane hides the loaded-block edge and gives the sky a
    // horizon to meet. It sits below the terrain and fades into fog.
    let backdrop_y = -24.0;
    let backdrop = 220_000.0;
    let bx0 = center.x - backdrop;
    let bx1 = center.x + backdrop;
    let bz0 = center.z - backdrop;
    let bz1 = center.z + backdrop;
    let bc = [44, 62, 40];
    model_mesh.push_triangle(
        SceneVertex::new(Vec3::new(bx0, backdrop_y, bz0), Vec3::Y, bc),
        SceneVertex::new(Vec3::new(bx1, backdrop_y, bz0), Vec3::Y, bc),
        SceneVertex::new(Vec3::new(bx0, backdrop_y, bz1), Vec3::Y, bc),
    );
    model_mesh.push_triangle(
        SceneVertex::new(Vec3::new(bx1, backdrop_y, bz0), Vec3::Y, bc),
        SceneVertex::new(Vec3::new(bx1, backdrop_y, bz1), Vec3::Y, bc),
        SceneVertex::new(Vec3::new(bx0, backdrop_y, bz1), Vec3::Y, bc),
    );

    Ok(SceneData {
        terrain: terrain_mesh,
        models: model_mesh,
        shadows: shadow_mesh,
        placements,
        height,
        collision,
        atmosphere: map.atmosphere.clone(),
        map,
        npc_spawns,
        player_spawn,
    })
}

fn find_walkable_spawn(
    height: &HeightGrid,
    collision: &WorldCollision,
    center: Vec3,
    terrains: &HashMap<(u32, u32), Terrain>,
) -> Result<Vec3> {
    for radius in 4..40 {
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                let world_x = center.x + dx as f32 * 128.0;
                let world_z = center.z + dy as f32 * 128.0;
                if !height.contains(world_x, world_z) {
                    continue;
                }
                let tile_x = (world_x / 128.0).floor() as i32;
                let tile_y = (world_z / 128.0).floor() as i32;
                let rx = (tile_x >> 6) as u32;
                let ry = (tile_y >> 6) as u32;
                let lx = (tile_x & 63) as usize;
                let ly = (tile_y & 63) as usize;
                if !collision.is_walkable_world(world_x, world_z) {
                    continue;
                }
                if let Some(terrain) = terrains.get(&(rx, ry)) {
                    let tile = terrain.tile(0, lx, ly);
                    if tile.underlay_id > 0
                        && tile.underlay_id != 55
                        && tile.overlay_id == 0
                        && tile.height > 4
                    {
                        return Ok(Vec3::new(
                            world_x,
                            height.height_at(world_x, world_z),
                            world_z,
                        ));
                    }
                }
            }
        }
    }
    Ok(Vec3::new(
        center.x,
        height.height_at(center.x, center.z),
        center.z,
    ))
}

fn build_npc_spawns(
    height: &HeightGrid,
    collision: &WorldCollision,
    center: Vec3,
    player_spawn: Vec3,
    terrains: &HashMap<(u32, u32), Terrain>,
) -> Vec<NpcSpawn> {
    let mut spots = Vec::new();
    for radius in 8..80 {
        for angle_step in 0..32 {
            let angle = angle_step as f32 * std::f32::consts::TAU / 32.0;
            let world_x = center.x + angle.cos() * radius as f32 * 128.0;
            let world_z = center.z + angle.sin() * radius as f32 * 128.0;
            if !height.contains(world_x, world_z) {
                continue;
            }
            if Vec3::new(world_x, 0.0, world_z).distance(Vec3::new(
                player_spawn.x,
                0.0,
                player_spawn.z,
            )) < 4.0 * 128.0
            {
                continue;
            }
            if spots.iter().any(|p: &Vec3| {
                Vec3::new(world_x, 0.0, world_z).distance(Vec3::new(p.x, 0.0, p.z)) < 4.0 * 128.0
            }) {
                continue;
            }
            if !collision.is_walkable_world(world_x, world_z) {
                continue;
            }
            let tile_x = (world_x / 128.0).floor() as i32;
            let tile_y = (world_z / 128.0).floor() as i32;
            let rx = (tile_x >> 6) as u32;
            let ry = (tile_y >> 6) as u32;
            let lx = (tile_x & 63) as usize;
            let ly = (tile_y & 63) as usize;
            let Some(terrain) = terrains.get(&(rx, ry)) else {
                continue;
            };
            let tile = terrain.tile(0, lx, ly);
            if tile.underlay_id > 0
                && tile.underlay_id != 55
                && tile.overlay_id == 0
                && tile.height > 4
            {
                spots.push(Vec3::new(
                    world_x,
                    height.height_at(world_x, world_z),
                    world_z,
                ));
                if spots.len() >= 8 {
                    break;
                }
            }
        }
        if spots.len() >= 8 {
            break;
        }
    }

    let mut spawns = Vec::new();
    for (i, pos) in spots.iter().enumerate() {
        if i % 4 == 3 {
            spawns.push(NpcSpawn {
                model_id: 10413,
                idle_sequence: 1721,
                walk_sequence: Some(2405),
                pos: *pos,
                scale: 1.6,
            });
        } else {
            spawns.push(NpcSpawn {
                model_id: 7744,
                idle_sequence: 222,
                walk_sequence: Some(219),
                pos: *pos,
                scale: 2.2,
            });
        }
    }
    spawns
}

fn is_blocking_object(def: &ObjectDefinition) -> bool {
    let name = def.name.to_lowercase();
    if name.contains("plant")
        || name.contains("flower")
        || name.contains("grass")
        || name.contains("fern")
        || name.contains("vine")
        || name.contains("reed")
    {
        return false;
    }
    if def.wall_or_door != 0 {
        return true;
    }
    if def.size_x > 1 || def.size_y > 1 {
        return true;
    }
    [
        "tree", "rock", "wall", "fence", "ruin", "pillar", "statue", "crate", "barrel", "table",
        "altar", "door", "gate", "bank", "chest", "stump",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

fn is_scenery(def: &ObjectDefinition) -> bool {
    let name = def.name.to_lowercase();
    name.contains("tree")
        || name.contains("palm")
        || name.contains("jungle")
        || name.contains("plant")
        || name.contains("bush")
        || name.contains("fern")
        || name.contains("cactus")
}

fn load_underlays(cache: &Cache) -> Result<HashMap<u32, UnderlayDefinition>> {
    let files = cache.archive_files_with_ids(2, 1)?;
    let mut out = HashMap::new();
    for (id, data) in files {
        if let Ok(def) = UnderlayDefinition::parse(id, &data) {
            out.insert(id, def);
        }
    }
    Ok(out)
}

fn load_overlays(cache: &Cache) -> Result<HashMap<u32, OverlayDefinition>> {
    let files = cache.archive_files_with_ids(2, 4)?;
    let mut out = HashMap::new();
    for (id, data) in files {
        if let Ok(def) = OverlayDefinition::parse(id, &data) {
            out.insert(id, def);
        }
    }
    Ok(out)
}

fn tile_color(
    terrain: &Terrain,
    underlays: &HashMap<u32, UnderlayDefinition>,
    overlays: &HashMap<u32, OverlayDefinition>,
    x: usize,
    y: usize,
) -> [u8; 3] {
    let tile = terrain.tile(0, x, y);

    if tile.overlay_id > 0 {
        if let Some(overlay) = overlays.get(&(tile.overlay_id as u32)) {
            let color = overlay.color();
            // 0xff00ff is a sentinel for "texture-only" ground overlays. Until
            // textures are sampled, fall through to the underlay colour.
            if color != [255, 0, 255] {
                return color;
            }
        }
    }
    if tile.underlay_id > 0 {
        if let Some(underlay) = underlays.get(&(tile.underlay_id as u32)) {
            return underlay.color();
        }
    }

    if tile.height == 0 {
        [31, 70, 105]
    } else if tile.height < 8 {
        [72, 112, 55]
    } else {
        [90, 105, 62]
    }
}

fn push_blob_quad(mesh: &mut MeshData, center: Vec3, size: f32, alpha: f32) {
    let y = center.y + 4.0;
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(SceneVertex::blob(
        Vec3::new(center.x - size, y, center.z - size),
        alpha,
        [0.0, 0.0],
    ));
    mesh.vertices.push(SceneVertex::blob(
        Vec3::new(center.x + size, y, center.z - size),
        alpha,
        [1.0, 0.0],
    ));
    mesh.vertices.push(SceneVertex::blob(
        Vec3::new(center.x - size, y, center.z + size),
        alpha,
        [0.0, 1.0],
    ));
    mesh.vertices.push(SceneVertex::blob(
        Vec3::new(center.x + size, y, center.z + size),
        alpha,
        [1.0, 1.0],
    ));
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
}

fn append_model(mesh: &mut MeshData, model: &Model, base: Vec3, angle: f32) {
    append_model_scaled(mesh, model, base, angle, 1.0);
}

fn append_model_scaled(mesh: &mut MeshData, model: &Model, base: Vec3, angle: f32, scale: f32) {
    let rot = Mat3::from_rotation_y(angle);

    for (face_idx, &[a, b, c]) in model.faces.iter().enumerate() {
        let pa = model_vertex_scaled(model, a as usize, base, rot, scale);
        let pb = model_vertex_scaled(model, b as usize, base, rot, scale);
        let pc = model_vertex_scaled(model, c as usize, base, rot, scale);

        let mut normal = (pb - pa).cross(pc - pa).normalize_or_zero();
        if normal.length_squared() < 1e-8 {
            normal = rot
                * Vec3::from_array(
                    model
                        .face_normals
                        .get(face_idx)
                        .copied()
                        .unwrap_or([0.0, 1.0, 0.0]),
                );
        }

        let textured = model
            .face_textures
            .as_ref()
            .and_then(|t| t.get(face_idx))
            .is_some_and(|&t| t >= 0);
        let rgb = if textured || model.face_colors[face_idx] == 127 {
            let n = model
                .face_normals
                .get(face_idx)
                .copied()
                .unwrap_or([0.0, 1.0, 0.0]);
            [(38.0 + n[1] * 16.0) as u8, (82.0 + n[1] * 22.0) as u8, 34]
        } else {
            hsl_to_rgb(model.face_colors[face_idx], 1.0)
        };

        mesh.push_triangle(
            SceneVertex::new(pa, normal, rgb),
            SceneVertex::new(pb, normal, rgb),
            SceneVertex::new(pc, normal, rgb),
        );
    }
}

pub fn model_vertex_scaled(model: &Model, i: usize, base: Vec3, rot: Mat3, scale: f32) -> Vec3 {
    let local = Vec3::new(
        model.vertex_x[i] as f32,
        -(model.vertex_y[i] as f32),
        model.vertex_z[i] as f32,
    ) * scale;
    base + rot * local
}

pub fn model_vertex_animated(
    positions: &[[f32; 3]],
    i: usize,
    base: Vec3,
    rot: Mat3,
    scale: f32,
) -> Vec3 {
    let p = positions.get(i).copied().unwrap_or([0.0; 3]);
    let local = Vec3::new(p[0], -p[1], p[2]) * scale;
    base + rot * local
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn srgb_endpoints_are_stable() {
        assert_eq!(srgb_to_linear(0.0), 0.0);
        assert!((srgb_to_linear(1.0) - 1.0).abs() < 1e-6);
        let mid = srgb_to_linear(0.5);
        assert!(mid > 0.2 && mid < 0.25);
    }

    #[test]
    fn scene_vertex_converts_colour_and_normalizes_normal() {
        let vertex = SceneVertex::new(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(0.0, 2.0, 0.0),
            [255, 0, 0],
        );
        assert!((vertex.normal[1] - 1.0).abs() < 1e-6);
        assert!((vertex.color[0] - 1.0).abs() < 1e-6);
        assert_eq!(vertex.color[1], 0.0);
        assert_eq!(vertex.material, 0);
    }

    #[test]
    fn unlit_and_blob_materials_are_tagged() {
        let unlit = SceneVertex::unlit(Vec3::ZERO, [10, 20, 30]);
        assert_eq!(unlit.material, 1);
        let blob = SceneVertex::blob(Vec3::ZERO, 0.4, [0.0, 0.0]);
        assert_eq!(blob.material, 2);
        assert!((blob.color[3] - 0.4).abs() < 1e-6);
    }

    #[test]
    fn atmosphere_defaults_are_sane() {
        let atmosphere = Atmosphere::default();
        assert!(atmosphere.fog_density > 0.0);
        assert!(atmosphere.fog_height > 0.0);
        assert!(atmosphere.sun_dir[1] > 0.0);
        assert!(atmosphere.sun_color.iter().all(|c| (0.0..=1.0).contains(c)));
    }
}
