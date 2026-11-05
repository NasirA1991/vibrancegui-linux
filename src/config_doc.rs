use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::AppPaths;

#[derive(Debug, Clone)]
pub struct ConfigDoc {
    pub path: PathBuf,
    pub lines: Vec<String>,
}

impl ConfigDoc {
    pub fn load(path: impl Into<PathBuf>) -> Result<Option<Self>> {
        let path = path.into();
        if !path.exists() {
            return Ok(None);
        }
        let text =
            fs::read_to_string(&path).with_context(|| format!("read config {}", path.display()))?;
        Ok(Some(Self {
            lines: text.lines().map(str::to_string).collect(),
            path,
        }))
    }

    pub fn get_f32(&self, key: &str) -> Option<f32> {
        for line in &self.lines {
            if let Some((k, v)) = parse_line(line)
                && k == key
            {
                return v.parse::<f32>().ok();
            }
        }
        None
    }

    /// Set a key. Existing active lines are updated in place; if absent, the
    /// key is appended. Commented template lines are left untouched.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        for line in &self.lines {
            if let Some((k, v)) = parse_line(line)
                && k == key
            {
                return match v.to_ascii_lowercase().as_str() {
                    "1" | "true" | "yes" | "on" => Some(true),
                    "0" | "false" | "no" | "off" => Some(false),
                    _ => None,
                };
            }
        }
        None
    }

    pub fn set_f32(&mut self, key: &str, value: f32) {
        let formatted = format_float(value);
        let mut found = false;
        for line in &mut self.lines {
            if let Some((k, _)) = parse_line(line)
                && k == key
            {
                let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                *line = format!("{indent}{key} = {formatted}");
                found = true;
                break;
            }
        }
        if !found {
            self.lines.push(format!("{key} = {formatted}"));
        }
    }

    pub fn remove_key(&mut self, key: &str) {
        self.lines
            .retain(|line| parse_line(line).map(|(k, _)| k != key).unwrap_or(true));
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.path = path;
    }

    pub fn save(&self, paths: &AppPaths) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            anyhow::bail!("config path is empty");
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        if self.path.exists() {
            let _ = backup(paths, &self.path);
        }
        let tmp = self.path.with_extension("conf.vibrancegui.tmp");
        let mut text = self.lines.join("\n");
        if !text.ends_with('\n') {
            text.push('\n');
        }
        fs::write(&tmp, text).with_context(|| format!("write temp config {}", tmp.display()))?;
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("replace config {}", self.path.display()))?;
        Ok(())
    }
}

fn parse_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }
    let without_comment = trimmed.split('#').next().unwrap_or(trimmed).trim();
    let (key, value) = without_comment.split_once('=')?;
    let key = key.trim();
    if key.is_empty() {
        return None;
    }
    Some((key.to_string(), value.trim().to_string()))
}

fn format_float(value: f32) -> String {
    let value = if value == 0.0 { 0.0 } else { value };
    format!("{value:.3}")
}

fn backup(paths: &AppPaths, path: &Path) -> Result<()> {
    fs::create_dir_all(&paths.backups_dir)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("config.conf");
    let dest = paths.backups_dir.join(format!("{name}.{stamp}.bak"));
    // Only keep one backup per second; slider writes should not fill the disk.
    if !dest.exists() {
        let _ = fs::copy(path, dest);
    }
    Ok(())
}

