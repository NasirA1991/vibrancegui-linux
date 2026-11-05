use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::paths::AppPaths;

#[derive(Debug, Clone, Default)]
pub struct VkSumiStatus {
    pub manifest: Option<PathBuf>,
    pub library: Option<PathBuf>,
    pub usable: bool,
    pub problems: Vec<String>,
}

impl VkSumiStatus {
    pub fn detect(paths: &AppPaths) -> Self {
        let candidates = manifest_candidates(paths);
        let mut problems = Vec::new();
        for manifest in candidates {
            if !manifest.is_file() {
                continue;
            }
            match validate_manifest(&manifest) {
                Ok(library) => {
                    return Self {
                        manifest: Some(manifest),
                        library: Some(library),
                        usable: true,
                        problems,
                    };
                }
                Err(err) => problems.push(format!("{}: {err}", manifest.display())),
            }
        }
        Self {
            manifest: None,
            library: None,
            usable: false,
            problems: if problems.is_empty() {
                vec!["vkSumi layer manifest not found.".into()]
            } else {
                problems
            },
        }
    }
}

fn manifest_candidates(paths: &AppPaths) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(data) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
        out.push(data.join("vulkan/implicit_layer.d/vksumi.json"));
    }
    out.push(
        paths
            .home
            .join(".local/share/vulkan/implicit_layer.d/vksumi.json"),
    );
    out.push(PathBuf::from(
        "/usr/local/share/vulkan/implicit_layer.d/vksumi.json",
    ));
    out.push(PathBuf::from(
        "/usr/share/vulkan/implicit_layer.d/vksumi.json",
    ));
    out.push(PathBuf::from("/etc/vulkan/implicit_layer.d/vksumi.json"));
    out
}

fn validate_manifest(path: &Path) -> Result<PathBuf> {
    let text = std::fs::read_to_string(path)?;
    let json: serde_json::Value = serde_json::from_str(&text)?;
    let layer = json
        .get("layer")
        .ok_or_else(|| anyhow::anyhow!("missing layer object"))?;
    let name = layer
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    if !name.contains("vksumi") {
        anyhow::bail!("unexpected layer name {name:?}");
    }
    let library = layer
        .get("library_path")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("missing library_path"))?;
    let library = PathBuf::from(library);
    let library = if library.is_absolute() {
        library
    } else {
        path.parent()
            .unwrap_or_else(|| Path::new("/"))
            .join(library)
    };
    if !library.is_file() {
        anyhow::bail!("library_path does not exist: {}", library.display());
    }
    let enable = layer
        .get("enable_environment")
        .and_then(|v| v.as_object())
        .and_then(|m| m.get("ENABLE_VKSUMI"))
        .and_then(|v| v.as_str());
    if enable != Some("1") {
        anyhow::bail!("manifest does not require ENABLE_VKSUMI=1");
    }
    Ok(library)
}
