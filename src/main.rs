mod app;
mod artwork;
mod config_doc;
mod paths;
mod process;
mod shortcuts;
mod state;
mod steam;
mod theme;
mod tray;
mod vdf;
mod vksumi;

use eframe::egui;

fn main() -> eframe::Result {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--doctor") {
        return doctor();
    }
    if args.iter().any(|arg| arg == "--tray-only") {
        return tray_only();
    }
    let use_tray = !args.iter().any(|arg| arg == "--no-tray");
    let start_hidden = args.iter().any(|arg| arg == "--hidden");

    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../packaging/vibrancegui.png"))
        .expect("valid app icon");
    let palette = if args.iter().any(|arg| arg == "--light") {
        theme::LIGHT
    } else if args.iter().any(|arg| arg == "--dark") {
        theme::DARK
    } else {
        theme::detect()
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([460.0, 660.0])
            .with_min_inner_size([420.0, 560.0])
            .with_title("VibranceGUI for Linux")
            .with_app_id("vibrancegui")
            .with_icon(icon),
        ..Default::default()
    };

    eframe::run_native(
        "vibrancegui",
        options,
        Box::new(|cc| {
            theme::apply(&cc.egui_ctx, &palette);
            Ok(Box::new(app::VibranceGuiApp::new(
                cc,
                use_tray,
                start_hidden,
                palette,
            )))
        }),
    )
}

fn tray_only() -> eframe::Result {
    let (tx, rx) = std::sync::mpsc::channel();
    let _tray = tray::spawn_headless(tx);
    if _tray.is_none() {
        eprintln!("Could not create tray icon");
        return Ok(());
    }
    if let Ok(tray::HeadlessTrayAction::Show) = rx.recv() {
        let exe = std::env::var_os("APPIMAGE")
            .map(std::path::PathBuf::from)
            .or_else(|| std::env::current_exe().ok());
        if let Some(exe) = exe {
            let _ = std::process::Command::new(exe).spawn();
        }
    }
    Ok(())
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
