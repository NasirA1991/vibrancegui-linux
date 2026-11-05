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
        let mut problems = Vec::new();
        for manifest in manifest_candidates(paths) {
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

        // A Flatpak vkSumi extension or a manual/user-local install may not
        // expose a host-visible manifest. Detecting the library still lets us
        // explain the situation and lets the game process confirm the layer.
        if let Some(library) = library_candidates(paths).into_iter().find(|p| p.is_file()) {
            problems.push(format!(
                "vkSumi library found at {} but no readable host manifest was found. \
                 This is normal for some Flatpak/manual installs; the running game is the source of truth.",
                library.display()
            ));
            return Self {
                manifest: None,
                library: Some(library),
                usable: true,
                problems,
            };
        }

        if problems.is_empty() {
            problems.push("vkSumi layer manifest and library not found.".into());
        }
        Self {
            manifest: None,
            library: None,
            usable: false,
            problems,
        }
    }

    pub fn status_text(&self) -> &'static str {
        if self.usable {
            "vkSumi detected"
        } else {
            "vkSumi not detected"
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
    out.push(PathBuf::from(
        "/usr/lib/extensions/vulkan/vkSumi/share/vulkan/implicit_layer.d/vksumi.json",
    ));
    out.push(PathBuf::from(
        "/app/share/vulkan/implicit_layer.d/vksumi.json",
    ));
    out
}

fn library_candidates(paths: &AppPaths) -> Vec<PathBuf> {
    vec![
        paths.home.join(".local/lib/libVkLayer_vksumi.so"),
        paths.home.join(".local/lib32/libVkLayer_vksumi.so"),
        paths
            .home
            .join(".local/share/goverlay/layers/vksumi/libVkLayer_vksumi.so"),
        PathBuf::from("/usr/lib/libVkLayer_vksumi.so"),
        PathBuf::from("/usr/lib64/libVkLayer_vksumi.so"),
        PathBuf::from("/usr/local/lib/libVkLayer_vksumi.so"),
        PathBuf::from(
            "/usr/lib/extensions/vulkan/vkSumi/lib/x86_64-linux-gnu/libVkLayer_vksumi.so",
        ),
        PathBuf::from("/usr/lib/extensions/vulkan/vkSumi/lib/i386-linux-gnu/libVkLayer_vksumi.so"),
        PathBuf::from(
            "/app/lib/extensions/vulkan/vkSumi/lib/x86_64-linux-gnu/libVkLayer_vksumi.so",
        ),
    ]
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
