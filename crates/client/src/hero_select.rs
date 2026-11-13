use crate::champion::ChampionConfig;

/// State for the pre-match champion picker.
pub struct HeroSelect {
    pub open: bool,
    pub selected: usize,
    pub hover: Option<usize>,
    pub champions: Vec<ChampionConfig>,
    /// Seconds spent on the screen, used for the selected-card pulse.
    pub time: f32,
}

impl HeroSelect {
    pub fn new(champions: Vec<ChampionConfig>, selected: usize, open: bool) -> Self {
        let selected = selected.min(champions.len().saturating_sub(1));
        Self {
            open,
            selected,
            hover: None,
            champions,
            time: 0.0,
        }
    }

    pub fn cycle(&mut self, delta: isize) {
        if self.champions.is_empty() {
            return;
        }
        let count = self.champions.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(count) as usize;
    }

    pub fn selected_champion(&self) -> Option<&ChampionConfig> {
        self.champions.get(self.selected)
    }
}

/// Card rectangle `(x, y, w, h)` for each champion.
pub fn card_rects(width: f32, height: f32) -> [(f32, f32, f32, f32); 3] {
    let gap = (width * 0.018).clamp(12.0, 26.0);
    let card_w = ((width - 112.0 - gap * 2.0) / 3.0).clamp(226.0, 306.0);
    let card_h = (height * 0.60).clamp(348.0, 454.0);
    let total = card_w * 3.0 + gap * 2.0;
    let x0 = (width - total) * 0.5;
    let y0 = (height * 0.155).max(82.0);
    std::array::from_fn(|index| (x0 + index as f32 * (card_w + gap), y0, card_w, card_h))
}

pub fn card_at(width: f32, height: f32, cursor: (f64, f64)) -> Option<usize> {
    card_rects(width, height)
        .iter()
        .position(|&rect| point_in_rect(rect, cursor))
}

/// Long gold button below the three cards.
pub fn confirm_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let (_, card_y, _, card_h) = card_rects(width, height)[0];
    let w = 286.0f32.min(width - 48.0);
    let h = 46.0;
    let x = (width - w) * 0.5;
    let y = (card_y + card_h + 18.0).min(height - h - 48.0);
    (x, y, w, h)
}

pub fn confirm_at(width: f32, height: f32, cursor: (f64, f64)) -> bool {
    point_in_rect(confirm_rect(width, height), cursor)
}

fn point_in_rect(rect: (f32, f32, f32, f32), cursor: (f64, f64)) -> bool {
    let (x, y, w, h) = rect;
    let mx = cursor.0 as f32;
    let my = cursor.1 as f32;
    mx >= x && mx <= x + w && my >= y && my <= y + h
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::champion::ChampionConfig;

    fn champion(id: &str) -> ChampionConfig {
        ChampionConfig {
            id: id.to_string(),
            name: id.to_string(),
            role: String::new(),
            tagline: String::new(),
            model: 0,
            idle: 0,
            walk: 0,
            scale: 1.0,
            health: 1.0,
            mana: 1.0,
            damage: 1.0,
            attack_range: 1.0,
            attack_interval: 1.0,
            speed: 1.0,
            ranged_auto: false,
            q_damage: 1.0,
            r_damage: 1.0,
            e_heal: 1.0,
            e_shield: 1.0,
            w_distance: 1.0,
        }
    }

    #[test]
    fn cards_are_distinct_and_hit_test_in_order() {
        let (width, height) = (1280.0, 720.0);
        let cards = card_rects(width, height);
        for (index, card) in cards.iter().enumerate() {
            let center = (
                (card.0 + card.2 * 0.5) as f64,
                (card.1 + card.3 * 0.5) as f64,
            );
            assert_eq!(card_at(width, height, center), Some(index));
        }
        assert!(cards[0].0 + cards[0].2 < cards[1].0);
        assert_eq!(card_at(width, height, (5.0, 5.0)), None);
    }

    #[test]
    fn confirm_button_sits_below_the_cards() {
        let (width, height) = (1280.0, 720.0);
        let (_, card_y, _, card_h) = card_rects(width, height)[0];
        let (_, button_y, _, _) = confirm_rect(width, height);
        assert!(button_y > card_y + card_h);
        let button = confirm_rect(width, height);
        assert!(confirm_at(
            width,
            height,
            (
                (button.0 + button.2 * 0.5) as f64,
                (button.1 + button.3 * 0.5) as f64
            )
        ));
    }

    #[test]
    fn cycling_wraps_the_roster() {
        let champions = vec![champion("a"), champion("b"), champion("c")];
        let mut select = HeroSelect::new(champions, 0, true);
        select.cycle(1);
        assert_eq!(select.selected, 1);
        select.cycle(-1);
        assert_eq!(select.selected, 0);
        select.cycle(-1);
        assert_eq!(select.selected, 2);
    }
}
