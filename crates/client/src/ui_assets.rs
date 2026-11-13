use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use image::imageops::FilterType;

const ATLAS_SIZE: u32 = 1024;
const ITEM_SIZE: u32 = 128;

/// A small shelf-packed atlas of OSRS item icons and UI sprites.
///
/// Assets are loaded from the gitignored `assets/osrs-ui` tree. Missing files
/// are skipped; the HUD falls back to its procedural icons when a UV is absent.
pub struct UiAtlas {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub white_uv: [f32; 4],
    item_uv: HashMap<String, [f32; 4]>,
    ui_uv: HashMap<String, [f32; 4]>,
}

impl UiAtlas {
    pub fn load() -> Result<Self> {
        let mut atlas = Self {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            pixels: vec![0; (ATLAS_SIZE * ATLAS_SIZE * 4) as usize],
            white_uv: [0.0; 4],
            item_uv: HashMap::new(),
            ui_uv: HashMap::new(),
        };
        // A single opaque white texel at (0,0) lets every untextured HUD quad
        // use the same textured pipeline.
        atlas.pixels[0..4].copy_from_slice(&[255, 255, 255, 255]);
        atlas.white_uv = [
            0.5 / ATLAS_SIZE as f32,
            0.5 / ATLAS_SIZE as f32,
            0.5 / ATLAS_SIZE as f32,
            0.5 / ATLAS_SIZE as f32,
        ];

        let mut cursor_x = 2u32;
        let mut cursor_y = 2u32;
        let mut row_height = 0u32;

        for key in [
            "bronze_sword",
            "iron_plate",
            "fury",
            "berserker",
            "ahrims",
            "boots",
        ] {
            let path = format!("assets/osrs-ui/items/{key}.png");
            if let Some(image) = load_rgba(&path)? {
                let trimmed = trim_alpha(&image);
                let filter = if trimmed.width() > ITEM_SIZE || trimmed.height() > ITEM_SIZE {
                    FilterType::Lanczos3
                } else {
                    FilterType::Nearest
                };
                let icon = fit_icon(&trimmed, ITEM_SIZE, filter);
                if cursor_x + ITEM_SIZE + 2 > ATLAS_SIZE {
                    cursor_x = 2;
                    cursor_y += row_height + 2;
                    row_height = 0;
                }
                let uv = atlas.copy(&icon, cursor_x, cursor_y);
                atlas.item_uv.insert(key.to_string(), uv);
                cursor_x += ITEM_SIZE + 2;
                row_height = row_height.max(ITEM_SIZE);
            }
        }

        cursor_x = 2;
        cursor_y += row_height + 2;
        row_height = 0;
        for key in ["tab", "button", "button_red", "header"] {
            let path = format!("assets/osrs-ui/ui/{key}.png");
            if let Some(image) = load_rgba(&path)? {
                if cursor_x + image.width() + 2 > ATLAS_SIZE {
                    cursor_x = 2;
                    cursor_y += row_height + 2;
                    row_height = 0;
                }
                let uv = atlas.copy(&image, cursor_x, cursor_y);
                atlas.ui_uv.insert(key.to_string(), uv);
                cursor_x += image.width() + 2;
                row_height = row_height.max(image.height());
            }
        }

        tracing::info!(
            "UI atlas: {}x{}, {} item icons, {} UI sprites",
            atlas.width,
            atlas.height,
            atlas.item_uv.len(),
            atlas.ui_uv.len()
        );
        Ok(atlas)
    }

    fn copy(&mut self, image: &image::RgbaImage, x: u32, y: u32) -> [f32; 4] {
        let w = image.width();
        let h = image.height();
        for py in 0..h {
            for px in 0..w {
                let src = ((py * w + px) * 4) as usize;
                let dst = (((y + py) * self.width + (x + px)) * 4) as usize;
                self.pixels[dst..dst + 4].copy_from_slice(&image.as_raw()[src..src + 4]);
            }
        }
        let aw = self.width as f32;
        let ah = self.height as f32;
        [
            x as f32 / aw,
            y as f32 / ah,
            (x + w) as f32 / aw,
            (y + h) as f32 / ah,
        ]
    }

    pub fn item_uv(&self, id: &str) -> Option<[f32; 4]> {
        self.item_uv.get(id).copied()
    }

    pub fn ui_uv(&self, name: &str) -> Option<[f32; 4]> {
        self.ui_uv.get(name).copied()
    }
}

fn load_rgba(path: &str) -> Result<Option<image::RgbaImage>> {
    if !Path::new(path).exists() {
        return Ok(None);
    }
    let image = image::open(path).with_context(|| format!("loading UI asset {path}"))?;
    Ok(Some(image.to_rgba8()))
}

fn trim_alpha(image: &image::RgbaImage) -> image::RgbaImage {
    let mut min_x = image.width();
    let mut min_y = image.height();
    let mut max_x = 0;
    let mut max_y = 0;
    for (x, y, p) in image.enumerate_pixels() {
        if p[3] > 8 {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
        }
    }
    if min_x > max_x || min_y > max_y {
        return image.clone();
    }
    image::imageops::crop_imm(image, min_x, min_y, max_x - min_x + 1, max_y - min_y + 1).to_image()
}

fn fit_icon(image: &image::RgbaImage, size: u32, filter: FilterType) -> image::RgbaImage {
    let scale = (size as f32 / image.width() as f32).min(size as f32 / image.height() as f32);
    let w = (image.width() as f32 * scale).round().max(1.0) as u32;
    let h = (image.height() as f32 * scale).round().max(1.0) as u32;
    let resized = image::imageops::resize(image, w, h, filter);
    let mut canvas = image::RgbaImage::new(size, size);
    let ox = (size - w) / 2;
    let oy = (size - h) / 2;
    image::imageops::overlay(&mut canvas, &resized, ox as i64, oy as i64);
    canvas
}
