use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use crate::paths::AppPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedState {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub games: BTreeMap<u32, GameMapping>,
    #[serde(default)]
    pub tracked_steam: BTreeSet<u32>,
    #[serde(default)]
    pub manual_games: BTreeMap<u32, ManualGame>,
    #[serde(default)]
    pub next_manual_id: u32,
    #[serde(default)]
    pub tracking_initialized: bool,
    #[serde(default)]
    pub shortcuts_initialized: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "auto".to_string()
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            schema_version: 2,
            games: BTreeMap::new(),
            tracked_steam: BTreeSet::new(),
            manual_games: BTreeMap::new(),
            next_manual_id: 1,
            tracking_initialized: false,
            shortcuts_initialized: false,
            theme: default_theme(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMapping {
    pub config_path: PathBuf,
    pub exe: String,
    pub updated_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManualGame {
    pub name: String,
    pub exe: PathBuf,
    pub icon: Option<PathBuf>,
    pub config_path: PathBuf,
}

impl SavedState {
    pub fn load(paths: &AppPaths) -> Self {
        let Ok(text) = std::fs::read_to_string(&paths.state_file) else {
            return Self::default();
        };
        let mut state: Self = serde_json::from_str(&text).unwrap_or_default();
        if state.schema_version < 2 {
            state.schema_version = 2;
        }
        state
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        if let Some(parent) = paths.state_file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = paths.state_file.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(self)?;
        std::fs::write(&tmp, text).context("write state temp file")?;
        std::fs::rename(&tmp, &paths.state_file).context("replace state file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_has_current_schema() {
        assert_eq!(SavedState::default().schema_version, 2);
        assert!(SavedState::default().games.is_empty());
    }
}
