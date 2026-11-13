use crate::scene::Vertex;

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

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        width: f32,
        height: f32,
        health: f32,
        max_health: f32,
        mana: f32,
        max_mana: f32,
        q_cooldown: f32,
        q_max: f32,
        nexus0: (f32, f32),
        nexus1: (f32, f32),
        wave_timer: f32,
        wave_interval: f32,
        match_over: Option<u8>,
        mouse: (f64, f64),
    ) {
        self.vertices.clear();
        self.indices.clear();

        // Health and mana panels, top-left.
        self.panel(
            width,
            height,
            22.0,
            22.0,
            280.0,
            22.0,
            health / max_health,
            [70, 190, 95],
        );
        self.panel(
            width,
            height,
            22.0,
            50.0,
            280.0,
            14.0,
            mana / max_mana,
            [70, 120, 235],
        );

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

        // Q cooldown box, bottom-centre.
        let qx = width * 0.5 - 34.0;
        let qy = height - 104.0;
        self.quad(width, height, qx - 3.0, qy - 3.0, 74.0, 74.0, [12, 12, 16]);
        self.quad(width, height, qx, qy, 68.0, 68.0, [38, 38, 46]);
        if q_cooldown > 0.0 {
            let frac = (q_cooldown / q_max).clamp(0.0, 1.0);
            let fill_h = 68.0 * frac;
            self.quad(
                width,
                height,
                qx,
                qy + 68.0 - fill_h,
                68.0,
                fill_h,
                [20, 20, 28],
            );
        } else {
            self.quad(
                width,
                height,
                qx + 22.0,
                qy + 22.0,
                24.0,
                24.0,
                [255, 150, 40],
            );
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
