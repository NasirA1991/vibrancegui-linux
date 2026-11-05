use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub fn cache_path(cache_dir: &Path, appid: u32) -> PathBuf {
    cache_dir.join("artwork").join(format!("{appid}.jpg"))
}

pub fn download(cache_dir: &Path, appid: u32) -> Result<PathBuf> {
    let dir = cache_dir.join("artwork");
    std::fs::create_dir_all(&dir)?;
    let dest = cache_path(cache_dir, appid);
    if dest.is_file() {
        return Ok(dest);
    }

    let urls = [
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/library_600x900.jpg"),
        format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{appid}/header.jpg"),
    ];
    let mut last_error = None;
    for url in urls {
        match fetch(&url) {
            Ok(bytes) if !bytes.is_empty() => {
                let tmp = dest.with_extension("jpg.part");
                let mut file = std::fs::File::create(&tmp)
                    .with_context(|| format!("create {}", tmp.display()))?;
                file.write_all(&bytes)?;
                file.sync_all()?;
                drop(file);
                std::fs::rename(&tmp, &dest)?;
                return Ok(dest);
            }
            Ok(_) => last_error = Some(anyhow::anyhow!("empty artwork response")),
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("no artwork URL")))
}

fn fetch(url: &str) -> Result<Vec<u8>> {
    let mut response = ureq::get(url)
        .header("User-Agent", "VibranceGUI-for-Linux/0.1")
        .call()
        .with_context(|| format!("GET {url}"))?;
    let body = response.body_mut().read_to_vec()?;
    Ok(body)
}

/// Extract the first group icon from a Windows PE executable into the cache.
/// This is what makes non-Steam shortcuts show their real game icon instead
/// of a generated initial tile.
pub fn extract_exe_icon(cache_dir: &Path, exe: &Path) -> Option<PathBuf> {
    if !exe.is_file() {
        return None;
    }
    let is_exe = exe
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("exe"))
        .unwrap_or(false);
    if !is_exe {
        return None;
    }

    let hash = {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        exe.hash(&mut h);
        h.finish()
    };
    let dir = cache_dir.join("icons");
    std::fs::create_dir_all(&dir).ok()?;
    let dest = dir.join(format!("{hash:016x}.ico"));
    if dest.is_file() {
        return Some(dest);
    }

    let extractor = icoextract_rs::IconExtractor::from_path(exe).ok()?;
    let icon = extractor
        .get_icon(icoextract_rs::IconSelector::Index(0))
        .ok()?;
    let bytes = icon.to_ico_bytes().ok()?;
    let tmp = dest.with_extension("ico.part");
    std::fs::write(&tmp, bytes).ok()?;
    std::fs::rename(&tmp, &dest).ok()?;
    Some(dest)
}
