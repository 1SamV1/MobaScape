use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ItemConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
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

fn default_category() -> String {
    "misc".to_string()
}

pub struct ItemShop {
    pub items: Vec<ItemConfig>,
    pub owned: [Option<usize>; 6],
    pub open: bool,
    pub selected: Option<usize>,
    pub category: usize,
}

pub const CATEGORY_LABELS: [&str; 6] = [
    "ALL",
    "WEAPONS",
    "ARMOUR",
    "JEWELLERY",
    "BOOTS",
    "CONSUMABLES",
];

impl ItemShop {
    pub fn load() -> Result<Self> {
        let data = std::fs::read_to_string("data/items/items.json")
            .context("reading data/items/items.json")?;
        Ok(Self {
            items: serde_json::from_str(&data)?,
            owned: [None; 6],
            open: false,
            selected: Some(0),
            category: 0,
        })
    }

    pub fn is_owned(&self, index: usize) -> bool {
        self.owned.iter().flatten().any(|&i| i == index)
    }

    pub fn owned_count(&self, index: usize) -> usize {
        self.owned.iter().flatten().filter(|&&i| i == index).count()
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

    /// Remove the first matching inventory slot.
    pub fn remove_owned(&mut self, index: usize) -> bool {
        if let Some(slot) = self.owned.iter_mut().find(|slot| **slot == Some(index)) {
            *slot = None;
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

    pub fn filtered_items(&self) -> Vec<usize> {
        self.items
            .iter()
            .enumerate()
            .filter(|(_, item)| match self.category {
                1 => item.category == "weapon",
                2 => item.category == "armour",
                3 => item.category == "jewellery",
                4 => item.category == "boots",
                5 => item.category == "consumable",
                _ => true,
            })
            .map(|(index, _)| index)
            .collect()
    }
}

pub fn point_in_rect(rect: (f32, f32, f32, f32), cursor: (f64, f64)) -> bool {
    let (x, y, w, h) = rect;
    let mx = cursor.0 as f32;
    let my = cursor.1 as f32;
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

pub fn shop_window_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let w = 760.0f32.min(width - 40.0);
    let h = 430.0f32.min(height - 40.0);
    ((width - w) * 0.5, (height - h) * 0.5, w, h)
}

pub fn shop_close_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let (x, y, w, _) = shop_window_rect(width, height);
    (x + w - 30.0, y + 8.0, 22.0, 22.0)
}

pub fn shop_category_rect(width: f32, height: f32, index: usize) -> (f32, f32, f32, f32) {
    let (x, y, _, _) = shop_window_rect(width, height);
    (x + 12.0, y + 44.0 + index as f32 * 27.0, 140.0, 23.0)
}

pub fn shop_card_rect(width: f32, height: f32, index: usize) -> (f32, f32, f32, f32) {
    let (x, y, _, _) = shop_window_rect(width, height);
    let col = index % 3;
    let row = index / 3;
    (
        x + 166.0 + col as f32 * 118.0,
        y + 44.0 + row as f32 * 116.0,
        110.0,
        108.0,
    )
}

pub fn shop_detail_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let (x, y, _, _) = shop_window_rect(width, height);
    (x + 526.0, y + 44.0, 222.0, 248.0)
}

pub fn shop_buy_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let (x, y, _, _) = shop_detail_rect(width, height);
    (x + 10.0, y + 194.0, 202.0, 30.0)
}

pub fn shop_sell_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let (x, y, _, _) = shop_detail_rect(width, height);
    (x + 10.0, y + 228.0, 202.0, 24.0)
}

pub fn shop_button_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    (width - 454.0, height - 132.0, 104.0, 24.0)
}

pub fn shop_category_at(width: f32, height: f32, cursor: (f64, f64)) -> Option<usize> {
    (0..CATEGORY_LABELS.len())
        .find(|&index| point_in_rect(shop_category_rect(width, height, index), cursor))
}

pub fn shop_card_at(width: f32, height: f32, cursor: (f64, f64), shop: &ItemShop) -> Option<usize> {
    shop.filtered_items()
        .into_iter()
        .enumerate()
        .find(|(slot, _)| point_in_rect(shop_card_rect(width, height, *slot), cursor))
        .map(|(_, item)| item)
}

