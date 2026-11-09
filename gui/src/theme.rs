//! Palettes, fonts and widget style. Colours come from the DREAM report figures
//! (blue #2471a3, red #c0392b); the dark palette lightens them to keep contrast.

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Shadow, Shape, Stroke,
    TextStyle, Theme, ThemePreference, Visuals, pos2, vec2,
};
use std::sync::Arc;

pub const RADIUS: u8 = 6;

pub struct Palette {
    pub bg: Color32,
    pub pane: Color32,
    pub ink: Color32,
    pub muted: Color32,
    pub hairline: Color32,
    pub hover: Color32,
    pub pressed: Color32,
    /// border of controls at rest; darker than hairline so empty checkboxes stay visible
    pub border: Color32,
    pub border_hover: Color32,
    /// progress lines, running state, links
    pub accent: Color32,
    /// fill behind white text (primary buttons, finished step numbers)
    pub accent_fill: Color32,
    pub accent_soft: Color32,
    pub danger: Color32,
}

pub const LIGHT: Palette = Palette {
    bg: Color32::from_rgb(0xFA, 0xFA, 0xF9),
    pane: Color32::WHITE,
    ink: Color32::from_rgb(0x1C, 0x20, 0x24),
    muted: Color32::from_rgb(0x66, 0x70, 0x7A),
    hairline: Color32::from_rgb(0xDA, 0xDD, 0xE1),
    hover: Color32::from_rgb(0xF2, 0xF4, 0xF6),
    pressed: Color32::from_rgb(0xE6, 0xE9, 0xEC),
    border: Color32::from_rgb(0xC6, 0xCB, 0xD1),
    border_hover: Color32::from_rgb(0xB8, 0xBE, 0xC5),
    accent: Color32::from_rgb(0x24, 0x71, 0xA3),
    accent_fill: Color32::from_rgb(0x24, 0x71, 0xA3),
    accent_soft: Color32::from_rgb(0xE8, 0xF0, 0xF7),
    danger: Color32::from_rgb(0xC0, 0x39, 0x2B),
};

pub const DARK: Palette = Palette {
    bg: Color32::from_rgb(0x14, 0x17, 0x19),
    pane: Color32::from_rgb(0x1C, 0x20, 0x24),
    ink: Color32::from_rgb(0xE4, 0xE7, 0xEA),
    muted: Color32::from_rgb(0x98, 0xA1, 0xAA),
    hairline: Color32::from_rgb(0x2C, 0x32, 0x38),
    hover: Color32::from_rgb(0x25, 0x2A, 0x2F),
    pressed: Color32::from_rgb(0x2F, 0x35, 0x3B),
    border: Color32::from_rgb(0x3E, 0x45, 0x4C),
    border_hover: Color32::from_rgb(0x52, 0x5A, 0x62),
    accent: Color32::from_rgb(0x5A, 0xA6, 0xDB),
    accent_fill: Color32::from_rgb(0x27, 0x71, 0xA8),
    accent_soft: Color32::from_rgb(0x1D, 0x34, 0x48),
    danger: Color32::from_rgb(0xE2, 0x67, 0x5C),
};

pub fn pal(visuals: &Visuals) -> &'static Palette {
    if visuals.dark_mode { &DARK } else { &LIGHT }
}

pub fn semibold(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn mono(size: f32) -> FontId {
    FontId::monospace(size)
}

pub fn preference_name(p: ThemePreference) -> &'static str {
    match p {
        ThemePreference::System => "Auto",
        ThemePreference::Light => "Light",
        ThemePreference::Dark => "Dark",
    }
}

