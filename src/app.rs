use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver};
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
use crate::tray::{self, TrayAction, VibranceTray};
use crate::vksumi::VkSumiStatus;

const ACCENT: Color32 = Color32::from_rgb(232, 153, 74);
const PANEL: Color32 = Color32::from_rgb(27, 27, 32);
const PANEL_2: Color32 = Color32::from_rgb(34, 34, 40);
const MUTED: Color32 = Color32::from_rgb(145, 145, 158);
const TEXT: Color32 = Color32::from_rgb(235, 235, 240);
const OK: Color32 = Color32::from_rgb(97, 201, 141);
const WARN: Color32 = Color32::from_rgb(236, 188, 91);

pub struct VibranceGuiApp {
    paths: AppPaths,
    layer: VkSumiStatus,
    steam: SteamData,
    state: SavedState,
    selected: Option<u32>,
    search: String,
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
    textures: HashMap<u32, TextureHandle>,
    texture_attempted: HashSet<u32>,
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
    pub fn new(cc: &eframe::CreationContext<'_>, use_tray: bool) -> Self {
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

        let (tray_tx, tray_rx) = mpsc::channel();
        let tray = if use_tray {
            tray::spawn(cc.egui_ctx.clone(), tray_tx)
        } else {
            None
        };

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
            state,
            selected: None,
            search: String::new(),
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
            textures: HashMap::new(),
            texture_attempted: HashSet::new(),
            status: initial_status,
            needs_editor_refresh: false,
        };
        app.select_first_game();
        app.refresh_observation();
        app
    }

    fn select_first_game(&mut self) {
        if self.selected.is_none() {
            self.selected = self.steam.games.first().map(|g| g.appid);
            self.open_selected();
        }
    }

    fn game(&self, appid: u32) -> Option<&SteamGame> {
        self.steam.games.iter().find(|g| g.appid == appid)
    }

    fn running_for(&self, appid: u32) -> Option<&ObservedGame> {
        self.observed
            .iter()
            .find(|o| o.appid == Some(appid) && !o.explicitly_disabled)
    }

    fn observed_path(&self, appid: u32) -> Option<PathBuf> {
        self.observed
            .iter()
            .find(|o| o.appid == Some(appid))
            .and_then(|o| o.config_path.clone())
    }

    fn config_path_for(&self, game: &SteamGame) -> Option<PathBuf> {
        if let Some(mapping) = self.state.games.get(&game.appid)
            && (mapping.config_path.exists() || self.paths.is_inside_vksumi(&mapping.config_path))
        {
            return Some(mapping.config_path.clone());
        }
        if let Some(env) = game.config_env() {
            return Some(expand_user_path(&env.to_string_lossy(), &self.paths.home));
        }
        self.observed_path(game.appid)
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
                    lines: vec![format!(
                        "# vkSumi profile for {} (Steam AppID {})",
                        game.name, game.appid
                    )],
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
        if changed {
            let _ = self.state.save(&self.paths);
            self.needs_editor_refresh = true;
        }
        if self.editor.is_none()
            && let Some(appid) = self
                .observed
                .iter()
                .find_map(|o| if o.explicitly_disabled { None } else { o.appid })
            && self.selected != Some(appid)
        {
            self.selected = Some(appid);
            self.needs_editor_refresh = true;
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
            .fill(PANEL)
            .inner_margin(Margin::symmetric(16, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("VibranceGUI").size(22.0).strong().color(TEXT));
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("for Linux · vkSumi, made simple")
                            .size(12.0)
                            .color(MUTED),
                    );
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ui.button("Refresh").clicked() {
                            self.refresh_library();
                        }
                        self.status_chip(
                            ui,
                            self.layer.status_text(),
                            if self.layer.usable { OK } else { WARN },
                        );
                        let account = self
                            .steam
                            .account_id
                            .clone()
                            .unwrap_or_else(|| "Steam".into());
                        self.status_chip(ui, &format!("Steam {account}"), MUTED);
                    });
                });
            });
    }

    fn status_chip(&self, ui: &mut egui::Ui, text: &str, color: Color32) {
        Frame::new()
            .fill(PANEL_2)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::symmetric(9, 4))
            .show(ui, |ui| {
                ui.label(RichText::new(text).size(11.0).color(color));
            });
    }

    fn draw_missing_layer(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(Color32::from_rgb(52, 43, 30))
            .corner_radius(CornerRadius::same(9))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new("vkSumi is not installed or could not be found.")
                            .strong()
                            .color(WARN),
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
                    .color(MUTED),
                );
                if !self.layer.problems.is_empty() {
                    ui.label(
                        RichText::new(&self.layer.problems[0])
                            .size(9.0)
                            .color(MUTED),
                    );
                }
            });
    }

    fn draw_default_card(&mut self, ui: &mut egui::Ui) {
        Frame::new()
            .fill(PANEL)
            .corner_radius(CornerRadius::same(10))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Default").strong().color(TEXT));
                    ui.label(
                        RichText::new("for games without a per-game override")
                            .size(11.0)
                            .color(MUTED),
                    );
                });
                let mut value = self.global_value("vibrance");
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Vibrance").size(11.0).color(MUTED));
                    ui.label(
                        RichText::new(format!("{value:+.2}"))
                            .monospace()
                            .size(10.0)
                            .color(MUTED),
                    );
                });
                let resp = value_slider(ui, &mut value, -1.0, 1.0, 0.01, true);
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
                    .color(MUTED),
                );
            });
    }

    fn draw_status(&self, ui: &mut egui::Ui) {
        let color = match self.status.kind {
            StatusKind::Info => MUTED,
            StatusKind::Good => OK,
            StatusKind::Warn => WARN,
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("●").color(color).size(10.0));
            ui.label(RichText::new(&self.status.text).size(11.0).color(color));
            if let Some(account) = &self.steam.account_id {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("account {account}"))
                            .size(10.0)
                            .color(MUTED),
                    );
                });
            }
        });
    }

    fn draw_library(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let enabled_count = self.steam.games.iter().filter(|g| g.vk_enabled()).count();
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!(
                    "Games · {} · {} enabled",
                    self.steam.games.len(),
                    enabled_count
                ))
                .strong()
                .color(TEXT),
            );
            ui.add_space(8.0);
            let search = egui::TextEdit::singleline(&mut self.search)
                .hint_text("Search games…")
                .desired_width(220.0);
            ui.add(search);
            if !self.search.is_empty() && ui.small_button("clear").clicked() {
                self.search.clear();
            }
            if !self.scan_available {
                ui.label(
                    RichText::new("/proc scan unavailable")
                        .size(10.0)
                        .color(WARN),
                );
            }
        });
        ui.add_space(10.0);

        let needle = self.search.to_lowercase();
        let mut indices: Vec<usize> = self
            .steam
            .games
            .iter()
            .enumerate()
            .filter(|(_, g)| needle.is_empty() || g.name.to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect();
        indices.sort_by(|&a, &b| {
            let ga = &self.steam.games[a];
            let gb = &self.steam.games[b];
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
                ui.label(RichText::new("No games found").color(MUTED));
                ui.label(
                    RichText::new("Check your Steam library paths or clear the search.")
                        .size(11.0)
                        .color(MUTED),
                );
            });
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(10.0, 12.0);
                    for idx in indices {
                        let game = self.steam.games[idx].clone();
                        self.draw_game_tile(ui, ctx, &game);
                    }
                });
                ui.add_space(12.0);
            });
    }

    fn draw_game_tile(&mut self, ui: &mut egui::Ui, ctx: &egui::Context, game: &SteamGame) {
        let selected = self.selected == Some(game.appid);
        let running = self.running_for(game.appid).is_some();
        let enabled = game.vk_enabled();
        let tile_size = Vec2::new(176.0, 136.0);
        let (rect, response) = ui.allocate_exact_size(tile_size, Sense::click());
        if response.clicked() {
            self.set_selected(game.appid);
        }
        let hover = response.hovered();
        let painter = ui.painter_at(rect);
        let bg = if selected {
            Color32::from_rgb(50, 42, 34)
        } else if hover {
            Color32::from_rgb(38, 38, 44)
        } else {
            PANEL
        };
        painter.rect_filled(rect, CornerRadius::same(9), bg);
        if selected {
            painter.rect_stroke(
                rect,
                CornerRadius::same(9),
                Stroke::new(1.5, ACCENT),
                StrokeKind::Inside,
            );
        }

        let image_rect = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), 94.0));
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
                    egui::pos2(rect.left(), rect.top() + 70.0),
                    Vec2::new(rect.width(), 24.0),
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
                FontId::proportional(30.0),
                Color32::from_white_alpha(210),
            );
        }

        let title = elide(&game.name, 25);
        painter.text(
            egui::pos2(rect.left() + 9.0, rect.top() + 103.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(13.0),
            TEXT,
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
        } else {
            "not enabled"
        };
        let sub_color = if running {
            OK
        } else if enabled {
            ACCENT
        } else {
            MUTED
        };
        painter.text(
            egui::pos2(rect.left() + 9.0, rect.top() + 119.0),
            Align2::LEFT_TOP,
            sub,
            FontId::proportional(10.0),
            sub_color,
        );
        if enabled || running {
            painter.circle_filled(
                egui::pos2(rect.right() - 10.0, rect.top() + 10.0),
                3.5,
                if running { OK } else { ACCENT },
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
        let path = game.artwork.as_ref()?;
        let image = image::open(path).ok()?;
        let image = image.resize_to_fill(352, 188, image::imageops::FilterType::Triangle);
        let rgba = image.to_rgba8();
        let size = [rgba.width() as usize, rgba.height() as usize];
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
        let texture = ctx.load_texture(
            format!("steam-art-{}", game.appid),
            color_image,
            TextureOptions::LINEAR,
        );
        self.textures.insert(game.appid, texture.clone());
        Some(texture)
    }

    fn draw_detail(&mut self, ui: &mut egui::Ui) {
        let Some(appid) = self.selected else {
            ui.add_space(40.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new("Select a game").size(18.0).color(MUTED));
                ui.label(
                    RichText::new(
                        "VibranceGUI edits vkSumi profiles. It does not inject into running games.",
                    )
                    .size(11.0)
                    .color(MUTED),
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
            ui.label(RichText::new(&game.name).size(20.0).strong().color(TEXT));
        });
        ui.label(
            RichText::new(format!(
                "Steam AppID {} · {}",
                game.appid,
                game.install_dir.display()
            ))
            .size(10.0)
            .color(MUTED),
        );
        ui.add_space(10.0);

        let running = self.running_for(appid).cloned();
        if let Some(running) = &running {
            let (text, color) = if running.layer_mapped {
                ("Detected · vkSumi layer mapped", OK)
            } else {
                ("Detected · layer unconfirmed", WARN)
            };
            Frame::new()
                .fill(PANEL_2)
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
                    .color(TEXT),
            );
            if game.vk_enabled() {
                ui.label(
                    RichText::new(
                        "vkSumi is enabled, but this profile is not linked yet. Launch the game once while VibranceGUI is open; it will detect the running game and link the config.",
                    )
                    .size(11.0)
                    .color(MUTED),
                );
            } else {
                ui.label(
                    RichText::new(
                        "After adding the launch option above, launch the game once with VibranceGUI open. The profile will link automatically.",
                    )
                    .size(11.0)
                    .color(MUTED),
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
                .fill(Color32::from_rgb(52, 43, 30))
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "enabled = false is set in this profile. The color grade is paused; remove that override if you want this game graded.",
                        )
                        .size(11.0)
                        .color(WARN),
                    );
                });
            ui.add_space(8.0);
        } else if self.global_doc.get_bool("enabled") == Some(false) {
            Frame::new()
                .fill(Color32::from_rgb(52, 43, 30))
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "The global vkSumi config has enabled = false. Effects are paused for games that do not override it.",
                        )
                        .size(11.0)
                        .color(WARN),
                    );
                });
            ui.add_space(8.0);
        }
        if editor.external {
            Frame::new()
                .fill(Color32::from_rgb(52, 43, 30))
                .corner_radius(CornerRadius::same(7))
                .inner_margin(Margin::symmetric(10, 7))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(
                            "External config: this path is outside ~/.config/vkSumi. VibranceGUI is showing it read-only.",
                        )
                        .size(11.0)
                        .color(WARN),
                    );
                });
            ui.add_space(8.0);
        }

        ui.label(RichText::new("Basic").strong().color(TEXT));
        ui.add_space(2.0);
        self.draw_knob(ui, 0);
        ui.add_space(8.0);
        ui.collapsing("Advanced", |ui| {
            for i in 1..KNOBS.len() {
                self.draw_knob(ui, i);
            }
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
                    .color(MUTED),
            );
        }
    }

    fn draw_enable_state(&mut self, ui: &mut egui::Ui, game: &SteamGame) {
        if game.disabled() {
            ui.label(
                RichText::new("DISABLE_VKSUMI=1 is set — vkSumi is explicitly disabled.")
                    .size(11.0)
                    .color(WARN),
            );
            return;
        }
        if game.vk_enabled() {
            ui.label(
                RichText::new("vkSumi is enabled in this game's Steam launch options.")
                    .size(11.0)
                    .color(OK),
            );
        } else {
            ui.label(
                RichText::new("vkSumi is not enabled for this game yet.")
                    .size(11.0)
                    .color(WARN),
            );
            ui.label(
                RichText::new(
                    "Add the line below once in Steam → Properties → Launch Options. VibranceGUI does not edit Steam's files.",
                )
                .size(10.0)
                .color(MUTED),
            );
            let suggestion = game.suggested_launch_options();
            let preview = suggestion.as_str();
            Frame::new()
                .fill(PANEL_2)
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
        let mut value = self.effective_value(knob.key);
        let Some(editor) = &mut self.editor else {
            return;
        };
        let overridden = editor.doc.get_f32(knob.key).is_some();
        let enabled = !editor.external;
        ui.horizontal(|ui| {
            ui.label(RichText::new(knob.label).size(12.0).color(if overridden {
                TEXT
            } else {
                MUTED
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
                        .color(if overridden { ACCENT } else { MUTED }),
                );
            });
        });
        let resp = value_slider(ui, &mut value, knob.min, knob.max, knob.step, enabled);
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
            .fill(PANEL)
            .inner_margin(Margin::symmetric(16, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    self.draw_status(ui);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if let Some(editor) = &self.editor {
                            if editor.external {
                                ui.label(RichText::new("read-only").size(10.0).color(WARN));
                            } else if editor.dirty {
                                ui.label(RichText::new("saving…").size(10.0).color(MUTED));
                            } else {
                                ui.label(RichText::new("saved").size(10.0).color(MUTED));
                            }
                        }
                    });
                });
            });
    }
}

