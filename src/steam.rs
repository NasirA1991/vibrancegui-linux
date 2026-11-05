use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::vdf;

#[derive(Debug, Clone)]
pub struct SteamGame {
    pub appid: u32,
    pub name: String,
    pub install_dir: PathBuf,
    pub artwork: Option<PathBuf>,
    pub launch_options: String,
    pub manual: bool,
    pub manual_exe: Option<PathBuf>,
    pub manual_config: Option<PathBuf>,
}

impl SteamGame {
    pub fn vk_enabled(&self) -> bool {
        token_present(&self.launch_options, "ENABLE_VKSUMI=1")
    }

    pub fn disabled(&self) -> bool {
        token_present(&self.launch_options, "DISABLE_VKSUMI=1")
    }

    pub fn config_env(&self) -> Option<PathBuf> {
        env_value(&self.launch_options, "VKSUMI_CONFIG_FILE")
    }

    pub fn suggested_launch_options(&self) -> String {
        let existing = self.launch_options.trim();
        if existing.is_empty() {
            "ENABLE_VKSUMI=1 %command%".to_string()
        } else if token_present(existing, "ENABLE_VKSUMI=1") {
            existing.to_string()
        } else if existing.contains("%command%") {
            format!("ENABLE_VKSUMI=1 {existing}")
        } else {
            format!("ENABLE_VKSUMI=1 %command% {existing}")
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SteamData {
    pub root: Option<PathBuf>,
    pub account_id: Option<String>,
    pub games: Vec<SteamGame>,
    pub warnings: Vec<String>,
}

impl SteamData {
    pub fn discover() -> Self {
        let mut data = Self::default();
        let roots = steam_roots();
        if roots.is_empty() {
            data.warnings
                .push("Steam was not found in the usual locations.".into());
            return data;
        }

        let mut libraries = Vec::new();
        let mut seen_libs = std::collections::BTreeSet::new();
        for root in &roots {
            if seen_libs.insert(root.clone()) {
                libraries.push(root.clone());
            }
            for lib in parse_library_folders(root) {
                if seen_libs.insert(lib.clone()) {
                    libraries.push(lib);
                }
            }
        }

        // Prefer the root that has the most manifests; this is normally the main install.
        let mut best_root = None;
        let mut best_count = 0usize;
        for root in &roots {
            let count = count_manifests(&root.join("steamapps"));
            if count > best_count {
                best_count = count;
                best_root = Some(root.clone());
            }
        }
        data.root = best_root.or_else(|| roots.first().cloned());

        if let Some(root) = &data.root {
            let launch_data = read_launch_options(root);
            let map = launch_data
                .as_ref()
                .map(|(_, map)| map.clone())
                .unwrap_or_default();
            if let Some((account, _)) = launch_data {
                data.account_id = Some(account);
            }

            let mut games = Vec::new();
            let mut seen = std::collections::BTreeSet::new();

            for library in &libraries {
                let steamapps = library.join("steamapps");
                let Ok(entries) = fs::read_dir(&steamapps) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                        continue;
                    };
                    if !name.starts_with("appmanifest_") || !name.ends_with(".acf") {
                        continue;
                    }
                    let Ok(parsed) = vdf::parse_file(&path) else {
                        continue;
                    };
                    let Some(appid) = parsed
                        .child("appid")
                        .and_then(|n| n.value_or_empty().parse::<u32>().ok())
                    else {
                        continue;
                    };
                    if appid == 0 || appid == 228980 || !seen.insert(appid) {
                        continue;
                    }
                    let game_name = parsed
                        .child("name")
                        .map(|n| n.value_or_empty().to_string())
                        .unwrap_or_else(|| format!("App {appid}"));
                    if game_name.starts_with("Steam Linux Runtime")
                        || game_name.starts_with("Proton ")
                    {
                        continue;
                    }
                    let state_flags = parsed
                        .child("StateFlags")
                        .and_then(|n| n.value_or_empty().parse::<u64>().ok())
                        .unwrap_or(0);
                    // 4 = fully installed. Some manifests omit flags; keep those.
                    if state_flags != 0 && state_flags & 4 == 0 {
                        continue;
                    }
                    let install_dir_name = parsed
                        .child("installdir")
                        .map(|n| n.value_or_empty().to_string())
                        .unwrap_or_default();
                    let install_dir = if install_dir_name.is_empty() {
                        library.join("steamapps/common")
                    } else {
                        library.join("steamapps/common").join(install_dir_name)
                    };
                    let launch_options = map.get(&appid).cloned().unwrap_or_default();
                    let artwork = find_artwork(root, appid);
                    games.push(SteamGame {
                        appid,
                        name: game_name,
                        install_dir,
                        artwork,
                        launch_options,
                        manual: false,
                        manual_exe: None,
                        manual_config: None,
                    });
                }
            }

            games.sort_by_key(|a| a.name.to_lowercase());
            data.games = games;
        }

        if data.games.is_empty() {
            data.warnings
                .push("No installed Steam games were found.".into());
        }
        data
    }
}

pub fn steam_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        return roots;
    };
    let candidates = [home.join(".steam/steam"), home.join(".local/share/Steam")];
    for path in candidates {
        if path.exists()
            && let Ok(canon) = path.canonicalize()
            && !roots.contains(&canon)
        {
            roots.push(canon);
        }
    }
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        let path = data.join("Steam");
        if path.exists()
            && let Ok(canon) = path.canonicalize()
            && !roots.contains(&canon)
        {
            roots.push(canon);
        }
    }
    roots
}

