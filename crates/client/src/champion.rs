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
    pub idle: u32,
    pub walk: u32,
    pub scale: f32,
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