impl VibranceGuiApp {
    fn refresh_library(&mut self) {
        self.steam = SteamData::discover();
        self.layer = VkSumiStatus::detect(&self.paths);
        self.textures.clear();
        self.texture_attempted.clear();
        self.status = StatusMessage::info("Steam library refreshed");
    }
}

impl eframe::App for VibranceGuiApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
        egui::Panel::right("detail")
            .resizable(true)
            .default_size(390.0)
            .min_size(330.0)
            .show(ui, |ui| {
                Frame::new()
                    .fill(PANEL)
                    .inner_margin(Margin::same(16))
                    .show(ui, |ui| {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .show(ui, |ui| {
                                self.draw_detail(ui);
                            });
                    });
            });
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(10.0);
            if !self.layer.usable {
                self.draw_missing_layer(ui);
                ui.add_space(10.0);
            }
            self.draw_default_card(ui);
            ui.add_space(12.0);
            self.draw_library(ui, &ctx);
        });

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

fn value_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    min: f32,
    max: f32,
    step: f32,
    enabled: bool,
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
    painter.rect_filled(rail, CornerRadius::same(2), PANEL_2);
    if fraction > 0.0 {
        let fill = egui::Rect::from_min_max(
            egui::pos2(left, center_y - 1.5),
            egui::pos2(thumb_x, center_y + 1.5),
        );
        painter.rect_filled(fill, CornerRadius::same(2), ACCENT);
    }
    painter.circle_filled(
        egui::pos2(thumb_x, center_y),
        6.0,
        if enabled { Color32::WHITE } else { MUTED },
    );
    painter.circle_stroke(
        egui::pos2(thumb_x, center_y),
        6.0,
        Stroke::new(1.0, if enabled { ACCENT } else { PANEL_2 }),
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
