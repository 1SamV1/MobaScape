use crate::animated::{AnimatedScene, UnitKind};
use crate::items::shop_row_rect;
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
    pub gold: u32,
    pub items: [Option<[u8; 3]>; 6],
    pub shop_open: bool,
    pub shop_hover: Option<usize>,
    pub shop_affordable: [bool; 6],
    pub shop_costs: [u32; 6],
    pub shop_colors: [[u8; 3]; 6],
    pub shop_owned: [bool; 6],
    pub shop_in_range: bool,
}

pub struct Hud {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Hud {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn build_minimap(&mut self, width: f32, height: f32, scene: &AnimatedScene) {
        let size = 190.0;
        let x = width - size - 18.0;
        let y = height - size - 18.0;
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
                    self.quad(width, height, ux - 1.0, uy - 1.0, 2.0, 2.0, color);
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
        if let Some((remaining, duration, color)) = player.buff {
            let fill = (remaining / duration.max(0.1)).clamp(0.0, 1.0);
            self.panel(width, height, 22.0, 82.0, 150.0, 10.0, fill, color);
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
    pub fn build_shop(&mut self, width: f32, height: f32, player: &PlayerHud) {
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

        for row in 0..6 {
            let (x, y, w, h) = shop_row_rect(width, height, row);
            let hover = player.shop_hover == Some(row);
            let affordable = player.shop_affordable[row] && player.shop_in_range;
            let border = if hover {
                [120, 200, 255]
            } else if affordable {
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
            let swatch = if affordable {
                player.shop_colors[row]
            } else {
                [56, 58, 66]
            };
            // The swatch is the item colour when affordable; owned items are
            // marked with a green corner so duplicates remain readable.
            self.quad(width, height, x + 8.0, y + 6.0, 24.0, 24.0, swatch);
            let cost_frac = (player.shop_costs[row] as f32 / 1000.0).clamp(0.0, 1.0);
            self.quad(width, height, x + 40.0, y + 27.0, 190.0, 4.0, [10, 12, 16]);
            self.quad(
                width,
                height,
                x + 40.0,
                y + 27.0,
                190.0 * cost_frac,
                4.0,
                if affordable {
                    [225, 185, 55]
                } else {
                    [110, 55, 55]
                },
            );
            if player.shop_owned[row] {
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
