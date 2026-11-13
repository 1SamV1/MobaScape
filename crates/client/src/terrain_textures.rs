use std::path::Path;

use anyhow::{Context, Result};
use image::RgbaImage;
use image::imageops::FilterType;

const SIZE: u32 = 128;
const MIPS: u32 = 8;
const LAYERS: u32 = 6;

/// Cache-native OSRS ground textures packed as a mipmapped 2D array.
///
/// Layer order: grass, dirt, stone, sand, water, path.
pub struct TerrainTextures {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub mip_levels: u32,
    pub pixels: Vec<u8>,
}

impl TerrainTextures {
    pub fn load() -> Result<Self> {
        let names = ["grass", "dirt", "stone", "sand", "water", "path"];
        let mut pixels = Vec::new();
        for (index, name) in names.iter().enumerate() {
            let path = format!("assets/osrs-ground/{name}.png");
            let image = if Path::new(&path).exists() {
                image::open(&path)
                    .with_context(|| format!("loading ground texture {path}"))?
                    .to_rgba8()
            } else {
                fallback_texture(index)
            };
            let image = if image.width() == SIZE && image.height() == SIZE {
                image
            } else {
                image::imageops::resize(&image, SIZE, SIZE, FilterType::Triangle)
            };
            let mut level = image;
            pixels.extend_from_slice(level.as_raw());
            for _ in 1..MIPS {
                let w = (level.width() / 2).max(1);
                let h = (level.height() / 2).max(1);
                level = image::imageops::resize(&level, w, h, FilterType::Triangle);
                pixels.extend_from_slice(level.as_raw());
            }
        }
        tracing::info!(
            "ground textures: {} layers, {} mips, {} bytes",
            LAYERS,
            MIPS,
            pixels.len()
        );
        Ok(Self {
            width: SIZE,
            height: SIZE,
            layers: LAYERS,
            mip_levels: MIPS,
            pixels,
        })
    }
}

fn fallback_texture(layer: usize) -> RgbaImage {
    let (base, variance) = match layer {
        0 => ([92, 132, 58], 26),
        1 => ([112, 82, 52], 28),
        2 => ([118, 118, 112], 24),
        3 => ([176, 158, 112], 22),
        4 => ([52, 96, 142], 20),
        _ => ([138, 126, 106], 24),
    };
    let mut image = RgbaImage::new(SIZE, SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let n = hash(x, y) * 2.0 - 1.0;
            let c = [
                (base[0] as f32 + n * variance as f32).clamp(0.0, 255.0) as u8,
                (base[1] as f32 + n * variance as f32).clamp(0.0, 255.0) as u8,
                (base[2] as f32 + n * variance as f32).clamp(0.0, 255.0) as u8,
                255,
            ];
            image.put_pixel(x, y, image::Rgba(c));
        }
    }
    image
}

fn hash(x: u32, y: u32) -> f32 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(y.wrapping_mul(668_265_263));
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    ((h ^ (h >> 16)) & 255) as f32 / 255.0
}
