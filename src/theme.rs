use egui::{Color32, CornerRadius, Stroke};

pub const DEFAULT_THEME: &str = "Pastel";

pub const APP_THEMES: &[&str] = &["Pastel", "Ocean", "Sage", "Rose", "Light"];
pub const CATEGORY_COLORS: &[&str] = &["Default", "Lavender", "Ocean", "Sage", "Rose", "Amber"];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemePalette {
    pub background: Color32,
    pub surface: Color32,
    pub input: Color32,
    pub button: Color32,
    pub overlay: Color32,
    pub foreground: Color32,
    pub muted: Color32,
    pub primary: Color32,
    pub on_primary: Color32,
    pub border: Color32,
    pub light: bool,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

const PASTEL: ThemePalette = ThemePalette {
    background: rgb(29, 29, 38),
    surface: rgb(42, 41, 54),
    input: rgb(53, 51, 67),
    button: rgb(63, 60, 77),
    overlay: rgb(39, 38, 51),
    foreground: rgb(241, 237, 246),
    muted: rgb(173, 166, 188),
    primary: rgb(186, 171, 218),
    on_primary: rgb(29, 25, 38),
    border: rgb(83, 77, 98),
    light: false,
};

const OCEAN: ThemePalette = ThemePalette {
    background: rgb(24, 31, 39),
    surface: rgb(34, 46, 57),
    input: rgb(44, 58, 70),
    button: rgb(53, 68, 80),
    overlay: rgb(31, 42, 52),
    foreground: rgb(232, 241, 245),
    muted: rgb(163, 183, 192),
    primary: rgb(151, 199, 215),
    on_primary: rgb(22, 36, 41),
    border: rgb(71, 94, 105),
    light: false,
};

const SAGE: ThemePalette = ThemePalette {
    background: rgb(27, 33, 30),
    surface: rgb(39, 49, 43),
    input: rgb(49, 61, 54),
    button: rgb(58, 72, 63),
    overlay: rgb(36, 46, 40),
    foreground: rgb(235, 242, 235),
    muted: rgb(170, 187, 172),
    primary: rgb(170, 203, 174),
    on_primary: rgb(25, 37, 27),
    border: rgb(77, 100, 82),
    light: false,
};

const ROSE: ThemePalette = ThemePalette {
    background: rgb(36, 28, 33),
    surface: rgb(51, 39, 46),
    input: rgb(64, 49, 57),
    button: rgb(76, 58, 67),
    overlay: rgb(48, 36, 43),
    foreground: rgb(246, 235, 240),
    muted: rgb(192, 170, 180),
    primary: rgb(220, 170, 190),
    on_primary: rgb(40, 24, 31),
    border: rgb(102, 77, 89),
    light: false,
};

const LIGHT: ThemePalette = ThemePalette {
    background: rgb(247, 245, 249),
    surface: rgb(235, 231, 240),
    input: rgb(255, 253, 255),
    button: rgb(225, 219, 234),
    overlay: rgb(250, 248, 252),
    foreground: rgb(44, 38, 52),
    muted: rgb(103, 95, 112),
    primary: rgb(111, 84, 146),
    on_primary: rgb(255, 255, 255),
    border: rgb(181, 171, 191),
    light: true,
};

pub fn valid_app_theme(name: &str) -> bool {
    matches!(name, "Pastel" | "Ocean" | "Sage" | "Rose" | "Light")
}

pub fn palette(name: &str) -> ThemePalette {
    match name {
        "Ocean" => OCEAN,
        "Sage" => SAGE,
        "Rose" => ROSE,
        "Light" => LIGHT,
        _ => PASTEL,
    }
}

pub fn valid_category_color(name: &str) -> bool {
    matches!(
        name,
        "" | "Lavender" | "Ocean" | "Sage" | "Rose" | "Amber"
    )
}

pub fn category_color(name: &str, app_theme: &str) -> Color32 {
    let p = palette(app_theme);
    if name.is_empty() {
        return p.surface;
    }
    if p.light {
        match name {
            "Lavender" => rgb(231, 222, 241),
            "Ocean" => rgb(216, 234, 239),
            "Sage" => rgb(222, 236, 221),
            "Rose" => rgb(242, 222, 231),
            "Amber" => rgb(242, 232, 213),
            _ => p.surface,
        }
    } else {
        match name {
            "Lavender" => rgb(61, 52, 74),
            "Ocean" => rgb(36, 61, 70),
            "Sage" => rgb(45, 64, 51),
            "Rose" => rgb(72, 48, 60),
            "Amber" => rgb(71, 60, 42),
            _ => p.surface,
        }
    }
}

fn blend_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let mix = |x: u8, y: u8| -> u8 {
        ((x as f32) * (1.0 - t) + (y as f32) * t).round().clamp(0.0, 255.0) as u8
    };
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

pub fn apply_egui_theme(ctx: &egui::Context, theme_name: &str) {
    let p = palette(theme_name);
    let mut visuals = if p.light {
        egui::Visuals::light()
    } else {
        egui::Visuals::dark()
    };

    visuals.dark_mode = !p.light;
    visuals.override_text_color = None;
    visuals.panel_fill = p.background;
    visuals.window_fill = p.overlay;
    visuals.extreme_bg_color = p.input;
    visuals.faint_bg_color = p.surface;
    visuals.code_bg_color = p.surface;
    visuals.window_stroke = Stroke::new(1.0_f32, p.border);
    visuals.window_corner_radius = CornerRadius::same(8);
    visuals.menu_corner_radius = CornerRadius::same(8);
    visuals.hyperlink_color = p.primary;

    let selection_bg = Color32::from_rgba_unmultiplied(p.primary.r(), p.primary.g(), p.primary.b(), 70);
    visuals.selection = egui::style::Selection {
        bg_fill: selection_bg,
        stroke: Stroke::new(1.0_f32, p.primary),
    };

    let hovered_bg = blend_color(p.button, p.primary, 0.16);
    let active_bg = blend_color(p.button, p.primary, 0.28);

    visuals.widgets.noninteractive.bg_fill = p.surface;
    visuals.widgets.noninteractive.weak_bg_fill = p.surface;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.foreground);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);

    visuals.widgets.inactive.bg_fill = p.button;
    visuals.widgets.inactive.weak_bg_fill = p.button;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0_f32, p.border);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.foreground);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(6);

    visuals.widgets.hovered.bg_fill = hovered_bg;
    visuals.widgets.hovered.weak_bg_fill = hovered_bg;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, p.primary);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.5_f32, p.foreground);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    visuals.widgets.hovered.expansion = 0.0;

    visuals.widgets.active.bg_fill = active_bg;
    visuals.widgets.active.weak_bg_fill = active_bg;
    visuals.widgets.active.bg_stroke = Stroke::new(1.5_f32, p.primary);
    visuals.widgets.active.fg_stroke = Stroke::new(1.5_f32, p.foreground);
    visuals.widgets.active.corner_radius = CornerRadius::same(6);
    visuals.widgets.active.expansion = 0.0;

    visuals.widgets.open.bg_fill = hovered_bg;
    visuals.widgets.open.weak_bg_fill = hovered_bg;
    visuals.widgets.open.bg_stroke = Stroke::new(1.0_f32, p.primary);
    visuals.widgets.open.fg_stroke = Stroke::new(1.5_f32, p.foreground);
    visuals.widgets.open.corner_radius = CornerRadius::same(6);

    ctx.set_theme(if p.light {
        egui::Theme::Light
    } else {
        egui::Theme::Dark
    });
    ctx.set_visuals(visuals);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(a: Color32, b: Color32) -> f64 {
        let luminance = |c: Color32| -> f64 {
            let component = |v: u8| -> f64 {
                let s = (v as f64) / 255.0;
                if s <= 0.04045 {
                    s / 12.92
                } else {
                    ((s + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * component(c.r()) + 0.7152 * component(c.g()) + 0.0722 * component(c.b())
        };
        let x = luminance(a);
        let y = luminance(b);
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn test_theme_preset_contrast() {
        for &name in APP_THEMES {
            let p = palette(name);
            for bg in [p.background, p.surface, p.input, p.button, p.overlay] {
                let ratio = contrast(p.foreground, bg);
                assert!(ratio >= 4.5, "{name} text contrast {ratio:.2} is too low");
            }
            let primary_ratio = contrast(p.on_primary, p.primary);
            assert!(
                primary_ratio >= 4.5,
                "{name} primary button contrast {primary_ratio:.2} is too low"
            );
            for &category in &CATEGORY_COLORS[1..] {
                let ratio = contrast(p.foreground, category_color(category, name));
                assert!(
                    ratio >= 4.5,
                    "{name} text on {category} category contrast {ratio:.2} is too low"
                );
            }
        }
    }
}