/// A small helper for the values the UI edits.
#[derive(Debug, Clone, Copy)]
pub struct Knob {
    pub key: &'static str,
    pub label: &'static str,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

pub const KNOBS: &[Knob] = &[
    Knob {
        key: "vibrance",
        label: "Vibrance",
        min: 0.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "saturation",
        label: "Saturation",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "temperature",
        label: "Temperature",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "contrast",
        label: "Contrast",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "brightness",
        label: "Brightness",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "gamma",
        label: "Gamma",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "exposure",
        label: "Exposure",
        min: -3.0,
        max: 3.0,
        step: 0.01,
    },
    Knob {
        key: "hue_deg",
        label: "Hue rotation (°)",
        min: -180.0,
        max: 180.0,
        step: 1.0,
    },
    Knob {
        key: "red_gain",
        label: "Red gain",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "green_gain",
        label: "Green gain",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "blue_gain",
        label: "Blue gain",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "shadows",
        label: "Shadows",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "midtones",
        label: "Midtones",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "highlights",
        label: "Highlights",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "red_hue",
        label: "Red hue",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "green_hue",
        label: "Green hue",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "blue_hue",
        label: "Blue hue",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "cyan_hue",
        label: "Cyan hue",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "magenta_hue",
        label: "Magenta hue",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "yellow_hue",
        label: "Yellow hue",
        min: -1.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "shadow_tint",
        label: "Shadow tint",
        min: 0.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "shadow_tint_hue",
        label: "Shadow tint hue (°)",
        min: 0.0,
        max: 360.0,
        step: 1.0,
    },
    Knob {
        key: "highlight_tint",
        label: "Highlight tint",
        min: 0.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "highlight_tint_hue",
        label: "Highlight tint hue (°)",
        min: 0.0,
        max: 360.0,
        step: 1.0,
    },
    Knob {
        key: "vignette",
        label: "Vignette",
        min: 0.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "daltonize_severity",
        label: "Daltonize severity",
        min: 0.0,
        max: 1.0,
        step: 0.01,
    },
    Knob {
        key: "daltonize_strength",
        label: "Daltonize strength",
        min: 0.0,
        max: 1.0,
        step: 0.01,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_active_and_commented_keys() {
        let doc = ConfigDoc {
            path: PathBuf::from("x.conf"),
            lines: vec![
                "# vibrance = 0.4".into(),
                "  saturation = 0.2 # inline".into(),
                "enabled = true".into(),
            ],
        };
        assert_eq!(doc.get_f32("vibrance"), None);
        assert_eq!(doc.get_f32("saturation"), Some(0.2));
        assert_eq!(doc.get_f32("enabled"), None);
    }

    #[test]
    fn updates_and_removes_keys() {
        let mut doc = ConfigDoc {
            path: PathBuf::from("x.conf"),
            lines: vec!["vibrance = 0.1".into(), "contrast = 0.0".into()],
        };
        doc.set_f32("vibrance", 0.25);
        assert_eq!(doc.lines[0], "vibrance = 0.250");
        doc.remove_key("contrast");
        assert_eq!(doc.lines, vec!["vibrance = 0.250"]);
    }

    #[test]
    fn appends_missing_key() {
        let mut doc = ConfigDoc {
            path: PathBuf::from("x.conf"),
            lines: vec!["# vibrance = 0.4".into()],
        };
        doc.set_f32("vibrance", 0.3);
        assert_eq!(doc.lines.last().unwrap(), "vibrance = 0.300");
    }
}

#[cfg(test)]
mod save_tests {
    use super::*;
    use crate::paths::AppPaths;

    fn test_paths(root: &std::path::Path) -> AppPaths {
        AppPaths {
            config_dir: root.join("config/vibrancegui"),
            cache_dir: root.join("cache/vibrancegui"),
            state_file: root.join("config/vibrancegui/state.json"),
            backups_dir: root.join("config/vibrancegui/backups"),
            vksumi_config_dir: root.join("config/vkSumi"),
            home: root.to_path_buf(),
        }
    }

    #[test]
    fn saves_atomically_and_keeps_backup() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());
        paths.ensure().unwrap();
        let path = paths.vksumi_games_dir().join("game.conf");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "vibrance = 0.100\n").unwrap();

        let mut doc = ConfigDoc::load(&path).unwrap().unwrap();
        doc.set_f32("vibrance", 0.5);
        doc.save(&paths).unwrap();

        let loaded = ConfigDoc::load(&path).unwrap().unwrap();
        assert_eq!(loaded.get_f32("vibrance"), Some(0.5));
        let backups: Vec<_> = std::fs::read_dir(&paths.backups_dir)
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
    }
}
