use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ChampionConfig {
    /// File stem used by the hero-select screen and portrait atlas.
    #[serde(default)]
    pub id: String,
    pub name: String,
    /// Short role label shown under the name on the hero-select card.
    #[serde(default)]
    pub role: String,
    /// One-line flavour text shown on the hero-select card.
    #[serde(default)]
    pub tagline: String,
    pub model: u32,
    /// Extra OSRS models rendered with the same pose. Multi-model NPCs (for
    /// example King Awowogei's crown) are authored as separate meshes.
    #[serde(default)]
    pub attachments: Vec<u32>,
    pub idle: u32,
    pub walk: u32,
    pub scale: f32,
    /// Extra yaw applied to the rendered model when its authored forward axis
    /// differs from the engine's +Z movement convention.
    #[serde(default)]
    pub model_yaw: f32,
    /// Multiplier for sideways rotation in the walk clip. Some cache walk
    /// animations (notably King Awowogei) twist the torso/arms far more than
    /// reads well at MOBA scale; 1.0 keeps the clip untouched.
    #[serde(default = "default_walk_sway")]
    pub walk_sway: f32,
    /// Multiplier for the walk clip's arm-group transforms. 0 freezes the
    /// arms in their rest pose while the torso and legs animate.
    #[serde(default = "one")]
    pub walk_arm: f32,
    /// Seconds from committing to an attack to the hit landing. Zero keeps
    /// instant attacks.
    #[serde(default)]
    pub attack_windup: f32,
    pub health: f32,
    pub mana: f32,
    pub damage: f32,
    pub attack_range: f32,
    pub attack_interval: f32,
    pub speed: f32,
    pub ranged_auto: bool,
    pub q_damage: f32,
    pub r_damage: f32,
    pub e_heal: f32,
    pub e_shield: f32,
    pub w_distance: f32,
    /// How heavy the champion is to move, 0 (feather) to 100 (colossus).
    /// Drives turn, acceleration and stopping time unless they are set by hand.
    #[serde(default)]
    pub weight: Option<f32>,
    /// Seconds to turn half way round. Defaults from `weight`.
    #[serde(default)]
    pub turn_time: Option<f32>,
    /// Seconds from standing to top speed.
    #[serde(default)]
    pub accel_time: Option<f32>,
    /// Seconds from top speed to a stop.
    #[serde(default)]
    pub stop_time: Option<f32>,
    /// World units covered in one pass of the walk animation. Zero derives it
    /// from top speed and the animation's own loop length.
    #[serde(default)]
    pub stride: f32,
    /// Scales the drawing-only spring in the champion's step.
    #[serde(default = "one")]
    pub bounce: f32,
}

fn default_walk_sway() -> f32 {
    1.0
}

fn one() -> f32 {
    1.0
}

/// Movement timings derived from a champion's weight, with per-file overrides.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Handling {
    pub turn_time: f32,
    pub accel_time: f32,
    pub stop_time: f32,
    /// 1 for the lightest champion, 0 for the heaviest.
    pub lightness: f32,
}

impl ChampionConfig {
    pub fn handling(&self) -> Handling {
        let from_weight = self.weight.map(|weight| {
            let weight = weight.clamp(0.0, 100.0);
            (
                0.06 + 0.0046 * weight,
                0.04 + 0.0031 * weight,
                0.005 + 0.0023 * weight,
            )
        });
        let (turn, accel, stop) = from_weight.unwrap_or((0.0, 0.0, 0.0));
        Handling {
            turn_time: self.turn_time.unwrap_or(turn).max(0.0),
            accel_time: self.accel_time.unwrap_or(accel).max(0.0),
            stop_time: self.stop_time.unwrap_or(stop).max(0.0),
            lightness: self
                .weight
                .map_or(0.0, |weight| 1.0 - weight.clamp(0.0, 100.0) / 100.0),
        }
    }
}

impl ChampionConfig {
    pub fn load(name: &str) -> Result<Self> {
        let path = format!("data/champions/{name}.json");
        let data =
            std::fs::read_to_string(&path).with_context(|| format!("reading champion {}", path))?;
        let mut config: Self = serde_json::from_str(&data)?;
        config.fill_defaults(name);
        Ok(config)
    }

    /// Load every champion in `data/champions` so the roster stays data-driven.
    pub fn load_all() -> Result<Vec<Self>> {
        let dir = Path::new("data/champions");
        let mut paths = std::fs::read_dir(dir)
            .with_context(|| format!("reading champion directory {}", dir.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect::<Vec<_>>();
        paths.sort();

        let mut champions = Vec::with_capacity(paths.len());
        for path in paths {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            champions.push(Self::load(stem)?);
        }
        if champions.is_empty() {
            anyhow::bail!("no champions found in {}", dir.display());
        }
        Ok(champions)
    }

    fn fill_defaults(&mut self, name: &str) {
        if self.id.is_empty() {
            self.id = name.to_string();
        }
        if self.role.is_empty() {
            self.role = if self.ranged_auto {
                "RANGED SKIRMISHER"
            } else if self.health >= 650.0 {
                "TANK MAGE"
            } else {
                "BALANCED FIGHTER"
            }
            .to_string();
        }
        if self.tagline.is_empty() {
            self.tagline = "A champion of the Karamja rift.".to_string();
        }
    }
}
