use anyhow::{Context, Result};
use glam::{Mat3, Vec3};
use image::{Rgba, RgbaImage};
use rift_cache::{AnimationClip, Cache, Model};

/// Final portrait size uploaded to the UI atlas.
pub const PORTRAIT_SIZE: u32 = 256;
const RENDER_SIZE: u32 = PORTRAIT_SIZE * 2;

/// Bake a front three-quarter portrait from a champion's idle animation.
///
/// The CPU rasteriser is deliberately tiny: flat-shaded cache faces, a
/// z-buffer and an orthographic camera. It keeps the hero-select screen
/// cache-native without adding a second render target or a screenshot
/// dependency.
pub fn bake_portrait(cache: &Cache, model_id: u32, idle_sequence: u32) -> Result<RgbaImage> {
    let clip = AnimationClip::load(cache, idle_sequence)
        .with_context(|| format!("loading champion idle sequence {idle_sequence}"))?;
    bake_model_with_clip(cache, model_id, &clip)
}

fn bake_model_with_clip(cache: &Cache, model_id: u32, clip: &AnimationClip) -> Result<RgbaImage> {
    let group = cache
        .group(7, model_id)
        .with_context(|| format!("loading model {model_id}"))?;
    let model = Model::load(model_id, &group.data)?;
    let positions = model.animated_positions(clip, 0.0);
    if positions.is_empty() || model.faces.is_empty() {
        anyhow::bail!("model {model_id} has no geometry");
    }

    // Cache-space Y points down; engine-space Y points up.
    let points = positions
        .iter()
        .map(|p| Vec3::new(p[0], -p[1], p[2]))
        .collect::<Vec<_>>();

    let mut min = Vec3::splat(f32::MAX);
    let mut max = Vec3::splat(f32::MIN);
    for point in &points {
        min = min.min(*point);
        max = max.max(*point);
    }
    let center = (min + max) * 0.5;

    // A slight yaw and downward pitch reads as a character select render
    // rather than a flat orthographic silhouette.
    let rot = Mat3::from_rotation_y(0.42) * Mat3::from_rotation_x(0.12);
    let rotated = points
        .iter()
        .map(|point| rot * (*point - center))
        .collect::<Vec<_>>();

    let mut rmin = Vec3::splat(f32::MAX);
    let mut rmax = Vec3::splat(f32::MIN);
    for point in &rotated {
        rmin = rmin.min(*point);
        rmax = rmax.max(*point);
    }
    let extent = (rmax - rmin).max(Vec3::splat(1.0));
    let scale = ((RENDER_SIZE as f32 * 0.84) / extent.x)
        .min((RENDER_SIZE as f32 * 0.88) / extent.y)
        .max(0.01);
    let mid = (rmin + rmax) * 0.5;
    let offset = Vec3::new(RENDER_SIZE as f32 * 0.5, RENDER_SIZE as f32 * 0.53, 0.0);

    let project = |point: Vec3| {
        let p = point - mid;
        Vec3::new(offset.x + p.x * scale, offset.y - p.y * scale, p.z * scale)
    };
    let screen = rotated
        .iter()
        .map(|point| project(*point))
        .collect::<Vec<_>>();

    let mut image = RgbaImage::new(RENDER_SIZE, RENDER_SIZE);
    paint_ground_shadow(&mut image, &screen);

    let mut depth = vec![f32::NEG_INFINITY; (RENDER_SIZE * RENDER_SIZE) as usize];
    let light = Vec3::new(0.42, 0.82, 0.48).normalize();

    for (face_index, &[a, b, c]) in model.faces.iter().enumerate() {
        let pa = screen[a as usize];
        let pb = screen[b as usize];
        let pc = screen[c as usize];
        let area = edge(pa, pb, pc);
        if area.abs() < 1e-5 {
            continue;
        }

        let mut normal = (pb - pa).cross(pc - pa).normalize_or_zero();
        if normal.length_squared() < 1e-6 {
            normal = rot
                * Vec3::from_array(
                    model
                        .face_normals
                        .get(face_index)
                        .copied()
                        .unwrap_or([0.0, 1.0, 0.0]),
                );
        }
        // The portrait camera looks down -Z, so flip inverted OSRS faces
        // toward it before shading.
        if normal.z < 0.0 {
            normal = -normal;
        }
        let diffuse = normal.dot(light).max(0.0);
        let rim = (1.0 - normal.z.abs()).powf(2.0) * 0.20;
        let shade = (0.58 + diffuse * 0.62 + rim).clamp(0.46, 1.28);
        let base = portrait_face_color(&model, face_index);
        let color = Rgba([
            (base[0] as f32 * shade).clamp(0.0, 255.0) as u8,
            (base[1] as f32 * shade).clamp(0.0, 255.0) as u8,
            (base[2] as f32 * shade).clamp(0.0, 255.0) as u8,
            255,
        ]);

        let min_x = pa.x.min(pb.x).min(pc.x).floor().max(0.0) as u32;
        let max_x =
            pa.x.max(pb.x)
                .max(pc.x)
                .ceil()
                .min(RENDER_SIZE as f32 - 1.0) as u32;
        let min_y = pa.y.min(pb.y).min(pc.y).floor().max(0.0) as u32;
        let max_y =
            pa.y.max(pb.y)
                .max(pc.y)
                .ceil()
                .min(RENDER_SIZE as f32 - 1.0) as u32;
        let inv_area = 1.0 / area;

        for py in min_y..=max_y {
            for px in min_x..=max_x {
                let p = Vec3::new(px as f32 + 0.5, py as f32 + 0.5, 0.0);
                let w0 = edge(pb, pc, p) * inv_area;
                let w1 = edge(pc, pa, p) * inv_area;
                let w2 = edge(pa, pb, p) * inv_area;
                if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                    continue;
                }
                let z = w0 * pa.z + w1 * pb.z + w2 * pc.z;
                let index = (py * RENDER_SIZE + px) as usize;
                if z <= depth[index] {
                    continue;
                }
                depth[index] = z;
                image.put_pixel(px, py, color);
            }
        }
    }

    Ok(downsample(&image))
}

