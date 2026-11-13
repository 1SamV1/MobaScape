//! Bakes a top-down terrain image for the HUD minimap.
//!
//! The minimap is a real map of the world: it bins the terrain mesh colours
//! (which already contain the biome pockets, the river and the baked lanes)
//! into a small RGBA image that the UI atlas uploads once at startup.

use glam::Vec2;
use image::{Rgba, RgbaImage};

use crate::scene::SceneData;

/// Edge length of the baked minimap texture in pixels.
pub const MINIMAP_SIZE: u32 = 256;

fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Rasterise the terrain into a minimap image covering `bounds`.
pub fn render(scene: &SceneData, bounds: (Vec2, Vec2), size: u32) -> RgbaImage {
    let mut image = RgbaImage::from_pixel(size, size, Rgba([8, 11, 14, 255]));
    let mut accum = vec![[0.0f32; 3]; (size * size) as usize];
    let mut count = vec![0u32; (size * size) as usize];

    let (min, max) = bounds;
    let span_x = (max.x - min.x).max(1.0);
    let span_z = (max.y - min.y).max(1.0);

    for vertex in &scene.terrain.vertices {
        let nx = (vertex.position[0] - min.x) / span_x;
        let nz = (vertex.position[2] - min.y) / span_z;
        if !(0.0..1.0).contains(&nx) || !(0.0..1.0).contains(&nz) {
            continue;
        }
        let px = (nx * (size - 1) as f32).round() as i32;
        // North (small z) is up, matching the world camera.
        let py = (nz * (size - 1) as f32).round() as i32;
        // Splat into a 3x3 neighbourhood: the terrain vertices are sparser
        // than the minimap pixels, so point binning would leave a grid of
        // unfilled background pixels.
        for dy in -1..=1i32 {
            for dx in -1..=1i32 {
                let x = px + dx;
                let y = py + dy;
                if x < 0 || y < 0 || x >= size as i32 || y >= size as i32 {
                    continue;
                }
                let index = y as usize * size as usize + x as usize;
                accum[index][0] += vertex.color[0];
                accum[index][1] += vertex.color[1];
                accum[index][2] += vertex.color[2];
                count[index] += 1;
            }
        }
    }

    for y in 0..size {
        for x in 0..size {
            let index = (y * size + x) as usize;
            if count[index] == 0 {
                continue;
            }
            let n = count[index] as f32;
            let r = linear_to_srgb((accum[index][0] / n * 1.35).min(1.0));
            let g = linear_to_srgb((accum[index][1] / n * 1.35).min(1.0));
            let b = linear_to_srgb((accum[index][2] / n * 1.35).min(1.0));
            image.put_pixel(
                x,
                y,
                Rgba([(r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8, 255]),
            );
        }
    }

    // A subtle vignette keeps the map readable under the HUD frame.
    for y in 0..size {
        for x in 0..size {
            let nx = x as f32 / (size - 1) as f32 * 2.0 - 1.0;
            let ny = y as f32 / (size - 1) as f32 * 2.0 - 1.0;
            let d = (nx * nx + ny * ny).sqrt();
            let shade = 1.0 - (d * 0.28).clamp(0.0, 0.35);
            let p = image.get_pixel_mut(x, y);
            p.0[0] = (p.0[0] as f32 * shade) as u8;
            p.0[1] = (p.0[1] as f32 * shade) as u8;
            p.0[2] = (p.0[2] as f32 * shade) as u8;
        }
    }

    image
}
