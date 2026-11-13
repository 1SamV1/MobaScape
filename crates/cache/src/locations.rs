use crate::buffer::Buffer;
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Location {
    pub id: u32,
    pub shape: u8,
    pub rotation: u8,
    pub x: u8,
    pub y: u8,
    pub z: u8,
}

#[derive(Debug, Clone, Default)]
pub struct Locations {
    pub region_x: u32,
    pub region_y: u32,
    pub locations: Vec<Location>,
}

impl Locations {
    pub fn load(region_x: u32, region_y: u32, data: &[u8]) -> Result<Self> {
        let mut b = Buffer::new(data);
        let mut locations = Vec::new();
        let mut id: i64 = -1;

        loop {
            let id_offset = b.read_unsigned_int_smart_short_compat()?;
            if id_offset == 0 {
                break;
            }
            id += id_offset as i64;

            let mut position: i32 = 0;
            loop {
                let position_offset = b.read_unsigned_short_smart()? as i32;
                if position_offset == 0 {
                    break;
                }
                position += position_offset - 1;

                let local_y = (position & 0x3f) as u8;
                let local_x = ((position >> 6) & 0x3f) as u8;
                let z = ((position >> 12) & 0x3) as u8;

                let attributes = b.read_u8()?;
                let shape = attributes >> 2;
                let rotation = attributes & 0x3;

                locations.push(Location {
                    id: id as u32,
                    shape,
                    rotation,
                    x: local_x,
                    y: local_y,
                    z,
                });
            }
        }

        Ok(Self {
            region_x,
            region_y,
            locations,
        })
    }

    pub fn counts_by_id(&self) -> std::collections::HashMap<u32, usize> {
        let mut counts = std::collections::HashMap::new();
        for loc in &self.locations {
            *counts.entry(loc.id).or_insert(0) += 1;
        }
        counts
    }
}
