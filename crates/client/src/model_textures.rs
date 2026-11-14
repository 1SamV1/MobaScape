use anyhow::{Context, Result};
use image::imageops::FilterType;
use image::{Rgba, RgbaImage};
use rift_cache::{Cache, SpriteDefinition, SpriteFrame, TextureDefinition};

const SIZE: u32 = 128;
const MIPS: u32 = 8;
const LAYERS: u32 = 256;

/// Cache-native model textures packed into a mipmapped 2D texture array.
///
/// Texture IDs are archive file IDs from index 9; each definition points at a
/// sprite in index 8. Layers that the archive does not define contain a muted
/// procedural fallback so a stale ID cannot turn a whole prop white.
pub struct ModelTextures {
    pub width: u32,
    pub height: u32,
    pub layers: u32,
    pub mip_levels: u32,
    pub pixels: Vec<u8>,
}

impl ModelTextures {
    pub fn load(cache: &Cache) -> Result<Self> {
        let files = cache
            .archive_files_with_ids(9, 0)
            .context("loading texture definitions")?;
        let layer_stride = layer_stride();
        let mut pixels = vec![0u8; layer_stride * LAYERS as usize];
        let fallback = fallback_texture();
        for id in 0..LAYERS as usize {
            write_layer(&mut pixels, id, &fallback);
        }

        let mut loaded = 0usize;
        for (id, data) in &files {
            if *id >= LAYERS {
                continue;
            }
            let definition = TextureDefinition::load(*id, data)?;
            let group = cache
                .group(8, definition.sprite_id)
                .with_context(|| format!("loading texture sprite {}", definition.sprite_id))?;
            let sprite = SpriteDefinition::load(definition.sprite_id, &group.data)?;
            let Some(frame) = sprite.frames.first() else {
                continue;
            };
            let image = frame_image(frame);
            write_layer(&mut pixels, *id as usize, &image);
            loaded += 1;
        }

        tracing::info!(
            "model textures: {loaded} cached sprites + {} fallback layers",
            LAYERS as usize - loaded
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

fn layer_stride() -> usize {
    let mut total = 0usize;
    let mut size = SIZE;
    for _ in 0..MIPS {
        total += (size * size * 4) as usize;
        size = (size / 2).max(1);
    }
    total
}

fn write_layer(pixels: &mut [u8], index: usize, image: &RgbaImage) {
    let base = index * layer_stride();
    let mut offset = base;
    let mut level = resize_texture(image);
    for mip in 0..MIPS {
        let bytes = level.as_raw();
        pixels[offset..offset + bytes.len()].copy_from_slice(bytes);
        offset += bytes.len();
        if mip + 1 < MIPS {
            let w = (level.width() / 2).max(1);
            let h = (level.height() / 2).max(1);
            level = image::imageops::resize(&level, w, h, FilterType::Triangle);
        }
    }
}

fn resize_texture(source: &RgbaImage) -> RgbaImage {
    let mut image = if source.width() == SIZE && source.height() == SIZE {
        source.clone()
    } else {
        image::imageops::resize(source, SIZE, SIZE, FilterType::Triangle)
    };

    // Texture sprites are normally opaque. If a source has a transparent
    // border, flood it with the average texture colour so wrap sampling does
    // not produce black seams.
    let mut sum = [0u64; 3];
    let mut count = 0u64;
    for pixel in image.pixels() {
        if pixel[3] > 8 {
            sum[0] += pixel[0] as u64;
            sum[1] += pixel[1] as u64;
            sum[2] += pixel[2] as u64;
            count += 1;
        }
    }
    let divisor = count.max(1);
    let mut avg = [
        (sum[0] / divisor) as u8,
        (sum[1] / divisor) as u8,
        (sum[2] / divisor) as u8,
    ];
    if count == 0 {
        avg = [96, 104, 88];
    }
    for pixel in image.pixels_mut() {
        if pixel[3] <= 8 {
            // Keep the cut-out transparent but fill its RGB with the average
            // colour so filtered edges do not darken toward black.
            *pixel = Rgba([avg[0], avg[1], avg[2], 0]);
        }
    }
    image
}

fn frame_image(frame: &SpriteFrame) -> RgbaImage {
    if let Some(image) = RgbaImage::from_raw(frame.width, frame.height, frame.pixels.clone()) {
        image
    } else {
        fallback_texture()
    }
}

fn fallback_texture() -> RgbaImage {
    let mut image = RgbaImage::new(SIZE, SIZE);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let noise = hash(x, y) * 26.0 - 13.0;
            let checker = ((x / 32 + y / 32) % 2) as f32 * 8.0;
            image.put_pixel(
                x,
                y,
                Rgba([
                    (66.0 + noise + checker) as u8,
                    (84.0 + noise + checker) as u8,
                    (58.0 + noise + checker * 0.5) as u8,
                    255,
                ]),
            );
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
