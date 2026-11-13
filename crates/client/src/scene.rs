use std::collections::HashMap;

use anyhow::{Context, Result};
use glam::{Mat3, Vec3};
use rift_cache::{
    Cache, Locations, Model, ObjectDefinition, OverlayDefinition, Terrain, UnderlayDefinition,
    model::hsl_to_rgb,
};

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl Vertex {
    fn new(position: Vec3, color: [u8; 3]) -> Self {
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
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    fn push_triangle(&mut self, a: Vertex, b: Vertex, c: Vertex) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[a, b, c]);
        self.indices.extend_from_slice(&[base, base + 1, base + 2]);
    }
}

pub struct SceneData {
    pub terrain: MeshData,
    pub models: MeshData,
    pub center: Vec3,
    pub placements: usize,
}

pub fn build(cache: &Cache, region_x: u32, region_y: u32, model_limit: usize) -> Result<SceneData> {
    let keys = cache
        .keys()
        .context("cache has no keys.json; cannot resolve map groups")?;
    let locations_group = keys
        .find_locations_group(region_x, region_y)
        .with_context(|| format!("no locations group for region {region_x},{region_y}"))?;
    let terrain_group = locations_group
        .checked_sub(1)
        .context("locations group has no terrain predecessor")?;

    let terrain = Terrain::load(region_x, region_y, &cache.group(5, terrain_group)?.data)?;
    let locations = Locations::load(region_x, region_y, &cache.group(5, locations_group)?.data)?;

    let underlays = load_underlays(cache)?;
    let overlays = load_overlays(cache)?;
    let object_files = cache
        .archive_files(2, 6)
        .context("loading object definitions")?;

    let mut terrain_mesh = MeshData::new();
    build_terrain(&terrain, &underlays, &overlays, &mut terrain_mesh);

    let mut model_mesh = MeshData::new();
    let mut model_cache: HashMap<u32, Model> = HashMap::new();
    let mut placements = 0usize;

    for loc in &locations.locations {
        if placements >= model_limit {
            break;
        }
        let Some(data) = object_files.get(loc.id as usize) else {
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

        let tile_x = loc.x as f32;
        let tile_y = loc.y as f32;
        let height = terrain.height(loc.z as usize, loc.x as usize, loc.y as usize) as f32 * 8.0;
        let base = Vec3::new(
            (region_x as f32 * 64.0 + tile_x + 0.5) * 128.0,
            height,
            (region_y as f32 * 64.0 + tile_y + 0.5) * 128.0,
        );
        let angle = loc.rotation as f32 * std::f32::consts::FRAC_PI_2;
        append_model(&mut model_mesh, model, base, angle, &def);
        placements += 1;
    }

    let center = Vec3::new(
        (region_x as f32 * 64.0 + 32.0) * 128.0,
        terrain.height(0, 32, 32) as f32 * 8.0,
        (region_y as f32 * 64.0 + 32.0) * 128.0,
    );

    tracing::info!(
        "scene: {} terrain verts, {} model verts, {} placements, center {:?}",
        terrain_mesh.vertices.len(),
        model_mesh.vertices.len(),
        placements,
        center
    );

    Ok(SceneData {
        terrain: terrain_mesh,
        models: model_mesh,
        center,
        placements,
    })
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
    let files = cache.archive_files(2, 1)?;
    let mut out = HashMap::new();
    for (id, data) in files.iter().enumerate() {
        if let Ok(def) = UnderlayDefinition::parse(id as u32, data) {
            out.insert(id as u32, def);
        }
    }
    Ok(out)
}

fn load_overlays(cache: &Cache) -> Result<HashMap<u32, OverlayDefinition>> {
    let files = cache.archive_files(2, 4)?;
    let mut out = HashMap::new();
    for (id, data) in files.iter().enumerate() {
        if let Ok(def) = OverlayDefinition::parse(id as u32, data) {
            out.insert(id as u32, def);
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
            return overlay.color();
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

fn build_terrain(
    terrain: &Terrain,
    underlays: &HashMap<u32, UnderlayDefinition>,
    overlays: &HashMap<u32, OverlayDefinition>,
    mesh: &mut MeshData,
) {
    const N: usize = 65;
    let mut heights = [[0.0f32; N]; N];
    let mut colors = [[[0u8; 3]; N]; N];

    for y in 0..N {
        for x in 0..N {
            let mut sum = 0.0f32;
            let mut color = [0u32; 3];
            let mut count = 0.0f32;

            for dy in -1..=0 {
                for dx in -1..=0 {
                    let tx = x as isize + dx;
                    let ty = y as isize + dy;
                    if tx < 0 || ty < 0 || tx >= 64 || ty >= 64 {
                        continue;
                    }
                    let tile = terrain.tile(0, tx as usize, ty as usize);
                    sum += tile.height as f32;
                    let c = tile_color(terrain, underlays, overlays, tx as usize, ty as usize);
                    color[0] += c[0] as u32;
                    color[1] += c[1] as u32;
                    color[2] += c[2] as u32;
                    count += 1.0;
                }
            }

            heights[y][x] = if count > 0.0 { sum / count } else { 0.0 };
            colors[y][x] = if count > 0.0 {
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

    let base_x = terrain.region_x as f32 * 64.0 * 128.0;
    let base_z = terrain.region_y as f32 * 64.0 * 128.0;

    for y in 0..N {
        for x in 0..N {
            let h = heights[y][x];
            let shade = 0.58 + 0.24 * (h / 80.0).clamp(0.0, 1.0);
            let c = colors[y][x];
            let color = [
                (c[0] as f32 * shade * 0.82) as u8,
                (c[1] as f32 * shade * 0.82) as u8,
                (c[2] as f32 * shade * 0.82) as u8,
            ];
            mesh.vertices.push(Vertex::new(
                Vec3::new(
                    base_x + x as f32 * 128.0,
                    h * 8.0,
                    base_z + y as f32 * 128.0,
                ),
                color,
            ));
        }
    }

    for y in 0..64 {
        for x in 0..64 {
            let a = (y * N + x) as u32;
            let b = a + 1;
            let c = a + N as u32;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, c, b, b, c, d]);
        }
    }
}

fn append_model(
    mesh: &mut MeshData,
    model: &Model,
    base: Vec3,
    angle: f32,
    _def: &ObjectDefinition,
) {
    let rot = Mat3::from_rotation_y(angle);
    let light = Vec3::new(0.35, 0.85, 0.4).normalize();

    for (face_idx, &[a, b, c]) in model.faces.iter().enumerate() {
        let pa = model_vertex(model, a as usize, base, rot);
        let pb = model_vertex(model, b as usize, base, rot);
        let pc = model_vertex(model, c as usize, base, rot);

        let normal = (pb - pa).cross(pc - pa).normalize_or_zero();
        let ndotl = normal.dot(light).clamp(0.0, 1.0);
        let brightness = 0.62 + 0.22 * (1.0 - ndotl);

        let textured = model
            .face_textures
            .as_ref()
            .and_then(|t| t.get(face_idx))
            .is_some_and(|&t| t >= 0);
        let rgb = if textured {
            let n = model
                .face_normals
                .get(face_idx)
                .copied()
                .unwrap_or([0.0, 1.0, 0.0]);
            let leaf = [44.0 + n[1] * 20.0, 92.0 + n[1] * 30.0, 38.0];
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

fn model_vertex(model: &Model, i: usize, base: Vec3, rot: Mat3) -> Vec3 {
    let local = Vec3::new(
        model.vertex_x[i] as f32,
        -(model.vertex_y[i] as f32),
        model.vertex_z[i] as f32,
    );
    base + rot * local
}