pub fn shop_buy_at(width: f32, height: f32, cursor: (f64, f64)) -> bool {
    point_in_rect(shop_buy_rect(width, height), cursor)
}

pub fn shop_sell_at(width: f32, height: f32, cursor: (f64, f64)) -> bool {
    point_in_rect(shop_sell_rect(width, height), cursor)
}

pub fn shop_close_at(width: f32, height: f32, cursor: (f64, f64)) -> bool {
    point_in_rect(shop_close_rect(width, height), cursor)
}

pub fn shop_button_at(width: f32, height: f32, cursor: (f64, f64)) -> bool {
    point_in_rect(shop_button_rect(width, height), cursor)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_shop() -> ItemShop {
        let json = r#"[
            {"id":"a","name":"A","category":"weapon","color":[1,2,3],"cost":100},
            {"id":"b","name":"B","category":"armour","color":[4,5,6],"cost":200},
            {"id":"c","name":"C","category":"weapon","color":[7,8,9],"cost":300},
            {"id":"d","name":"D","category":"jewellery","color":[10,11,12],"cost":400},
            {"id":"e","name":"E","category":"boots","color":[13,14,15],"cost":500},
            {"id":"f","name":"F","category":"consumable","color":[16,17,18],"cost":600}
        ]"#;
        ItemShop {
            items: serde_json::from_str(json).expect("item fixture"),
            owned: [None; 6],
            open: false,
            selected: Some(0),
            category: 0,
        }
    }

    #[test]
    fn shop_cards_hit_test() {
        let height = 720.0;
        let (x, y, w, h) = shop_card_rect(1280.0, height, 0);
        assert!(point_in_rect(
            (x, y, w, h),
            ((x + 10.0) as f64, (y + 10.0) as f64)
        ));
        assert!(!point_in_rect((x, y, w, h), (10.0, 10.0)));
        let (cx, cy, _, _) = shop_category_rect(1280.0, height, 0);
        assert_eq!(
            shop_category_at(1280.0, height, ((cx + 4.0) as f64, (cy + 4.0) as f64)),
            Some(0)
        );
    }

    #[test]
    fn shop_controls_hit_test() {
        let width = 1280.0;
        let height = 720.0;
        let center = |rect: (f32, f32, f32, f32)| {
            (
                (rect.0 + rect.2 * 0.5) as f64,
                (rect.1 + rect.3 * 0.5) as f64,
            )
        };
        assert!(shop_close_at(
            width,
            height,
            center(shop_close_rect(width, height))
        ));
        assert!(shop_buy_at(
            width,
            height,
            center(shop_buy_rect(width, height))
        ));
        assert!(shop_sell_at(
            width,
            height,
            center(shop_sell_rect(width, height))
        ));
        assert!(shop_button_at(
            width,
            height,
            center(shop_button_rect(width, height))
        ));
        let shop = test_shop();
        let card = shop_card_rect(width, height, 0);
        assert_eq!(shop_card_at(width, height, center(card), &shop), Some(0));
    }

    #[test]
    fn category_filtering_works() {
        let mut shop = test_shop();
        assert_eq!(shop.filtered_items(), vec![0, 1, 2, 3, 4, 5]);
        shop.category = 1;
        assert_eq!(shop.filtered_items(), vec![0, 2]);
        shop.category = 2;
        assert_eq!(shop.filtered_items(), vec![1]);
        shop.category = 3;
        assert_eq!(shop.filtered_items(), vec![3]);
        shop.category = 4;
        assert_eq!(shop.filtered_items(), vec![4]);
        shop.category = 5;
        assert_eq!(shop.filtered_items(), vec![5]);
    }

    #[test]
    fn inventory_fills_and_removes_six_slots() {
        let mut shop = test_shop();
        for _ in 0..6 {
            assert!(shop.add_owned(0));
        }
        assert!(!shop.has_free_slot());
        assert!(!shop.add_owned(1));
        assert_eq!(shop.item_count(), 6);
        assert_eq!(shop.owned_count(0), 6);
        assert!(shop.remove_owned(0));
        assert!(shop.has_free_slot());
        assert_eq!(shop.owned_count(0), 5);
        assert!(!shop.remove_owned(2));
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
}
