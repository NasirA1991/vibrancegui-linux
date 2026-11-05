use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, FontId, Frame, Layout, Margin, RichText, Sense,
    Stroke, StrokeKind, TextureHandle, TextureOptions, Vec2,
};

use crate::config_doc::{ConfigDoc, KNOBS};
use crate::paths::{AppPaths, expand_user_path};
use crate::process::{self, Observation, ObservedGame};
use crate::state::{GameMapping, SavedState};
use crate::steam::{SteamData, SteamGame};
use crate::theme::{self, Palette};
use crate::tray::{self, TrayAction, VibranceTray};
use crate::vksumi::VkSumiStatus;

pub struct VibranceGuiApp {
    paths: AppPaths,
    layer: VkSumiStatus,
    steam: SteamData,
    games: Vec<SteamGame>,
    state: SavedState,
    selected: Option<u32>,
    search: String,
    add_steam_open: bool,
    add_steam_search: String,
    add_manual_open: bool,
    manual_name: String,
    manual_exe: String,
    manual_icon: String,
    manual_config: String,
    autostart: bool,
    global_doc: ConfigDoc,
    global_dirty: bool,
    global_last_change: Instant,
    editor: Option<GameEditor>,
    observed: Vec<ObservedGame>,
    scan_available: bool,
    observation_rx: Receiver<Observation>,
    observation_stop: Arc<AtomicBool>,
    tray: Option<ksni::blocking::Handle<VibranceTray>>,
    tray_rx: Receiver<TrayAction>,
    quit_requested: bool,
    palette: Palette,
    textures: HashMap<u32, TextureHandle>,
    texture_attempted: HashSet<u32>,
    artwork_paths: HashMap<u32, PathBuf>,
    artwork_tx: Sender<u32>,
    artwork_rx: Receiver<(u32, Option<PathBuf>)>,
    artwork_requested: HashSet<u32>,
    status: StatusMessage,
    needs_editor_refresh: bool,
}

#[derive(Debug, Clone)]
struct StatusMessage {
    text: String,
    kind: StatusKind,
    until: Option<Instant>,
}

#[derive(Debug, Clone, Copy)]
enum StatusKind {
    Info,
    Good,
    Warn,
}

impl StatusMessage {
    fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Info,
            until: None,
        }
    }
    fn good(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Good,
            until: Some(Instant::now() + Duration::from_secs(4)),
        }
    }
    fn warn(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            kind: StatusKind::Warn,
            until: None,
        }
    }
}

struct GameEditor {
    appid: u32,
    path: PathBuf,
    doc: ConfigDoc,
    external: bool,
    dirty: bool,
    last_change: Instant,
}

