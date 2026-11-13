use crate::buffer::Buffer;
use crate::error::{Error, Result};

const FLAG_VERTICAL: u8 = 0x01;
const FLAG_ALPHA: u8 = 0x02;

/// One frame of a cache sprite. Pixels are RGBA8, row-major.
#[derive(Debug, Clone)]
pub struct SpriteFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// A sprite archive entry. Item icons and texture sprites live in index 8.
#[derive(Debug, Clone, Default)]
pub struct SpriteDefinition {
    pub id: u32,
    pub frames: Vec<SpriteFrame>,
}

impl SpriteDefinition {
    /// Decode a sprite group from index 8.
    ///
    /// Ported from the public osrscachereader `SpriteLoader` (BSD-2-Clause).
    /// The footer stores frame dimensions, offsets and a 24-bit palette;
    /// frame pixel/index data is read from the front of the buffer.
    pub fn load(id: u32, data: &[u8]) -> Result<Self> {
        if data.len() < 9 {
            return Err(Error::parse(format!("sprite {id}: too short")));
        }
        let mut footer = data.len() - 2;
        let sprite_count = u16::from_be_bytes([data[footer], data[footer + 1]]) as usize;
        if sprite_count == 0 || sprite_count > 1024 {
            return Err(Error::parse(format!(
                "sprite {id}: invalid frame count {sprite_count}"
            )));
        }
        let metadata_len = sprite_count
            .checked_mul(8)
            .and_then(|len| len.checked_add(7))
            .ok_or_else(|| Error::parse(format!("sprite {id}: metadata overflow")))?;
        if data.len() < metadata_len {
            return Err(Error::parse(format!("sprite {id}: metadata out of range")));
        }
        footer = data.len() - metadata_len;

        let mut b = Buffer::new(&data[footer..data.len() - 2]);
        let width = b.read_u16()? as u32;
        let height = b.read_u16()? as u32;
        let palette_len = b.read_u8()? as usize + 1;

        let mut offsets_x = Vec::with_capacity(sprite_count);
        let mut offsets_y = Vec::with_capacity(sprite_count);
        let mut widths = Vec::with_capacity(sprite_count);
        let mut heights = Vec::with_capacity(sprite_count);
        for _ in 0..sprite_count {
            offsets_x.push(b.read_u16()?);
        }
        for _ in 0..sprite_count {
            offsets_y.push(b.read_u16()?);
        }
        for _ in 0..sprite_count {
            widths.push(b.read_u16()? as u32);
        }
        for _ in 0..sprite_count {
            heights.push(b.read_u16()? as u32);
        }
        let _ = (width, height, offsets_x, offsets_y);

        let palette_bytes = (palette_len - 1)
            .checked_mul(3)
            .ok_or_else(|| Error::parse(format!("sprite {id}: palette overflow")))?;
        let palette_start = footer
            .checked_sub(palette_bytes)
            .ok_or_else(|| Error::parse(format!("sprite {id}: palette out of range")))?;
        let mut palette = vec![0u32; palette_len];
        let mut p = palette_start;
        for entry in palette.iter_mut().skip(1) {
            let rgb = ((data[p] as u32) << 16) | ((data[p + 1] as u32) << 8) | data[p + 2] as u32;
            *entry = if rgb == 0 { 1 } else { rgb };
            p += 3;
        }

        let mut cursor = 0usize;
        let mut frames = Vec::with_capacity(sprite_count);
        for frame_index in 0..sprite_count {
            let frame_width = widths[frame_index];
            let frame_height = heights[frame_index];
            let dimension = (frame_width as usize)
                .checked_mul(frame_height as usize)
                .ok_or_else(|| Error::parse(format!("sprite {id}: frame overflow")))?;
            if dimension == 0 {
                frames.push(SpriteFrame {
                    width: frame_width,
                    height: frame_height,
                    pixels: Vec::new(),
                });
                continue;
            }
            if cursor >= data.len() {
                return Err(Error::parse(format!(
                    "sprite {id}: frame flags out of range"
                )));
            }
            let flags = data[cursor];
            cursor += 1;

            let mut indices = vec![0u8; dimension];
            if flags & FLAG_VERTICAL == 0 {
                read_exact(data, &mut cursor, &mut indices, id)?;
            } else {
                for x in 0..frame_width as usize {
                    for y in 0..frame_height as usize {
                        indices[y * frame_width as usize + x] = data[cursor];
                        cursor += 1;
                    }
                }
            }

            let mut alphas = vec![0u8; dimension];
            if flags & FLAG_ALPHA != 0 {
                if flags & FLAG_VERTICAL == 0 {
                    read_exact(data, &mut cursor, &mut alphas, id)?;
                } else {
                    for x in 0..frame_width as usize {
                        for y in 0..frame_height as usize {
                            alphas[y * frame_width as usize + x] = data[cursor];
                            cursor += 1;
                        }
                    }
                }
            } else {
                for (index, alpha) in indices.iter().zip(&mut alphas) {
                    if *index != 0 {
                        *alpha = 0xff;
                    }
                }
            }

            let mut pixels = Vec::with_capacity(dimension * 4);
            for (&palette_index, &alpha) in indices.iter().zip(&alphas) {
                let rgb = palette.get(palette_index as usize).copied().unwrap_or(0);
                pixels.extend_from_slice(&[
                    ((rgb >> 16) & 0xff) as u8,
                    ((rgb >> 8) & 0xff) as u8,
                    (rgb & 0xff) as u8,
                    if palette_index == 0 { 0 } else { alpha },
                ]);
            }
            frames.push(SpriteFrame {
                width: frame_width,
                height: frame_height,
                pixels,
            });
        }

        Ok(Self { id, frames })
    }
}

fn read_exact(data: &[u8], cursor: &mut usize, out: &mut [u8], id: u32) -> Result<()> {
    let end = cursor
        .checked_add(out.len())
        .ok_or_else(|| Error::parse(format!("sprite {id}: pixel range overflow")))?;
    let bytes = data
        .get(*cursor..end)
        .ok_or_else(|| Error::parse(format!("sprite {id}: pixel data out of range")))?;
    out.copy_from_slice(bytes);
    *cursor = end;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_truncated_sprite() {
        assert!(SpriteDefinition::load(1, &[0, 0]).is_err());
    }
}
