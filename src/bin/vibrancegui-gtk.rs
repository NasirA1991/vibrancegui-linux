#![allow(dead_code)]

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

use config_doc::ConfigDoc;
use paths::AppPaths;
use state::SavedState;
use steam::SteamData;

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
    let state = SavedState::load(&paths);

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("VibranceGUI for Linux")
        .default_width(460)
        .default_height(680)
        .build();
    let header = adw::HeaderBar::new();
    header.set_title_widget(Some(&adw::WindowTitle::new("VibranceGUI", "for Linux")));

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
    settings_box.append(&gtk::CheckButton::with_label(
        "Start with my desktop session",
    ));
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

    let heading = gtk::Label::new(Some("Program Settings"));
    heading.set_xalign(0.0);
    content.append(&heading);
    let flow = gtk::FlowBox::new();
    flow.set_max_children_per_line(4);
    flow.set_column_spacing(8);
    flow.set_row_spacing(8);
    flow.set_homogeneous(true);
    let games = steam
        .games
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
        let image = if let Some(p) = game.artwork.as_ref().filter(|p| p.is_file()) {
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

    window.set_titlebar(Some(&header));
    window.set_child(Some(&content));
    window.present();
}
