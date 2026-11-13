use glam::Vec2;

use crate::animated::{AnimatedScene, UnitKind};
use crate::font;
use crate::items::{ItemShop, shop_row_rect};
use crate::scene::Vertex;

pub struct PlayerHud {
    pub health: f32,
    pub max_health: f32,
    pub mana: f32,
    pub max_mana: f32,
    pub xp: f32,
    pub xp_next: f32,
    pub cooldowns: [f32; 4],
    pub shield: f32,
    pub shield_timer: f32,
    pub buff: Option<(f32, f32, [u8; 3])>,
    pub buff_label: Option<&'static str>,
    pub level: u32,
    pub gold: u32,
    pub items: [Option<[u8; 3]>; 6],
    pub shop_open: bool,
    pub shop_hover: Option<usize>,
    pub shop_in_range: bool,
}

pub struct Hud {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

/// Screen-space rectangle of the minimap: `(x, y, width, height)`.
pub fn minimap_rect(width: f32, height: f32) -> (f32, f32, f32, f32) {
    let size = 190.0;
    (width - size - 18.0, height - size - 18.0, size, size)
}

/// Convert a cursor position over the minimap into world `(x, z)`.
/// Returns `None` when the cursor is outside the minimap.
pub fn minimap_world_at(
    width: f32,
    height: f32,
    cursor: (f64, f64),
    bounds: (Vec2, Vec2),
) -> Option<(f32, f32)> {
    let (x, y, size, _) = minimap_rect(width, height);
    let mx = cursor.0 as f32;
    let my = cursor.1 as f32;
    if mx < x || mx > x + size || my < y || my > y + size {
        return None;
    }
    let nx = ((mx - x) / size).clamp(0.0, 1.0);
    let nz = 1.0 - ((my - y) / size).clamp(0.0, 1.0);
    Some((
        bounds.0.x + nx * (bounds.1.x - bounds.0.x),
        bounds.0.y + nz * (bounds.1.y - bounds.0.y),
    ))
}

impl Hud {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Draw text as one quad per horizontal pixel run, with a one-pixel drop
    /// shadow. Lowercase is folded to uppercase by the font module.
    #[allow(clippy::too_many_arguments)]
    pub fn text(
        &mut self,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        scale: f32,
        text: &str,
        color: [u8; 3],
    ) {
        self.text_raw(width, height, x + scale, y + scale, scale, text, [0, 0, 0]);
        self.text_raw(width, height, x, y, scale, text, color);
    }

