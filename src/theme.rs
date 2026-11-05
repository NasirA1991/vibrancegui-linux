use eframe::egui::{self, Color32, Stroke};

#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub accent: Color32,
    pub background: Color32,
    pub panel: Color32,
    pub panel_2: Color32,
    pub muted: Color32,
    pub text: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub inactive: Color32,
    pub extreme: Color32,
    pub window: Color32,
}

pub const DARK: Palette = Palette {
    dark: true,
    accent: Color32::from_rgb(232, 153, 74),
    background: Color32::from_rgb(18, 18, 21),
    panel: Color32::from_rgb(27, 27, 32),
    panel_2: Color32::from_rgb(34, 34, 40),
    muted: Color32::from_rgb(145, 145, 158),
    text: Color32::from_rgb(235, 235, 240),
    ok: Color32::from_rgb(97, 201, 141),
    warn: Color32::from_rgb(236, 188, 91),
    hover: Color32::from_rgb(49, 49, 57),
    selected: Color32::from_rgb(50, 42, 34),
    inactive: Color32::from_rgb(38, 38, 45),
    extreme: Color32::from_rgb(12, 12, 15),
    window: Color32::from_rgb(27, 27, 32),
};

pub const LIGHT: Palette = Palette {
    dark: false,
    accent: Color32::from_rgb(201, 106, 35),
    background: Color32::from_rgb(242, 242, 246),
    panel: Color32::from_rgb(255, 255, 255),
    panel_2: Color32::from_rgb(232, 232, 238),
    muted: Color32::from_rgb(92, 92, 104),
    text: Color32::from_rgb(28, 28, 34),
    ok: Color32::from_rgb(30, 130, 72),
    warn: Color32::from_rgb(155, 98, 0),
    hover: Color32::from_rgb(229, 229, 235),
    selected: Color32::from_rgb(255, 239, 224),
    inactive: Color32::from_rgb(241, 241, 245),
    extreme: Color32::from_rgb(250, 250, 252),
    window: Color32::from_rgb(255, 255, 255),
};

pub fn detect() -> Palette {
    match dark_light::detect() {
        Ok(dark_light::Mode::Light) => LIGHT,
        _ => DARK,
    }
}

pub fn apply(ctx: &egui::Context, palette: &Palette) {
    let mut visuals = if palette.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    visuals.panel_fill = palette.background;
    visuals.window_fill = palette.window;
    visuals.extreme_bg_color = palette.extreme;
    visuals.faint_bg_color = palette.panel_2;
    visuals.override_text_color = Some(palette.text);
    visuals.selection.bg_fill = palette.accent;
    visuals.selection.stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.bg_fill = palette.accent;
    visuals.widgets.hovered.bg_fill = palette.hover;
    visuals.widgets.inactive.bg_fill = palette.inactive;
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, palette.text);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, palette.muted);
    visuals.hyperlink_color = palette.accent;
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
