use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Clone, Deserialize)]
pub struct XteaKeyEntry {
    pub archive: u32,
    pub group: u32,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub mapsquare: i32,
    pub key: [i32; 4],
}

#[derive(Debug, Clone, Default)]
pub struct XteaKeys {
    keys: HashMap<(u32, u32), [u32; 4]>,
    names: HashMap<(u32, u32), String>,
}

impl XteaKeys {
    pub fn from_json(path: impl AsRef<Path>) -> Result<Self> {
        let data = std::fs::read(path)?;
        let entries: Vec<XteaKeyEntry> = serde_json::from_slice(&data)?;
        let mut keys = HashMap::with_capacity(entries.len());
        let mut names = HashMap::with_capacity(entries.len());
        for e in entries {
            let key = e.key.map(|v| v as u32);
            keys.insert((e.archive, e.group), key);
            if !e.name.is_empty() {
                names.insert((e.archive, e.group), e.name);
            }
        }
        Ok(Self { keys, names })
    }

    pub fn get(&self, index: u32, group: u32) -> Option<[u32; 4]> {
        self.keys.get(&(index, group)).copied()
    }

    pub fn name(&self, index: u32, group: u32) -> Option<&str> {
        self.names.get(&(index, group)).map(String::as_str)
    }

    /// Find the locations group for a map region, e.g. `l44_47`.
    pub fn find_locations_group(&self, region_x: u32, region_y: u32) -> Option<u32> {
        let target = format!("l{region_x}_{region_y}");
        self.names
            .iter()
            .find(|((index, _), name)| *index == 5 && **name == target)
            .map(|((_, group), _)| *group)
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }
}
