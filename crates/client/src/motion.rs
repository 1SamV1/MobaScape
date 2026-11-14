//! Weighty player locomotion.
//!
//! Turn time, acceleration and stopping time come from the champion's
//! `weight` (or explicit timings), so a king turns like a king instead of
//! snapping to a new heading. The walk animation advances from distance
//! covered rather than wall-clock time, which keeps the feet planted while
//! speeding up, slowing down and turning.
//!
//! This is deliberately float-based and lives on the render tick; minions and
//! structures keep their existing simple movement.

use std::f32::consts::{PI, TAU};

use glam::Vec3;

use crate::champion::ChampionConfig;
use crate::scene::{HeightGrid, WorldCollision};

/// The most a step lifts the lightest champion, in world units.
const STEP_LIFT: f32 = 14.0;
const SETTLE_LIFT: f32 = 10.0;
const HOVER_LIFT: f32 = 7.0;
const HOVER_PERIOD: f32 = 2.6;

#[derive(Debug, Clone, Copy)]
pub struct Locomotion {
    /// Radians per second. A heavy champion needs longer to come round.
    pub turn_rate: f32,
    pub accel: f32,
    pub decel: f32,
    pub stride: f32,
    /// 1 for a feather, 0 for a colossus.
    pub lightness: f32,
    pub bounce: f32,
}

impl Locomotion {
    pub fn new(champion: &ChampionConfig, walk_loop_ms: f32) -> Self {
        let handling = champion.handling();
        let turn_time = handling.turn_time.max(0.02);
        let accel_time = handling.accel_time.max(0.001);
        let stop_time = handling.stop_time.max(0.001);
        let stride = if champion.stride > 0.0 {
            champion.stride
        } else {
            champion.speed * walk_loop_ms / 1000.0
        };
        Self {
            // handling.turn_time is the time for a half turn.
            turn_rate: PI / turn_time,
            accel: champion.speed / accel_time,
            decel: champion.speed / stop_time,
            stride: stride.max(1.0),
            lightness: handling.lightness,
            bounce: champion.bounce.max(0.0),
        }
    }
}

pub struct PlayerMotion {
    locomotion: Locomotion,
    speed: f32,
    travelled: f32,
    resting: f32,
    was_moving: bool,
    pub lift: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct StepResult {
    pub pos: Vec3,
    pub yaw: f32,
    pub moving: bool,
    /// Remaining angle to the requested heading, in radians.
    pub facing_error: f32,
}

fn angle_delta(target: f32, current: f32) -> f32 {
    let mut delta = (target - current) % TAU;
    if delta > PI {
        delta -= TAU;
    } else if delta < -PI {
        delta += TAU;
    }
    delta
}

impl PlayerMotion {
    pub fn new(champion: &ChampionConfig, walk_loop_ms: f32) -> Self {
        Self {
            locomotion: Locomotion::new(champion, walk_loop_ms),
            speed: 0.0,
            travelled: 0.0,
            resting: 0.0,
            was_moving: false,
            lift: 0.0,
        }
    }

    pub fn reset(&mut self) {
        self.speed = 0.0;
        self.travelled = 0.0;
        self.resting = 0.0;
        self.was_moving = false;
        self.lift = 0.0;
    }

    fn lift_for(&mut self, dt: f32, moved: f32, moving: bool) -> f32 {
        let spring = self.locomotion.lightness * self.locomotion.lightness * self.locomotion.bounce;
        if spring <= 0.0 {
            return 0.0;
        }
        let want = if moving {
            self.was_moving = true;
            self.resting = 0.0;
            self.travelled += moved;
            let phase = self.travelled / self.locomotion.stride * 2.0;
            STEP_LIFT * (phase * PI).sin().abs()
        } else {
            if self.was_moving {
                self.was_moving = false;
                self.resting = 0.0;
                self.travelled = 0.0;
            }
            self.resting += dt;
            let settle =
                SETTLE_LIFT * (-self.resting / 0.12).exp() * (self.resting * PI / 0.11).sin().abs();
            let eased = (self.resting / 0.6).min(1.0);
            let hover =
                HOVER_LIFT * eased * (0.5 - 0.5 * (self.resting * TAU / HOVER_PERIOD).cos());
            settle + hover
        };
        let blend = 1.0 - (-dt / 0.04).exp();
        self.lift += (want * spring - self.lift) * blend;
        self.lift.max(0.0)
    }