impl VibranceGuiApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        use_tray: bool,
        start_hidden: bool,
        palette: Palette,
    ) -> Self {
        let paths = AppPaths::discover().expect("home directory");
        let _ = paths.ensure();
        let layer = VkSumiStatus::detect(&paths);
        let steam = SteamData::discover();
        let state = SavedState::load(&paths);

        let global_path = paths.global_vksumi_conf();
        let mut global_doc = ConfigDoc::load(&global_path)
            .ok()
            .flatten()
            .unwrap_or_else(|| {
                let mut d = ConfigDoc {
                    path: global_path,
                    lines: Vec::new(),
                };
                d.lines
                    .push("# vkSumi global config (edited by VibranceGUI)".into());
                d
            });
        if global_doc.path.as_os_str().is_empty() {
            global_doc.set_path(paths.global_vksumi_conf());
        }

        let (artwork_req_tx, artwork_req_rx) = mpsc::channel::<u32>();
        let (artwork_done_tx, artwork_rx) = mpsc::channel();
        let artwork_cache = paths.cache_dir.clone();
        let artwork_ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while let Ok(appid) = artwork_req_rx.recv() {
                let path = crate::artwork::download(&artwork_cache, appid).ok();
                let _ = artwork_done_tx.send((appid, path));
                artwork_ctx.request_repaint();
            }
        });

        let (tray_tx, tray_rx) = mpsc::channel();
        let tray = if use_tray {
            tray::spawn(cc.egui_ctx.clone(), tray_tx)
        } else {
            None
        };
        if start_hidden && tray.is_some() {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        let (tx, observation_rx) = mpsc::channel();
        let stop = Arc::new(AtomicBool::new(false));
        let scan_paths = paths.clone();
        let scan_stop = stop.clone();
        let repaint_ctx = cc.egui_ctx.clone();
        std::thread::spawn(move || {
            while !scan_stop.load(Ordering::Relaxed) {
                let observation = process::scan(&scan_paths);
                if tx.send(observation).is_err() {
                    break;
                }
                repaint_ctx.request_repaint();
                std::thread::sleep(Duration::from_secs(2));
            }
        });

        let initial_status = if layer.usable {
            if steam.games.is_empty() {
                StatusMessage::warn("vkSumi ready · no Steam games found")
            } else {
                StatusMessage::info(format!(
                    "vkSumi ready · {} Steam games found",
                    steam.games.len()
                ))
            }
        } else {
            StatusMessage::warn("vkSumi is not installed or its layer is incomplete")
        };

        let mut app = Self {
            paths,
            layer,
            steam,
            games: Vec::new(),
            state,
            selected: None,
            search: String::new(),
            add_steam_open: false,
            add_steam_search: String::new(),
            add_manual_open: false,
            manual_name: String::new(),
            manual_exe: String::new(),
            manual_icon: String::new(),
            manual_config: String::new(),
            autostart: false,
            global_doc,
            global_dirty: false,
            global_last_change: Instant::now(),
            editor: None,
            observed: Vec::new(),
            scan_available: true,
            observation_rx,
            observation_stop: stop,
            tray,
            tray_rx,
            quit_requested: false,
            palette,
            textures: HashMap::new(),
            texture_attempted: HashSet::new(),
            artwork_paths: HashMap::new(),
            artwork_tx: artwork_req_tx,
            artwork_rx,
            artwork_requested: HashSet::new(),
            status: initial_status,
            needs_editor_refresh: false,
        };
        app.ensure_default_tracking();
        app.rebuild_games();
        app.autostart = app.autostart_enabled();
        app.select_first_game();
        if app.layer.usable && !app.steam.games.is_empty() {
            app.status = StatusMessage::info(format!(
                "vkSumi ready · {} tracked · {} installed",
                app.games.len(),
                app.steam.games.len()
            ));
        }
        app.refresh_observation();
        app
    }

    fn ensure_default_tracking(&mut self) {
        let mut changed = false;
        if !self.state.shortcuts_initialized {
            for game in &self.steam.games {
                if game.shortcut {
                    self.state.tracked_steam.insert(game.appid);
                }
            }
            self.state.shortcuts_initialized = true;
            changed = true;
        }
        if !self.state.tracking_initialized {
            for game in &self.steam.games {
                if game.vk_enabled() || self.state.games.contains_key(&game.appid) {
                    self.state.tracked_steam.insert(game.appid);
                }
            }
            self.state.tracking_initialized = true;
            changed = true;
        }
        if changed {
            let _ = self.state.save(&self.paths);
        }
    }

    fn manual_to_game(id: u32, manual: &crate::state::ManualGame) -> SteamGame {
        SteamGame {
            appid: id,
            name: manual.name.clone(),
            install_dir: manual
                .exe
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            artwork: manual.icon.clone(),
            launch_options: String::new(),
            manual: true,
            manual_exe: Some(manual.exe.clone()),
            manual_config: Some(manual.config_path.clone()),
            shortcut: false,
        }
    }

    fn rebuild_games(&mut self) {
        let mut games: Vec<SteamGame> = self
            .steam
            .games
            .iter()
            .filter(|g| self.state.tracked_steam.contains(&g.appid))
            .cloned()
            .collect();
        games.extend(
            self.state
                .manual_games
                .iter()
                .map(|(id, m)| Self::manual_to_game(*id, m)),
        );
        games.sort_by_key(|a| a.name.to_lowercase());
        self.games = games;

        if let Some(appid) = self.selected
            && !self.games.iter().any(|g| g.appid == appid)
        {
            self.selected = None;
            self.editor = None;
        }
    }

    fn select_first_game(&mut self) {
        // Start on the program grid, like VibranceGUI. Only a detected running
        // game opens its settings automatically.
        self.selected = None;
        self.editor = None;
    }

    fn game(&self, appid: u32) -> Option<&SteamGame> {
        self.games.iter().find(|g| g.appid == appid)
    }

    fn running_for(&self, appid: u32) -> Option<&ObservedGame> {
        let game = self.game(appid)?;
        if game.manual {
            let exe = game
                .manual_exe
                .as_ref()
                .and_then(|p| p.file_stem())
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            self.observed.iter().find(|o| {
                !o.explicitly_disabled && o.exe.eq_ignore_ascii_case(exe) && o.appid.is_none()
            })
        } else {
            self.observed
                .iter()
                .find(|o| o.appid == Some(appid) && !o.explicitly_disabled)
        }
    }

    fn observed_path(&self, appid: u32) -> Option<PathBuf> {
        self.observed
            .iter()
            .find(|o| o.appid == Some(appid))
            .and_then(|o| o.config_path.clone())
    }

    fn config_path_for(&self, game: &SteamGame) -> Option<PathBuf> {
        if let Some(path) = &game.manual_config {
            return Some(path.clone());
        }
        if let Some(mapping) = self.state.games.get(&game.appid)
            && (mapping.config_path.exists() || self.paths.is_inside_vksumi(&mapping.config_path))
        {
            return Some(mapping.config_path.clone());
        }
        if let Some(env) = game.config_env() {
            return Some(expand_user_path(&env.to_string_lossy(), &self.paths.home));
        }
        if game.shortcut
            && let Some(exe) = game.manual_exe.as_ref().and_then(|p| p.file_name())
        {
            return Some(
                self.paths
                    .vksumi_games_dir()
                    .join(format!("{}.conf", exe.to_string_lossy())),
            );
        }
        self.observed_path(game.appid)
    }

    fn autostart_dir(&self) -> PathBuf {
        self.paths
            .vksumi_config_dir
            .parent()
            .unwrap_or(&self.paths.home)
            .join("autostart")
    }

    fn autostart_file(&self) -> PathBuf {
        self.autostart_dir().join("vibrancegui.desktop")
    }

    fn autostart_enabled(&self) -> bool {
        self.autostart_file().is_file()
    }

    fn set_autostart(&mut self, enabled: bool) {
        let path = self.autostart_file();
        if enabled {
            let exe = std::env::var_os("APPIMAGE")
                .map(PathBuf::from)
                .or_else(|| std::env::current_exe().ok())
                .unwrap_or_else(|| PathBuf::from("vibrancegui"));
            let exec = if exe.to_string_lossy().contains(' ') {
                format!("\"{}\" --hidden", exe.display())
            } else {
                format!("{} --hidden", exe.display())
            };
            let desktop = format!(
                "[Desktop Entry]\nType=Application\nName=VibranceGUI for Linux\nComment=Start VibranceGUI for Linux in the tray\nExec={exec}\nIcon=vibrancegui\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
            );
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::write(&path, desktop) {
                Ok(()) => {
                    self.autostart = true;
                    self.status = StatusMessage::good("Will start with your desktop session");
                }
                Err(err) => {
                    self.status = StatusMessage::warn(format!("Could not enable autostart: {err}"));
                }
            }
        } else {
            match std::fs::remove_file(&path) {
                Ok(()) | Err(_) if !path.exists() => {
                    self.autostart = false;
                    self.status = StatusMessage::info("Autostart disabled");
                }
                Err(err) => {
                    self.status =
                        StatusMessage::warn(format!("Could not disable autostart: {err}"));
                }
                Ok(()) => {
                    self.autostart = false;
                    self.status = StatusMessage::info("Autostart disabled");
                }
            }
        }
    }

    fn add_steam_game(&mut self, appid: u32) {
        self.state.tracked_steam.insert(appid);
        let _ = self.state.save(&self.paths);
        self.rebuild_games();
        self.selected = Some(appid);
        self.open_selected();
        self.add_steam_open = false;
        self.status = StatusMessage::good("Game added");
    }

    fn add_manual_game(&mut self) {
        let exe = PathBuf::from(self.manual_exe.trim());
        if !exe.is_file() {
            self.status = StatusMessage::warn("Executable path does not exist");
            return;
        }
        let name = if self.manual_name.trim().is_empty() {
            exe.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Manual game")
                .to_string()
        } else {
            self.manual_name.trim().to_string()
        };
        let stem = exe.file_stem().and_then(|s| s.to_str()).unwrap_or("game");
        let config = if self.manual_config.trim().is_empty() {
            self.paths.vksumi_games_dir().join(format!("{stem}.conf"))
        } else {
            PathBuf::from(self.manual_config.trim())
        };
        let icon = if self.manual_icon.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(self.manual_icon.trim()))
        };
        let id = 0x8000_0000u32 | self.state.next_manual_id.max(1);
        self.state.next_manual_id = self.state.next_manual_id.saturating_add(1).max(1);
        self.state.manual_games.insert(
            id,
            crate::state::ManualGame {
                name,
                exe,
                icon,
                config_path: config,
            },
        );
        let _ = self.state.save(&self.paths);
        self.rebuild_games();
        self.selected = Some(id);
        self.open_selected();
        self.add_manual_open = false;
        self.manual_name.clear();
        self.manual_exe.clear();
        self.manual_icon.clear();
        self.manual_config.clear();
        self.status = StatusMessage::good("Manual game added");
    }

    fn remove_selected(&mut self) {
        let Some(id) = self.selected else {
            return;
        };
        if self.state.manual_games.remove(&id).is_some() {
            let _ = self.state.save(&self.paths);
        } else {
            self.state.tracked_steam.remove(&id);
            let _ = self.state.save(&self.paths);
        }
        self.selected = None;
        self.editor = None;
        self.rebuild_games();
        self.status = StatusMessage::info("Removed from the list; config files were not deleted");
    }

    fn open_selected(&mut self) {
        self.needs_editor_refresh = false;
        let Some(appid) = self.selected else {
            self.editor = None;
            return;
        };
        let Some(game) = self.game(appid).cloned() else {
            self.editor = None;
            return;
        };
        let Some(path) = self.config_path_for(&game) else {
            self.editor = None;
            return;
        };
        if self.editor.as_ref().map(|e| e.appid) == Some(appid)
            && self.editor.as_ref().map(|e| e.path.clone()) == Some(path.clone())
        {
            return;
        }
        let external = !self.paths.is_inside_vksumi(&path);
        let mut doc = match ConfigDoc::load(&path) {
            Ok(Some(doc)) => doc,
            Ok(None) => {
                let mut doc = ConfigDoc {
                    path: path.clone(),
                    lines: vec![if game.manual {
                        format!("# vkSumi profile for {}", game.name)
                    } else {
                        format!(
                            "# vkSumi profile for {} (Steam AppID {})",
                            game.name, game.appid
                        )
                    }],
                };
                doc.set_path(path.clone());
                doc
            }
            Err(err) => {
                self.status = StatusMessage::warn(format!("Could not read profile: {err}"));
                self.editor = None;
                return;
            }
        };
        doc.set_path(path.clone());
        self.editor = Some(GameEditor {
            appid,
            path,
            doc,
            external,
            dirty: false,
            last_change: Instant::now(),
        });
    }

    fn refresh_observation(&mut self) {
        let mut changed = false;
        while let Ok(observation) = self.observation_rx.try_recv() {
            self.scan_available = observation.scan_available;
            self.observed = observation.games;
            for obs in &self.observed {
                let Some(appid) = obs.appid else {
                    continue;
                };
                let Some(path) = obs.config_path.clone() else {
                    continue;
                };
                let needs_update = self
                    .state
                    .games
                    .get(&appid)
                    .map(|m| m.config_path != path || m.exe != obs.exe)
                    .unwrap_or(true);
                if needs_update {
                    self.state.games.insert(
                        appid,
                        GameMapping {
                            config_path: path,
                            exe: obs.exe.clone(),
                            updated_unix: now_unix(),
                        },
                    );
                    changed = true;
                }
            }
        }
        // Link non-Steam processes by executable name and keep their effective
        // VKSUMI_CONFIG_FILE, just like the Steam path above.
        for obs in &self.observed {
            if obs.explicitly_disabled {
                continue;
            }
            let Some(config_path) = obs.config_path.clone() else {
                continue;
            };
            for manual in self.state.manual_games.values_mut() {
                let stem = manual
                    .exe
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or_default();
                if obs.appid.is_none()
                    && manual_exe_matches(Some(&manual.exe), &obs.exe, stem)
                    && manual.config_path != config_path
                {
                    manual.config_path = config_path.clone();
                    changed = true;
                }
            }
        }
        // Any unknown process that was launched with ENABLE_VKSUMI=1 is useful
        // evidence of a non-Steam game. Adopt it automatically; Add manually is
        // now only a fallback for games that are not running and not Steam shortcuts.
        let mut adopted = false;
        for obs in &self.observed {
            if obs.explicitly_disabled || obs.appid.is_some() {
                continue;
            }
            let Some(exe_path) = obs.exe_path.clone() else {
                continue;
            };
            let already = self.state.manual_games.values().any(|m| {
                m.exe == exe_path
                    || m.exe.file_name().and_then(|s| s.to_str()) == Some(obs.exe.as_str())
            });
            if already {
                continue;
            }
            let stem = exe_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&obs.exe)
                .to_string();
            let id = 0x8000_0000u32 | self.state.next_manual_id.max(1);
            self.state.next_manual_id = self.state.next_manual_id.saturating_add(1).max(1);
            self.state.manual_games.insert(
                id,
                crate::state::ManualGame {
                    name: stem,
                    exe: exe_path,
                    icon: None,
                    config_path: obs.config_path.clone().unwrap_or_else(|| {
                        self.paths
                            .vksumi_games_dir()
                            .join(format!("{}.conf", obs.exe))
                    }),
                },
            );
            adopted = true;
        }
        if adopted {
            changed = true;
            self.rebuild_games();
        }
        if changed {
            let _ = self.state.save(&self.paths);
            self.needs_editor_refresh = true;
        }
        if self.editor.is_none() {
            let running_id = self.observed.iter().find_map(|o| {
                if o.explicitly_disabled {
                    return None;
                }
                if let Some(appid) = o.appid {
                    return self.games.iter().any(|g| g.appid == appid).then_some(appid);
                }
                self.games.iter().find_map(|g| {
                    if !g.manual {
                        return None;
                    }
                    let stem = g
                        .manual_exe
                        .as_ref()
                        .and_then(|p| p.file_stem())
                        .and_then(|s| s.to_str())
                        .unwrap_or_default();
                    manual_exe_matches(g.manual_exe.as_deref(), &o.exe, stem).then_some(g.appid)
                })
            });
            if let Some(id) = running_id
                && self.selected != Some(id)
            {
                self.selected = Some(id);
                self.needs_editor_refresh = true;
            }
        }
        if self.needs_editor_refresh {
            self.open_selected();
        }
    }

    fn save_global_if_due(&mut self) {
        if !self.global_dirty || self.global_last_change.elapsed() < Duration::from_millis(80) {
            return;
        }
        match self.global_doc.save(&self.paths) {
            Ok(()) => {
                self.global_dirty = false;
                self.status = StatusMessage::good("Default vibrance saved");
            }
            Err(err) => {
                self.global_dirty = false;
                self.status = StatusMessage::warn(format!("Could not save default: {err}"));
            }
        }
    }

    fn save_editor_if_due(&mut self) {
        let Some(editor) = &mut self.editor else {
            return;
        };
        if !editor.dirty
            || editor.external
            || editor.last_change.elapsed() < Duration::from_millis(80)
        {
            return;
        }
        match editor.doc.save(&self.paths) {
            Ok(()) => {
                editor.dirty = false;
                self.status = StatusMessage::good("Profile saved · vkSumi hot reloads it");
            }
            Err(err) => {
                editor.dirty = false;
                self.status = StatusMessage::warn(format!("Could not save profile: {err}"));
            }
        }
    }

    fn set_selected(&mut self, appid: u32) {
        self.selected = Some(appid);
        self.open_selected();
    }

    fn copy_launch_option(&mut self, ui: &egui::Ui, game: &SteamGame) {
        let text = game.suggested_launch_options();
        ui.ctx().copy_text(text);
        self.status = StatusMessage::good(
            "Launch option copied — paste it into Steam → Properties → Launch Options",
        );
    }

    fn global_value(&self, key: &str) -> f32 {
        self.global_doc.get_f32(key).unwrap_or(0.0)
    }

    fn effective_value(&self, key: &str) -> f32 {
        if let Some(editor) = &self.editor
            && let Some(value) = editor.doc.get_f32(key)
        {
            return value;
        }
        self.global_value(key)
    }

    fn draw_header(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(self.palette.panel)
            .inner_margin(Margin::symmetric(16, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("VibranceGUI")
                            .size(22.0)
                            .strong()
                            .color(self.palette.text),
                    );
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("for Linux")
                            .size(12.0)
                            .color(self.palette.muted),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Refresh").clicked() {
                            self.refresh_library();
                        }
                        self.status_chip(
                            ui,
                            self.layer.status_text(),
                            if self.layer.usable {
                                self.palette.ok
                            } else {
                                self.palette.warn
                            },
                        );
                    });
                });
            });
    }

    fn status_chip(&self, ui: &mut egui::Ui, text: &str, color: Color32) {
        Frame::new()
            .fill(self.palette.panel_2)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(9, 4))
            .show(ui, |ui| {
                ui.label(RichText::new(text).size(11.0).color(color));
            });
    }

    fn draw_missing_layer(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(self.palette.panel_2)
            .corner_radius(CornerRadius::same(9))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("vkSumi is not installed or could not be found.")
                            .strong()
                            .color(self.palette.warn),
                    );
                    if ui.small_button("Copy install command").clicked() {
                        ui.ctx().copy_text("paru -S vksumi".to_string());
                        self.status = StatusMessage::good(
                            "Copied `paru -S vksumi` — or install the Flatpak runtime/manual layer",
                        );
                    }
                });
                ui.label(
                    RichText::new(
                        "The app can still edit profiles, but the game will not be graded until vkSumi is installed and ENABLE_VKSUMI=1 is set. If it is installed via Flatpak or manually, make sure the layer is visible to the game, then click Refresh.",
                    )
                    .size(10.0)
                    .color(self.palette.muted),
                );
                if !self.layer.problems.is_empty() {
                    ui.label(
                        RichText::new(&self.layer.problems[0])
                            .size(9.0)
                            .color(self.palette.muted),
                    );
                }
            });
    }

    fn draw_default_card(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(self.palette.panel)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Settings").strong().color(self.palette.text));
                    ui.label(
                        RichText::new("applies to games without a per-game override")
                            .size(10.0)
                            .color(self.palette.muted),
                    );
                });
                let mut autostart = self.autostart;
                if ui
                    .checkbox(&mut autostart, "Start VibranceGUI with my desktop session")
                    .changed()
                {
                    self.set_autostart(autostart);
                }
                let mut light = !self.palette.dark;
                if ui.checkbox(&mut light, "Light theme").changed() {
                    self.palette = if light { theme::LIGHT } else { theme::DARK };
                    theme::apply(ui.ctx(), &self.palette);
                }
                let mut value = self.global_value("vibrance").clamp(0.0, 1.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("Default Vibrance Level")
                            .size(11.0)
                            .color(self.palette.muted),
                    );
                    ui.label(
                        RichText::new(format!("{:.0}%", value * 100.0))
                            .monospace()
                            .size(10.0)
                            .color(self.palette.muted),
                    );
                });
                let resp = value_slider(ui, &mut value, 0.0, 1.0, 0.01, true, &self.palette);
                if resp.changed() {
                    self.global_doc.set_f32("vibrance", value);
                    self.global_dirty = true;
                    self.global_last_change = Instant::now();
                    ui.ctx().request_repaint();
                }
                ui.label(
                    RichText::new(format!(
                        "Writes only `vibrance` to {}",
                        self.paths.global_vksumi_conf().display()
                    ))
                    .size(10.0)
                    .color(self.palette.muted),
                );
            });
    }

    fn draw_status(&self, ui: &mut egui::Ui) {
        let color = match self.status.kind {
            StatusKind::Info => self.palette.muted,
            StatusKind::Good => self.palette.ok,
            StatusKind::Warn => self.palette.warn,
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("●").color(color).size(10.0));
            ui.label(RichText::new(&self.status.text).size(11.0).color(color));
            if let Some(account) = &self.steam.account_id {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("account {account}"))
                            .size(10.0)
                            .color(self.palette.muted),
                    );
                });
            }
        });
    }

    fn draw_library(&mut self, ui: &mut egui::Ui) {
        let enabled_count = self
            .games
            .iter()
            .filter(|g| g.vk_enabled() || self.running_for(g.appid).is_some())
            .count();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("Program Settings")
                    .strong()
                    .color(self.palette.text),
            );
            ui.add_space(6.0);
            if ui.button("Add").clicked() {
                self.add_steam_open = true;
                self.add_steam_search.clear();
            }
            if ui.button("Add manually").clicked() {
                self.add_manual_open = true;
            }
            if ui
                .add_enabled(self.selected.is_some(), egui::Button::new("Remove"))
                .clicked()
            {
                self.remove_selected();
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if !self.scan_available {
                    ui.label(
                        RichText::new("scan unavailable")
                            .size(10.0)
                            .color(self.palette.warn),
                    );
                }
            });
        });
        ui.add_space(3.0);
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "{} tracked · {} enabled",
                    self.games.len(),
                    enabled_count
                ))
                .size(10.0)
                .color(self.palette.muted),
            );
            ui.add_space(6.0);
            let search = egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search…")
                .desired_width(150.0);
            ui.add(search);
            if !self.search.is_empty() && ui.small_button("clear").clicked() {
                self.search.clear();
            }
        });
        ui.add_space(8.0);

        let needle = self.search.to_lowercase();
        let mut indices: Vec<usize> = self
            .games
            .iter()
            .enumerate()
            .filter(|(_, g)| needle.is_empty() || g.name.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect();
        indices.sort_by(|&a, &b| {
            let ga = &self.games[a];
            let gb = &self.games[b];
            let score = |g: &SteamGame| {
                let running = self.running_for(g.appid).is_some() as u8;
                let enabled = g.vk_enabled() as u8;
                let mapped =
                    (self.state.games.contains_key(&g.appid) || g.config_env().is_some()) as u8;
                running * 4 + enabled * 2 + mapped
            };
            score(gb)
                .cmp(&score(ga))
                .then_with(|| ga.name.to_lowercase().cmp(&gb.name.to_lowercase()))
        });

        if indices.is_empty() {
            ui.add_space(30.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("No programs yet").color(self.palette.muted));
                ui.label(
                    RichText::new(
                        "Use Add for a Steam game or Add manually for another executable.",
                    )
                    .size(11.0)
                    .color(self.palette.muted),
                );
            });
            return;
        }

        let ctx = ui.ctx().clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 8.0);
                    for idx in indices {
                        let game = self.games[idx].clone();
                        self.draw_game_tile(ui, &ctx, &game);
                    }
                });
                ui.add_space(12.0);
            });
    }

    fn draw_game_tile(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, game: &SteamGame) {
        let selected = self.selected == Some(game.appid);
        let running = self.running_for(game.appid).is_some();
        let enabled = game.vk_enabled();
        let tile_size = Vec2::new(78.0, 96.0);
        let (rect, response) = ui.allocate_exact_size(tile_size, Sense::click());
        if response.clicked() {
            self.set_selected(game.appid);
        }
        let hover = response.hovered();
        let painter = ui.painter_at(rect);
        let bg = if selected {
            self.palette.selected
        } else if hover {
            self.palette.hover
        } else {
            self.palette.panel
        };
        painter.rect_filled(rect, CornerRadius::same(8), bg);
        if selected {
            painter.rect_stroke(
                rect,
                CornerRadius::same(9),
                Stroke::new(1.5, self.palette.accent),
                StrokeKind::Inside,
            );
        }

        let image_rect = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), 60.0));
        if let Some(texture) = self.texture_for(ctx, game) {
            painter.image(
                texture.id(),
                image_rect,
                egui::Rect::from_min_max(egui::Pos2::ZERO, egui::Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            // Darken the bottom slightly so the title remains readable.
            painter.rect_filled(
                egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.top() + 44.0),
                    Vec2::new(rect.width(), 16.0),
                ),
                CornerRadius::ZERO,
                Color32::from_black_alpha(70),
            );
        } else {
            let color = color_for(game.appid);
            painter.rect_filled(image_rect, CornerRadius::same(9), color);
            let initials = initials(&game.name);
            painter.text(
                image_rect.center(),
                Align2::CENTER_CENTER,
                initials,
                FontId::proportional(19.0),
                Color32::from_white_alpha(210),
            );
        }

        let title = elide(&game.name, 13);
        painter.text(
            egui::pos2(rect.left() + 4.0, rect.top() + 63.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(9.0),
            self.palette.text,
        );
        let sub = if running {
            if self
                .running_for(game.appid)
                .map(|o| o.layer_mapped)
                .unwrap_or(false)
            {
                "running · layer mapped"
            } else {
                "running · unconfirmed"
            }
        } else if enabled {
            "vkSumi enabled"
        } else if game.manual {
            "non-Steam"
        } else {
            "not enabled"
        };
        let sub_color = if running {
            self.palette.ok
        } else if enabled {
            self.palette.accent
        } else {
            self.palette.muted
        };
        painter.text(
            egui::pos2(rect.left() + 4.0, rect.top() + 77.0),
            Align2::LEFT_TOP,
            sub,
            FontId::proportional(7.5),
            sub_color,
        );
        if enabled || running {
            painter.circle_filled(
                egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
                3.5,
                if running {
                    self.palette.ok
                } else {
                    self.palette.accent
                },
            );
        }
    }

    fn texture_for(&mut self, ctx: &egui::Context, game: &SteamGame) -> Option<TextureHandle> {
        if let Some(texture) = self.textures.get(&game.appid) {
            return Some(texture.clone());
        }
        if self.texture_attempted.contains(&game.appid) {
            return None;
        }
        self.texture_attempted.insert(game.appid);

        let local = game.artwork.as_ref().filter(|p| p.is_file());
        let cached = self.artwork_paths.get(&game.appid).cloned().or_else(|| {
            let path = crate::artwork::cache_path(&self.paths.cache_dir, game.appid);
            path.is_file().then_some(path)
        });
        let path = local.or(cached.as_ref())?;
        let Ok(image) = image::open(path) else {
            return None;
        };
        let image = image.resize_to_fill(256, 256, image::imageops::FilterType::Triangle);
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let texture = ctx.load_texture(
            format!("art-{}", game.appid),
            color_image,
            TextureOptions::LINEAR,
        );
        self.textures.insert(game.appid, texture.clone());
        Some(texture)
    }

    fn draw_game_window(&mut self, ctx: &egui::Context) {
        let Some(appid) = self.selected else {
            return;
        };
        let Some(game) = self.game(appid).cloned() else {
            return;
        };
        let mut open = true;
        egui::Window::new(format!("{} — settings", game.name))
            .open(&mut open)
            .default_width(430.0)
            .min_width(360.0)
            .max_height(640.0)
            .resizable(true)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.draw_detail(ui);
                    });
            });
        if !open {
            self.selected = None;
            self.editor = None;
        }
    }

    fn draw_add_steam_window(&mut self, ctx: &egui::Context) {
        if !self.add_steam_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Add a Steam game")
            .open(&mut open)
            .default_width(430.0)
            .max_height(520.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("Installed Steam games")
                        .size(11.0)
                        .color(self.palette.muted),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut self.add_steam_search)
                        .hint_text("Search Steam games…"),
                );
                ui.add_space(4.0);
                let needle = self.add_steam_search.to_lowercase();
                let candidates: Vec<(u32, String)> = self
                    .steam
                    .games
                    .iter()
                    .filter(|g| !self.state.tracked_steam.contains(&g.appid))
                    .filter(|g| needle.is_empty() || g.name.to_lowercase().contains(&needle))
                    .map(|g| (g.appid, g.name.clone()))
                    .collect();
                egui::ScrollArea::vertical()
                    .max_height(420.0)
                    .show(ui, |ui| {
                        for (appid, name) in candidates {
                            if ui.button(format!("+ {name}")).clicked() {
                                self.add_steam_game(appid);
                            }
                        }
                    });
            });
        if !open {
            self.add_steam_open = false;
        }
    }

    fn draw_add_manual_window(&mut self, ctx: &egui::Context) {
        if !self.add_manual_open {
            return;
        }
        let mut open = true;
        egui::Window::new("Add a non-Steam game")
            .open(&mut open)
            .default_width(440.0)
            .resizable(false)
            .show(ctx, |ui| {
                ui.label(RichText::new("Executable").size(10.0).color(self.palette.muted));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.manual_exe)
                            .hint_text("/path/to/game or game.exe")
                            .desired_width(310.0),
                    );
                    if ui.button("Browse…").clicked()
                        && let Some(path) = rfd::FileDialog::new().pick_file()
                    {
                        if self.manual_name.trim().is_empty() {
                            self.manual_name = path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .unwrap_or_default()
                                .to_string();
                        }
                        self.manual_exe = path.display().to_string();
                    }
                });
                ui.label(RichText::new("Display name").size(10.0).color(self.palette.muted));
                ui.add(egui::TextEdit::singleline(&mut self.manual_name).hint_text("Optional"));
                ui.label(RichText::new("Icon path").size(10.0).color(self.palette.muted));
                ui.horizontal(|ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut self.manual_icon)
                            .hint_text("Optional PNG/JPG path")
                            .desired_width(310.0),
                    );
                    if ui.button("Browse…").clicked()
                        && let Some(path) = rfd::FileDialog::new()
                            .add_filter("Image", &["png", "jpg", "jpeg", "webp"])
                            .pick_file()
                    {
                        self.manual_icon = path.display().to_string();
                    }
                });
                ui.label(RichText::new("vkSumi config").size(10.0).color(self.palette.muted));
                ui.add(
                    egui::TextEdit::singleline(&mut self.manual_config)
                        .hint_text("Optional; derived from executable name"),
                );
                ui.label(
                    RichText::new(
                        "Launch the game with ENABLE_VKSUMI=1 from its launcher; this app only manages the profile.",
                    )
                    .size(9.0)
                    .color(self.palette.muted),
                );
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Add game").clicked() {
                        self.add_manual_game();
                    }
                    if ui.button("Cancel").clicked() {
                        self.add_manual_open = false;
                    }
                });
            });
        if !open {
            self.add_manual_open = false;
        }
    }

    fn draw_detail(&mut self, ui: &mut egui::Ui) {
        let Some(appid) = self.selected else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Select a game")
                        .size(18.0)
                        .color(self.palette.muted),
                );
                ui.label(
                    RichText::new(
                        "VibranceGUI edits vkSumi profiles. It does not inject into running games.",
                    )
                    .size(11.0)
                    .color(self.palette.muted),
                );
            });
            return;
        };
        let Some(game) = self.game(appid).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            if ui.small_button("←").clicked() {
                self.selected = None;
                self.editor = None;
            }
            ui.label(
                RichText::new(&game.name)
                    .size(20.0)
                    .strong()
                    .color(self.palette.text),
            );
        });
        let subtitle = if game.manual {
            format!(
                "Non-Steam · {}",
                game.manual_exe
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "unknown executable".into())
            )
        } else if game.shortcut {
            format!(
                "Steam shortcut · {}",
                game.manual_exe
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "unknown executable".into())
            )
        } else {
            format!(
                "Steam AppID {} · {}",
                game.appid,
                game.install_dir.display()
            )
        };
        ui.label(RichText::new(subtitle).size(10.0).color(self.palette.muted));
        ui.add_space(10.0);

        let running = self.running_for(appid).cloned();
        if let Some(running) = &running {
            let (text, color) = if running.layer_mapped {
                ("Detected · vkSumi layer mapped", self.palette.ok)
            } else {
                ("Detected · layer unconfirmed", self.palette.warn)
            };
            Frame::new()
                .fill(self.palette.panel_2)
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.label(RichText::new(text).size(11.0).color(color));
                });
            ui.add_space(6.0);
        }

        self.draw_enable_state(ui, &game);
        ui.add_space(10.0);

        let Some(editor) = &self.editor else {
            ui.label(
                RichText::new("No linked vkSumi profile yet.")
                    .strong()
                    .color(self.palette.text),
            );
            if game.vk_enabled() {
                ui.label(
                    RichText::new(
                        "vkSumi is enabled, but this profile is not linked yet. Launch the game once while VibranceGUI is open; it will detect the running game and link the config.",
                    )
                    .size(11.0)
                    .color(self.palette.muted),
                );
            } else {
                ui.label(
                    RichText::new(
                        "After adding the launch option above, launch the game once with VibranceGUI open. The profile will link automatically.",
                    )
                    .size(11.0)
                    .color(self.palette.muted),
                );
            }
            ui.add_space(8.0);
            if ui.button("Open config folder").clicked() {
                let _ = std::process::Command::new("xdg-open")
                    .arg(self.paths.vksumi_games_dir())
                    .spawn();
            }
            return;
        };

        if editor.doc.get_bool("enabled") == Some(false) {
            Frame::new()
                .fill(self.palette.panel_2)
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "enabled = false is set in this profile. The color grade is paused; remove that override if you want this game graded.",
                        )
                        .size(11.0)
                        .color(self.palette.warn),
                    );
                });
            ui.add_space(8.0);
        } else if self.global_doc.get_bool("enabled") == Some(false) {
            Frame::new()
                .fill(self.palette.panel_2)
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "The global vkSumi config has enabled = false. Effects are paused for games that do not override it.",
                        )
                        .size(11.0)
                        .color(self.palette.warn),
                    );
                });
            ui.add_space(8.0);
        }
        if editor.external {
            Frame::new()
                .fill(self.palette.panel_2)
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "External config: this path is outside ~/.config/vkSumi. VibranceGUI is showing it read-only.",
                        )
                        .size(11.0)
                        .color(self.palette.warn),
                    );
                });
            ui.add_space(8.0);
        }

        ui.label(RichText::new("Basic").strong().color(self.palette.text));
        ui.add_space(2.0);
        self.draw_knob(ui, 0);
        ui.add_space(8.0);
        ui.collapsing("Advanced (all vkSumi sliders)", |ui| {
            for i in 1..KNOBS.len() {
                self.draw_knob(ui, i);
            }
            ui.label(
                RichText::new(
                    "Daltonize mode and the censor-box editor are not exposed yet; they remain editable in the config file.",
                )
                .size(9.0)
                .color(self.palette.muted),
            );
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            let read_only = self.editor.as_ref().map(|e| e.external).unwrap_or(false);
            if ui
                .add_enabled(!read_only, egui::Button::new("Reset overrides"))
                .clicked()
                && let Some(editor) = &mut self.editor
            {
                for knob in KNOBS {
                    editor.doc.remove_key(knob.key);
                }
                editor.dirty = true;
                editor.last_change = Instant::now();
                self.status = StatusMessage::info("Overrides cleared · using global defaults");
            }
            if ui.button("Open config").clicked()
                && let Some(editor) = &self.editor
            {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&editor.path)
                    .spawn();
            }
        });
        ui.add_space(6.0);
        if let Some(editor) = &self.editor {
            ui.label(
                RichText::new(editor.path.display().to_string())
                    .size(9.0)
                    .color(self.palette.muted),
            );
        }
    }

    fn draw_enable_state(&mut self, ui: &mut egui::Ui, game: &SteamGame) {
        if game.manual {
            if self.running_for(game.appid).is_some() {
                ui.label(
                    RichText::new("Detected while running with ENABLE_VKSUMI=1.")
                        .size(11.0)
                        .color(self.palette.ok),
                );
            } else {
                ui.label(
                    RichText::new(
                        "Launch this executable with ENABLE_VKSUMI=1 to activate the layer.",
                    )
                    .size(11.0)
                    .color(self.palette.warn),
                );
                let exe = game
                    .manual_exe
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "game".into());
                let command = format!("ENABLE_VKSUMI=1 {exe}");
                Frame::new()
                    .fill(self.palette.panel_2)
                    .corner_radius(CornerRadius::same(6))
                    .inner_margin(Margin::symmetric(8, 6))
                    .show(ui, |ui| {
                        ui.add(
                            egui::Label::new(RichText::new(&command).monospace().size(10.0)).wrap(),
                        );
                    });
                if ui.button("Copy launch command").clicked() {
                    ui.ctx().copy_text(command);
                    self.status = StatusMessage::good("Launch command copied");
                }
            }
            return;
        }
        if game.disabled() {
            ui.label(
                RichText::new("DISABLE_VKSUMI=1 is set — vkSumi is explicitly disabled.")
                    .size(11.0)
                    .color(self.palette.warn),
            );
            return;
        }
        if game.vk_enabled() {
            ui.label(
                RichText::new("vkSumi is enabled in this game's Steam launch options.")
                    .size(11.0)
                    .color(self.palette.ok),
            );
        } else {
            ui.label(
                RichText::new("vkSumi is not enabled for this game yet.")
                    .size(11.0)
                    .color(self.palette.warn),
            );
            ui.label(
                RichText::new(
                    "Add the line below once in Steam → Properties → Launch Options. VibranceGUI does not edit Steam's files.",
                )
                .size(10.0)
                .color(self.palette.muted),
            );
            let suggestion = game.suggested_launch_options();
            let preview = suggestion.as_str();
            Frame::new()
                .fill(self.palette.panel_2)
                .corner_radius(CornerRadius::same(6))
                .inner_margin(Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.add(egui::Label::new(RichText::new(preview).monospace().size(10.0)).wrap());
                });
            if ui.button("Copy launch option").clicked() {
                self.copy_launch_option(ui, game);
            }
        }
    }

    fn draw_knob(&mut self, ui: &mut egui::Ui, index: usize) {
        let knob = KNOBS[index];
        let mut value = self.effective_value(knob.key).clamp(knob.min, knob.max);
        let Some(editor) = &mut self.editor else {
            return;
        };
        let overridden = editor.doc.get_f32(knob.key).is_some();
        let enabled = !editor.external;
        ui.horizontal(|ui| {
            ui.label(RichText::new(knob.label).size(12.0).color(if overridden {
                self.palette.text
            } else {
                self.palette.muted
            }));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if overridden && enabled && ui.small_button("reset").clicked() {
                    editor.doc.remove_key(knob.key);
                    editor.dirty = true;
                    editor.last_change = Instant::now();
                    self.status = StatusMessage::info(format!("{} reset to default", knob.label));
                }
                ui.label(
                    RichText::new(format!("{value:+.2}"))
                        .size(11.0)
                        .monospace()
                        .color(if overridden {
                            self.palette.accent
                        } else {
                            self.palette.muted
                        }),
                );
            });
        });
        let resp = value_slider(
            ui,
            &mut value,
            knob.min,
            knob.max,
            knob.step,
            enabled,
            &self.palette,
        );
        if resp.changed()
            && enabled
            && let Some(editor) = &mut self.editor
        {
            editor.doc.set_f32(knob.key, value);
            editor.dirty = true;
            editor.last_change = Instant::now();
            ui.ctx().request_repaint();
        }
    }

    fn draw_bottom_status(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(self.palette.panel)
            .inner_margin(Margin::symmetric(16, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.draw_status(ui);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(editor) = &self.editor {
                            if editor.external {
                                ui.label(
                                    RichText::new("read-only")
                                        .size(10.0)
                                        .color(self.palette.warn),
                                );
                            } else if editor.dirty {
                                ui.label(
                                    RichText::new("saving…")
                                        .size(10.0)
                                        .color(self.palette.muted),
                                );
                            } else {
                                ui.label(
                                    RichText::new("saved").size(10.0).color(self.palette.muted),
                                );
                            }
                        }
                    });
                });
            });
    }
}

