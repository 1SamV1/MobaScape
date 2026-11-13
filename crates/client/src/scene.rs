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
    sight_grid: Grid,
    origin_tile_x: i32,
    origin_tile_y: i32,
}

impl WorldCollision {
    pub fn new(grid: Grid, sight_grid: Grid, origin_tile_x: i32, origin_tile_y: i32) -> Self {
        Self {
            grid,
            sight_grid,
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

    /// Trees, walls, and structures block sight; water does not.
    pub fn blocks_sight_tile(&self, tx: i32, ty: i32) -> bool {
        !self.sight_grid.is_walkable(tx, ty)
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
    /// A lane watchtower. It acquires a target inside its short aggro radius
    /// and then keeps firing until the target dies or leaves the longer
    /// locked range.
    Watchtower,
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
    pub aggro: f32,
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
    /// Range at which a watchtower first acquires a target. Once it has a
    /// target it keeps firing from `range`, which is the larger lock range.
    #[serde(default)]
    aggro: f32,
}

/// A biome pocket: a rectangle of a real OSRS region transplanted into the
/// map so each side of the rift has its own scenery and ground palette.
#[derive(Debug, Clone, Deserialize)]
struct BiomeJson {
    id: String,
    #[serde(default)]
    side: String,
    #[serde(default)]
    label: String,
    /// OSRS region (region_x, region_y) the pocket is cut from.
    region: [u32; 2],
    /// Source rectangle inside the region: x, y, width, height (tiles).
    #[serde(default)]
    rect: Option<[u32; 4]>,
    /// Map tile where the source rectangle is placed.
    origin: [f32; 2],
    /// Ground tint multiplier applied to the pocket terrain.
    #[serde(default)]
    tint: Option<[f32; 3]>,
    /// Scenery budget for the pocket.
    #[serde(default = "default_budget")]
    budget: usize,
}

fn default_budget() -> usize {
    90
}

/// The river that splits the map between the radiant and dire halves.
#[derive(Debug, Clone, Deserialize)]
struct RiverJson {
    points: Vec<[f32; 2]>,
    /// Half width of the river in tiles.
    #[serde(default = "default_river_width")]
    width: f32,
    /// How far the bed is cut below the banks, in world units.
    #[serde(default = "default_river_depth")]
    depth: f32,
    /// Tiles where a lane crosses the river and stays walkable.
    #[serde(default)]
    bridges: Vec<[f32; 2]>,
    /// Water surface height at the first and last point (world units).
    #[serde(default)]
    levels: Option<[f32; 2]>,
}

fn default_river_width() -> f32 {
    3.5
}

fn default_river_depth() -> f32 {
    30.0
}

#[derive(Debug, Clone, Deserialize)]
struct LaneJson {
    id: String,
    waypoints: Vec<[f32; 2]>,
    towers: Vec<StructureJson>,
    inhibitors: Vec<StructureJson>,
    #[serde(default)]
    watchtowers: Vec<StructureJson>,
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
    biomes: Vec<BiomeJson>,
    #[serde(default)]
    river: Option<RiverJson>,
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
    #[serde(default)]
    damage: f32,
    #[serde(default)]
    range: f32,
    #[serde(default)]
    interval: f32,
    #[serde(default)]
    speed: f32,
    #[serde(default)]
    aggro: f32,
    #[serde(default)]
    leash: f32,
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
    pub damage: f32,
    pub range: f32,
    pub interval: f32,
    pub speed: f32,
    pub aggro: f32,
    pub leash: f32,
}

#[derive(Debug, Clone)]
pub struct BiomePocket {
    pub id: String,
    pub side: String,
    pub label: String,
    pub region: (u32, u32),
    pub rect: (u32, u32, u32, u32),
    pub origin: (i32, i32),
    pub tint: [f32; 3],
    pub budget: usize,
}

#[derive(Debug, Clone)]
pub struct RiverSpec {
    pub points: Vec<(f32, f32)>,
    pub width: f32,
    pub depth: f32,
    pub bridges: Vec<(f32, f32)>,
    pub levels: Option<(f32, f32)>,
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
    pub biomes: Vec<BiomePocket>,
    pub river: Option<RiverSpec>,
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
            aggro: s.aggro,
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
            for s in &lane.watchtowers {
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
                damage: camp.damage,
                range: camp.range,
                interval: camp.interval,
                speed: camp.speed,
                aggro: camp.aggro,
                leash: camp.leash,
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

        let biomes = map
            .biomes
            .iter()
            .map(|biome| {
                let rect = biome.rect.unwrap_or([0, 0, 64, 64]);
                BiomePocket {
                    id: biome.id.clone(),
                    side: biome.side.clone(),
                    label: biome.label.clone(),
                    region: (biome.region[0], biome.region[1]),
                    rect: (rect[0], rect[1], rect[2], rect[3]),
                    origin: (
                        biome.origin[0].round() as i32,
                        biome.origin[1].round() as i32,
                    ),
                    tint: biome.tint.unwrap_or([1.0, 1.0, 1.0]),
                    budget: biome.budget,
                }
            })
            .collect();

        let river = map.river.as_ref().map(|river| RiverSpec {
            points: river.points.iter().map(|p| (p[0], p[1])).collect(),
            width: river.width,
            depth: river.depth,
            bridges: river.bridges.iter().map(|p| (p[0], p[1])).collect(),
            levels: river.levels.map(|l| (l[0], l[1])),
        });

        Ok(Self {
            name: map.name,
            lanes,
            structures,
            camps,
            brush,
            biomes,
            river,
            wave_interval: map.wave_interval,
            atmosphere: map.atmosphere,
        })
    }

    /// True when a map tile falls inside one of the biome pockets.
    pub fn in_biome(&self, tile_x: i32, tile_y: i32) -> bool {
        self.biomes.iter().any(|biome| {
            tile_x >= biome.origin.0
                && tile_y >= biome.origin.1
                && tile_x < biome.origin.0 + biome.rect.2 as i32
                && tile_y < biome.origin.1 + biome.rect.3 as i32
        })
    }

    /// Which side of the river a tile sits on: negative is the dire half,
    /// positive is the radiant half.
    pub fn river_side(&self, tile_x: f32, tile_y: f32) -> f32 {
        let Some(river) = &self.river else {
            return 1.0;
        };
        if river.points.len() < 2 {
            return 1.0;
        }
        let (_, segment, _) = nearest_on_polyline(&river.points, tile_x, tile_y);
        let (ax, ay) = river.points[segment];
        let (bx, by) = river.points[segment + 1];
        let cross = (bx - ax) * (tile_y - ay) - (by - ay) * (tile_x - ax);
        if cross < 0.0 { -1.0 } else { 1.0 }
    }

    /// Closest point on any lane centre line plus the distance to it.
    pub fn nearest_lane_point(&self, x: f32, z: f32) -> (Vec2, f32) {
        let p = Vec2::new(x, z);
        let mut best = (p, f32::MAX);
        for lane in &self.lanes {
            for pair in lane.waypoints.windows(2) {
                let a = Vec2::new(pair[0].x, pair[0].z);
                let b = Vec2::new(pair[1].x, pair[1].z);
                let ab = b - a;
                let len2 = ab.length_squared();
                let t = if len2 <= 1e-6 {
                    0.0
                } else {
                    ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
                };
                let closest = a + ab * t;
                let d = closest.distance(p);
                if d < best.1 {
                    best = (closest, d);
                }
            }
        }
        best
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

    // The map layout defines the biome pockets and the river, so it has to
    // be read before the terrain is finalised.
    let provisional_height = HeightGrid {
        origin_tile_x,
        origin_tile_y,
        size,
        heights: heights.clone(),
    };
    let mut map = MapLayout::load(map_path, &provisional_height)?;

    // ------------------------------------------------------------------
    // Biome pockets: rectangles of real OSRS regions transplanted into the
    // map so the radiant and dire halves carry their own scenery and
    // ground palette. The dire pockets hold dark magic, vampires, ogres
    // and goblins; the radiant pockets hold human cities, elven woods and
    // the gnome stronghold.
    // ------------------------------------------------------------------
    let mut pocket_data: HashMap<String, (Terrain, Locations)> = HashMap::new();
    for biome in &map.biomes {
        let Some(locations_group) = keys.find_locations_group(biome.region.0, biome.region.1)
        else {
            tracing::warn!(
                "biome {}: region {:?} missing from cache",
                biome.id,
                biome.region
            );
            continue;
        };
        let terrain_group = locations_group.saturating_sub(1);
        let terrain = Terrain::load(
            biome.region.0,
            biome.region.1,
            &cache.group(5, terrain_group)?.data,
        )?;
        let locations = Locations::load(
            biome.region.0,
            biome.region.1,
            &cache.group(5, locations_group)?.data,
        )?;
        pocket_data.insert(biome.id.clone(), (terrain, locations));
    }

    let mut pocket_water: Vec<(i32, i32)> = Vec::new();
    for biome in &map.biomes {
        let Some((terrain, _)) = pocket_data.get(&biome.id) else {
            continue;
        };
        let (rx, ry, pw, ph) = biome.rect;
        for ly in 0..ph {
            for lx in 0..pw {
                let sx = rx + lx;
                let sy = ry + ly;
                if sx >= 64 || sy >= 64 {
                    continue;
                }
                let map_tx = biome.origin.0 + lx as i32;
                let map_ty = biome.origin.1 + ly as i32;
                let gx = map_tx - origin_tile_x;
                let gy = map_ty - origin_tile_y;
                if gx < 0 || gy < 0 || gx >= size as i32 - 1 || gy >= size as i32 - 1 {
                    continue;
                }
                let idx = gy as usize * size + gx as usize;
                let tile = terrain.tile(0, sx as usize, sy as usize);
                // Blend the pocket into the surrounding ground over the
                // outer seven tiles so the seam is hidden.
                let edge = lx
                    .min(ly)
                    .min(pw.saturating_sub(1).saturating_sub(lx))
                    .min(ph.saturating_sub(1).saturating_sub(ly)) as f32;
                let blend = (edge / 7.0).clamp(0.0, 1.0);
                let blend = blend * blend * (3.0 - 2.0 * blend);
                let pocket_h = tile.height as f32 * 8.0;
                heights[idx] = heights[idx] * (1.0 - blend) + pocket_h * blend;
                let c = tile_color(terrain, &underlays, &overlays, sx as usize, sy as usize);
                let tinted = [
                    (c[0] as f32 * biome.tint[0]).clamp(0.0, 255.0) as u8,
                    (c[1] as f32 * biome.tint[1]).clamp(0.0, 255.0) as u8,
                    (c[2] as f32 * biome.tint[2]).clamp(0.0, 255.0) as u8,
                ];
                for k in 0..3 {
                    colors[idx][k] = (colors[idx][k] as f32 * (1.0 - blend)
                        + tinted[k] as f32 * blend)
                        .clamp(0.0, 255.0) as u8;
                }
                if tile.underlay_id == 0 {
                    pocket_water.push((gx, gy));
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // The river: a diagonal waterway that splits the rift into a radiant
    // half and a dire half, with a walkable crossing in the centre.
    // ------------------------------------------------------------------
    let mut river_blocked: Vec<(i32, i32)> = Vec::new();
    if let Some(river) = &map.river {
        let points = &river.points;
        if points.len() >= 2 {
            let mut levels: Vec<f32> = Vec::with_capacity(points.len());
            for (index, point) in points.iter().enumerate() {
                let gx = point.0.round() as i32 - origin_tile_x;
                let gy = point.1.round() as i32 - origin_tile_y;
                let sampled = if gx >= 0 && gy >= 0 && (gx as usize) < size && (gy as usize) < size
                {
                    heights[gy as usize * size + gx as usize]
                } else {
                    0.0
                };
                let level = match river.levels {
                    Some((start, end)) => {
                        let t = index as f32 / (points.len() - 1).max(1) as f32;
                        start + (end - start) * t
                    }
                    None => sampled,
                };
                levels.push(level);
            }
            // Water always runs downhill.
            for index in 1..levels.len() {
                if levels[index] > levels[index - 1] - 0.5 {
                    levels[index] = levels[index - 1] - 0.5;
                }
            }

            for gy in 0..size - 1 {
                for gx in 0..size - 1 {
                    let tx = (origin_tile_x + gx as i32) as f32;
                    let ty = (origin_tile_y + gy as i32) as f32;
                    let (dist, segment, along) = nearest_on_polyline(points, tx, ty);
                    if dist > river.width * 2.2 {
                        continue;
                    }
                    let level = levels[segment] + (levels[segment + 1] - levels[segment]) * along;
                    let k = 1.0 - smoothstep(river.width * 0.45, river.width * 1.9, dist);
                    if k <= 0.001 {
                        continue;
                    }
                    let bridge = river
                        .bridges
                        .iter()
                        .any(|b| (b.0 - tx) * (b.0 - tx) + (b.1 - ty) * (b.1 - ty) < 30.0);
                    let depth = if bridge {
                        river.depth * 0.2
                    } else {
                        river.depth
                    };
                    let idx = gy * size + gx;
                    let bed = (level - depth).min(heights[idx]);
                    heights[idx] = heights[idx] * (1.0 - k) + bed * k;
                    let water = [40u8, 84, 126];
                    for c in 0..3 {
                        colors[idx][c] = (colors[idx][c] as f32 * (1.0 - k) + water[c] as f32 * k)
                            .clamp(0.0, 255.0) as u8;
                    }
                    if !bridge && k > 0.55 {
                        river_blocked.push((gx as i32, gy as i32));
                    }
                }
            }
        }
    }

    // Smooth the per-tile color field before triangulating. Two box-blur
    // passes remove the hard square boundaries between adjacent tiles; the
    // resulting per-corner colors interpolate across the terrain mesh.
    let mut smooth_colors = colors.clone();
    for _ in 0..2 {
        smooth_colors = box_blur_colors(&smooth_colors, size, 2);
    }

    // ------------------------------------------------------------------
    // Lanes are baked into the terrain itself instead of being drawn as a
    // ribbon on top of it: the mask lives in the terrain vertex alpha and
    // the shader blends the OSRS path textures into the ground. That keeps
    // the road seamless with the world and stops the overlay from
    // z-fighting with the terrain.
    // ------------------------------------------------------------------
    const LANE_HALF_WIDTH: f32 = 300.0;
    const LANE_PATH_COLOUR: [u8; 3] = [122, 96, 66];
    let mut lane_mask = vec![0.0f32; size * size];
    for gy in 0..size {
        for gx in 0..size {
            let wx = (origin_tile_x + gx as i32) as f32 * 128.0;
            let wz = (origin_tile_y + gy as i32) as f32 * 128.0;
            let (nearest, distance) = map.nearest_lane_point(wx, wz);
            // A slow noise keeps the road edge from looking ruler straight.
            let wobble = (value_noise(wx * 0.0035, wz * 0.0035) - 0.5) * 150.0;
            let t = ((distance + wobble) / LANE_HALF_WIDTH).clamp(0.0, 2.0);
            let mask = 1.0 - smoothstep(0.78, 1.22, t);
            let idx = gy * size + gx;
            lane_mask[idx] = mask;
            if mask > 0.001 {
                for c in 0..3 {
                    smooth_colors[idx][c] = (smooth_colors[idx][c] as f32 * (1.0 - mask * 0.92)
                        + LANE_PATH_COLOUR[c] as f32 * mask * 0.92)
                        .clamp(0.0, 255.0) as u8;
                }
                // Level the road along the lane centre line.
                let centre = heights[(nearest.y / 128.0 - origin_tile_y as f32)
                    .clamp(0.0, size as f32 - 1.0) as usize
                    * size
                    + (nearest.x / 128.0 - origin_tile_x as f32).clamp(0.0, size as f32 - 1.0)
                        as usize];
                heights[idx] = heights[idx] * (1.0 - mask * 0.65) + centre * mask * 0.65;
            }
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

            let c00 = smooth_colors[idx];
            let c10 = smooth_colors[idx + 1];
            let c01 = smooth_colors[idx + size];
            let c11 = smooth_colors[idx + size + 1];

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
            let ao_at = |vx: usize, vy: usize| {
                let i = vy * size + vx;
                let h = heights[i];
                let hl = if vx > 0 { heights[i - 1] } else { h };
                let hr = if vx + 1 < size { heights[i + 1] } else { h };
                let hd = if vy > 0 { heights[i - size] } else { h };
                let hu = if vy + 1 < size { heights[i + size] } else { h };
                let cavity = (((hl + hr + hd + hu) * 0.25 - h) / 96.0).clamp(-1.0, 1.0);
                (0.82 - cavity * 0.22).clamp(0.58, 1.0)
            };
            let ao00 = ao_at(gx, gy);
            let ao10 = ao_at(gx + 1, gy);
            let ao01 = ao_at(gx, gy + 1);
            let ao11 = ao_at(gx + 1, gy + 1);

            let mut v00 = SceneVertex::new(p00, n00, c00);
            let mut v10 = SceneVertex::new(p10, n10, c10);
            let mut v01 = SceneVertex::new(p01, n01, c01);
            let mut v11 = SceneVertex::new(p11, n11, c11);
            // The lane mask rides in the vertex alpha for the terrain.
            v00.color[3] = lane_mask[idx];
            v10.color[3] = lane_mask[idx + 1];
            v01.color[3] = lane_mask[idx + size];
            v11.color[3] = lane_mask[idx + size + 1];
            v00.ao = ao00;
            v10.ao = ao10;
            v01.ao = ao01;
            v11.ao = ao11;
            v00.material = 4;
            v10.material = 4;
            v01.material = 4;
            v11.material = 4;
            terrain_mesh.push_triangle(v00, v01, v10);
            terrain_mesh.push_triangle(v10, v01, v11);
        }
    }

    let height = HeightGrid {
        origin_tile_x,
        origin_tile_y,
        size,
        heights,
    };

    // Re-seat map features on the final terrain: pockets and the river can
    // shift the ground under a lane or a camp.
    for structure in &mut map.structures {
        structure.pos.y = height.height_at(structure.pos.x, structure.pos.z);
    }
    for camp in &mut map.camps {
        camp.pos.y = height.height_at(camp.pos.x, camp.pos.z);
    }
    for lane in &mut map.lanes {
        for waypoint in &mut lane.waypoints {
            waypoint.y = height.height_at(waypoint.x, waypoint.z);
        }
    }

    // Collision: water and blocking scenery footprints. This is deliberately
    // conservative; the editor will replace it with authored collision later.
    let tiles_x = ((max_x - min_x + 1) * 64) as usize;
    let tiles_y = ((max_y - min_y + 1) * 64) as usize;
    let mut collision_grid = Grid::new(tiles_x, tiles_y);
    let mut sight_grid = Grid::new(tiles_x, tiles_y);
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

    for (gx, gy) in &pocket_water {
        collision_grid.set_blocked(*gx, *gy, true);
    }
    for (gx, gy) in &river_blocked {
        collision_grid.set_blocked(*gx, *gy, true);
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
            sight_grid.block_rect(gx, gy, w, h);
        }
    }

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
        append_model_foliage(&mut model_mesh, model, base, angle, is_plant_object(&def));
        let shadow_size = (def.size_x.max(def.size_y) as f32 * 128.0 * 0.55).clamp(55.0, 360.0);
        push_blob_quad(&mut shadow_mesh, base, shadow_size, 0.24);
        placements += 1;
    }

    // ------------------------------------------------------------------
    // Biome pocket scenery: real OSRS objects taken from each pocket's
    // source region and transplanted into the map.
    // ------------------------------------------------------------------
    for biome in &map.biomes {
        let Some((_, locations)) = pocket_data.get(&biome.id) else {
            continue;
        };
        let (rx, ry, pw, ph) = biome.rect;
        let center_x = biome.origin.0 as f32 + pw as f32 * 0.5;
        let center_y = biome.origin.1 as f32 + ph as f32 * 0.5;
        let mut candidates: Vec<(u8, f32, u32, u32, u32, u32, u8)> = Vec::new();
        for loc in &locations.locations {
            let loc_x = loc.x as u32;
            let loc_y = loc.y as u32;
            if loc_x < rx || loc_y < ry || loc_x >= rx + pw || loc_y >= ry + ph {
                continue;
            }
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
            if !is_pocket_scenery(def) {
                continue;
            }
            let Some(model_id) = def.first_model() else {
                continue;
            };
            let dx = loc.x as f32 + 0.5 - center_x;
            let dy = loc.y as f32 + 0.5 - center_y;
            candidates.push((
                scenery_priority(def),
                dx * dx + dy * dy,
                loc.id,
                loc_x,
                loc_y,
                model_id,
                loc.rotation,
            ));
        }
        candidates.sort_by(|a, b| {
            a.0.cmp(&b.0)
                .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        });

        let mut placed = 0usize;
        for (_, _, object_id, sx, sy, model_id, rotation) in candidates {
            if placed >= biome.budget {
                break;
            }
            let Some(def) = def_cache.get(&object_id) else {
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
            let map_tx = biome.origin.0 + sx as i32 - rx as i32;
            let map_ty = biome.origin.1 + sy as i32 - ry as i32;
            let wx = (map_tx as f32 + 0.5) * 128.0;
            let wz = (map_ty as f32 + 0.5) * 128.0;
            if !height.contains(wx, wz) {
                continue;
            }
            // Pockets can overlap a lane; never drop a door or a wall in the
            // middle of the road.
            if map.distance_to_lane(wx, wz) < 6.5 * 128.0 {
                continue;
            }
            let base = Vec3::new(wx, height.height_at(wx, wz), wz);
            let angle = rotation as f32 * std::f32::consts::FRAC_PI_2;
            append_model_scaled_tinted(
                &mut model_mesh,
                model,
                base,
                angle,
                1.0,
                biome.tint,
                is_plant_object(def),
            );
            let shadow_size = (def.size_x.max(def.size_y) as f32 * 128.0 * 0.55).clamp(55.0, 360.0);
            push_blob_quad(&mut shadow_mesh, base, shadow_size, 0.24);

            let mut w = def.size_x.max(1) as i32;
            let mut h = def.size_y.max(1) as i32;
            if rotation % 2 == 1 {
                std::mem::swap(&mut w, &mut h);
            }
            collision_grid.block_rect(map_tx - origin_tile_x, map_ty - origin_tile_y, w, h);
            sight_grid.block_rect(map_tx - origin_tile_x, map_ty - origin_tile_y, w, h);
            placed += 1;
        }
        placements += placed;
        tracing::info!(
            "biome {} ({}): {} models from region {:?}",
            biome.id,
            biome.side,
            placed,
            biome.region
        );
    }

    // Lanes stay walkable even where a biome pocket overlaps them, and the
    // river is only fordable where a lane crosses it.
    map.clear_collision(&mut collision_grid, origin_tile_x, origin_tile_y, 4.5);
    map.block_structures(&mut collision_grid, origin_tile_x, origin_tile_y);
    map.block_structures(&mut sight_grid, origin_tile_x, origin_tile_y);
    tracing::info!(
        "river tiles blocked: {}, pocket tiles: {}",
        river_blocked.len(),
        pocket_water.len()
    );

    // ------------------------------------------------------------------
    // Tree lines: OSRS tree models form a natural barrier along both sides
    // of every lane. Spacing, distance, scale and species are seeded from a
    // fixed RNG so the forest looks hand scattered but every match is the
    // same world.
    // ------------------------------------------------------------------
    let structure_positions: Vec<Vec3> = map.structures.iter().map(|s| s.pos).collect();
    let tree_context = TreeContext {
        cache,
        map: &map,
        height: &height,
        structures: &structure_positions,
        river_blocked: &river_blocked,
        origin: (origin_tile_x, origin_tile_y),
    };
    let mut tree_count = 0usize;
    let mut rng = Rng::new(0x5EED_9E37_79B9_7F4A);
    {
        let mut sink = ScenerySink {
            model_cache: &mut model_cache,
            model_mesh: &mut model_mesh,
            shadow_mesh: &mut shadow_mesh,
            collision_grid: &mut collision_grid,
        };
        for lane in &map.lanes {
            for pair in lane.waypoints.windows(2) {
                let a = pair[0];
                let b = pair[1];
                let delta = b - a;
                let length = delta.length();
                if length < 1.0 {
                    continue;
                }
                let dir = delta / length;
                let perp = Vec3::new(-dir.z, 0.0, dir.x);
                let mut travelled = rng.range(0.0, 220.0);
                while travelled < length {
                    let point = a + dir * travelled;
                    for side in [-1.0f32, 1.0] {
                        // Leave occasional gaps so the tree wall is not a
                        // perfect hedge.
                        if rng.chance(0.12) {
                            continue;
                        }
                        let offset = rng.range(4.9, 9.8) * 128.0;
                        let along = rng.range(-1.1, 1.1) * 128.0;
                        let pos = point + perp * (offset * side) + dir * along;
                        let seed = rng.next_u32();
                        place_tree(&tree_context, &mut sink, pos, seed, &mut tree_count);
                        // Occasional cluster behind the first tree.
                        if rng.chance(0.24) {
                            let extra = point
                                + perp * (rng.range(6.5, 12.0) * 128.0 * side)
                                + dir * rng.range(-1.6, 1.6) * 128.0;
                            place_tree(
                                &tree_context,
                                &mut sink,
                                extra,
                                rng.next_u32(),
                                &mut tree_count,
                            );
                        }
                    }
                    travelled += rng.range(170.0, 470.0);
                }
            }
        }
    }
    tracing::info!("tree lines: {} trees", tree_count);

    // ------------------------------------------------------------------
    // Castle wall ruins: broken OSRS walls and ruin clusters ring the
    // outside of every lane, scattered with the same fixed seed.
    // ------------------------------------------------------------------
    const RUIN_MODELS: &[(u32, i32)] = &[(16918, 2), (16918, 2), (9233, 2), (9625, 3)];
    let mut ruin_count = 0usize;
    {
        let mut sink = ScenerySink {
            model_cache: &mut model_cache,
            model_mesh: &mut model_mesh,
            shadow_mesh: &mut shadow_mesh,
            collision_grid: &mut collision_grid,
        };
        for (lane_index, lane) in map.lanes.iter().enumerate() {
            for pair in lane.waypoints.windows(2) {
                let a = pair[0];
                let b = pair[1];
                let delta = b - a;
                let length = delta.length();
                if length < 1.0 {
                    continue;
                }
                let dir = delta / length;
                let perp = Vec3::new(-dir.z, 0.0, dir.x);
                let mut travelled = rng.range(0.0, 900.0);
                while travelled < length {
                    let point = a + dir * travelled;
                    if rng.chance(0.28) {
                        travelled += rng.range(700.0, 1800.0);
                        continue;
                    }
                    let side = if (rng.next_u32() as usize + lane_index) % 2 == 0 {
                        -1.0
                    } else {
                        1.0
                    };
                    let offset = rng.range(10.5, 15.0) * 128.0;
                    let along = rng.range(-2.0, 2.0) * 128.0;
                    let (model_id, footprint) =
                        RUIN_MODELS[rng.next_u32() as usize % RUIN_MODELS.len()];
                    place_ruin(
                        &tree_context,
                        &mut sink,
                        point + perp * (offset * side) + dir * along,
                        rng.next_u32(),
                        model_id,
                        footprint,
                        &mut ruin_count,
                    );
                    travelled += rng.range(650.0, 1900.0);
                }
            }
        }
    }
    tracing::info!("castle ruins: {} pieces", ruin_count);

    let collision = WorldCollision::new(collision_grid, sight_grid, origin_tile_x, origin_tile_y);

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

fn box_blur_colors(colors: &[[u8; 3]], size: usize, radius: i32) -> Vec<[u8; 3]> {
    let mut horizontal = vec![[0u8; 3]; colors.len()];
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0u32; 3];
            let mut count = 0u32;
            for dx in -radius..=radius {
                let sx = x as i32 + dx;
                if sx < 0 || sx >= size as i32 {
                    continue;
                }
                let c = colors[y * size + sx as usize];
                sum[0] += c[0] as u32;
                sum[1] += c[1] as u32;
                sum[2] += c[2] as u32;
                count += 1;
            }
            horizontal[y * size + x] = [
                (sum[0] / count) as u8,
                (sum[1] / count) as u8,
                (sum[2] / count) as u8,
            ];
        }
    }
    let mut out = vec![[0u8; 3]; colors.len()];
    for y in 0..size {
        for x in 0..size {
            let mut sum = [0u32; 3];
            let mut count = 0u32;
            for dy in -radius..=radius {
                let sy = y as i32 + dy;
                if sy < 0 || sy >= size as i32 {
                    continue;
                }
                let c = horizontal[sy as usize * size + x];
                sum[0] += c[0] as u32;
                sum[1] += c[1] as u32;
                sum[2] += c[2] as u32;
                count += 1;
            }
            out[y * size + x] = [
                (sum[0] / count) as u8,
                (sum[1] / count) as u8,
                (sum[2] / count) as u8,
            ];
        }
    }
    out
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

/// Small deterministic xorshift RNG. The map is generated from a fixed seed
/// so every match starts from the same world.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 32) as u32
    }

    fn f32(&mut self) -> f32 {
        self.next_u32() as f32 / u32::MAX as f32
    }

    fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    fn chance(&mut self, p: f32) -> bool {
        self.f32() < p
    }
}

/// Smooth value noise used to wobble the lane edges.
fn value_noise(x: f32, z: f32) -> f32 {
    let ix = x.floor();
    let iz = z.floor();
    let fx = x - ix;
    let fz = z - iz;
    let ux = fx * fx * (3.0 - 2.0 * fx);
    let uz = fz * fz * (3.0 - 2.0 * fz);
    let h = |dx: u32, dz: u32| {
        let mut seed = (ix as i32 as u32).wrapping_mul(73856093);
        seed ^= (iz as i32 as u32).wrapping_mul(19349663);
        seed ^= dx << 8;
        seed ^= dz;
        hash01(seed)
    };
    let a = h(0, 0);
    let b = h(1, 0);
    let c = h(0, 1);
    let d = h(1, 1);
    let top = a + (b - a) * ux;
    let bottom = c + (d - c) * ux;
    top + (bottom - top) * uz
}

/// Deterministic 0..1 hash used for scenery jitter.
fn hash01(seed: u32) -> f32 {
    let mut h = seed ^ 0x9E37_79B9;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^= h >> 16;
    (h & 0x00FF_FFFF) as f32 / 0x0100_0000 as f32
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Distance to a polyline plus the index of the closest segment and how far
/// along that segment the closest point sits.
fn nearest_on_polyline(points: &[(f32, f32)], x: f32, y: f32) -> (f32, usize, f32) {
    let mut best = (f32::MAX, 0usize, 0.0f32);
    for (index, pair) in points.windows(2).enumerate() {
        let (ax, ay) = pair[0];
        let (bx, by) = pair[1];
        let abx = bx - ax;
        let aby = by - ay;
        let len2 = abx * abx + aby * aby;
        if len2 <= 1e-6 {
            continue;
        }
        let t = (((x - ax) * abx + (y - ay) * aby) / len2).clamp(0.0, 1.0);
        let px = ax + abx * t;
        let py = ay + aby * t;
        let d = ((x - px) * (x - px) + (y - py) * (y - py)).sqrt();
        if d < best.0 {
            best = (d, index, t);
        }
    }
    best
}

/// True for the OSRS objects that are plants, so their flat coloured faces
/// can be replaced with foliage green.
fn is_plant_object(def: &ObjectDefinition) -> bool {
    let name = def.name.to_lowercase();
    [
        "tree", "palm", "jungle", "bush", "plant", "leaves", "sapling",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

/// Scenery that is worth the draw call inside a biome pocket.
fn is_pocket_scenery(def: &ObjectDefinition) -> bool {
    let name = def.name.to_lowercase();
    if name.is_empty() {
        return false;
    }
    ![
        "flower", "daisies", "grass", "fern", "plant", "weed", "herb", "sapling", "mushroom",
    ]
    .iter()
    .any(|needle| name.contains(needle))
}

/// Order pocket scenery so landmark objects are placed before clutter.
fn scenery_priority(def: &ObjectDefinition) -> u8 {
    let name = def.name.to_lowercase();
    if name.contains("tree") || name.contains("palm") || name.contains("jungle") {
        return 0;
    }
    if [
        "pillar", "statue", "ruin", "castle", "bank", "shop", "door", "wall", "fence", "grave",
        "tent", "altar", "monument", "cauldron", "fountain", "banner", "hut", "village", "tower",
    ]
    .iter()
    .any(|needle| name.contains(needle))
    {
        return 1;
    }
    2
}

struct ScenerySink<'a> {
    model_cache: &'a mut HashMap<u32, Model>,
    model_mesh: &'a mut MeshData,
    shadow_mesh: &'a mut MeshData,
    collision_grid: &'a mut Grid,
}

struct TreeContext<'a> {
    cache: &'a Cache,
    map: &'a MapLayout,
    height: &'a HeightGrid,
    structures: &'a [Vec3],
    river_blocked: &'a [(i32, i32)],
    origin: (i32, i32),
}

/// Place one broken castle wall or ruin cluster. Ruins sit behind the tree
/// line so they never block a lane.
#[allow(clippy::too_many_arguments)]
fn place_ruin(
    ctx: &TreeContext<'_>,
    sink: &mut ScenerySink<'_>,
    pos: Vec3,
    seed: u32,
    model_id: u32,
    footprint: i32,
    count: &mut usize,
) {
    let tile_x = (pos.x / 128.0).floor() as i32;
    let tile_y = (pos.z / 128.0).floor() as i32;
    if !ctx.height.contains(pos.x, pos.z) {
        return;
    }
    if ctx
        .structures
        .iter()
        .any(|s| s.distance(pos) < 10.0 * 128.0)
    {
        return;
    }
    if ctx.map.distance_to_lane(pos.x, pos.z) < 8.0 * 128.0 {
        return;
    }
    if ctx.map.in_biome(tile_x, tile_y) {
        return;
    }
    let gx = tile_x - ctx.origin.0;
    let gy = tile_y - ctx.origin.1;
    if ctx.river_blocked.contains(&(gx, gy)) {
        return;
    }

    let model = if let Some(model) = sink.model_cache.get(&model_id) {
        model
    } else {
        let Ok(group) = ctx.cache.group(7, model_id) else {
            return;
        };
        let Ok(model) = Model::load(model_id, &group.data) else {
            return;
        };
        sink.model_cache.insert(model_id, model);
        sink.model_cache.get(&model_id).unwrap()
    };

    let base = Vec3::new(pos.x, ctx.height.height_at(pos.x, pos.z), pos.z);
    let angle = (hash01(seed ^ 0x1234_5678) * 4.0).floor() * std::f32::consts::FRAC_PI_2;
    let scale = 0.9 + hash01(seed.wrapping_add(3)) * 0.35;
    append_model_scaled_tinted(
        sink.model_mesh,
        model,
        base,
        angle,
        scale,
        [0.92, 0.9, 0.86],
        false,
    );
    push_blob_quad(sink.shadow_mesh, base, 150.0, 0.22);
    sink.collision_grid
        .block_rect(gx - footprint / 2, gy - footprint / 2, footprint, footprint);
    *count += 1;
}

/// Plant one OSRS tree model. Trees skirt the lanes, avoid structures and
/// pockets, and tint themselves by the side of the river they stand on.
fn place_tree(
    ctx: &TreeContext<'_>,
    sink: &mut ScenerySink<'_>,
    pos: Vec3,
    seed: u32,
    count: &mut usize,
) {
    let tile_x = (pos.x / 128.0).floor() as i32;
    let tile_y = (pos.z / 128.0).floor() as i32;
    if !ctx.height.contains(pos.x, pos.z) {
        return;
    }
    if ctx.structures.iter().any(|s| s.distance(pos) < 9.0 * 128.0) {
        return;
    }
    if ctx.map.distance_to_lane(pos.x, pos.z) < 5.2 * 128.0 {
        return;
    }
    if ctx.map.in_biome(tile_x, tile_y) {
        return;
    }
    let gx = tile_x - ctx.origin.0;
    let gy = tile_y - ctx.origin.1;
    if ctx.river_blocked.contains(&(gx, gy)) {
        return;
    }

    // Dire side gets dead and dark species, the radiant side keeps the lush
    // green canopy.
    let side = ctx.map.river_side(tile_x as f32, tile_y as f32);
    let pool: &[u32] = if side < 0.0 {
        &[1715, 1716, 1621, 1676, 1687, 1605, 1611, 1715]
    } else {
        &[1570, 1637, 1681, 1683, 1611, 1611, 1611, 1605, 1676]
    };
    let model_id = pool[(seed as usize) % pool.len()];
    let model = if let Some(model) = sink.model_cache.get(&model_id) {
        model
    } else {
        let Ok(group) = ctx.cache.group(7, model_id) else {
            return;
        };
        let Ok(model) = Model::load(model_id, &group.data) else {
            return;
        };
        sink.model_cache.insert(model_id, model);
        sink.model_cache.get(&model_id).unwrap()
    };

    let base = Vec3::new(pos.x, ctx.height.height_at(pos.x, pos.z), pos.z);
    let angle = hash01(seed ^ 0x5bf0_3635) * std::f32::consts::TAU;
    let scale = 0.72 + hash01(seed.wrapping_add(7)) * 0.46;
    let tint = if side < 0.0 {
        [0.86f32, 0.90, 0.80]
    } else {
        [1.0, 1.06, 0.92]
    };
    append_model_scaled_tinted(sink.model_mesh, model, base, angle, scale, tint, true);
    push_blob_quad(sink.shadow_mesh, base, 150.0, 0.22);
    sink.collision_grid.block_rect(gx, gy, 2, 2);
    *count += 1;
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

fn append_model_foliage(mesh: &mut MeshData, model: &Model, base: Vec3, angle: f32, foliage: bool) {
    append_model_scaled_tinted(mesh, model, base, angle, 1.0, [1.0, 1.0, 1.0], foliage);
}

/// OSRS tree models are often plain grey or white. When the object is a
/// plant, swap those faces for the foliage green so the tree reads as a
/// tree instead of a white cone.
fn foliage_tint(rgb: [u8; 3], foliage: bool) -> [u8; 3] {
    if !foliage {
        return rgb;
    }
    let max = rgb[0].max(rgb[1]).max(rgb[2]) as f32;
    let min = rgb[0].min(rgb[1]).min(rgb[2]) as f32;
    if max - min < 40.0 && max > 70.0 {
        return [70, 128, 58];
    }
    rgb
}

/// True when most of a model's faces are textured, which means it is a
/// canopy model whose plain faces are inner geometry.
fn model_is_textured(model: &Model) -> bool {
    let mut textured = 0usize;
    for (index, colour) in model.face_colors.iter().enumerate() {
        let is_textured = model
            .face_textures
            .as_ref()
            .and_then(|t| t.get(index))
            .is_some_and(|&t| t >= 0);
        if is_textured || *colour == 127 {
            textured += 1;
        }
    }
    textured * 2 > model.faces.len()
}

/// Scale a colour up until its brightest channel reaches `min`, so dark
/// OSRS vertex colours stay readable in shadow.
fn lift_brightness(rgb: [u8; 3], min: f32) -> [u8; 3] {
    let peak = rgb[0].max(rgb[1]).max(rgb[2]) as f32;
    if peak >= min || peak <= 1.0 {
        return rgb;
    }
    let scale = min / peak;
    [
        (rgb[0] as f32 * scale).clamp(0.0, 255.0) as u8,
        (rgb[1] as f32 * scale).clamp(0.0, 255.0) as u8,
        (rgb[2] as f32 * scale).clamp(0.0, 255.0) as u8,
    ]
}

fn append_model_scaled_tinted(
    mesh: &mut MeshData,
    model: &Model,
    base: Vec3,
    angle: f32,
    scale: f32,
    tint: [f32; 3],
    foliage: bool,
) {
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
        // Several OSRS tree models mix textured canopy faces with plain white
        // inner faces. Cap the plain faces so the tree reads as foliage
        // instead of a white cone.
        let rgb = if textured || model.face_colors[face_idx] == 127 {
            let n = model
                .face_normals
                .get(face_idx)
                .copied()
                .unwrap_or([0.0, 1.0, 0.0]);
            if foliage {
                [(84.0 + n[1] * 26.0) as u8, (142.0 + n[1] * 30.0) as u8, 62]
            } else {
                // Buildings, walls and ruins read as stone rather than grass.
                [
                    (116.0 + n[1] * 22.0) as u8,
                    (112.0 + n[1] * 20.0) as u8,
                    104,
                ]
            }
        } else {
            hsl_to_rgb(model.face_colors[face_idx], 1.0)
        };
        let rgb = foliage_tint(rgb, foliage);
        let rgb = if textured {
            rgb
        } else if model_is_textured(model) {
            [rgb[0].min(120), rgb[1].min(96), rgb[2].min(74)]
        } else {
            rgb
        };
        let rgb = [
            (rgb[0] as f32 * tint[0]).clamp(0.0, 255.0) as u8,
            (rgb[1] as f32 * tint[1]).clamp(0.0, 255.0) as u8,
            (rgb[2] as f32 * tint[2]).clamp(0.0, 255.0) as u8,
        ];
        // OSRS models carry very dark vertex colours; lift them so the
        // scenery never reads as a black blob.
        let rgb = lift_brightness(rgb, 62.0);

        let ao_at = |p: Vec3| {
            (0.78 + 0.22 * ((p.y - base.y).max(0.0) / 400.0).clamp(0.0, 1.0)).clamp(0.70, 1.0)
        };
        let mut va = SceneVertex::new(pa, normal, rgb);
        let mut vb = SceneVertex::new(pb, normal, rgb);
        let mut vc = SceneVertex::new(pc, normal, rgb);
        va.ao = ao_at(pa);
        vb.ao = ao_at(pb);
        vc.ao = ao_at(pc);
        mesh.push_triangle(va, vb, vc);
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
