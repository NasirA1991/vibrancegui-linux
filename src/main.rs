mod app;
mod config_doc;
mod paths;
mod process;
mod state;
mod steam;
mod tray;
mod vdf;
mod vksumi;

use eframe::egui;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--doctor") {
        return doctor();
    }
    let use_tray = !args.iter().any(|arg| arg == "--no-tray");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1040.0, 720.0])
            .with_min_inner_size([780.0, 540.0])
            .with_title("VibranceGUI for Linux")
            .with_app_id("vibrancegui"),
        ..Default::default()
    };

    eframe::run_native(
        "vibrancegui",
        options,
        Box::new(|cc| {
            configure_theme(&cc.egui_ctx);
            Ok(Box::new(app::VibranceGuiApp::new(cc, use_tray)))
        }),
    )
}

fn doctor() -> eframe::Result {
    let paths = paths::AppPaths::discover().expect("home directory");
    let layer = vksumi::VkSumiStatus::detect(&paths);
    let steam = steam::SteamData::discover();
    println!("VibranceGUI for Linux doctor");
    println!("  vkSumi usable: {}", layer.usable);
    if let Some(manifest) = &layer.manifest {
        println!("  manifest: {}", manifest.display());
    }
    if let Some(library) = &layer.library {
        println!("  library: {}", library.display());
    }
    for problem in &layer.problems {
        println!("  problem: {problem}");
    }
    println!("  Steam root: {:?}", steam.root);
    println!("  Steam account: {:?}", steam.account_id);
    println!("  Steam games: {}", steam.games.len());
    for game in &steam.games {
        println!(
            "    {:>8}  {:<40} enabled={}  options={:?}",
            game.appid,
            game.name,
            game.vk_enabled(),
            game.launch_options
        );
    }
    for warning in &steam.warnings {
        println!("  warning: {warning}");
    }
    if std::env::args().any(|arg| arg == "--observe") {
        let observation = process::scan(&paths);
        println!("  observed processes: {}", observation.games.len());
        for game in &observation.games {
            println!(
                "    pid={} appid={:?} exe={:?} mapped={} disabled={} config={:?}",
                game.pid,
                game.appid,
                game.exe,
                game.layer_mapped,
                game.explicitly_disabled,
                game.config_path
            );
        }
    }
    Ok(())
}

fn configure_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = egui::Color32::from_rgb(18, 18, 21);
    visuals.window_fill = egui::Color32::from_rgb(27, 27, 32);
    visuals.extreme_bg_color = egui::Color32::from_rgb(12, 12, 15);
    visuals.faint_bg_color = egui::Color32::from_rgb(34, 34, 40);
    visuals.selection.bg_fill = egui::Color32::from_rgb(232, 153, 74);
    visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(232, 153, 74);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(49, 49, 57);
    visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(38, 38, 45);
    visuals.window_corner_radius = egui::CornerRadius::same(10);
    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        style.spacing.slider_width = 250.0;
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(20.0));
    });
}
