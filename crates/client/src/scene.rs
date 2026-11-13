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
                color[0] as f32 / 255.0,
                color[1] as f32 / 255.0,
                color[2] as f32 / 255.0,
                1.0,
            ],
        }
    }
}

pub struct MeshData {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl MeshData {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn push_triangle(&mut self, a: Vertex, b: Vertex, c: Vertex) {
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
struct MapJson {
    #[serde(default)]
    name: String,
    wave_interval: f32,
    nexus: Vec<StructureJson>,
    lanes: Vec<LaneJson>,
    #[serde(default)]
    camps: Vec<CampJson>,
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
    pub wave_interval: f32,
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
        for lane in &map.lanes {
            if lane.waypoints.len() < 2 {
                anyhow::bail!("lane {} needs at least two waypoints", lane.id);
            }
            lanes.push(LaneSpawn {
                id: lane.id.clone(),
                waypoints: lane.waypoints.iter().map(|&tile| world_of(tile)).collect(),
            });
            for s in &lane.towers {
                structures.push(to_spawn(s));
            }
            for s in &lane.inhibitors {
                structures.push(to_spawn(s));
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

        Ok(Self {
            name: map.name,
            lanes,
            structures,
            camps,
            wave_interval: map.wave_interval,
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
                        Vertex::new(v0, color),
                        Vertex::new(v1, color),
                        Vertex::new(v2, color),
                    );
                    mesh.push_triangle(
                        Vertex::new(v2, color),
                        Vertex::new(v1, color),
                        Vertex::new(v3, color),
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
    pub center: Vec3,
    pub placements: usize,
    pub height: HeightGrid,
    pub collision: WorldCollision,
    pub map: MapLayout,
    pub npc_spawns: Vec<NpcSpawn>,
    pub player_spawn: Vec3,
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
    for gy in 0..size {
        for gx in 0..size {
            let idx = gy * size + gx;
            let h = heights[idx];
            let shade = 0.58 + 0.24 * (h / 800.0).clamp(0.0, 1.0);
            let c = colors[idx];
            let color = [
                (c[0] as f32 * shade * 0.82) as u8,
                (c[1] as f32 * shade * 0.82) as u8,
                (c[2] as f32 * shade * 0.82) as u8,
            ];
            terrain_mesh.vertices.push(Vertex::new(
                Vec3::new(
                    (origin_tile_x + gx as i32) as f32 * 128.0,
                    h,
                    (origin_tile_y + gy as i32) as f32 * 128.0,
                ),
                color,
            ));
        }
    }
    for gy in 0..size - 1 {
        for gx in 0..size - 1 {
            let a = (gy * size + gx) as u32;
            let b = a + 1;
            let c = a + size as u32;
            let d = c + 1;
            terrain_mesh.indices.extend_from_slice(&[a, c, b, b, c, d]);
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

    Ok(SceneData {
        terrain: terrain_mesh,
        models: model_mesh,
        center,
        placements,
        height,
        collision,
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

fn append_model(mesh: &mut MeshData, model: &Model, base: Vec3, angle: f32) {
    append_model_scaled(mesh, model, base, angle, 1.0);
}

fn append_model_scaled(mesh: &mut MeshData, model: &Model, base: Vec3, angle: f32, scale: f32) {
    let rot = Mat3::from_rotation_y(angle);
    let light = Vec3::new(0.35, 0.85, 0.4).normalize();

    for (face_idx, &[a, b, c]) in model.faces.iter().enumerate() {
        let pa = model_vertex_scaled(model, a as usize, base, rot, scale);
        let pb = model_vertex_scaled(model, b as usize, base, rot, scale);
        let pc = model_vertex_scaled(model, c as usize, base, rot, scale);

        let normal = (pb - pa).cross(pc - pa).normalize_or_zero();
        let ndotl = normal.dot(light).clamp(0.0, 1.0);
        let brightness = 0.62 + 0.22 * (1.0 - ndotl);

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
            let leaf = [38.0 + n[1] * 16.0, 82.0 + n[1] * 22.0, 34.0];
            [
                (leaf[0] * brightness) as u8,
                (leaf[1] * brightness) as u8,
                (leaf[2] * brightness) as u8,
            ]
        } else {
            hsl_to_rgb(model.face_colors[face_idx], brightness)
        };

        mesh.push_triangle(
            Vertex::new(pa, rgb),
            Vertex::new(pb, rgb),
            Vertex::new(pc, rgb),
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
