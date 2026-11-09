//! Palette, fonts and widget style. Colours come from the DREAM report figures.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Shadow, Stroke,
    TextStyle, Theme, vec2,
};
use std::sync::Arc;

pub const BG: Color32 = Color32::from_rgb(0xFA, 0xFA, 0xF9);
pub const PANE: Color32 = Color32::WHITE;
pub const INK: Color32 = Color32::from_rgb(0x1C, 0x20, 0x24);
pub const MUTED: Color32 = Color32::from_rgb(0x66, 0x70, 0x7A);
pub const HAIRLINE: Color32 = Color32::from_rgb(0xDA, 0xDD, 0xE1);
pub const HOVER: Color32 = Color32::from_rgb(0xF2, 0xF4, 0xF6);
pub const PRESSED: Color32 = Color32::from_rgb(0xE6, 0xE9, 0xEC);
pub const ACCENT: Color32 = Color32::from_rgb(0x24, 0x71, 0xA3);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(0xE8, 0xF0, 0xF7);
pub const DANGER: Color32 = Color32::from_rgb(0xC0, 0x39, 0x2B);

pub const RADIUS: u8 = 6;

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn install(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("inter", &include_bytes!("../assets/fonts/Inter-Regular.ttf")[..]),
        ("inter-semibold", &include_bytes!("../assets/fonts/Inter-SemiBold.ttf")[..]),
        ("jetbrains-mono", &include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")[..]),
    ] {
        fonts.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    // egui's bundled fonts stay as fallbacks for symbols Inter lacks
    fonts.families.entry(FontFamily::Proportional).or_default().insert(0, "inter".into());
    fonts.families.entry(FontFamily::Monospace).or_default().insert(0, "jetbrains-mono".into());
    let fallback = fonts.families[&FontFamily::Proportional].clone();
    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), [vec!["inter-semibold".into()], fallback].concat());
    ctx.set_fonts(fonts);

    ctx.set_theme(Theme::Light);
    ctx.style_mut_of(Theme::Light, |style| {
        style.text_styles = [
            (TextStyle::Heading, semibold(20.0)),
            (TextStyle::Body, FontId::proportional(14.0)),
            (TextStyle::Button, FontId::proportional(14.0)),
            (TextStyle::Monospace, mono(12.5)),
            (TextStyle::Small, FontId::proportional(12.0)),
        ]
        .into();

        let s = &mut style.spacing;
        s.item_spacing = vec2(8.0, 8.0);
        s.button_padding = vec2(12.0, 6.0);
        s.interact_size.y = 30.0;

        let v = &mut style.visuals;
        v.dark_mode = false;
        v.panel_fill = BG;
        v.window_fill = PANE;
        v.window_stroke = Stroke::new(1.0, HAIRLINE);
        v.window_shadow = Shadow::NONE;
        v.popup_shadow = Shadow::NONE;
        v.window_corner_radius = CornerRadius::same(RADIUS);
        v.menu_corner_radius = CornerRadius::same(RADIUS);
        v.extreme_bg_color = PANE;
        v.text_edit_bg_color = Some(PANE);
        v.faint_bg_color = HOVER;
        v.code_bg_color = HOVER;
        v.hyperlink_color = ACCENT;
        v.error_fg_color = DANGER;
        v.selection.bg_fill = ACCENT_SOFT;
        v.selection.stroke = Stroke::new(1.0, ACCENT);
        v.override_text_color = None;

        let w = &mut v.widgets;
        w.noninteractive.bg_stroke = Stroke::new(1.0, HAIRLINE);
        w.noninteractive.fg_stroke = Stroke::new(1.0, INK);
        for (state, fill, border) in [
            // a shade darker than HAIRLINE so empty checkboxes and fields stay visible on white
            (&mut w.inactive, PANE, Color32::from_rgb(0xC6, 0xCB, 0xD1)),
            (&mut w.hovered, HOVER, Color32::from_rgb(0xC4, 0xC9, 0xCF)),
            (&mut w.active, PRESSED, Color32::from_rgb(0xB8, 0xBE, 0xC5)),
            (&mut w.open, PRESSED, Color32::from_rgb(0xB8, 0xBE, 0xC5)),
        ] {
            state.bg_fill = fill;
            state.weak_bg_fill = fill;
            state.bg_stroke = Stroke::new(1.0, border);
            state.fg_stroke = Stroke::new(1.0, INK);
            state.corner_radius = CornerRadius::same(RADIUS);
            state.expansion = 0.0;
        }
    });
}
