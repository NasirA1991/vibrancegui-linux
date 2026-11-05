use anyhow::{Context, Result};
use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub state_file: PathBuf,
    pub backups_dir: PathBuf,
    pub vksumi_config_dir: PathBuf,
    pub home: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let base = BaseDirs::new().context("could not determine the home directory")?;
        let home = base.home_dir().to_path_buf();

        let project = ProjectDirs::from("", "", "vibrancegui");
        let config_dir = project
            .as_ref()
            .map(|p| p.config_dir().to_path_buf())
            .unwrap_or_else(|| base.config_dir().join("vibrancegui"));
        let cache_dir = project
            .as_ref()
            .map(|p| p.cache_dir().to_path_buf())
            .unwrap_or_else(|| base.cache_dir().join("vibrancegui"));

        let vksumi_config_dir = base.config_dir().join("vkSumi");

        Ok(Self {
            state_file: config_dir.join("state.json"),
            backups_dir: config_dir.join("backups"),
            config_dir,
            cache_dir,
            vksumi_config_dir,
            home,
        })
    }

    pub fn ensure(&self) -> Result<()> {
        std::fs::create_dir_all(&self.config_dir)?;
        std::fs::create_dir_all(&self.backups_dir)?;
        // Artwork is optional; a read-only cache must not prevent the app
        // from saving profiles or state.
        let _ = std::fs::create_dir_all(&self.cache_dir);
        Ok(())
    }

    pub fn global_vksumi_conf(&self) -> PathBuf {
        self.vksumi_config_dir.join("vkSumi.conf")
    }

    pub fn vksumi_games_dir(&self) -> PathBuf {
        self.vksumi_config_dir.join("games")
    }

    pub fn is_inside_vksumi(&self, path: &Path) -> bool {
        path.starts_with(&self.vksumi_config_dir)
    }
}

/// Expand the same simple forms vkSumi accepts in `VKSUMI_CONFIG_FILE`.
pub fn expand_user_path(value: &str, home: &Path) -> PathBuf {
    let mut s = value.to_string();
    if let Some(rest) = s.strip_prefix("~/") {
        s = home.join(rest).to_string_lossy().into_owned();
    } else if s == "~" {
        s = home.to_string_lossy().into_owned();
    }
    if let Ok(user) = std::env::var("USER") {
        s = s.replace("$USER", &user);
    }
    s = s.replace("$HOME", &home.to_string_lossy());
    PathBuf::from(s)
}