impl VibranceGuiApp {
    fn poll_artwork(&mut self, ctx: &egui::Context) {
        let request: Vec<u32> = self
            .games
            .iter()
            .filter(|g| !g.manual)
            .filter(|g| !self.textures.contains_key(&g.appid))
            .filter(|g| self.texture_attempted.contains(&g.appid))
            .map(|g| g.appid)
            .filter(|appid| self.artwork_requested.insert(*appid))
            .collect();
        for appid in request {
            if self.artwork_tx.send(appid).is_err() {
                break;
            }
            ctx.request_repaint();
        }
    }

    fn refresh_library(&mut self) {
        self.steam = SteamData::discover();
        self.layer = VkSumiStatus::detect(&self.paths);
        self.textures.clear();
        self.texture_attempted.clear();
        self.rebuild_games();
        self.status = StatusMessage::info("Steam library refreshed");
    }
}

impl eframe::App for VibranceGuiApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok((appid, path)) = self.artwork_rx.try_recv() {
            if let Some(path) = path {
                self.artwork_paths.insert(appid, path);
            }
            self.texture_attempted.remove(&appid);
            ctx.request_repaint();
        }
        self.poll_artwork(ctx);
        if ctx.input(|i| i.key_pressed(egui::Key::Q) && i.modifiers.command) {
            self.quit_requested = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        while let Ok(action) = self.tray_rx.try_recv() {
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                TrayAction::Refresh => {
                    self.refresh_library();
                }
                TrayAction::Quit => {
                    self.quit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }

        if self.tray.is_some()
            && !self.quit_requested
            && ctx.input(|i| i.viewport().close_requested())
        {
            // Close button means "hide to tray", not "quit".
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.refresh_observation();

        // Expire transient status messages.
        if let Some(until) = self.status.until
            && Instant::now() > until
        {
            self.status.until = None;
        }

        egui::Panel::top("header").show(ui, |ui| {
            self.draw_header(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| {
            self.draw_bottom_status(ui);
        });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(8.0);
            if !self.layer.usable {
                self.draw_missing_layer(ui);
                ui.add_space(8.0);
            }
            self.draw_default_card(ui);
            ui.add_space(10.0);
            self.draw_library(ui);
            ui.add_space(6.0);
        });

        self.draw_game_window(&ctx);
        self.draw_add_steam_window(&ctx);
        self.draw_add_manual_window(&ctx);

        self.save_global_if_due();
        self.save_editor_if_due();
        if self.global_dirty || self.editor.as_ref().map(|e| e.dirty).unwrap_or(false) {
            ctx.request_repaint_after(Duration::from_millis(100));
        }
    }
}

impl Drop for VibranceGuiApp {
    fn drop(&mut self) {
        self.observation_stop.store(true, Ordering::Relaxed);
    }
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn manual_exe_matches(path: Option<&Path>, observed: &str, stem: &str) -> bool {
    let Some(path) = path else {
        return false;
    };
    let file = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    file.eq_ignore_ascii_case(observed) || stem.eq_ignore_ascii_case(observed)
}

fn value_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
    enabled: bool,
    palette: &Palette,
) -> egui::Response {
    let width = ui
        .available_width()
        .min(ui.spacing().slider_width)
        .max(150.0);
    let sense = if enabled {
        Sense::click_and_drag()
    } else {
        Sense::hover()
    };
    let (rect, mut response) = ui.allocate_exact_size(Vec2::new(width, 20.0), sense);
    let left = rect.left() + 9.0;
    let right = rect.right() - 9.0;
    let center_y = rect.center().y;
    let span = (right - left).max(1.0);

    if enabled
        && (response.clicked() || response.dragged())
        && let Some(pointer) = response.interact_pointer_pos()
    {
        let fraction = ((pointer.x - left) / span).clamp(0.0, 1.0);
        let raw = min + fraction * (max - min);
        let stepped = ((raw - min) / step).round() * step + min;
        let new_value = stepped.clamp(min, max);
        if (new_value - *value).abs() > f32::EPSILON {
            *value = new_value;
            response.mark_changed();
        }
    }

    let fraction = ((*value - min) / (max - min)).clamp(0.0, 1.0);
    let thumb_x = left + fraction * span;
    let painter = ui.painter();
    let rail = egui::Rect::from_min_max(
        egui::pos2(left, center_y - 1.5),
        egui::pos2(right, center_y + 1.5),
    );
    painter.rect_filled(rail, CornerRadius::same(2), palette.panel_2);
    if fraction > 0.0 {
        let fill = egui::Rect::from_min_max(
            egui::pos2(left, center_y - 1.5),
            egui::pos2(thumb_x, center_y + 1.5),
        );
        painter.rect_filled(fill, CornerRadius::same(2), palette.accent);
    }
    painter.circle_filled(
        egui::pos2(thumb_x, center_y),
        6.0,
        if enabled { palette.text } else { palette.muted },
    );
    painter.circle_stroke(
        egui::pos2(thumb_x, center_y),
        6.0,
        Stroke::new(
            1.0,
            if enabled {
                palette.accent
            } else {
                palette.panel_2
            },
        ),
    );
    response
}

fn initials(name: &str) -> String {
    name.split_whitespace()
        .filter_map(|part| part.chars().next())
        .take(2)
        .collect::<String>()
        .to_uppercase()
}

fn elide(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn color_for(appid: u32) -> Color32 {
    let hue = (appid as f32 * 47.0) % 360.0;
    let (r, g, b) = hsv_to_rgb(hue, 0.45, 0.45);
    Color32::from_rgb(r, g, b)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match h as u32 / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}