    /// Advance one frame. `desired` is the direction the player is pointing
    /// at; `None` means coast to a stop in the current facing.
    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        mut pos: Vec3,
        yaw: f32,
        desired: Option<Vec3>,
        top_speed: f32,
        speed_mult: f32,
        dt: f32,
        height: &HeightGrid,
        collision: &WorldCollision,
    ) -> StepResult {
        let dt = dt.clamp(0.0, 0.25);
        let desired = desired.and_then(|direction| {
            let flat = Vec3::new(direction.x, 0.0, direction.z);
            (flat.length_squared() > 1e-6).then(|| flat.normalize())
        });

        let mut yaw = yaw;
        let (target_speed, facing_error) = if let Some(direction) = desired {
            let wanted_yaw = direction.x.atan2(direction.z);
            let delta = angle_delta(wanted_yaw, yaw);
            let max_turn = self.locomotion.turn_rate * dt;
            yaw += delta.clamp(-max_turn, max_turn);
            let error = angle_delta(wanted_yaw, yaw).abs();
            // Move while turning, but slower the further the body is from
            // the requested heading. The floor keeps the first tick moving.
            let agreement = error.cos().max(0.0);
            let scale = 0.2 + 0.8 * agreement;
            (top_speed * speed_mult * scale, error)
        } else {
            (0.0, 0.0)
        };

        if self.speed < target_speed {
            self.speed = (self.speed + self.locomotion.accel * dt).min(target_speed);
        } else {
            self.speed = (self.speed - self.locomotion.decel * dt).max(target_speed);
        }

        let facing = Vec3::new(yaw.sin(), 0.0, yaw.cos());
        let mut moved = 0.0;
        if self.speed > 0.01 {
            let next = collision.slide_move(pos, facing * self.speed * dt, height);
            moved = Vec3::new(next.x - pos.x, 0.0, next.z - pos.z).length();
            if moved <= 1e-4 && desired.is_none() {
                self.speed = 0.0;
            }
            pos = next;
            pos.y = height.height_at(pos.x, pos.z);
        }

        let moving = self.speed > 2.0;
        self.lift_for(dt, moved, moving);

        StepResult {
            pos,
            yaw,
            moving,
            facing_error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::champion::ChampionConfig;

    fn champion(weight: f32) -> ChampionConfig {
        serde_json::from_str(&format!(
            r#"{{
                "name": "test",
                "model": 1,
                "idle": 1,
                "walk": 2,
                "scale": 1.0,
                "health": 100,
                "mana": 100,
                "damage": 1,
                "attack_range": 100,
                "attack_interval": 1.0,
                "speed": 400,
                "ranged_auto": false,
                "q_damage": 1,
                "r_damage": 1,
                "e_heal": 1,
                "e_shield": 1,
                "w_distance": 1,
                "weight": {weight}
            }}"#
        ))
        .unwrap()
    }

    #[test]
    fn heavier_champions_turn_and_accelerate_slower() {
        let light = Locomotion::new(&champion(10.0), 700.0);
        let heavy = Locomotion::new(&champion(90.0), 700.0);
        assert!(heavy.turn_rate < light.turn_rate);
        assert!(heavy.accel < light.accel);
        assert!(heavy.decel < light.decel);
        assert!(heavy.lightness < light.lightness);
    }

    #[test]
    fn default_stride_follows_top_speed_and_walk_loop() {
        let locomotion = Locomotion::new(&champion(50.0), 500.0);
        assert!((locomotion.stride - 200.0).abs() < 0.01);
    }

    #[test]
    fn angle_delta_takes_the_short_way_round() {
        assert!((angle_delta(0.1, 6.2) - 0.183).abs() < 0.01);
        assert!((angle_delta(6.2, 0.1) + 0.183).abs() < 0.01);
    }
}
