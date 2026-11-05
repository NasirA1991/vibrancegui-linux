#![allow(dead_code)]

#[path = "../artwork.rs"]
mod artwork;
#[path = "../config_doc.rs"]
mod config_doc;
#[path = "../paths.rs"]
mod paths;
#[path = "../process.rs"]
mod process;
#[path = "../shortcuts.rs"]
pub mod shortcuts;
#[path = "../state.rs"]
mod state;
#[path = "../steam.rs"]
mod steam;
#[path = "../vdf.rs"]
pub mod vdf;
#[path = "../vksumi.rs"]
mod vksumi;

use adw::prelude::*;
use gtk::prelude::*;
use gtk4 as gtk;
use libadwaita as adw;

use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use config_doc::{ConfigDoc, KNOBS};
use paths::AppPaths;
use state::SavedState;
use steam::SteamData;

#[derive(Debug, Clone, Copy)]
enum TrayAction {
    Show,
    Quit,
}

struct GtkTray {
    tx: Sender<TrayAction>,
}

impl ksni::Tray for GtkTray {
    const MENU_ON_ACTIVATE: bool = false;
    fn id(&self) -> String {
        "vibrancegui".into()
    }
    fn title(&self) -> String {
        "VibranceGUI for Linux".into()
    }
    fn icon_name(&self) -> String {
        "vibrancegui".into()
    }
    fn activate(&mut self, _x: i32, _y: i32) {
        let _ = self.tx.send(TrayAction::Show);
    }
    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        vec![
            StandardItem {
                label: "Show VibranceGUI".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(TrayAction::Show);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(|t: &mut Self| {
                    let _ = t.tx.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn spawn_tray() -> (
    Option<ksni::blocking::Handle<GtkTray>>,
    Receiver<TrayAction>,
) {
    use ksni::blocking::TrayMethods;
    let (tx, rx) = mpsc::channel();
    let handle = GtkTray { tx }.spawn().ok();
    (handle, rx)
}

fn main() -> gtk::glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("io.github.nasira1991.VibranceGUI")
        .build();
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let paths = AppPaths::discover().expect("home directory");
    let _ = paths.ensure();
    let steam = SteamData::discover();
    let mut state = SavedState::load(&paths);
    for obs in process::scan(&paths).games {
        if obs.appid.is_some() || obs.explicitly_disabled {
            continue;
        }
        let Some(exe) = obs.exe_path.clone() else {
            continue;
        };
        if state.manual_games.values().any(|m| m.exe == exe) {
            continue;
        }
        let id = 0x8000_0000u32 | state.next_manual_id.max(1);
        state.next_manual_id = state.next_manual_id.saturating_add(1).max(1);
        let stem = exe
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("game")
            .to_string();
        state.manual_games.insert(
            id,
            state::ManualGame {
                name: stem.clone(),
                icon: artwork::extract_exe_icon(&paths.cache_dir, &exe),
                exe,
                config_path: obs
                    .config_path
                    .unwrap_or_else(|| paths.vksumi_games_dir().join(format!("{stem}.conf"))),
            },
        );
    }
    let _ = state.save(&paths);
    let mut all_games = steam.games.clone();
    for (id, m) in &state.manual_games {
        all_games.push(steam::SteamGame {
            appid: *id,
            name: m.name.clone(),
            install_dir: m.exe.parent().map(|p| p.to_path_buf()).unwrap_or_default(),
            artwork: m.icon.clone(),
            launch_options: String::new(),
            manual: true,
            manual_exe: Some(m.exe.clone()),
            manual_config: Some(m.config_path.clone()),
            shortcut: false,
        });
    }

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("VibranceGUI for Linux")
        .default_width(460)
        .default_height(680)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("VibranceGUI", "for Linux")));
    let refresh = gtk::Button::with_label("Refresh");
    refresh.connect_clicked(|_| restart_app());
    header.pack_start(&refresh);

    let menu = gtk::MenuButton::builder()
        .icon_name("open-menu-symbolic")
        .build();
    let popover = gtk::Popover::new();
    let menu_box = gtk::Box::new(gtk::Orientation::Vertical, 4);
    menu_box.set_margin_top(8);
    menu_box.set_margin_bottom(8);
    menu_box.set_margin_start(8);
    menu_box.set_margin_end(8);
    let style = adw::StyleManager::default();
    for (label, scheme) in [
        ("System theme", adw::ColorScheme::Default),
        ("Dark theme", adw::ColorScheme::ForceDark),
        ("Light theme", adw::ColorScheme::ForceLight),
    ] {
        let b = gtk::Button::with_label(label);
        b.set_has_frame(false);
        let style = style.clone();
        b.connect_clicked(move |_| style.set_color_scheme(scheme));
        menu_box.append(&b);
    }
    menu_box.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let about = gtk::Button::with_label("About VibranceGUI");
    about.set_has_frame(false);
    let parent = window.clone();
    about.connect_clicked(move |_| {
        let dialog = gtk::AboutDialog::builder()
            .program_name("VibranceGUI for Linux")
            .version(env!("CARGO_PKG_VERSION"))
            .comments("Unofficial vkSumi profile manager for Steam and non-Steam games.")
            .website("https://github.com/NasirA1991/vibrancegui-linux")
            .website_label("GitHub repository")
            .build();
        dialog.set_transient_for(Some(&parent));
        dialog.present();
    });
    menu_box.append(&about);
    popover.set_child(Some(&menu_box));
    menu.set_popover(Some(&popover));
    header.pack_end(&menu);

    let missing_art: Vec<u32> = all_games
        .iter()
        .filter(|g| !g.manual)
        .filter(|g| {
            g.artwork.as_ref().map(|p| !p.is_file()).unwrap_or(true)
                && !artwork::cache_path(&paths.cache_dir, g.appid).is_file()
        })
        .map(|g| g.appid)
        .collect();
    if !missing_art.is_empty() {
        let cache = paths.cache_dir.clone();
        std::thread::spawn(move || {
            let mut downloaded = false;
            for appid in missing_art {
                if artwork::download(&cache, appid).is_ok() {
                    downloaded = true;
                }
            }
            if downloaded {
                gtk::glib::idle_add(|| {
                    restart_app();
                    gtk::glib::ControlFlow::Continue
                });
            }
        });
    }

    let stack = gtk::Stack::new();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let settings = gtk::Frame::builder().label("Settings").build();
    let settings_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    settings_box.set_margin_top(8);
    settings_box.set_margin_bottom(8);
    settings_box.set_margin_start(10);
    settings_box.set_margin_end(10);
    let autostart = gtk::CheckButton::with_label("Start with my desktop session");
    autostart.set_active(
        paths
            .vksumi_config_dir
            .parent()
            .unwrap_or(&paths.home)
            .join("autostart/vibrancegui.desktop")
            .is_file(),
    );
    let autostart_paths = paths.clone();
    let autostart_button = autostart.clone();
    autostart.connect_toggled(move |_| {
        let path = autostart_paths
            .vksumi_config_dir
            .parent()
            .unwrap_or(&autostart_paths.home)
            .join("autostart/vibrancegui.desktop");
        if autostart_button.is_active() {
            let exe = std::env::var_os("APPIMAGE")
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::current_exe().ok())
                .unwrap_or_else(|| std::path::PathBuf::from("vibrancegui"));
            let exec = if exe.to_string_lossy().contains(' ') {
                format!("\"{}\" --hidden", exe.display())
            } else {
                format!("{} --hidden", exe.display())
            };
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let body = format!("[Desktop Entry]\nType=Application\nName=VibranceGUI for Linux\nExec={exec}\nIcon=vibrancegui\nTerminal=false\nX-GNOME-Autostart-enabled=true\n");
            let _ = std::fs::write(path, body);
        } else {
            let _ = std::fs::remove_file(path);
        }
    });
    settings_box.append(&autostart);
    let label = gtk::Label::new(Some("Default Vibrance Level"));
    label.set_xalign(0.0);
    settings_box.append(&label);
    let global_path = paths.global_vksumi_conf();
    let mut doc = ConfigDoc::load(&global_path)
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            let mut d = ConfigDoc {
                path: global_path.clone(),
                lines: Vec::new(),
            };
            d.set_path(global_path.clone());
            d
        });
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
    scale.set_value(doc.get_f32("vibrance").unwrap_or(0.0).clamp(0.0, 1.0) as f64);
    scale.set_hexpand(true);
    scale.set_draw_value(false);
    let save_paths = paths.clone();
    let doc = std::rc::Rc::new(std::cell::RefCell::new(doc));
    let doc_clone = doc.clone();
    scale.connect_value_changed(move |s| {
        let mut doc = doc_clone.borrow_mut();
        doc.set_f32("vibrance", s.value() as f32);
        let _ = doc.save(&save_paths);
    });
    settings_box.append(&scale);
    settings.set_child(Some(&settings_box));
    content.append(&settings);

    let heading_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    let heading = gtk::Label::new(Some("Program Settings"));
    heading.set_xalign(0.0);
    heading.set_hexpand(true);
    heading_row.append(&heading);
    let add_steam = gtk::Button::with_label("Add Steam game");
    heading_row.append(&add_steam);
    let steam_games = steam.games.clone();
    let add_steam_paths = paths.clone();
    let add_steam_window = window.clone();
    add_steam.connect_clicked(move |_| {
        let dialog = gtk::Window::builder()
            .title("Add a Steam game")
            .transient_for(&add_steam_window)
            .modal(true)
            .default_width(420)
            .default_height(520)
            .build();
        let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 2);
        let paths = add_steam_paths.clone();
        let dialog_clone = dialog.clone();
        let mut state = SavedState::load(&paths);
        for game in &steam_games {
            if state.tracked_steam.contains(&game.appid) {
                continue;
            }
            let button = gtk::Button::with_label(&game.name);
            button.set_has_frame(false);
            let paths = paths.clone();
            let dialog = dialog_clone.clone();
            let appid = game.appid;
            button.connect_clicked(move |_| {
                let mut state = SavedState::load(&paths);
                state.tracked_steam.insert(appid);
                let _ = state.save(&paths);
                dialog.close();
                restart_app();
            });
            list.append(&button);
        }
        scroll.set_child(Some(&list));
        dialog.set_child(Some(&scroll));
        dialog.present();
    });
    let add_manual = gtk::Button::with_label("Add manually");
    heading_row.append(&add_manual);
    let add_paths = paths.clone();
    let add_window = window.clone();
    add_manual.connect_clicked(move |_| {
        let dialog = gtk::FileChooserNative::new(
            Some("Choose game executable"),
            Some(&add_window),
            gtk::FileChooserAction::Open,
            Some("Open"),
            Some("Cancel"),
        );
        let paths = add_paths.clone();
        dialog.connect_response(move |dialog, response| {
            if response != gtk::ResponseType::Accept {
                return;
            }
            let Some(exe) = dialog.file().and_then(|f| f.path()) else {
                return;
            };
            let stem = exe
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("game")
                .to_string();
            let mut state = SavedState::load(&paths);
            let id = 0x8000_0000u32 | state.next_manual_id.max(1);
            state.next_manual_id = state.next_manual_id.saturating_add(1).max(1);
            state.manual_games.insert(
                id,
                state::ManualGame {
                    name: stem.clone(),
                    icon: artwork::extract_exe_icon(&paths.cache_dir, &exe),
                    config_path: paths.vksumi_games_dir().join(format!("{stem}.conf")),
                    exe,
                },
            );
            let _ = state.save(&paths);
            restart_app();
        });
        dialog.show();
    });
    content.append(&heading_row);
    let flow = gtk::FlowBox::new();
    flow.set_max_children_per_line(4);
    flow.set_column_spacing(8);
    flow.set_row_spacing(8);
    flow.set_homogeneous(true);
    let games = all_games
        .iter()
        .filter(|g| g.vk_enabled() || g.shortcut || g.manual);
    for game in games {
        let b = gtk::Button::new();
        b.add_css_class("flat");
        b.add_css_class("card");
        b.set_tooltip_text(Some(&game.name));
        let card = gtk::Box::new(gtk::Orientation::Vertical, 4);
        card.set_margin_top(6);
        card.set_margin_bottom(6);
        card.set_margin_start(6);
        card.set_margin_end(6);
        let icon_path = game
            .artwork
            .clone()
            .filter(|p| p.is_file())
            .or_else(|| {
                let cached = artwork::cache_path(&paths.cache_dir, game.appid);
                cached.is_file().then_some(cached)
            })
            .or_else(|| {
                game.manual_exe
                    .as_ref()
                    .and_then(|exe| artwork::extract_exe_icon(&paths.cache_dir, exe))
            });
        let image = if let Some(p) = icon_path.as_ref() {
            gtk::Image::from_file(p)
        } else {
            gtk::Image::from_icon_name("application-x-executable")
        };
        image.set_pixel_size(64);
        card.append(&image);
        let name = gtk::Label::new(Some(&game.name));
        name.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name.set_max_width_chars(16);
        name.set_xalign(0.0);
        card.append(&name);
        b.set_child(Some(&card));
        let stack_clone = stack.clone();
        let paths_clone = paths.clone();
        let state_clone = state.clone();
        let game_clone = game.clone();
        b.connect_clicked(move |_| {
            let detail = build_detail_page(&paths_clone, &state_clone, &game_clone, &stack_clone);
            if let Some(old) = stack_clone.child_by_name("detail") {
                stack_clone.remove(&old);
            }
            stack_clone.add_named(&detail, Some("detail"));
            stack_clone.set_visible_child_name("detail");
        });
        flow.insert(&b, -1);
    }
    let scroll = gtk::ScrolledWindow::builder()
        .hscrollbar_policy(gtk::PolicyType::Never)
        .vexpand(true)
        .build();
    scroll.set_child(Some(&flow));
    content.append(&scroll);

    let status = gtk::Label::new(Some(&format!(
        "vkSumi detected · {} tracked · {} installed",
        state.games.len(),
        steam.games.len()
    )));
    status.set_xalign(0.0);
    status.add_css_class("dim-label");
    content.append(&status);

    stack.add_named(&content, Some("games"));
    window.set_titlebar(Some(&header));
    window.set_child(Some(&stack));
    window.present();

    let (tray, tray_rx) = spawn_tray();
    let tray_window = window.clone();
    gtk::glib::timeout_add_local(Duration::from_millis(200), move || {
        let _keep_alive = &tray;
        while let Ok(action) = tray_rx.try_recv() {
            match action {
                TrayAction::Show => tray_window.present(),
                TrayAction::Quit => tray_window.close(),
            }
        }
        gtk::glib::ControlFlow::Continue
    });

    let detect_paths = paths.clone();
    gtk::glib::timeout_add_seconds_local(3, move || {
        let mut state = SavedState::load(&detect_paths);
        let mut adopted = false;
        for obs in process::scan(&detect_paths).games {
            if obs.appid.is_some() || obs.explicitly_disabled {
                continue;
            }
            let Some(exe) = obs.exe_path.clone() else {
                continue;
            };
            if state.manual_games.values().any(|m| m.exe == exe) {
                continue;
            }
            let id = 0x8000_0000u32 | state.next_manual_id.max(1);
            state.next_manual_id = state.next_manual_id.saturating_add(1).max(1);
            let stem = exe
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("game")
                .to_string();
            state.manual_games.insert(
                id,
                state::ManualGame {
                    name: stem.clone(),
                    icon: artwork::extract_exe_icon(&detect_paths.cache_dir, &exe),
                    exe,
                    config_path: obs.config_path.unwrap_or_else(|| {
                        detect_paths.vksumi_games_dir().join(format!("{stem}.conf"))
                    }),
                },
            );
            adopted = true;
        }
        if adopted {
            let _ = state.save(&detect_paths);
            restart_app();
        }
        gtk::glib::ControlFlow::Continue
    });
}