    #[allow(clippy::too_many_arguments)]
    fn text_raw(
        &mut self,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        scale: f32,
        text: &str,
        color: [u8; 3],
    ) {
        let mut cx = x;
        for ch in text.chars() {
            let rows = font::glyph(ch);
            for (row, bits) in rows.iter().enumerate() {
                let mut col = 0;
                while col < font::GLYPH_W {
                    if bits & (1 << (font::GLYPH_W - 1 - col)) == 0 {
                        col += 1;
                        continue;
                    }
                    let start = col;
                    while col < font::GLYPH_W && bits & (1 << (font::GLYPH_W - 1 - col)) != 0 {
                        col += 1;
                    }
                    self.quad(
                        width,
                        height,
                        cx + start as f32 * scale,
                        y + row as f32 * scale,
                        (col - start) as f32 * scale,
                        scale,
                        color,
                    );
                }
            }
            cx += font::ADVANCE as f32 * scale;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text_centered(
        &mut self,
        width: f32,
        height: f32,
        center_x: f32,
        y: f32,
        scale: f32,
        text: &str,
        color: [u8; 3],
    ) {
        let text_width = font::text_width(text, scale);
        self.text(
            width,
            height,
            center_x - text_width * 0.5,
            y,
            scale,
            text,
            color,
        );
    }

    pub fn build_minimap(&mut self, width: f32, height: f32, scene: &AnimatedScene) {
        let (x, y, size, _) = minimap_rect(width, height);
        self.quad(
            width,
            height,
            x - 2.0,
            y - 2.0,
            size + 4.0,
            size + 4.0,
            [70, 75, 85],
        );
        self.quad(width, height, x, y, size, size, [10, 12, 14]);

        let (min, max) = scene.minimap_bounds;
        let to_map = |p: glam::Vec3| {
            let nx = ((p.x - min.x) / (max.x - min.x)).clamp(0.0, 1.0);
            let nz = ((p.z - min.y) / (max.y - min.y)).clamp(0.0, 1.0);
            (x + nx * size, y + (1.0 - nz) * size)
        };

        for lane in &scene.lanes {
            for pair in lane.windows(2) {
                let (x0, y0) = to_map(pair[0]);
                let (x1, y1) = to_map(pair[1]);
                let steps = (((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt() / 3.0) as i32;
                for i in 0..=steps {
                    let t = i as f32 / steps.max(1) as f32;
                    self.quad(
                        width,
                        height,
                        x0 + (x1 - x0) * t - 1.5,
                        y0 + (y1 - y0) * t - 1.5,
                        3.0,
                        3.0,
                        [105, 105, 95],
                    );
                }
            }
        }

        // Coarse fog: cover 8x8-tile blocks that are entirely unseen. Larger
        // blocks keep the HUD mesh cheap while still reading as fog.
        let (ox, oy) = scene.vision_origin;
        let block = 8usize;
        let mut by = 0;
        while by < scene.visibility.height {
            let mut bx = 0;
            while bx < scene.visibility.width {
                let mut unseen = true;
                'tiles: for ty in by..(by + block).min(scene.visibility.height) {
                    for tx in bx..(bx + block).min(scene.visibility.width) {
                        if scene.visibility.is_visible(tx as i32, ty as i32) {
                            unseen = false;
                            break 'tiles;
                        }
                    }
                }
                if unseen {
                    let wx0 = (ox as f32 + bx as f32) * 128.0;
                    let wz0 = (oy as f32 + by as f32) * 128.0;
                    let wx1 = wx0 + block as f32 * 128.0;
                    let wz1 = wz0 + block as f32 * 128.0;
                    let (x0, y0) = to_map(glam::Vec3::new(wx0, 0.0, wz0));
                    let (x1, y1) = to_map(glam::Vec3::new(wx1, 0.0, wz1));
                    self.quad(
                        width,
                        height,
                        x0.min(x1),
                        y0.min(y1),
                        (x1 - x0).abs().max(1.0),
                        (y1 - y0).abs().max(1.0),
                        [4, 6, 10],
                    );
                }
                bx += block;
            }
            by += block;
        }

        for unit in &scene.units {
            if !unit.visible {
                continue;
            }
            let (ux, uy) = to_map(unit.pos);
            if unit.health <= 0.0 {
                if unit.kind.is_structure() {
                    self.quad(width, height, ux - 2.5, uy - 2.5, 5.0, 5.0, [45, 45, 48]);
                }
                continue;
            }
            match unit.kind {
                UnitKind::Nexus => {
                    let color = if unit.team == 0 {
                        [80, 150, 255]
                    } else {
                        [220, 65, 60]
                    };
                    self.quad(width, height, ux - 4.0, uy - 4.0, 8.0, 8.0, color);
                }
                UnitKind::Inhibitor => {
                    let color = if unit.team == 0 {
                        [80, 150, 255]
                    } else {
                        [220, 65, 60]
                    };
                    self.quad(width, height, ux - 3.0, uy - 3.0, 6.0, 6.0, color);
                }
                UnitKind::Tower => {
                    let color = if unit.team == 0 {
                        [80, 150, 255]
                    } else {
                        [220, 65, 60]
                    };
                    self.quad(width, height, ux - 2.0, uy - 2.0, 4.0, 4.0, color);
                }
                UnitKind::Camp => {
                    self.quad(width, height, ux - 2.0, uy - 2.0, 4.0, 4.0, [230, 180, 70]);
                }
                UnitKind::Player => {
                    self.quad(width, height, ux - 3.0, uy - 3.0, 6.0, 6.0, [235, 235, 235]);
                }
                UnitKind::MeleeMinion | UnitKind::RangedMinion => {
                    let color = if unit.team == 0 {
                        [90, 160, 255]
                    } else {
                        [235, 90, 80]
                    };
                    if unit.super_minion {
                        self.quad(width, height, ux - 2.5, uy - 2.5, 5.0, 5.0, [255, 215, 90]);
                        self.quad(width, height, ux - 1.0, uy - 1.0, 2.0, 2.0, color);
                    } else {
                        self.quad(width, height, ux - 1.0, uy - 1.0, 2.0, 2.0, color);
                    }
                }
                UnitKind::Dummy => {
                    self.quad(width, height, ux - 1.5, uy - 1.5, 3.0, 3.0, [220, 180, 70]);
                }
                UnitKind::Ambient => {}
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        width: f32,
        height: f32,
        player: &PlayerHud,
        nexus0: (f32, f32),
        nexus1: (f32, f32),
        wave_timer: f32,
        wave_interval: f32,
        match_over: Option<u8>,
        mouse: (f64, f64),
    ) {
        self.vertices.clear();
        self.indices.clear();

        // Health, mana, XP, and buff panels, top-left.
        self.panel(
            width,
            height,
            22.0,
            22.0,
            280.0,
            22.0,
            player.health / player.max_health,
            [70, 190, 95],
        );
        self.panel(
            width,
            height,
            22.0,
            50.0,
            280.0,
            14.0,
            player.mana / player.max_mana.max(1.0),
            [70, 120, 235],
        );
        self.panel(
            width,
            height,
            22.0,
            70.0,
            280.0,
            6.0,
            (player.xp / player.xp_next.max(1.0)).clamp(0.0, 1.0),
            [170, 90, 230],
        );
        if player.shield > 0.0 && player.shield_timer > 0.0 {
            self.panel(
                width,
                height,
                22.0,
                45.0,
                280.0,
                3.0,
                (player.shield / 120.0).clamp(0.0, 1.0),
                [90, 255, 140],
            );
        }
        self.text(
            width,
            height,
            28.0,
            25.0,
            1.4,
            &format!("HP {:.0}/{:.0}", player.health, player.max_health),
            [240, 245, 240],
        );
        self.text(
            width,
            height,
            28.0,
            51.5,
            1.0,
            &format!("MP {:.0}/{:.0}", player.mana, player.max_mana),
            [225, 235, 255],
        );
        self.text(
            width,
            height,
            22.0,
            78.0,
            0.9,
            &format!(
                "LV {}  XP {:.0}/{:.0}",
                player.level, player.xp, player.xp_next
            ),
            [205, 180, 240],
        );
        self.text(width, height, 310.0, 24.0, 1.3, "GOLD", [215, 190, 110]);
        self.text(
            width,
            height,
            310.0,
            40.0,
            1.3,
            &format!("{}G", player.gold),
            [245, 205, 70],
        );
        if let Some((remaining, duration, color)) = player.buff {
            let fill = (remaining / duration.max(0.1)).clamp(0.0, 1.0);
            self.panel(width, height, 22.0, 94.0, 150.0, 10.0, fill, color);
            if let Some(label) = player.buff_label {
                self.text(
                    width,
                    height,
                    26.0,
                    95.0,
                    0.9,
                    &format!("{label} {remaining:.0}S"),
                    [250, 250, 250],
                );
            }
        }

        // Nexus health bars, top-centre.
        let bar_w = 220.0;
        let gap = 24.0;
        let x0 = (width - (bar_w * 2.0 + gap)) * 0.5;
        self.panel(
            width,
            height,
            x0,
            18.0,
            bar_w,
            16.0,
            (nexus0.0 / nexus0.1.max(1.0)).clamp(0.0, 1.0),
            [80, 150, 255],
        );
        self.panel(
            width,
            height,
            x0 + bar_w + gap,
            18.0,
            bar_w,
            16.0,
            (nexus1.0 / nexus1.1.max(1.0)).clamp(0.0, 1.0),
            [220, 65, 60],
        );
        let wave_fill = if wave_interval > 0.0 {
            (1.0 - wave_timer / wave_interval).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.panel(
            width,
            height,
            width * 0.5 - 90.0,
            40.0,
            180.0,
            6.0,
            wave_fill,
            [220, 190, 60],
        );
        self.text(
            width,
            height,
            x0,
            4.0,
            1.1,
            &format!("BLUE {:.0}", nexus0.0),
            [110, 175, 255],
        );
        self.text(
            width,
            height,
            x0 + bar_w + gap,
            4.0,
            1.1,
            &format!("RED {:.0}", nexus1.0),
            [255, 120, 110],
        );
        self.text_centered(
            width,
            height,
            width * 0.5,
            48.0,
            0.9,
            &format!("WAVE {:.0}S", wave_timer),
            [230, 210, 120],
        );

        if let Some(winner) = match_over {
            let color = if winner == 0 {
                [80, 150, 255]
            } else {
                [220, 65, 60]
            };
            self.quad(width, height, 0.0, 0.0, width, 5.0, color);
            self.quad(width, height, 0.0, height - 5.0, width, 5.0, color);
            self.quad(width, height, 0.0, 0.0, 5.0, height, color);
            self.quad(width, height, width - 5.0, 0.0, 5.0, height, color);
            self.text_centered(
                width,
                height,
                width * 0.5,
                height * 0.34,
                5.0,
                if winner == 0 { "VICTORY" } else { "DEFEAT" },
                color,
            );
        }

        // Four ability boxes, bottom-centre.
        let box_size = 58.0;
        let gap = 8.0;
        let total = box_size * 4.0 + gap * 3.0;
        let x0 = (width - total) * 0.5;
        let y0 = height - 96.0;
        let colors = [
            [255, 150, 40],
            [90, 220, 255],
            [90, 255, 140],
            [255, 90, 60],
        ];
        let maxima = [5.0f32, 8.0, 12.0, 45.0];
        for (i, color) in colors.iter().enumerate() {
            let bx = x0 + i as f32 * (box_size + gap);
            self.quad(
                width,
                height,
                bx - 2.0,
                y0 - 2.0,
                box_size + 4.0,
                box_size + 4.0,
                [70, 75, 85],
            );
            self.quad(width, height, bx, y0, box_size, box_size, [32, 34, 40]);
            let cooldown = player.cooldowns[i];
            if cooldown > 0.0 {
                let frac = (cooldown / maxima[i]).clamp(0.0, 1.0);
                let fill_h = box_size * frac;
                self.quad(
                    width,
                    height,
                    bx,
                    y0 + box_size - fill_h,
                    box_size,
                    fill_h,
                    [16, 18, 24],
                );
            } else {
                self.quad(width, height, bx + 17.0, y0 + 17.0, 24.0, 24.0, *color);
            }
            self.text_centered(
                width,
                height,
                bx + box_size * 0.5,
                y0 + 18.0,
                2.0,
                ["Q", "W", "E", "R"][i],
                [245, 245, 245],
            );
            if cooldown > 0.0 {
                self.text_centered(
                    width,
                    height,
                    bx + box_size * 0.5,
                    y0 + 40.0,
                    1.1,
                    &format!("{:.0}", cooldown.ceil()),
                    [220, 220, 220],
                );
            }
        }

        // Crosshair at the cursor.
        let mx = mouse.0 as f32;
        let my = mouse.1 as f32;
        self.quad(
            width,
            height,
            mx - 8.0,
            my - 1.0,
            16.0,
            2.0,
            [235, 235, 235],
        );
        self.quad(
            width,
            height,
            mx - 1.0,
            my - 8.0,
            2.0,
            16.0,
            [235, 235, 235],
        );
    }

    /// Draw the six inventory slots and, when open, the fountain shop panel.
    /// Geometry only: the window title carries item names and gold.
    pub fn build_shop(&mut self, width: f32, height: f32, player: &PlayerHud, shop: &ItemShop) {
        // Inventory slots, bottom-left.
        let slot = 30.0;
        let gap = 6.0;
        let ix = 22.0;
        let iy = height - 96.0;
        for i in 0..6 {
            let x = ix + i as f32 * (slot + gap);
            self.quad(
                width,
                height,
                x - 1.0,
                iy - 1.0,
                slot + 2.0,
                slot + 2.0,
                [70, 75, 85],
            );
            self.quad(width, height, x, iy, slot, slot, [20, 22, 26]);
            if let Some(color) = player.items[i] {
                self.quad(
                    width,
                    height,
                    x + 5.0,
                    iy + 5.0,
                    slot - 10.0,
                    slot - 10.0,
                    color,
                );
                if let Some(index) = shop.owned[i] {
                    if let Some(item) = shop.items.get(index) {
                        if let Some(initial) = item.name.chars().next() {
                            self.text_centered(
                                width,
                                height,
                                x + slot * 0.5,
                                iy + 18.0,
                                1.0,
                                &initial.to_string(),
                                [245, 245, 245],
                            );
                        }
                    }
                }
            } else {
                self.quad(
                    width,
                    height,
                    x + slot * 0.5 - 2.0,
                    iy + slot * 0.5 - 2.0,
                    4.0,
                    4.0,
                    [48, 52, 60],
                );
            }
        }

        if !player.shop_open {
            return;
        }

        // Shop panel backdrop and header gold bar.
        let panel_x = 16.0;
        let panel_y = height - 368.0;
        self.quad(width, height, panel_x, panel_y, 262.0, 272.0, [70, 75, 85]);
        self.quad(
            width,
            height,
            panel_x + 2.0,
            panel_y + 2.0,
            258.0,
            268.0,
            [14, 16, 20],
        );
        self.quad(
            width,
            height,
            panel_x + 8.0,
            panel_y + 8.0,
            246.0,
            20.0,
            [10, 12, 16],
        );
        let gold_fill = (player.gold as f32 / 2000.0).clamp(0.0, 1.0);
        self.quad(
            width,
            height,
            panel_x + 10.0,
            panel_y + 10.0,
            242.0 * gold_fill,
            16.0,
            if player.shop_in_range {
                [225, 185, 55]
            } else {
                [130, 60, 60]
            },
        );
        self.text(
            width,
            height,
            panel_x + 10.0,
            panel_y + 10.0,
            1.2,
            "SHOP",
            [245, 245, 245],
        );
        let gold_text = format!("{}G", player.gold);
        let gold_width = font::text_width(&gold_text, 1.2);
        self.text(
            width,
            height,
            panel_x + 250.0 - gold_width,
            panel_y + 10.0,
            1.2,
            &gold_text,
            [245, 245, 245],
        );

        let affordable = shop.affordable(player.gold);
        for (row, &can_buy_item) in affordable.iter().enumerate() {
            let (x, y, w, h) = shop_row_rect(width, height, row);
            let hover = player.shop_hover == Some(row);
            let can_buy = can_buy_item && player.shop_in_range;
            let owned = shop.is_owned(row);
            let item = shop.items.get(row);
            let border = if hover {
                [120, 200, 255]
            } else if can_buy {
                [88, 94, 108]
            } else {
                [54, 56, 64]
            };
            self.quad(width, height, x, y, w, h, border);
            self.quad(
                width,
                height,
                x + 1.0,
                y + 1.0,
                w - 2.0,
                h - 2.0,
                if hover { [34, 38, 46] } else { [22, 24, 30] },
            );
            let swatch = if can_buy {
                item.map(|item| item.color).unwrap_or([90, 90, 90])
            } else {
                [56, 58, 66]
            };
            // The swatch is the item colour when affordable; owned items are
            // marked with a green corner so duplicates remain readable.
            self.quad(width, height, x + 8.0, y + 6.0, 24.0, 24.0, swatch);
            let cost = item.map(|item| item.cost).unwrap_or(0);
            let cost_frac = (cost as f32 / 1000.0).clamp(0.0, 1.0);
            self.quad(width, height, x + 40.0, y + 27.0, 190.0, 4.0, [10, 12, 16]);
            self.quad(
                width,
                height,
                x + 40.0,
                y + 27.0,
                190.0 * cost_frac,
                4.0,
                if can_buy {
                    [225, 185, 55]
                } else {
                    [110, 55, 55]
                },
            );
            let name = item.map(|item| item.name.as_str()).unwrap_or("?");
            let name_color = if owned {
                [170, 255, 190]
            } else if can_buy {
                [238, 238, 238]
            } else {
                [135, 135, 145]
            };
            self.text(width, height, x + 40.0, y + 4.0, 1.25, name, name_color);
            self.text(
                width,
                height,
                x + 40.0,
                y + 17.0,
                0.9,
                &format!("{cost}G"),
                if can_buy {
                    [245, 205, 70]
                } else {
                    [170, 95, 95]
                },
            );
            if owned {
                self.quad(
                    width,
                    height,
                    x + w - 16.0,
                    y + 8.0,
                    8.0,
                    8.0,
                    [80, 255, 120],
                );
            }
        }

        // Hovered-item tooltip, to the right of the shop panel.
        if let Some(row) = player.shop_hover {
            if let Some(item) = shop.items.get(row) {
                let (_, row_y, _, _) = shop_row_rect(width, height, row);
                let mut lines = Vec::new();
                if item.damage > 0.0 {
                    lines.push(format!("+{:.0} DMG", item.damage));
                }
                if item.health > 0.0 {
                    lines.push(format!("+{:.0} HP", item.health));
                }
                if item.mana > 0.0 {
                    lines.push(format!("+{:.0} MP", item.mana));
                }
                if item.speed > 0.0 {
                    lines.push(format!("+{:.0} SPD", item.speed));
                }
                if item.ability > 0.0 {
                    lines.push(format!("+{:.0}% AP", item.ability));
                }
                if lines.is_empty() {
                    lines.push("NO BONUS".to_string());
                }

                let panel_w = 190.0;
                let panel_h = 36.0 + lines.len() as f32 * 12.0;
                let px = 282.0;
                let py = row_y.min(height - panel_h - 8.0).max(8.0);
                self.quad(width, height, px, py, panel_w, panel_h, [90, 160, 220]);
                self.quad(
                    width,
                    height,
                    px + 1.0,
                    py + 1.0,
                    panel_w - 2.0,
                    panel_h - 2.0,
                    [14, 16, 22],
                );
                self.text(
                    width,
                    height,
                    px + 8.0,
                    py + 6.0,
                    1.1,
                    &item.name,
                    [238, 238, 238],
                );
                let mut ty = py + 22.0;
                for line in &lines {
                    self.text(width, height, px + 8.0, ty, 1.0, line, [200, 220, 240]);
                    ty += 12.0;
                }
                let can_buy = shop.affordable(player.gold)[row] && player.shop_in_range;
                let (status, color) = if !player.shop_in_range {
                    ("RETURN TO FOUNTAIN", [255, 120, 110])
                } else if shop.is_owned(row) {
                    ("OWNED", [170, 255, 190])
                } else if !shop.affordable(player.gold)[row] {
                    ("NEED GOLD", [255, 120, 110])
                } else if !shop.has_free_slot() {
                    ("INVENTORY FULL", [255, 170, 90])
                } else if can_buy {
                    ("CLICK TO BUY", [245, 205, 70])
                } else {
                    ("UNAVAILABLE", [150, 150, 150])
                };
                self.text(width, height, px + 8.0, ty + 2.0, 0.9, status, color);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn panel(
        &mut self,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: f32,
        color: [u8; 3],
    ) {
        self.quad(width, height, x, y, w, h, [12, 12, 16]);
        self.quad(
            width,
            height,
            x + 2.0,
            y + 2.0,
            (w - 4.0) * fill.clamp(0.0, 1.0),
            h - 4.0,
            color,
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn quad(&mut self, width: f32, height: f32, x: f32, y: f32, w: f32, h: f32, color: [u8; 3]) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let x0 = 2.0 * x / width - 1.0;
        let x1 = 2.0 * (x + w) / width - 1.0;
        let y0 = 1.0 - 2.0 * y / height;
        let y1 = 1.0 - 2.0 * (y + h) / height;

        let base = self.vertices.len() as u32;
        self.vertices
            .push(Vertex::new(glam::Vec3::new(x0, y0, 0.0), color));
        self.vertices
            .push(Vertex::new(glam::Vec3::new(x1, y0, 0.0), color));
        self.vertices
            .push(Vertex::new(glam::Vec3::new(x0, y1, 0.0), color));
        self.vertices
            .push(Vertex::new(glam::Vec3::new(x1, y1, 0.0), color));
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_world_maps_corners_and_center() {
        let bounds = (Vec2::new(0.0, 0.0), Vec2::new(1000.0, 2000.0));
        let (x, y, size, _) = minimap_rect(1280.0, 720.0);
        let (cx, cz) = minimap_world_at(
            1280.0,
            720.0,
            ((x + size * 0.5) as f64, (y + size * 0.5) as f64),
            bounds,
        )
        .expect("center inside minimap");
        assert!((cx - 500.0).abs() < 0.01);
        assert!((cz - 1000.0).abs() < 0.01);

        let (lx, lz) = minimap_world_at(1280.0, 720.0, (x as f64, (y + size) as f64), bounds)
            .expect("bottom-left inside minimap");
        assert!(lx.abs() < 0.01);
        assert!(lz.abs() < 0.01);

        let (rx, rz) = minimap_world_at(1280.0, 720.0, ((x + size) as f64, y as f64), bounds)
            .expect("top-right inside minimap");
        assert!((rx - 1000.0).abs() < 0.01);
        assert!((rz - 2000.0).abs() < 0.01);
    }

    #[test]
    fn minimap_world_rejects_clicks_outside() {
        let bounds = (Vec2::ZERO, Vec2::ONE);
        assert!(minimap_world_at(1280.0, 720.0, (10.0, 10.0), bounds).is_none());
    }
}
