use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ItemConfig {
    pub id: String,
    pub name: String,
    pub color: [u8; 3],
    pub cost: u32,
    #[serde(default)]
    pub damage: f32,
    #[serde(default)]
    pub health: f32,
    #[serde(default)]
    pub mana: f32,
    #[serde(default)]
    pub speed: f32,
    #[serde(default)]
    pub ability: f32,
}

pub struct ItemShop {
    pub items: Vec<ItemConfig>,
    pub owned: [Option<usize>; 6],
    pub open: bool,
}

impl ItemShop {
    pub fn load() -> Result<Self> {
        let data = std::fs::read_to_string("data/items/items.json")
            .context("reading data/items/items.json")?;
        Ok(Self {
            items: serde_json::from_str(&data)?,
            owned: [None; 6],
            open: false,
        })
    }

    pub fn owned_colors(&self) -> [Option<[u8; 3]>; 6] {
        let mut colors = [None; 6];
        for (slot, item) in self.owned.iter().enumerate() {
            if let Some(index) = item {
                if let Some(item) = self.items.get(*index) {
                    colors[slot] = Some(item.color);
                }
            }
        }
        colors
    }

    pub fn is_owned(&self, index: usize) -> bool {
        self.owned.iter().flatten().any(|&i| i == index)
    }

    pub fn has_free_slot(&self) -> bool {
        self.owned.iter().any(Option::is_none)
    }

    /// Record a completed purchase in the first free inventory slot.
    pub fn add_owned(&mut self, index: usize) -> bool {
        if let Some(slot) = self.owned.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(index);
            true
        } else {
            false
        }
    }

    pub fn affordable(&self, gold: u32) -> [bool; 6] {
        std::array::from_fn(|i| self.items.get(i).is_some_and(|item| item.cost <= gold))
    }

    pub fn item_count(&self) -> usize {
        self.owned.iter().flatten().count()
    }
}

pub fn shop_row_rect(_width: f32, height: f32, row: usize) -> (f32, f32, f32, f32) {
    let x = 22.0;
    let y = height - 336.0 + row as f32 * 40.0;
    (x, y, 250.0, 36.0)
}

pub fn shop_row_at(_width: f32, height: f32, cursor: (f64, f64)) -> Option<usize> {
    let (mx, my) = (cursor.0 as f32, cursor.1 as f32);
    for row in 0..6 {
        let (x, y, w, h) = shop_row_rect(_width, height, row);
        if mx >= x && mx <= x + w && my >= y && my <= y + h {
            return Some(row);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shop() -> ItemShop {
        let json = r#"[
            {"id":"a","name":"A","color":[1,2,3],"cost":100},
            {"id":"b","name":"B","color":[4,5,6],"cost":200},
            {"id":"c","name":"C","color":[7,8,9],"cost":300},
            {"id":"d","name":"D","color":[10,11,12],"cost":400},
            {"id":"e","name":"E","color":[13,14,15],"cost":500},
            {"id":"f","name":"F","color":[16,17,18],"cost":600}
        ]"#;
        ItemShop {
            items: serde_json::from_str(json).expect("item fixture"),
            owned: [None; 6],
            open: false,
        }
    }

    #[test]
    fn shop_rows_hit_test() {
        let height = 720.0;
        assert_eq!(shop_row_at(1280.0, height, (100.0, 400.0)), Some(0));
        assert_eq!(shop_row_at(1280.0, height, (100.0, 600.0)), Some(5));
        assert_eq!(shop_row_at(1280.0, height, (100.0, 100.0)), None);
        assert_eq!(shop_row_at(1280.0, height, (300.0, 400.0)), None);
    }

    #[test]
    fn inventory_fills_six_slots() {
        let mut shop = test_shop();
        for _ in 0..6 {
            assert!(shop.add_owned(0));
        }
        assert!(!shop.has_free_slot());
        assert!(!shop.add_owned(1));
        assert_eq!(shop.item_count(), 6);
        assert!(shop.is_owned(0));
        assert!(!shop.is_owned(1));
    }

    #[test]
    fn affordability_tracks_gold() {
        let shop = test_shop();
        assert_eq!(
            shop.affordable(300),
            [true, true, true, false, false, false]
        );
        assert_eq!(
            shop.affordable(0),
            [false, false, false, false, false, false]
        );
    }

    #[test]
    fn owned_colors_follow_inventory_slots() {
        let mut shop = test_shop();
        assert!(shop.add_owned(2));
        assert!(shop.add_owned(5));
        let colors = shop.owned_colors();
        assert_eq!(colors[0], Some([7, 8, 9]));
        assert_eq!(colors[1], Some([16, 17, 18]));
        assert_eq!(colors[2], None);
    }
}
