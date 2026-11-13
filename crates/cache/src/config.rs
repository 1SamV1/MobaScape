use std::collections::HashMap;

use crate::buffer::Buffer;
use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum ParamValue {
    Int(i64),
    Str(String),
}

#[derive(Debug, Clone, Default)]
pub struct UnderlayDefinition {
    pub id: u32,
    pub rgb: u32,
}

impl UnderlayDefinition {
    pub fn parse(id: u32, data: &[u8]) -> Result<Self> {
        let mut def = Self { id, rgb: 0 };
        let mut b = Buffer::new(data);
        loop {
            match b.read_u8()? {
                0 => break,
                1 => def.rgb = b.read_u24()?,
                other => {
                    return Err(Error::parse(format!(
                        "underlay {id}: unknown opcode {other}"
                    )));
                }
            }
        }
        Ok(def)
    }

    pub fn color(&self) -> [u8; 3] {
        [
            ((self.rgb >> 16) & 0xff) as u8,
            ((self.rgb >> 8) & 0xff) as u8,
            (self.rgb & 0xff) as u8,
        ]
    }
}

#[derive(Debug, Clone, Default)]
pub struct OverlayDefinition {
    pub id: u32,
    pub rgb: u32,
    pub secondary_rgb: Option<u32>,
    pub texture: u8,
    pub hide_underlay: bool,
}

impl OverlayDefinition {
    pub fn parse(id: u32, data: &[u8]) -> Result<Self> {
        let mut def = Self {
            id,
            hide_underlay: true,
            ..Default::default()
        };
        let mut b = Buffer::new(data);
        loop {
            match b.read_u8()? {
                0 => break,
                1 => def.rgb = b.read_u24()?,
                2 => def.texture = b.read_u8()?,
                5 => def.hide_underlay = false,
                7 => def.secondary_rgb = Some(b.read_u24()?),
                other => {
                    return Err(Error::parse(format!(
                        "overlay {id}: unknown opcode {other}"
                    )));
                }
            }
        }
        Ok(def)
    }

    pub fn color(&self) -> [u8; 3] {
        [
            ((self.rgb >> 16) & 0xff) as u8,
            ((self.rgb >> 8) & 0xff) as u8,
            (self.rgb & 0xff) as u8,
        ]
    }
}

#[derive(Debug, Clone, Default)]
pub struct ObjectDefinition {
    pub id: u32,
    pub name: String,
    pub models: Vec<u32>,
    pub model_types: Vec<u8>,
    pub size_x: u8,
    pub size_y: u8,
    pub animation_id: i32,
    pub wall_or_door: i32,
    pub contoured_ground: i32,
    pub merge_normals: bool,
    pub model_clipped: bool,
    pub ambient: i8,
    pub contrast: i16,
    pub recolor_to_find: Vec<i16>,
    pub recolor_to_replace: Vec<i16>,
    pub retexture_to_find: Vec<i16>,
    pub texture_to_replace: Vec<i16>,
    pub model_size_x: u16,
    pub model_size_height: u16,
    pub model_size_y: u16,
    pub offset_x: u16,
    pub offset_height: u16,
    pub offset_y: u16,
    pub obstructs_ground: bool,
    pub hollow: bool,
    pub supports_items: i32,
    pub interact_type: i32,
    pub blocks_projectile: bool,
    pub map_scene_id: u16,
    pub blocking_mask: i8,
    pub raise: u8,
    pub ops: [Option<String>; 5],
    pub params: HashMap<u32, ParamValue>,
}

