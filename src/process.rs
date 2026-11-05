use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::paths::{AppPaths, expand_user_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedGame {
    pub pid: i32,
    pub appid: Option<u32>,
    pub exe: String,
    pub config_path: Option<PathBuf>,
    pub layer_mapped: bool,
    pub explicitly_disabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Observation {
    pub games: Vec<ObservedGame>,
    pub scan_available: bool,
}

pub fn scan(paths: &AppPaths) -> Observation {
    let mut candidates: BTreeMap<String, ObservedGame> = BTreeMap::new();
    let mut scan_available = false;

    let Ok(entries) = fs::read_dir("/proc") else {
        return Observation {
            games: Vec::new(),
            scan_available: false,
        };
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        if pid <= 1 {
            continue;
        }
        let proc_dir = entry.path();
        let Some(env) = read_environ(&proc_dir) else {
            continue;
        };
        scan_available = true;
        if env.get("ENABLE_VKSUMI").map(String::as_str) != Some("1") {
            continue;
        }
        let disabled = env.get("DISABLE_VKSUMI").map(String::as_str) == Some("1");
        let appid = env
            .get("SteamAppId")
            .or_else(|| env.get("SteamGameId"))
            .and_then(|s| s.parse::<u32>().ok());
        let Some(exe) = executable_basename(&proc_dir) else {
            continue;
        };
        let layer_mapped = layer_mapped(&proc_dir);
        let config_path = env
            .get("VKSUMI_CONFIG_FILE")
            .filter(|s| !s.is_empty())
            .map(|s| {
                let p = expand_user_path(s, &paths.home);
                if p.is_absolute() {
                    p
                } else {
                    fs::read_link(proc_dir.join("cwd"))
                        .map(|cwd| cwd.join(&p))
                        .unwrap_or(p)
                }
            })
            .or_else(|| {
                let p = paths.vksumi_games_dir().join(format!("{exe}.conf"));
                Some(p)
            });

        let key = appid
            .map(|id| format!("app:{id}"))
            .unwrap_or_else(|| format!("exe:{exe}"));
        let observed = ObservedGame {
            pid,
            appid,
            exe,
            config_path,
            layer_mapped,
            explicitly_disabled: disabled,
        };
        match candidates.get_mut(&key) {
            Some(existing) if !existing.layer_mapped && observed.layer_mapped => {
                *existing = observed;
            }
            Some(_) => {}
            None => {
                candidates.insert(key, observed);
            }
        }
    }

    let mut games: Vec<_> = candidates.into_values().collect();
    games.sort_by(|a, b| a.exe.cmp(&b.exe));
    Observation {
        games,
        scan_available,
    }
}

fn read_environ(proc_dir: &Path) -> Option<BTreeMap<String, String>> {
    let bytes = fs::read(proc_dir.join("environ")).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let mut map = BTreeMap::new();
    for part in bytes.split(|b| *b == 0) {
        if part.is_empty() {
            continue;
        }
        let s = String::from_utf8_lossy(part);
        if let Some((key, value)) = s.split_once('=') {
            map.insert(key.to_string(), value.to_string());
        }
    }
    Some(map)
}

fn executable_basename(proc_dir: &Path) -> Option<String> {
    if let Ok(bytes) = fs::read(proc_dir.join("cmdline")) {
        let args: Vec<String> = bytes
            .split(|b| *b == 0)
            .filter(|p| !p.is_empty())
            .map(|p| String::from_utf8_lossy(p).into_owned())
            .collect();
        for arg in args.iter().rev() {
            if arg.to_lowercase().ends_with(".exe")
                && let Some(name) = Path::new(arg).file_name().and_then(|n| n.to_str())
            {
                return Some(strip_exe(name));
            }
        }
    }
    let exe = fs::read_link(proc_dir.join("exe")).ok()?;
    let name = exe.file_name()?.to_str()?;
    Some(strip_exe(name))
}

fn strip_exe(name: &str) -> String {
    if name.to_lowercase().ends_with(".exe") {
        name[..name.len() - 4].to_string()
    } else {
        name.to_string()
    }
}

fn layer_mapped(proc_dir: &Path) -> bool {
    let Ok(maps) = fs::read(proc_dir.join("maps")) else {
        return false;
    };
    let text = String::from_utf8_lossy(&maps);
    text.lines()
        .any(|line| line.contains("libVkLayer_vksumi.so"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_windows_extension() {
        assert_eq!(strip_exe("Game.exe"), "Game");
        assert_eq!(strip_exe("Game.EXE"), "Game");
        assert_eq!(strip_exe("game"), "game");
    }
}