/// Points along a circle from angle `from` to `to` (degrees, 0 = right, 90 = down).
fn arc(c: Pos2, r: f32, from: f32, to: f32) -> Vec<Pos2> {
    (0..=18)
        .map(|i| from + (to - from) * i as f32 / 18.0)
        .map(|deg| {
            let a = deg.to_radians();
            pos2(c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

fn rays(painter: &Painter, c: Pos2, angles: impl Iterator<Item = f32>, color: Color32) {
    for deg in angles {
        let a = deg.to_radians();
        let d = vec2(a.cos(), a.sin());
        painter.line_segment([c + d * 5.6, c + d * 7.6], Stroke::new(1.5, color));
    }
}

/// Theme icons, drawn rather than taken from a font: sun for Light, full moon for Dark,
/// and for Auto a sun's left half joined to a moon's right half. `cutout` is the
/// background colour, used for the moon's craters.
pub fn icon(painter: &Painter, c: Pos2, preference: ThemePreference, color: Color32, cutout: Color32) {
    match preference {
        ThemePreference::Light => {
            painter.circle_filled(c, 3.6, color);
            rays(painter, c, (0..8).map(|k| k as f32 * 45.0), color);
        }
        ThemePreference::Dark => {
            painter.circle_filled(c, 6.5, color);
            painter.circle_filled(c + vec2(-2.0, -1.6), 1.3, cutout);
            painter.circle_filled(c + vec2(2.2, 1.9), 0.9, cutout);
        }
        ThemePreference::System => {
            painter.add(Shape::convex_polygon(arc(c, 6.5, -90.0, 90.0), color, Stroke::NONE));
            painter.circle_filled(c + vec2(2.6, 1.8), 0.9, cutout);
            painter.add(Shape::convex_polygon(arc(c, 3.6, 90.0, 270.0), color, Stroke::NONE));
            rays(painter, c, [112.5, 157.5, 202.5, 247.5].into_iter(), color);
        }
    }
}

pub fn parse(s: &str) -> ThemePreference {
    match s.trim() {
        "light" | "Light" => ThemePreference::Light,
        "dark" | "Dark" => ThemePreference::Dark,
        _ => ThemePreference::System,
    }
}

pub fn install(ctx: &egui::Context, preference: ThemePreference) {
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

    for (theme, p) in [(Theme::Light, &LIGHT), (Theme::Dark, &DARK)] {
        ctx.style_mut_of(theme, |style| {
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
            v.dark_mode = theme == Theme::Dark;
            v.panel_fill = p.bg;
            v.window_fill = p.pane;
            v.window_stroke = Stroke::new(1.0, p.hairline);
            v.window_shadow = Shadow::NONE;
            v.popup_shadow = Shadow::NONE;
            v.window_corner_radius = CornerRadius::same(RADIUS);
            v.menu_corner_radius = CornerRadius::same(RADIUS);
            v.extreme_bg_color = p.pane;
            v.text_edit_bg_color = Some(p.pane);
            v.faint_bg_color = p.hover;
            v.code_bg_color = p.hover;
            v.hyperlink_color = p.accent;
            v.error_fg_color = p.danger;
            v.weak_text_color = Some(p.muted);
            v.selection.bg_fill = p.accent_soft;
            v.selection.stroke = Stroke::new(1.0, p.accent);
            v.override_text_color = None;

            let w = &mut v.widgets;
            w.noninteractive.bg_fill = p.pane;
            w.noninteractive.bg_stroke = Stroke::new(1.0, p.hairline);
            w.noninteractive.fg_stroke = Stroke::new(1.0, p.ink);
            for (state, fill, border) in [
                (&mut w.inactive, p.pane, p.border),
                (&mut w.hovered, p.hover, p.border_hover),
                (&mut w.active, p.pressed, p.border_hover),
                (&mut w.open, p.pressed, p.border_hover),
            ] {
                state.bg_fill = fill;
                state.weak_bg_fill = fill;
                state.bg_stroke = Stroke::new(1.0, border);
                state.fg_stroke = Stroke::new(1.0, p.ink);
                state.corner_radius = CornerRadius::same(RADIUS);
                state.expansion = 0.0;
            }
        });
    }
    ctx.set_theme(preference);
}