fn config_path_for(
    paths: &AppPaths,
    state: &SavedState,
    game: &steam::SteamGame,
) -> Option<PathBuf> {
    if let Some(mapping) = state.games.get(&game.appid) {
        return Some(mapping.config_path.clone());
    }
    if let Some(env) = game.config_env() {
        return Some(paths::expand_user_path(&env.to_string_lossy(), &paths.home));
    }
    let stem = game.manual_exe.as_ref()?.file_stem()?.to_str()?;
    Some(paths.vksumi_games_dir().join(format!("{stem}.conf")))
}

fn build_detail_page(
    paths: &AppPaths,
    state: &SavedState,
    game: &steam::SteamGame,
    stack: &gtk::Stack,
) -> gtk::Widget {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 10);
    page.set_margin_top(12);
    page.set_margin_bottom(12);
    page.set_margin_start(12);
    page.set_margin_end(12);
    let back = gtk::Button::with_label("← Games");
    let stack_clone = stack.clone();
    back.connect_clicked(move |_| {
        stack_clone.set_visible_child_name("games");
    });
    let reload = gtk::Button::with_label("Reload");
    reload.connect_clicked(|_| restart_app());
    let remove = gtk::Button::with_label("Remove");
    let remove_paths = paths.clone();
    let appid = game.appid;
    remove.connect_clicked(move |_| {
        let mut state = SavedState::load(&remove_paths);
        state.tracked_steam.remove(&appid);
        state.manual_games.remove(&appid);
        let _ = state.save(&remove_paths);
        restart_app();
    });
    let top = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    top.append(&back);
    top.append(&reload);
    top.append(&remove);
    page.append(&top);
    let title = gtk::Label::new(Some(&game.name));
    title.set_xalign(0.0);
    title.add_css_class("title-2");
    page.append(&title);
    page.append(&gtk::Label::new(Some(if game.vk_enabled() {
        "vkSumi is enabled in this game's launch options."
    } else {
        "vkSumi is not enabled for this game yet."
    })));
    let Some(path) = config_path_for(paths, state, game) else {
        page.append(&gtk::Label::new(Some(
            "No profile linked yet. Launch with ENABLE_VKSUMI=1, then Refresh.",
        )));
        return page.upcast();
    };
    let mut doc = match ConfigDoc::load(&path) {
        Ok(Some(doc)) => doc,
        Ok(None) => {
            let mut d = ConfigDoc {
                path: path.clone(),
                lines: Vec::new(),
            };
            d.set_path(path.clone());
            d
        }
        Err(err) => {
            page.append(&gtk::Label::new(Some(&format!(
                "Could not read profile: {err}"
            ))));
            return page.upcast();
        }
    };
    page.append(&gtk::Label::new(Some(&path.display().to_string())));
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    row.append(&gtk::Label::new(Some("Vibrance")));
    let slider = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
    slider.set_hexpand(true);
    slider.set_draw_value(false);
    slider.set_value(doc.get_f32("vibrance").unwrap_or(0.0).clamp(0.0, 1.0) as f64);
    let value = gtk::Label::new(Some(&format!("{:.0}%", slider.value() * 100.0)));
    let value_clone = value.clone();
    let paths_save = paths.clone();
    let doc = std::rc::Rc::new(std::cell::RefCell::new(doc));
    let doc_clone = doc.clone();
    slider.connect_value_changed(move |s| {
        value_clone.set_label(&format!("{:.0}%", s.value() * 100.0));
        let mut d = doc_clone.borrow_mut();
        d.set_f32("vibrance", s.value() as f32);
        let _ = d.save(&paths_save);
    });
    row.append(&slider);
    row.append(&value);
    page.append(&row);

    let advanced = gtk::Expander::new(Some("Advanced (all vkSumi sliders)"));
    let adv_box = gtk::Box::new(gtk::Orientation::Vertical, 6);
    for knob in KNOBS.iter().skip(1) {
        let knob_row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        let label = gtk::Label::new(Some(knob.label));
        label.set_xalign(0.0);
        label.set_hexpand(true);
        knob_row.append(&label);
        let knob_scale = gtk::Scale::with_range(
            gtk::Orientation::Horizontal,
            knob.min as f64,
            knob.max as f64,
            knob.step as f64,
        );
        knob_scale.set_hexpand(true);
        knob_scale.set_draw_value(true);
        knob_scale.set_value(
            doc.borrow()
                .get_f32(knob.key)
                .unwrap_or(0.0)
                .clamp(knob.min, knob.max) as f64,
        );
        let paths_save = paths.clone();
        let doc_clone = doc.clone();
        let key = knob.key.to_string();
        knob_scale.connect_value_changed(move |s| {
            let mut d = doc_clone.borrow_mut();
            d.set_f32(&key, s.value() as f32);
            let _ = d.save(&paths_save);
        });
        knob_row.append(&knob_scale);
        adv_box.append(&knob_row);
    }
    advanced.set_child(Some(&adv_box));
    page.append(&advanced);
    page.upcast()
}

fn restart_app() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    std::process::exit(0);
}
