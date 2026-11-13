use std::sync::Arc;

use glam::Vec2;

use crate::animated::{AnimatedScene, UnitKind};
use crate::font;
use crate::items::{
    CATEGORY_LABELS, ItemConfig, ItemShop, shop_button_rect, shop_buy_rect, shop_card_rect,
    shop_category_rect, shop_close_rect, shop_detail_rect, shop_sell_rect, shop_window_rect,
};
use crate::scene::srgb_to_linear;
use crate::ui_assets::UiAtlas;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

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
    pub shop_open: bool,
    pub shop_hover: Option<usize>,
    pub shop_selected: Option<usize>,
    pub shop_in_range: bool,
    pub champion_name: String,
    pub minions: usize,
    pub visible_enemies: usize,
    pub towers: (usize, usize),
    pub inhibitors: (usize, usize),
    pub items_owned: usize,
    pub events: Vec<String>,
}

pub struct Hud {
    pub vertices: Vec<UiVertex>,
    pub indices: Vec<u32>,
    pub atlas: Arc<UiAtlas>,
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
    pub fn new(atlas: Arc<UiAtlas>) -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            atlas,
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
        shop: &ItemShop,
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
            self.panel(width, height, 22.0, 86.0, 150.0, 10.0, fill, color);
            if let Some(label) = player.buff_label {
                self.text(
                    width,
                    height,
                    26.0,
                    87.0,
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
            34.0,
            bar_w,
            16.0,
            (nexus0.0 / nexus0.1.max(1.0)).clamp(0.0, 1.0),
            [80, 150, 255],
        );
        self.panel(
            width,
            height,
            x0 + bar_w + gap,
            34.0,
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
            66.0,
            180.0,
            6.0,
            wave_fill,
            [220, 190, 60],
        );
        self.text(
            width,
            height,
            x0,
            52.0,
            1.1,
            &format!("BLUE {:.0}", nexus0.0),
            [110, 175, 255],
        );
        self.text(
            width,
            height,
            x0 + bar_w + gap,
            52.0,
            1.1,
            &format!("RED {:.0}", nexus1.0),
            [255, 120, 110],
        );
        self.text_centered(
            width,
            height,
            width * 0.5,
            74.0,
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

        // Top resource bar.
        let bar_x = width * 0.5 - 240.0;
        self.quad(width, height, bar_x - 1.0, 3.0, 482.0, 28.0, [82, 76, 58]);
        self.quad(width, height, bar_x, 4.0, 480.0, 26.0, [20, 20, 24]);
        self.text(
            width,
            height,
            bar_x + 12.0,
            9.0,
            1.0,
            &format!("GOLD {}", player.gold),
            [245, 205, 70],
        );
        self.text(
            width,
            height,
            bar_x + 108.0,
            9.0,
            1.0,
            &format!("WAVE {:.0}S", wave_timer),
            [225, 225, 225],
        );
        self.text(
            width,
            height,
            bar_x + 202.0,
            9.0,
            1.0,
            &format!("MINIONS {}", player.minions),
            [225, 225, 225],
        );
        self.text(
            width,
            height,
            bar_x + 306.0,
            9.0,
            1.0,
            &format!("ITEMS {}/6", player.items_owned),
            [225, 225, 225],
        );
        self.text(
            width,
            height,
            bar_x + 404.0,
            9.0,
            1.0,
            &format!("VIS {}", player.visible_enemies),
            [225, 225, 225],
        );

        // Team panel, top-right.
        let tx = width - 238.0;
        self.quad(width, height, tx - 1.0, 3.0, 222.0, 82.0, [82, 76, 58]);
        self.quad(width, height, tx, 4.0, 220.0, 80.0, [20, 20, 24]);
        self.text(
            width,
            height,
            tx + 8.0,
            10.0,
            1.0,
            "YOUR TEAM",
            [220, 200, 150],
        );
        self.text(
            width,
            height,
            tx + 8.0,
            24.0,
            1.1,
            &player.champion_name,
            [240, 240, 240],
        );
        self.text(
            width,
            height,
            tx + 8.0,
            42.0,
            0.9,
            &format!("LV {}   {}/6 ITEMS", player.level, player.items_owned),
            [200, 200, 200],
        );
        self.text(
            width,
            height,
            tx + 8.0,
            54.0,
            0.9,
            &format!("HP {:.0}/{:.0}", player.health, player.max_health),
            [120, 230, 150],
        );
        self.text(
            width,
            height,
            tx + 8.0,
            66.0,
            0.9,
            &format!("GOLD {}", player.gold),
            [245, 205, 70],
        );

        // Objectives panel, left.
        let ox = 22.0;
        let oy = 104.0;
        self.quad(
            width,
            height,
            ox - 1.0,
            oy - 1.0,
            232.0,
            114.0,
            [82, 76, 58],
        );
        self.quad(width, height, ox, oy, 230.0, 112.0, [18, 20, 24]);
        self.text(
            width,
            height,
            ox + 8.0,
            oy + 6.0,
            1.0,
            "OBJECTIVES",
            [220, 200, 150],
        );
        self.text(
            width,
            height,
            ox + 8.0,
            oy + 22.0,
            0.9,
            &format!("NEXUS {:.0}/{:.0}", nexus0.0, nexus0.1),
            [110, 175, 255],
        );
        self.text(
            width,
            height,
            ox + 8.0,
            oy + 36.0,
            0.9,
            &format!("INHIB {}/{}", player.inhibitors.0, player.inhibitors.1),
            [220, 180, 120],
        );
        self.text(
            width,
            height,
            ox + 8.0,
            oy + 50.0,
            0.9,
            &format!("TOWERS {}/{}", player.towers.0, player.towers.1),
            [220, 180, 120],
        );
        let buff_line = player
            .buff
            .map(|(remaining, _, _)| {
                format!(
                    "BUFF {} {:.0}S",
                    player.buff_label.unwrap_or("ACTIVE"),
                    remaining
                )
            })
            .unwrap_or_else(|| "BUFF NONE".to_string());
        self.text(
            width,
            height,
            ox + 8.0,
            oy + 64.0,
            0.9,
            &buff_line,
            [255, 170, 80],
        );
        self.text(
            width,
            height,
            ox + 8.0,
            oy + 78.0,
            0.9,
            &format!("VISIBLE {}", player.visible_enemies),
            [200, 220, 240],
        );
        self.text(
            width,
            height,
            ox + 8.0,
            oy + 92.0,
            0.9,
            &format!("MINIONS {}", player.minions),
            [200, 220, 240],
        );

        // Event log, bottom-left.
        let lx = 18.0;
        let ly = height - 170.0;
        self.quad(
            width,
            height,
            lx - 1.0,
            ly - 1.0,
            342.0,
            122.0,
            [82, 76, 58],
        );
        self.quad(width, height, lx, ly, 340.0, 120.0, [16, 18, 22]);
        self.text(
            width,
            height,
            lx + 8.0,
            ly + 6.0,
            1.0,
            "EVENT LOG",
            [220, 200, 150],
        );
        if player.events.is_empty() {
            self.text(
                width,
                height,
                lx + 8.0,
                ly + 22.0,
                0.85,
                "NO EVENTS YET",
                [180, 185, 195],
            );
        } else {
            for (i, event) in player.events.iter().take(5).enumerate() {
                let line: String = event.chars().take(46).collect();
                self.text(
                    width,
                    height,
                    lx + 8.0,
                    ly + 22.0 + i as f32 * 15.0,
                    0.85,
                    &line,
                    [210, 215, 225],
                );
            }
        }

        // Compact health/mana bars above the ability row.
        let cx = width * 0.5 - 120.0;
        self.panel(
            width,
            height,
            cx,
            height - 138.0,
            240.0,
            10.0,
            (player.health / player.max_health.max(1.0)).clamp(0.0, 1.0),
            [70, 190, 95],
        );
        self.text(
            width,
            height,
            cx + 4.0,
            height - 137.0,
            0.75,
            &format!("HP {:.0}/{:.0}", player.health, player.max_health),
            [245, 245, 245],
        );
        self.panel(
            width,
            height,
            cx,
            height - 124.0,
            240.0,
            8.0,
            (player.mana / player.max_mana.max(1.0)).clamp(0.0, 1.0),
            [70, 120, 235],
        );
        self.text(
            width,
            height,
            cx + 4.0,
            height - 123.0,
            0.7,
            &format!("MP {:.0}/{:.0}", player.mana, player.max_mana),
            [230, 235, 255],
        );

        // Inventory and shop button, bottom-right.
        let inv_x = width - 454.0;
        let inv_y = height - 96.0;
        self.draw_inventory_slots(width, height, inv_x, inv_y, 34.0, 6.0, shop);
        let (bx, by, bw, bh) = shop_button_rect(width, height);
        self.quad(
            width,
            height,
            bx - 1.0,
            by - 1.0,
            bw + 2.0,
            bh + 2.0,
            [82, 76, 58],
        );
        if let Some(uv) = self.atlas.ui_uv("tab") {
            self.sprite(width, height, bx, by, bw, bh, uv, [255, 240, 200], 1.0);
            self.quad_alpha(
                width,
                height,
                bx + 3.0,
                by + 3.0,
                bw - 6.0,
                bh - 6.0,
                [24, 24, 28],
                0.82,
            );
        } else {
            self.quad(width, height, bx, by, bw, bh, [30, 28, 22]);
        }
        self.text_centered(
            width,
            height,
            bx + bw * 0.5,
            by + 7.0,
            0.9,
            "SHOP [B]",
            [245, 220, 150],
        );

        // Crosshair at the cursor.
        if player.shop_open {
            return;
        }
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
        if !player.shop_open {
            return;
        }
        let (wx, wy, ww, wh) = shop_window_rect(width, height);
        let affordable = shop.affordable(player.gold);

        // Modal dimmer and window frame.
        self.quad_alpha(width, height, 0.0, 0.0, width, height, [0, 0, 0], 0.42);
        self.quad(
            width,
            height,
            wx - 2.0,
            wy - 2.0,
            ww + 4.0,
            wh + 4.0,
            [96, 82, 50],
        );
        self.quad_alpha(width, height, wx, wy, ww, wh, [18, 20, 26], 0.97);
        if let Some(uv) = self.atlas.ui_uv("tab") {
            self.sprite(
                width,
                height,
                wx + 2.0,
                wy + 2.0,
                ww - 4.0,
                32.0,
                uv,
                [255, 255, 255],
                1.0,
            );
            self.quad_alpha(
                width,
                height,
                wx + 5.0,
                wy + 5.0,
                ww - 10.0,
                26.0,
                [18, 20, 26],
                0.88,
            );
        } else {
            self.quad(
                width,
                height,
                wx + 2.0,
                wy + 2.0,
                ww - 4.0,
                32.0,
                [34, 31, 24],
            );
        }
        self.quad(
            width,
            height,
            wx + 2.0,
            wy + 34.0,
            ww - 4.0,
            2.0,
            [120, 100, 60],
        );

        self.text_centered(
            width,
            height,
            wx + ww * 0.5,
            wy + 10.0,
            1.6,
            "FOUNTAIN SHOP",
            [245, 220, 160],
        );
        let gold_text = format!("{}G", player.gold);
        let gold_width = font::text_width(&gold_text, 1.1);
        self.text(
            width,
            height,
            wx + ww - 160.0 - gold_width,
            wy + 12.0,
            1.1,
            &gold_text,
            [245, 205, 70],
        );
        let (cx, cy, cw, ch) = shop_close_rect(width, height);
        self.quad(width, height, cx, cy, cw, ch, [110, 55, 50]);
        self.quad(
            width,
            height,
            cx + 2.0,
            cy + 2.0,
            cw - 4.0,
            ch - 4.0,
            [40, 20, 20],
        );
        self.text_centered(
            width,
            height,
            cx + cw * 0.5,
            cy + 6.0,
            1.2,
            "X",
            [255, 220, 210],
        );

        // Category sidebar.
        for (index, label) in CATEGORY_LABELS.iter().enumerate() {
            let (x, y, w, h) = shop_category_rect(width, height, index);
            let selected = shop.category == index;
            if let Some(uv) = self.atlas.ui_uv("tab") {
                self.sprite(
                    width,
                    height,
                    x,
                    y,
                    w,
                    h,
                    uv,
                    if selected {
                        [255, 230, 160]
                    } else {
                        [220, 210, 190]
                    },
                    1.0,
                );
                self.quad_alpha(
                    width,
                    height,
                    x + 3.0,
                    y + 3.0,
                    w - 6.0,
                    h - 6.0,
                    [22, 22, 26],
                    0.86,
                );
            } else {
                self.quad(
                    width,
                    height,
                    x,
                    y,
                    w,
                    h,
                    if selected { [78, 64, 34] } else { [28, 30, 36] },
                );
            }
            if selected {
                self.quad(width, height, x, y, 3.0, h, [225, 185, 90]);
            }
            self.text(
                width,
                height,
                x + 9.0,
                y + 7.0,
                0.9,
                label,
                if selected {
                    [250, 225, 160]
                } else {
                    [185, 190, 200]
                },
            );
        }

        // Item cards.
        let filtered = shop.filtered_items();
        for (slot, item_index) in filtered.iter().enumerate().take(6) {
            let (x, y, w, h) = shop_card_rect(width, height, slot);
            let item = &shop.items[*item_index];
            let hover = player.shop_hover == Some(*item_index);
            let selected = player.shop_selected == Some(*item_index);
            let can_buy = affordable[*item_index] && player.shop_in_range;
            let border = if selected {
                [230, 190, 90]
            } else if hover {
                [120, 200, 255]
            } else {
                [82, 76, 58]
            };
            self.quad(width, height, x, y, w, h, border);
            self.quad(
                width,
                height,
                x + 2.0,
                y + 2.0,
                w - 4.0,
                h - 4.0,
                if hover || selected {
                    [38, 40, 48]
                } else {
                    [24, 26, 32]
                },
            );
            let icon_alpha = if can_buy { 1.0 } else { 0.45 };
            self.draw_item_icon(
                width,
                height,
                x + (w - 46.0) * 0.5,
                y + 8.0,
                46.0,
                item,
                icon_alpha,
            );
            let name: String = item.name.chars().take(16).collect();
            self.text_centered(
                width,
                height,
                x + w * 0.5,
                y + 60.0,
                0.8,
                &name,
                if can_buy {
                    [235, 235, 235]
                } else {
                    [135, 135, 145]
                },
            );
            self.text_centered(
                width,
                height,
                x + w * 0.5,
                y + 74.0,
                0.9,
                &format!("{}G", item.cost),
                if can_buy {
                    [245, 205, 70]
                } else {
                    [170, 95, 95]
                },
            );
            let owned = shop.owned_count(*item_index);
            if owned > 0 {
                self.quad(
                    width,
                    height,
                    x + w - 16.0,
                    y + 6.0,
                    9.0,
                    9.0,
                    [80, 255, 120],
                );
                if owned > 1 {
                    self.text(
                        width,
                        height,
                        x + w - 26.0,
                        y + 17.0,
                        0.7,
                        &format!("x{owned}"),
                        [170, 255, 190],
                    );
                }
            }
        }

        // Detail pane.
        let (dx, dy, dw, dh) = shop_detail_rect(width, height);
        self.quad(width, height, dx, dy, dw, dh, [82, 76, 58]);
        self.quad(
            width,
            height,
            dx + 2.0,
            dy + 2.0,
            dw - 4.0,
            dh - 4.0,
            [18, 20, 25],
        );
        let detail_index = player.shop_hover.or(player.shop_selected).or(shop.selected);
        if let Some(index) = detail_index {
            if let Some(item) = shop.items.get(index) {
                let can_buy = affordable[index] && player.shop_in_range && shop.has_free_slot();
                let name: String = item.name.chars().take(20).collect();
                self.text(
                    width,
                    height,
                    dx + 10.0,
                    dy + 8.0,
                    1.0,
                    &name,
                    [240, 235, 220],
                );
                self.text(
                    width,
                    height,
                    dx + 10.0,
                    dy + 22.0,
                    0.9,
                    &format!("{}G", item.cost),
                    [245, 205, 70],
                );
                self.draw_item_icon(width, height, dx + 10.0, dy + 40.0, 52.0, item, 1.0);

                let mut lines = Vec::new();
                if item.damage > 0.0 {
                    lines.push(format!("+{:.0} DAMAGE", item.damage));
                }
                if item.health > 0.0 {
                    lines.push(format!("+{:.0} HEALTH", item.health));
                }
                if item.mana > 0.0 {
                    lines.push(format!("+{:.0} MANA", item.mana));
                }
                if item.speed > 0.0 {
                    lines.push(format!("+{:.0} SPEED", item.speed));
                }
                if item.ability > 0.0 {
                    lines.push(format!("+{:.0}% ABILITY", item.ability));
                }
                if lines.is_empty() {
                    lines.push("NO BONUS".to_string());
                }
                for (i, line) in lines.iter().take(5).enumerate() {
                    self.text(
                        width,
                        height,
                        dx + 10.0,
                        dy + 100.0 + i as f32 * 12.0,
                        0.85,
                        line,
                        [200, 220, 240],
                    );
                }

                let owned = shop.owned_count(index);
                let status = if !player.shop_in_range {
                    "RETURN TO FOUNTAIN"
                } else if owned > 0 {
                    "OWNED"
                } else if !affordable[index] {
                    "NEED GOLD"
                } else if !shop.has_free_slot() {
                    "INVENTORY FULL"
                } else {
                    "IN STOCK"
                };
                self.text(
                    width,
                    height,
                    dx + 10.0,
                    dy + 166.0,
                    0.8,
                    status,
                    if can_buy {
                        [160, 235, 170]
                    } else {
                        [230, 130, 120]
                    },
                );

                let (bx, by, bw, bh) = shop_buy_rect(width, height);
                if let Some(uv) = self.atlas.ui_uv("tab") {
                    self.sprite(
                        width,
                        height,
                        bx,
                        by,
                        bw,
                        bh,
                        uv,
                        if can_buy {
                            [150, 255, 170]
                        } else {
                            [150, 150, 150]
                        },
                        if can_buy { 0.95 } else { 0.55 },
                    );
                    self.quad_alpha(
                        width,
                        height,
                        bx + 4.0,
                        by + 4.0,
                        bw - 8.0,
                        bh - 8.0,
                        [18, 20, 24],
                        0.82,
                    );
                } else {
                    self.quad(
                        width,
                        height,
                        bx,
                        by,
                        bw,
                        bh,
                        if can_buy { [52, 120, 58] } else { [55, 55, 62] },
                    );
                }
                self.text_centered(
                    width,
                    height,
                    bx + bw * 0.5,
                    by + 9.0,
                    0.9,
                    &format!("BUY  {}G", item.cost),
                    [245, 245, 245],
                );

                let (sx, sy, sw, sh) = shop_sell_rect(width, height);
                let can_sell = owned > 0;
                if let Some(uv) = self
                    .atlas
                    .ui_uv(if can_sell { "button_red" } else { "button" })
                {
                    self.sprite(
                        width,
                        height,
                        sx,
                        sy,
                        sw,
                        sh,
                        uv,
                        if can_sell {
                            [255, 200, 140]
                        } else {
                            [140, 140, 140]
                        },
                        if can_sell { 0.95 } else { 0.50 },
                    );
                    self.quad_alpha(
                        width,
                        height,
                        sx + 4.0,
                        sy + 3.0,
                        sw - 8.0,
                        sh - 6.0,
                        [18, 20, 24],
                        0.82,
                    );
                } else {
                    self.quad(
                        width,
                        height,
                        sx,
                        sy,
                        sw,
                        sh,
                        if can_sell {
                            [120, 72, 34]
                        } else {
                            [48, 48, 54]
                        },
                    );
                }
                let sell_text = if can_sell {
                    format!("SELL  +{}G", item.cost / 2)
                } else {
                    "SELL".to_string()
                };
                self.text_centered(
                    width,
                    height,
                    sx + sw * 0.5,
                    sy + 7.0,
                    0.8,
                    &sell_text,
                    if can_sell {
                        [255, 225, 180]
                    } else {
                        [130, 130, 140]
                    },
                );
            }
        }

        // Inventory row and footer.
        self.text(
            width,
            height,
            wx + 166.0,
            wy + 282.0,
            0.9,
            "INVENTORY",
            [220, 200, 150],
        );
        self.draw_inventory_slots(width, height, wx + 166.0, wy + 300.0, 38.0, 6.0, shop);
        self.text(
            width,
            height,
            wx + 520.0,
            wy + 282.0,
            0.9,
            &format!("GOLD {}G", player.gold),
            [245, 205, 70],
        );
        self.text_centered(
            width,
            height,
            wx + ww * 0.5,
            wy + wh - 18.0,
            0.8,
            "CLICK ITEM TO SELECT  ·  BUY  ·  B/ESC TO CLOSE",
            [160, 160, 170],
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_item_icon(
        &mut self,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        size: f32,
        item: &ItemConfig,
        alpha: f32,
    ) {
        if let Some(uv) = self.atlas.item_uv(&item.id) {
            self.sprite(width, height, x, y, size, size, uv, [255, 255, 255], alpha);
            return;
        }
        self.quad_alpha(width, height, x, y, size, size, [12, 14, 18], 0.92 * alpha);
        let c = item.color;
        let light = [
            c[0].saturating_add(45),
            c[1].saturating_add(45),
            c[2].saturating_add(45),
        ];
        let dark = [c[0] / 2, c[1] / 2, c[2] / 2];
        let q = |hud: &mut Hud, fx: f32, fy: f32, fw: f32, fh: f32, col: [u8; 3]| {
            hud.quad_alpha(
                width,
                height,
                x + fx * size,
                y + fy * size,
                fw * size,
                fh * size,
                col,
                alpha,
            );
        };
        match item.id.as_str() {
            "bronze_sword" => {
                q(self, 0.46, 0.12, 0.08, 0.50, light);
                q(self, 0.30, 0.58, 0.40, 0.08, c);
                q(self, 0.46, 0.66, 0.08, 0.18, dark);
            }
            "iron_plate" => {
                q(self, 0.34, 0.26, 0.32, 0.42, c);
                q(self, 0.22, 0.22, 0.16, 0.18, light);
                q(self, 0.62, 0.22, 0.16, 0.18, light);
                q(self, 0.46, 0.26, 0.08, 0.10, dark);
            }
            "fury" => {
                q(self, 0.30, 0.20, 0.08, 0.18, light);
                q(self, 0.62, 0.20, 0.08, 0.18, light);
                q(self, 0.38, 0.34, 0.24, 0.10, c);
                q(self, 0.44, 0.46, 0.12, 0.18, light);
            }
            "berserker" => {
                q(self, 0.28, 0.28, 0.44, 0.44, c);
                q(self, 0.39, 0.39, 0.22, 0.22, dark);
            }
            "ahrims" => {
                q(self, 0.46, 0.16, 0.08, 0.66, c);
                q(self, 0.36, 0.10, 0.28, 0.28, light);
                q(self, 0.42, 0.16, 0.16, 0.16, dark);
            }
            "boots" => {
                q(self, 0.34, 0.24, 0.22, 0.42, c);
                q(self, 0.34, 0.60, 0.42, 0.16, light);
                q(self, 0.34, 0.72, 0.42, 0.06, dark);
            }
            _ => {
                q(self, 0.28, 0.28, 0.44, 0.44, c);
                if let Some(ch) = item.name.chars().next() {
                    self.text_centered(
                        width,
                        height,
                        x + size * 0.5,
                        y + size * 0.36,
                        1.2,
                        &ch.to_string(),
                        [245, 245, 245],
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_inventory_slots(
        &mut self,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        slot: f32,
        gap: f32,
        shop: &ItemShop,
    ) {
        for i in 0..6 {
            let sx = x + i as f32 * (slot + gap);
            self.quad(
                width,
                height,
                sx - 1.0,
                y - 1.0,
                slot + 2.0,
                slot + 2.0,
                [82, 76, 58],
            );
            self.quad(width, height, sx, y, slot, slot, [18, 20, 25]);
            if let Some(index) = shop.owned[i] {
                if let Some(item) = shop.items.get(index) {
                    self.draw_item_icon(width, height, sx + 3.0, y + 3.0, slot - 6.0, item, 1.0);
                    let count = shop.owned_count(index);
                    if count > 1 {
                        self.text(
                            width,
                            height,
                            sx + slot - 12.0,
                            y + slot - 12.0,
                            0.8,
                            &format!("x{count}"),
                            [245, 220, 140],
                        );
                    }
                }
            } else {
                self.quad(
                    width,
                    height,
                    sx + slot * 0.5 - 2.0,
                    y + slot * 0.5 - 2.0,
                    4.0,
                    4.0,
                    [48, 52, 60],
                );
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
    pub fn sprite(
        &mut self,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        uv: [f32; 4],
        tint: [u8; 3],
        alpha: f32,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let x0 = 2.0 * x / width - 1.0;
        let x1 = 2.0 * (x + w) / width - 1.0;
        let y0 = 1.0 - 2.0 * y / height;
        let y1 = 1.0 - 2.0 * (y + h) / height;
        let color = [
            srgb_to_linear(tint[0] as f32 / 255.0),
            srgb_to_linear(tint[1] as f32 / 255.0),
            srgb_to_linear(tint[2] as f32 / 255.0),
            alpha,
        ];
        let base = self.vertices.len() as u32;
        self.vertices.push(UiVertex {
            position: [x0, y0, 0.0],
            uv: [uv[0], uv[1]],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x1, y0, 0.0],
            uv: [uv[2], uv[1]],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x0, y1, 0.0],
            uv: [uv[0], uv[3]],
            color,
        });
        self.vertices.push(UiVertex {
            position: [x1, y1, 0.0],
            uv: [uv[2], uv[3]],
            color,
        });
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }

    #[allow(clippy::too_many_arguments)]
    fn quad_alpha(
        &mut self,
        width: f32,
        height: f32,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: [u8; 3],
        alpha: f32,
    ) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        let x0 = 2.0 * x / width - 1.0;
        let x1 = 2.0 * (x + w) / width - 1.0;
        let y0 = 1.0 - 2.0 * y / height;
        let y1 = 1.0 - 2.0 * (y + h) / height;

        let base = self.vertices.len() as u32;
        let uv = self.atlas.white_uv;
        let rgba = [
            srgb_to_linear(color[0] as f32 / 255.0),
            srgb_to_linear(color[1] as f32 / 255.0),
            srgb_to_linear(color[2] as f32 / 255.0),
            alpha,
        ];
        self.vertices.push(UiVertex {
            position: [x0, y0, 0.0],
            uv: [uv[0], uv[1]],
            color: rgba,
        });
        self.vertices.push(UiVertex {
            position: [x1, y0, 0.0],
            uv: [uv[0], uv[1]],
            color: rgba,
        });
        self.vertices.push(UiVertex {
            position: [x0, y1, 0.0],
            uv: [uv[0], uv[1]],
            color: rgba,
        });
        self.vertices.push(UiVertex {
            position: [x1, y1, 0.0],
            uv: [uv[0], uv[1]],
            color: rgba,
        });
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 1, base + 3]);
    }

    #[allow(clippy::too_many_arguments)]
    fn quad(&mut self, width: f32, height: f32, x: f32, y: f32, w: f32, h: f32, color: [u8; 3]) {
        self.quad_alpha(width, height, x, y, w, h, color, 1.0);
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