impl ObjectDefinition {
    pub fn parse(id: u32, data: &[u8]) -> Result<Self> {
        let mut def = Self {
            id,
            animation_id: -1,
            wall_or_door: -1,
            supports_items: -1,
            blocks_projectile: true,
            size_x: 1,
            size_y: 1,
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
                    let count = b.read_u8()? as usize;
                    def.models.clear();
                    def.model_types.clear();
                    for _ in 0..count {
                        def.models.push(b.read_u16()? as u32);
                        def.model_types.push(b.read_u8()?);
                    }
                }
                2 => def.name = b.read_string()?,
                5 => {
                    let count = b.read_u8()? as usize;
                    def.models.clear();
                    def.model_types.clear();
                    for _ in 0..count {
                        def.models.push(b.read_u16()? as u32);
                    }
                }
                6 => {
                    let count = b.read_u8()? as usize;
                    def.models.clear();
                    def.model_types.clear();
                    for _ in 0..count {
                        def.models.push(b.read_i32()? as u32);
                        def.model_types.push(b.read_u8()?);
                    }
                }
                7 => {
                    let count = b.read_u8()? as usize;
                    def.models.clear();
                    def.model_types.clear();
                    for _ in 0..count {
                        def.models.push(b.read_i32()? as u32);
                    }
                }
                14 => def.size_x = b.read_u8()?,
                15 => def.size_y = b.read_u8()?,
                17 => {
                    def.interact_type = 0;
                    def.blocks_projectile = false;
                }
                18 => def.blocks_projectile = false,
                19 => def.wall_or_door = b.read_u8()? as i32,
                21 => def.contoured_ground = 0,
                22 => def.merge_normals = true,
                23 => def.model_clipped = true,
                24 => {
                    let v = b.read_u16()?;
                    def.animation_id = if v == 0xffff { -1 } else { v as i32 };
                }
                27 => def.interact_type = 1,
                28 => {
                    b.read_u8()?;
                }
                29 => def.ambient = b.read_i8()?,
                30..=34 => {
                    let idx = (opcode - 30) as usize;
                    let text = b.read_string()?;
                    if !text.eq_ignore_ascii_case("hidden") {
                        def.ops[idx] = Some(text);
                    }
                }
                39 => def.contrast = b.read_i8()? as i16 * 25,
                40 => {
                    let count = b.read_u8()? as usize;
                    def.recolor_to_find.clear();
                    def.recolor_to_replace.clear();
                    for _ in 0..count {
                        def.recolor_to_find.push(b.read_i16()?);
                        def.recolor_to_replace.push(b.read_i16()?);
                    }
                }
                41 => {
                    let count = b.read_u8()? as usize;
                    def.retexture_to_find.clear();
                    def.texture_to_replace.clear();
                    for _ in 0..count {
                        def.retexture_to_find.push(b.read_i16()?);
                        def.texture_to_replace.push(b.read_i16()?);
                    }
                }
                42 => {
                    b.read_u16()?;
                }
                61 => {
                    b.read_u16()?;
                }
                62 => {}
                64 => {}
                65 => def.model_size_x = b.read_u16()?,
                66 => def.model_size_height = b.read_u16()?,
                67 => def.model_size_y = b.read_u16()?,
                68 => def.map_scene_id = b.read_u16()?,
                69 => def.blocking_mask = b.read_i8()?,
                70 => def.offset_x = b.read_u16()?,
                71 => def.offset_height = b.read_u16()?,
                72 => def.offset_y = b.read_u16()?,
                73 => def.obstructs_ground = true,
                74 => def.hollow = true,
                75 => def.supports_items = b.read_u8()? as i32,
                77 | 92 => {
                    let _varbit = b.read_u16()?;
                    let _varp = b.read_u16()?;
                    if opcode == 92 {
                        let _var = b.read_u16()?;
                    }
                    let count = b.read_u8()? as usize;
                    for _ in 0..=count {
                        let _ = b.read_u16()?;
                    }
                }
                78 => {
                    let _sound = b.read_u16()?;
                    let _distance = b.read_u8()?;
                    let _retain = b.read_u8()?;
                }
                79 => {
                    let _min = b.read_u16()?;
                    let _max = b.read_u16()?;
                    let _distance = b.read_u8()?;
                    let _retain = b.read_u8()?;
                    let count = b.read_u8()? as usize;
                    for _ in 0..count {
                        let _ = b.read_u16()?;
                    }
                }
                81 => def.contoured_ground = b.read_u8()? as i32 * 256,
                82 => {
                    b.read_u16()?;
                }
                89 | 90 | 94 => {}
                91 => {
                    b.read_u8()?;
                }
                93 => {
                    let _in_curve = b.read_u8()?;
                    let _in_duration = b.read_u16()?;
                    let _out_curve = b.read_u8()?;
                    let _out_duration = b.read_u16()?;
                }
                95 => {
                    b.read_u8()?;
                }
                96 => def.raise = b.read_u8()?,
                97 | 98 => {
                    let _varbit = b.read_u16()?;
                    let _varp = b.read_u16()?;
                    if opcode == 98 {
                        let _var = b.read_i24()?;
                    }
                    let count = b.read_u8()? as usize;
                    for _ in 0..=count {
                        let _ = b.read_i24()?;
                    }
                }
                100 => {
                    let _index = b.read_u8()?;
                    let _sub_id = b.read_u8()?;
                    let _text = b.read_string()?;
                }
                101 => {
                    let _index = b.read_u8()?;
                    let _varp = b.read_u16()?;
                    let _varb = b.read_u16()?;
                    let _min = b.read_i32()?;
                    let _max = b.read_i32()?;
                    let _text = b.read_string()?;
                }
                102 => {
                    let _index = b.read_u8()?;
                    let _sub_id = b.read_u16()?;
                    let _varp = b.read_u16()?;
                    let _varb = b.read_u16()?;
                    let _min = b.read_i32()?;
                    let _max = b.read_i32()?;
                    let _text = b.read_string()?;
                }
                249 => {
                    let count = b.read_u8()? as usize;
                    for _ in 0..count {
                        let kind = b.read_u8()?;
                        let key = b.read_u24()?;
                        let value = match kind {
                            1 => ParamValue::Str(b.read_string()?),
                            2 => ParamValue::Int(b.read_u64()? as i64),
                            _ => ParamValue::Int(b.read_i32()? as i64),
                        };
                        def.params.insert(key, value);
                    }
                }
                other => {
                    return Err(Error::parse(format!(
                        "object {}: unknown opcode {} at offset {}",
                        id,
                        other,
                        b.pos()
                    )));
                }
            }
        }

        if def.wall_or_door == -1 {
            def.wall_or_door =
                if def.models.is_empty() || def.model_types.first().copied().unwrap_or(10) == 10 {
                    1
                } else {
                    0
                };
        }
        if def.supports_items == -1 {
            def.supports_items = if def.interact_type != 0 { 1 } else { 0 };
        }

        Ok(def)
    }

    pub fn first_model(&self) -> Option<u32> {
        self.models.first().copied()
    }
}
