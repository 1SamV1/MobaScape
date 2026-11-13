use crate::buffer::Buffer;
use crate::error::Result;

pub const REGION_SIZE: usize = 64;
pub const PLANES: usize = 4;
pub const TILES_PER_PLANE: usize = REGION_SIZE * REGION_SIZE;
pub const TOTAL_TILES: usize = PLANES * TILES_PER_PLANE;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tile {
    pub height: u8,
    pub overlay_id: i16,
    pub overlay_path: u8,
    pub overlay_rotation: u8,
    pub settings: u8,
    pub underlay_id: i16,
}

#[derive(Debug, Clone)]
pub struct Terrain {
    pub region_x: u32,
    pub region_y: u32,
    tiles: Vec<Tile>,
}

impl Terrain {
    pub fn load(region_x: u32, region_y: u32, data: &[u8]) -> Result<Self> {
        let mut b = Buffer::new(data);
        let mut tiles = vec![Tile::default(); TOTAL_TILES];

        for z in 0..PLANES {
            for x in 0..REGION_SIZE {
                for y in 0..REGION_SIZE {
                    let tile = &mut tiles[z * TILES_PER_PLANE + x * REGION_SIZE + y];

                    loop {
                        let attribute = b.read_u16()?;
                        match attribute {
                            0 => break,
                            1 => {
                                tile.height = b.read_u8()?;
                                break;
                            }
                            2..=49 => {
                                tile.overlay_id = b.read_i16()?;
                                tile.overlay_path = ((attribute - 2) / 4) as u8;
                                tile.overlay_rotation = ((attribute - 2) & 3) as u8;
                            }
                            50..=81 => {
                                tile.settings = (attribute - 49) as u8;
                            }
                            _ => {
                                tile.underlay_id = (attribute - 81) as i16;
                            }
                        }
                    }
                }
            }
        }

        Ok(Self {
            region_x,
            region_y,
            tiles,
        })
    }

    pub fn tile(&self, z: usize, x: usize, y: usize) -> &Tile {
        &self.tiles[z * TILES_PER_PLANE + x * REGION_SIZE + y]
    }

    pub fn height(&self, z: usize, x: usize, y: usize) -> u8 {
        self.tile(z, x, y).height
    }

    pub fn tiles(&self) -> &[Tile] {
        &self.tiles
    }

    pub fn world_x(&self, local_x: usize) -> i32 {
        (self.region_x as i32 * REGION_SIZE as i32 + local_x as i32) * 128
    }

    pub fn world_y(&self, local_y: usize) -> i32 {
        (self.region_y as i32 * REGION_SIZE as i32 + local_y as i32) * 128
    }

    /// Height in world units for a tile (OSRS tiles are 128 units tall per level).
    pub fn world_height(&self, z: usize, x: usize, y: usize) -> i32 {
        self.height(z, x, y) as i32 * 128
    }

    pub fn stats(&self) -> TerrainStats {
        let mut heights = [0usize; 256];
        let mut overlays = 0usize;
        let mut underlays = 0usize;
        let mut blocked = 0usize;

        for tile in &self.tiles {
            heights[tile.height as usize] += 1;
            if tile.overlay_id > 0 {
                overlays += 1;
            }
            if tile.underlay_id > 0 {
                underlays += 1;
            }
            if tile.settings & 0x01 != 0 {
                blocked += 1;
            }
        }

        TerrainStats {
            heights,
            overlays,
            underlays,
            blocked,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerrainStats {
    pub heights: [usize; 256],
    pub overlays: usize,
    pub underlays: usize,
    pub blocked: usize,
}
