use std::collections::HashMap;

use crate::buffer::Buffer;
use crate::error::{Error, Result};
use crate::store::Cache;

#[derive(Debug, Clone, Default)]
pub struct Sequence {
    pub id: u32,
    pub frame_ids: Vec<u32>,
    pub frame_lengths: Vec<u16>,
    pub frame_step: u16,
    pub max_loops: u8,
    pub forced_priority: u8,
    pub priority: i32,
    pub stretches: bool,
    pub anim_maya_id: i32,
    pub vertical_offset: i8,
    pub debug_name: String,
}

impl Sequence {
    pub fn load(id: u32, data: &[u8]) -> Result<Self> {
        let mut def = Self {
            id,
            frame_step: 0,
            max_loops: 99,
            forced_priority: 5,
            priority: -1,
            anim_maya_id: -1,
            ..Default::default()
        };
        let mut b = Buffer::new(data);

        loop {
            let opcode = b.read_u8()?;
            if opcode == 0 {
                break;
            }
            match opcode {
                1 => {
                    let count = b.read_u16()? as usize;
                    def.frame_lengths = Vec::with_capacity(count);
                    for _ in 0..count {
                        def.frame_lengths.push(b.read_u16()?);
                    }
                    def.frame_ids = Vec::with_capacity(count);
                    for _ in 0..count {
                        def.frame_ids.push(b.read_u16()? as u32);
                    }
                    for i in 0..count {
                        def.frame_ids[i] |= (b.read_u16()? as u32) << 16;
                    }
                }
                2 => def.frame_step = b.read_u16()?,
                3 => {
                    let count = b.read_u8()? as usize;
                    for _ in 0..count {
                        b.read_u8()?;
                    }
                }
                4 => def.stretches = true,
                5 => def.forced_priority = b.read_u8()?,
                6 | 7 => {
                    b.read_u16()?;
                }
                8 => def.max_loops = b.read_u8()?,
                9 => {
                    b.read_u8()?;
                }
                10 => def.priority = b.read_u8()? as i32,
                11 => {
                    b.read_u8()?;
                }
                12 => {
                    let count = b.read_u8()? as usize;
                    for _ in 0..count {
                        b.read_u16()?;
                    }
                    for _ in 0..count {
                        b.read_u16()?;
                    }
                }
                13 => def.anim_maya_id = b.read_i32()?,
                14 => {
                    let count = b.read_u16()? as usize;
                    for _ in 0..count {
                        b.read_u16()?;
                        b.read_u16()?;
                        b.read_u8()?;
                        b.read_u8()?;
                        b.read_u8()?;
                        b.read_u8()?;
                    }
                }
                15 => {
                    b.read_u16()?;
                    b.read_u16()?;
                }
                16 => def.vertical_offset = b.read_i8()?,
                17 => {
                    let count = b.read_u8()? as usize;
                    for _ in 0..count {
                        b.read_u8()?;
                    }
                }
                18 => def.debug_name = b.read_string()?,
                19 => {}
                other => {
                    return Err(Error::parse(format!(
                        "sequence {id}: unknown opcode {other} at offset {}",
                        b.pos()
                    )));
                }
            }
        }

        Ok(def)
    }
}

#[derive(Debug, Clone, Default)]
pub struct AnimationClip {
    pub sequence: Sequence,
    pub frames: Vec<Frame>,
    pub framemaps: Vec<Framemap>,
}