fn portrait_face_color(model: &Model, face: usize) -> [u8; 3] {
    let textured = model
        .face_textures
        .as_ref()
        .and_then(|textures| textures.get(face))
        .is_some_and(|&texture| texture >= 0);
    if textured || model.face_colors.get(face).copied().unwrap_or(0) == 127 {
        return [126, 142, 110];
    }
    let rgb = model.face_rgb(face, 1.0);
    // OSRS palette colours are very dark; lift them so a portrait still
    // reads under the select screen's dimmer.
    let peak = rgb[0].max(rgb[1]).max(rgb[2]) as f32;
    if peak < 84.0 {
        let lift = 84.0 / peak.max(1.0);
        [
            (rgb[0] as f32 * lift).min(255.0) as u8,
            (rgb[1] as f32 * lift).min(255.0) as u8,
            (rgb[2] as f32 * lift).min(255.0) as u8,
        ]
    } else {
        rgb
    }
}

fn paint_ground_shadow(image: &mut RgbaImage, screen: &[Vec3]) {
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    for point in screen {
        min_x = min_x.min(point.x);
        max_x = max_x.max(point.x);
        max_y = max_y.max(point.y);
    }
    let cx = (min_x + max_x) * 0.5;
    let rx = ((max_x - min_x) * 0.42).max(30.0);
    let ry = (rx * 0.16).clamp(10.0, 26.0);
    let cy = (max_y - ry * 0.4).min(RENDER_SIZE as f32 - 4.0);
    let x0 = (cx - rx).floor().max(0.0) as u32;
    let x1 = (cx + rx).ceil().min(RENDER_SIZE as f32 - 1.0) as u32;
    let y0 = (cy - ry).floor().max(0.0) as u32;
    let y1 = (cy + ry).ceil().min(RENDER_SIZE as f32 - 1.0) as u32;
    for py in y0..=y1 {
        for px in x0..=x1 {
            let dx = (px as f32 + 0.5 - cx) / rx;
            let dy = (py as f32 + 0.5 - cy) / ry;
            let d = dx * dx + dy * dy;
            if d >= 1.0 {
                continue;
            }
            let alpha = (1.0 - d).powf(1.6) * 0.38;
            let pixel = image.get_pixel_mut(px, py);
            *pixel = Rgba([0, 0, 0, (alpha * 255.0) as u8]);
        }
    }
}

/// Resample 2x with premultiplied alpha so the silhouette edges stay clean.
fn downsample(source: &RgbaImage) -> RgbaImage {
    let mut out = RgbaImage::new(PORTRAIT_SIZE, PORTRAIT_SIZE);
    for y in 0..PORTRAIT_SIZE {
        for x in 0..PORTRAIT_SIZE {
            let mut rgb = [0.0f32; 3];
            let mut alpha = 0.0f32;
            for sy in 0..2 {
                for sx in 0..2 {
                    let pixel = source.get_pixel(x * 2 + sx, y * 2 + sy);
                    let a = pixel[3] as f32 / 255.0;
                    rgb[0] += pixel[0] as f32 * a;
                    rgb[1] += pixel[1] as f32 * a;
                    rgb[2] += pixel[2] as f32 * a;
                    alpha += a;
                }
            }
            let inv = if alpha > 0.0001 { 1.0 / alpha } else { 0.0 };
            out.put_pixel(
                x,
                y,
                Rgba([
                    (rgb[0] * inv).clamp(0.0, 255.0) as u8,
                    (rgb[1] * inv).clamp(0.0, 255.0) as u8,
                    (rgb[2] * inv).clamp(0.0, 255.0) as u8,
                    (alpha * 63.75).clamp(0.0, 255.0) as u8,
                ]),
            );
        }
    }
    out
}

/// Twice the signed area of the triangle `(a, b, p)`.
fn edge(a: Vec3, b: Vec3, p: Vec3) -> f32 {
    (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_sign_matches_winding() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(1.0, 0.0, 0.0);
        let c = Vec3::new(0.0, 1.0, 0.0);
        assert!(edge(a, b, c) > 0.0);
        assert!(edge(a, c, b) < 0.0);
        assert_eq!(edge(a, b, a), 0.0);
    }

    #[test]
    fn portrait_face_colour_lifts_dark_palette_values() {
        let model = Model {
            id: 0,
            vertex_count: 0,
            face_count: 0,
            num_texture_faces: 0,
            vertex_x: Vec::new(),
            vertex_y: Vec::new(),
            vertex_z: Vec::new(),
            faces: Vec::new(),
            face_colors: vec![0],
            face_render_types: Vec::new(),
            face_textures: None,
            texture_coords: None,
            face_normals: Vec::new(),
            vertex_groups: None,
            priority: 0,
        };
        let rgb = portrait_face_color(&model, 0);
        assert!(rgb.iter().any(|&channel| channel > 0));
        assert!(rgb[0].max(rgb[1]).max(rgb[2]) >= 84);
    }
}
