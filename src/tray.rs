use std::sync::mpsc::Sender;

use eframe::egui;
use ksni::blocking::Handle;
use ksni::menu::{MenuItem, StandardItem};
use ksni::{Icon, Tray};

#[derive(Debug, Clone, Copy)]
pub enum TrayAction {
    Show,
    Refresh,
    Quit,
}

pub struct VibranceTray {
    tx: Sender<TrayAction>,
    ctx: egui::Context,
}

impl VibranceTray {
    fn send(&self, action: TrayAction) {
        let _ = self.tx.send(action);
        // Show directly from the tray thread. While the window is hidden,
        // eframe may not run a normal UI pass, so waiting for `logic` to
        // process the channel can otherwise leave the window invisible.
        if matches!(action, TrayAction::Show) {
            self.ctx
                .send_viewport_cmd(egui::ViewportCommand::Visible(true));
            self.ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        }
        self.ctx.request_repaint();
    }
}

impl Tray for VibranceTray {
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        "vibrancegui".to_string()
    }

    fn title(&self) -> String {
        "VibranceGUI for Linux".to_string()
    }

    fn icon_name(&self) -> String {
        "vibrancegui".to_string()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        vec![make_icon()]
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.send(TrayAction::Show);
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        vec![
            StandardItem {
                label: "Show VibranceGUI".to_string(),
                icon_name: "window-new".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayAction::Show)),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Refresh Steam library".to_string(),
                icon_name: "view-refresh".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayAction::Refresh)),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".to_string(),
                icon_name: "application-exit".to_string(),
                activate: Box::new(|tray: &mut Self| tray.send(TrayAction::Quit)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

pub fn spawn(ctx: egui::Context, tx: Sender<TrayAction>) -> Option<Handle<VibranceTray>> {
    use ksni::blocking::TrayMethods;
    VibranceTray { tx, ctx }.spawn().ok()
}

fn make_icon() -> Icon {
    // 32x32 ARGB32. A small orange ring with a white V, matching the app icon.
    const SIZE: usize = 32;
    let mut data = vec![0u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - 16.0;
            let dy = y as f32 - 16.0;
            let r = (dx * dx + dy * dy).sqrt();
            let mut rgba = [0u8, 0, 0, 0];
            if r <= 14.5 {
                rgba = [255, 28, 28, 34];
            }
            if (11.5..=14.5).contains(&r) {
                rgba = [255, 232, 145, 65];
            }
            // V strokes from (9,11) to (16,23) to (23,11).
            let y_in_v = (10..=23).contains(&y);
            let on_v = (y_in_v && (((x as i32 - 9) - (y as i32 - 10)) / 2).abs() <= 1)
                || (y_in_v && (((23 - x as i32) - (y as i32 - 10)) / 2).abs() <= 1);
            if on_v {
                rgba = [255, 245, 238, 229];
            }
            let i = (y * SIZE + x) * 4;
            data[i] = rgba[0];
            data[i + 1] = rgba[1];
            data[i + 2] = rgba[2];
            data[i + 3] = rgba[3];
        }
    }
    Icon {
        width: SIZE as i32,
        height: SIZE as i32,
        data,
    }
}