impl AnimationClip {
    pub fn load(cache: &Cache, sequence_id: u32) -> Result<Self> {
        let sequence_data = cache.file(2, 12, sequence_id)?;
        let sequence = Sequence::load(sequence_id, &sequence_data)?;

        let mut frames = Vec::with_capacity(sequence.frame_ids.len());
        let mut framemaps = Vec::with_capacity(sequence.frame_ids.len());
        let mut framemap_cache: HashMap<u32, Framemap> = HashMap::new();

        for &frame_id in &sequence.frame_ids {
            let archive_id = frame_id >> 16;
            let file_id = frame_id & 0xffff;
            let frame_data = cache.file(0, archive_id, file_id)?;
            if frame_data.len() < 3 {
                return Err(Error::parse(format!(
                    "sequence {sequence_id}: frame {frame_id} too short"
                )));
            }
            let framemap_id = u16::from_be_bytes([frame_data[0], frame_data[1]]) as u32;
            let framemap = if let Some(framemap) = framemap_cache.get(&framemap_id) {
                framemap.clone()
            } else {
                let group = cache.group(1, framemap_id)?;
                let framemap = Framemap::load(framemap_id, &group.data)?;
                framemap_cache.insert(framemap_id, framemap.clone());
                framemap
            };
            let frame = Frame::load(file_id, &framemap, &frame_data)?;
            frames.push(frame);
            framemaps.push(framemap);
        }

        Ok(Self {
            sequence,
            frames,
            framemaps,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Framemap {
    pub id: u32,
    pub length: u8,
    pub types: Vec<u8>,
    pub frame_maps: Vec<Vec<u8>>,
}

impl Framemap {
    pub fn load(id: u32, data: &[u8]) -> Result<Self> {
        let mut b = Buffer::new(data);
        let length = b.read_u8()?;
        let mut types = Vec::with_capacity(length as usize);
        for _ in 0..length {
            types.push(b.read_u8()?);
        }
        let mut frame_maps = Vec::with_capacity(length as usize);
        for _ in 0..length {
            let count = b.read_u8()? as usize;
            frame_maps.push(vec![0u8; count]);
        }
        for map in &mut frame_maps {
            for value in map {
                *value = b.read_u8()?;
            }
        }
        Ok(Self {
            id,
            length,
            types,
            frame_maps,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub id: u32,
    pub framemap_id: u16,
    pub index_frame_ids: Vec<u16>,
    pub translator_x: Vec<i32>,
    pub translator_y: Vec<i32>,
    pub translator_z: Vec<i32>,
    pub showing: bool,
}

impl Frame {
    pub fn load(id: u32, framemap: &Framemap, data: &[u8]) -> Result<Self> {
        let mut header = Buffer::new(data);
        let framemap_id = header.read_u16()?;
        let length = header.read_u8()? as usize;

        if framemap_id as u32 != framemap.id {
            return Err(Error::parse(format!(
                "frame {id}: framemap id {framemap_id} does not match {}",
                framemap.id
            )));
        }
        if length > framemap.types.len() {
            return Err(Error::parse(format!(
                "frame {id}: length {length} exceeds framemap {} types {}",
                framemap.id,
                framemap.types.len()
            )));
        }

        let mut flags = Buffer::new(data);
        flags.set_pos(3);
        let mut transforms = Buffer::new(data);
        transforms.set_pos(3 + length);

        let mut index_frame_ids = Vec::with_capacity(length + 1);
        let mut tx = Vec::with_capacity(length + 1);
        let mut ty = Vec::with_capacity(length + 1);
        let mut tz = Vec::with_capacity(length + 1);
        let mut last_i: i32 = -1;
        let mut showing = false;

        for i in 0..length {
            let flags_value = flags.read_u8()?;
            if flags_value == 0 {
                continue;
            }

            if framemap.types[i] != 0 {
                let mut found = false;
                let mut j = i as i32 - 1;
                while j > last_i {
                    if framemap.types[j as usize] == 0 {
                        index_frame_ids.push(j as u16);
                        tx.push(0);
                        ty.push(0);
                        tz.push(0);
                        found = true;
                        break;
                    }
                    j -= 1;
                }
                let _ = found;
            }

            index_frame_ids.push(i as u16);
            let default = if framemap.types[i] == 3 { 128 } else { 0 };
            tx.push(if flags_value & 1 != 0 {
                transforms.read_short_smart()?
            } else {
                default
            });
            ty.push(if flags_value & 2 != 0 {
                transforms.read_short_smart()?
            } else {
                default
            });
            tz.push(if flags_value & 4 != 0 {
                transforms.read_short_smart()?
            } else {
                default
            });

            last_i = i as i32;
            if framemap.types[i] == 5 {
                showing = true;
            }
        }

        if transforms.pos() != data.len() {
            return Err(Error::parse(format!(
                "frame {id}: {} bytes left after transforms",
                data.len().saturating_sub(transforms.pos())
            )));
        }

        Ok(Self {
            id,
            framemap_id,
            index_frame_ids,
            translator_x: tx,
            translator_y: ty,
            translator_z: tz,
            showing,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_framemap_and_frame() {
        let framemap = Framemap::load(0, &[2, 0, 0, 1, 1, 0, 1]).unwrap();
        assert_eq!(framemap.types, vec![0, 0]);
        assert_eq!(framemap.frame_maps, vec![vec![0], vec![1]]);

        let frame = Frame::load(0, &framemap, &[0, 0, 2, 1, 1, 0x40, 0x40]).unwrap();
        assert_eq!(frame.framemap_id, 0);
        assert_eq!(frame.index_frame_ids, vec![0, 1]);
        assert_eq!(frame.translator_x, vec![0, 0]);
    }
}
