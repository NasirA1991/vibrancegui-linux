use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::paths::AppPaths;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavedState {
    #[serde(default)]
    pub schema_version: u32,
    #[serde(default)]
    pub games: BTreeMap<u32, GameMapping>,
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            schema_version: 1,
            games: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameMapping {
    pub config_path: PathBuf,
    pub exe: String,
    pub updated_unix: u64,
}

impl SavedState {
    pub fn load(paths: &AppPaths) -> Self {
        let Ok(text) = std::fs::read_to_string(&paths.state_file) else {
            return Self::default();
        };
        let mut state: Self = serde_json::from_str(&text).unwrap_or_default();
        if state.schema_version == 0 {
            state.schema_version = 1;
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
        assert_eq!(SavedState::default().schema_version, 1);
        assert!(SavedState::default().games.is_empty());
    }
}
