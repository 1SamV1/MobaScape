use crate::error::{Error, Result};

/// A texture definition from archive 9.
///
/// In the pinned 235 cache each definition is a compact seven-byte record.
/// The first big-endian u16 is the sprite archive in index 8 that holds the
/// actual pixels. The remaining fields drive material animation and are kept
/// for future use.
#[derive(Debug, Clone)]
pub struct TextureDefinition {
    pub id: u32,
    pub sprite_id: u32,
    pub field1777: u8,
    pub field1778: u16,
    pub animation_direction: u8,
    pub animation_speed: u8,
}

impl TextureDefinition {
    pub fn load(id: u32, data: &[u8]) -> Result<Self> {
        if data.len() < 7 {
            return Err(Error::parse(format!("texture {id}: too short")));
        }
        Ok(Self {
            id,
            sprite_id: u16::from_be_bytes([data[0], data[1]]) as u32,
            field1777: data[2],
            field1778: u16::from_be_bytes([data[3], data[4]]),
            animation_direction: data[5],
            animation_speed: data[6],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sprite_reference() {
        let texture =
            TextureDefinition::load(0, &[0x01, 0xbf, 0x16, 0x16, 0x01, 0, 0]).expect("texture");
        assert_eq!(texture.sprite_id, 447);
        assert_eq!(texture.field1777, 0x16);
        assert_eq!(texture.field1778, 0x1601);
    }

    #[test]
    fn rejects_short_definition() {
        assert!(TextureDefinition::load(0, &[1, 2]).is_err());
    }
}