fn parse_library_folders(root: &Path) -> Vec<PathBuf> {
    let path = root.join("steamapps/libraryfolders.vdf");
    let Ok(text) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let Ok(parsed) = vdf::parse_str(&text) else {
        return Vec::new();
    };
    let folders = parsed.child("libraryfolders").unwrap_or(&parsed);
    if folders.children.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for folder in &folders.children {
        if let Some(path) = folder.child("path").map(|n| n.value_or_empty())
            && !path.is_empty()
        {
            let p = PathBuf::from(path);
            if p.exists() {
                out.push(p.canonicalize().unwrap_or(p));
            }
        }
    }
    out
}

fn count_manifests(steamapps: &Path) -> usize {
    fs::read_dir(steamapps)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("appmanifest_"))
                .count()
        })
        .unwrap_or(0)
}

fn read_launch_options(root: &Path) -> Option<(String, BTreeMap<u32, String>)> {
    let userdata = root.join("userdata");
    let entries = fs::read_dir(&userdata).ok()?;
    let mut configs = Vec::new();
    for entry in entries.flatten() {
        let account = entry.file_name().to_string_lossy().to_string();
        let path = entry.path().join("config/localconfig.vdf");
        if path.exists() {
            let mtime = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            configs.push((mtime, account, path));
        }
    }
    configs.sort_by_key(|(mtime, _, _)| *mtime);
    let (_, account, path) = configs.pop()?;
    let text = fs::read_to_string(&path).ok()?;
    let parsed = vdf::parse_str(&text).ok()?;
    let apps = parsed
        .path(&["Software", "Valve", "Steam", "apps"])
        .or_else(|| parsed.path(&["UserLocalConfigStore", "Software", "Valve", "Steam", "apps"]))?;

    let mut map = BTreeMap::new();
    for app in &apps.children {
        let Ok(appid) = app.key.parse::<u32>() else {
            continue;
        };
        if let Some(options) = app.child("LaunchOptions") {
            map.insert(appid, options.value_or_empty().to_string());
        }
    }
    Some((account, map))
}

fn find_artwork(root: &Path, appid: u32) -> Option<PathBuf> {
    let cache = root.join("appcache/librarycache");
    let candidates = [
        format!("{appid}_library_600x900.jpg"),
        format!("{appid}_header.jpg"),
        format!("{appid}_library_hero.jpg"),
        format!("{appid}_logo.png"),
    ];
    for candidate in candidates {
        let path = cache.join(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn token_present(text: &str, token: &str) -> bool {
    text.split_whitespace().any(|part| part == token)
}

pub fn env_value(text: &str, key: &str) -> Option<PathBuf> {
    let prefix = format!("{key}=");
    for token in shell_words::split(text).ok()? {
        if let Some(value) = token.strip_prefix(&prefix) {
            if value.is_empty() {
                return None;
            }
            return Some(PathBuf::from(value));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_tokens_exactly() {
        assert!(token_present(
            "ENABLE_VKSUMI=1 %command%",
            "ENABLE_VKSUMI=1"
        ));
        assert!(!token_present(
            "XENABLE_VKSUMI=1 %command%",
            "ENABLE_VKSUMI=1"
        ));
        assert!(!token_present(
            "ENABLE_VKSUMI=0 %command%",
            "ENABLE_VKSUMI=1"
        ));
    }

    #[test]
    fn extracts_env_value() {
        let v = env_value(
            "ENABLE_VKSUMI=1 VKSUMI_CONFIG_FILE='/tmp/a b.conf' %command%",
            "VKSUMI_CONFIG_FILE",
        );
        assert_eq!(v, Some(PathBuf::from("/tmp/a b.conf")));
    }

    #[test]
    fn suggests_prefixed_options() {
        let game = SteamGame {
            appid: 1,
            name: "X".into(),
            install_dir: PathBuf::new(),
            artwork: None,
            launch_options: "mangohud %command%".into(),
            manual: false,
            manual_exe: None,
            manual_config: None,
        };
        assert_eq!(
            game.suggested_launch_options(),
            "ENABLE_VKSUMI=1 mangohud %command%"
        );
    }
}
