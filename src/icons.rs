use std::collections::HashMap;

use egui::{pos2, vec2, Color32, ColorImage, IconData, Pos2, Rect, Stroke, TextureHandle, TextureOptions, Ui};
use twemoji_assets::png::codes;

pub fn emoji_png_bytes(emoji: &str) -> Option<&'static [u8]> {
    let asset: &'static [u8] = match emoji {
        "🏠" => &codes::U_1F3E0,
        "💼" => &codes::U_1F4BC,
        "💰" => &codes::U_1F4B0,
        "📚" => &codes::U_1F4DA,
        "🛒" => &codes::U_1F6D2,
        "🎯" => &codes::U_1F3AF,
        "❤️" => &codes::U_2764,
        "💪" => &codes::U_1F4AA,
        "🍽️" => &codes::U_1F37D,
        "✈️" => &codes::U_2708,
        "🎨" => &codes::U_1F3A8,
        "🎵" => &codes::U_1F3B5,
        "🌱" => &codes::U_1F331,
        "🐾" => &codes::U_1F43E,
        "💻" => &codes::U_1F4BB,
        "🔧" => &codes::U_1F527,
        "📅" => &codes::U_1F4C5,
        "⭐" => &codes::U_2B50,
        "🎮" => &codes::U_1F3AE,
        "🚗" => &codes::U_1F697,
        "🏖️" => &codes::U_1F3D6,
        "🎁" => &codes::U_1F381,
        "☕" => &codes::U_2615,
        "📝" => &codes::U_1F4DD,
        "🏥" => &codes::U_1F3E5,
        "🎓" => &codes::U_1F393,
        "📈" => &codes::U_1F4C8,
        "🏦" => &codes::U_1F3E6,
        "🧹" => &codes::U_1F9F9,
        "🧺" => &codes::U_1F9FA,
        "🛏️" => &codes::U_1F6CF,
        "🚿" => &codes::U_1F6BF,
        "🧘" => &codes::U_1F9D8,
        "🏃" => &codes::U_1F3C3,
        "🥗" => &codes::U_1F957,
        "🍕" => &codes::U_1F355,
        "🌍" => &codes::U_1F30D,
        "🌤️" => &codes::U_1F324,
        "🌧️" => &codes::U_1F327,
        "🌸" => &codes::U_1F338,
        "🌳" => &codes::U_1F333,
        "🐶" => &codes::U_1F436,
        "🐱" => &codes::U_1F431,
        "👶" => &codes::U_1F476,
        "👪" => &codes::U_1F46A,
        "📱" => &codes::U_1F4F1,
        "📷" => &codes::U_1F4F7,
        "🔑" => &codes::U_1F511,
        _ => return None,
    };
    Some(asset)
}

#[derive(Clone, Default)]
struct EmojiTextureCache {
    textures: HashMap<String, TextureHandle>,
}

pub fn emoji_texture(ctx: &egui::Context, emoji: &str) -> Option<TextureHandle> {
    if emoji.is_empty() {
        return None;
    }
    let cache_id = egui::Id::new("scrivi_emoji_textures");
    let existing = ctx.data(|d| {
        d.get_temp::<EmojiTextureCache>(cache_id)
            .and_then(|c| c.textures.get(emoji).cloned())
    });
    if let Some(handle) = existing {
        return Some(handle);
    }

    let bytes = emoji_png_bytes(emoji)?;
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok()?;
    let rgba = decoded.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let color_image = ColorImage::from_rgba_unmultiplied(size, rgba.as_raw());
    let handle = ctx.load_texture(
        format!("emoji_{emoji}"),
        color_image,
        TextureOptions::LINEAR,
    );
    ctx.data_mut(|d| {
        let mut cache = d.get_temp::<EmojiTextureCache>(cache_id).unwrap_or_default();
        cache.textures.insert(emoji.to_string(), handle.clone());
        d.insert_temp(cache_id, cache);
    });
    Some(handle)
}

pub fn paint_emoji_or_plus(ui: &Ui, rect: Rect, emoji: &str, color: Color32, icon_fraction: f32) {
    if emoji.is_empty() {
        paint_plus(ui, rect, color);
        return;
    }
    if let Some(texture) = emoji_texture(ui.ctx(), emoji) {
        let side = rect.width().min(rect.height()) * icon_fraction;
        let target = Rect::from_center_size(rect.center(), vec2(side, side));
        ui.painter().image(
            texture.id(),
            target,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
}

pub fn load_app_icon() -> Option<IconData> {
    let bytes = include_bytes!("../assets/icon.ico");
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Ico).ok()?;
    let rgba = decoded.to_rgba8();
    Some(IconData {
        rgba: rgba.into_raw(),
        width: decoded.width(),
        height: decoded.height(),
    })
}

pub fn paint_drag_handle(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let dx = rect.width().min(rect.height()) * 0.16;
    let dy = rect.width().min(rect.height()) * 0.28;
    let r = (rect.width().min(rect.height()) * 0.085).max(1.4);
    for row in [-1.0_f32, 0.0, 1.0] {
        for col in [-1.0_f32, 1.0] {
            p.circle_filled(pos2(c.x + col * dx, c.y + row * dy), r, color);
        }
    }
}

pub fn paint_plus(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.28;
    let stroke = Stroke::new(2.0_f32, color);
    p.line_segment([pos2(c.x - r, c.y), pos2(c.x + r, c.y)], stroke);
    p.line_segment([pos2(c.x, c.y - r), pos2(c.x, c.y + r)], stroke);
}

/// A heading bar with two indented bullet rows, used for "add sub-category".
pub fn paint_subcategory(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let stroke = Stroke::new((s * 0.11).max(1.6), color);
    let left = c.x - s * 0.34;
    let right = c.x + s * 0.34;
    let top = c.y - s * 0.26;
    p.line_segment([pos2(left, top), pos2(right, top)], stroke);
    for y in [c.y + s * 0.02, c.y + s * 0.28] {
        p.circle_filled(pos2(left + s * 0.18, y), (s * 0.07).max(1.2), color);
        p.line_segment([pos2(left + s * 0.34, y), pos2(right, y)], stroke);
    }
}

pub fn paint_check(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let stroke = Stroke::new((s * 0.13).max(1.8), color);
    let a = pos2(c.x - s * 0.26, c.y + s * 0.02);
    let b = pos2(c.x - s * 0.06, c.y + s * 0.22);
    let d = pos2(c.x + s * 0.28, c.y - s * 0.20);
    p.line_segment([a, b], stroke);
    p.line_segment([b, d], stroke);
}

pub fn paint_pin(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height()) * 0.65;
    let c = rect.center();
    let stroke = Stroke::new(1.6_f32, color);
    // Top cap
    p.line_segment(
        [pos2(c.x - s * 0.28, c.y - s * 0.36), pos2(c.x + s * 0.28, c.y - s * 0.36)],
        stroke,
    );
    // Body
    p.rect_filled(
        Rect::from_min_max(
            pos2(c.x - s * 0.18, c.y - s * 0.36),
            pos2(c.x + s * 0.18, c.y + s * 0.04),
        ),
        1.5,
        color,
    );
    // Crossbar
    p.line_segment(
        [pos2(c.x - s * 0.34, c.y + s * 0.06), pos2(c.x + s * 0.34, c.y + s * 0.06)],
        Stroke::new(2.0_f32, color),
    );
    // Needle
    p.line_segment(
        [pos2(c.x, c.y + s * 0.06), pos2(c.x, c.y + s * 0.42)],
        stroke,
    );
}

pub fn paint_settings(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let outer_r = s * 0.26;
    let inner_r = s * 0.11;
    let tooth_r = s * 0.35;
    p.circle_stroke(c, outer_r, Stroke::new(2.2_f32, color));
    p.circle_stroke(c, inner_r, Stroke::new(1.2_f32, color));
    for i in 0..8 {
        let angle = (i as f32) * std::f32::consts::FRAC_PI_4;
        let dir = vec2(angle.cos(), angle.sin());
        p.line_segment(
            [c + dir * (outer_r * 0.9), c + dir * tooth_r],
            Stroke::new(2.1_f32, color),
        );
    }
}

pub fn paint_palette(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    p.circle_stroke(c, s * 0.34, Stroke::new(1.7_f32, color));
    for (dx, dy) in [(-0.14, -0.10), (0.04, -0.17), (0.17, -0.02)] {
        p.circle_filled(pos2(c.x + s * dx, c.y + s * dy), (s * 0.06).max(1.4), color);
    }
    p.circle_stroke(
        pos2(c.x + s * 0.10, c.y + s * 0.14),
        s * 0.065,
        Stroke::new(1.3_f32, color),
    );
}

pub fn paint_edit(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let stroke = Stroke::new(1.8_f32, color);
    let tip = pos2(c.x - s * 0.28, c.y + s * 0.28);
    let mid = pos2(c.x - s * 0.16, c.y + s * 0.26);
    let top = pos2(c.x + s * 0.26, c.y - s * 0.16);
    let top2 = pos2(c.x + s * 0.16, c.y - s * 0.26);
    let shoulder = pos2(c.x - s * 0.26, c.y + s * 0.16);
    p.line_segment([tip, mid], stroke);
    p.line_segment([mid, top], stroke);
    p.line_segment([top, top2], stroke);
    p.line_segment([top2, shoulder], stroke);
    p.line_segment([shoulder, tip], stroke);
}

pub fn paint_trash(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    let stroke = Stroke::new(1.6_f32, color);
    // Lid + handle
    p.line_segment(
        [pos2(c.x - s * 0.28, c.y - s * 0.18), pos2(c.x + s * 0.28, c.y - s * 0.18)],
        stroke,
    );
    p.line_segment(
        [pos2(c.x - s * 0.10, c.y - s * 0.28), pos2(c.x + s * 0.10, c.y - s * 0.28)],
        stroke,
    );
    // Bin outline
    let body = Rect::from_min_max(
        pos2(c.x - s * 0.21, c.y - s * 0.14),
        pos2(c.x + s * 0.21, c.y + s * 0.28),
    );
    p.rect_stroke(body, 1.5, stroke, egui::StrokeKind::Middle);
    // Vertical ribs
    for dx in [-0.07_f32, 0.07] {
        p.line_segment(
            [pos2(c.x + s * dx, c.y - s * 0.06), pos2(c.x + s * dx, c.y + s * 0.20)],
            Stroke::new(1.3_f32, color),
        );
    }
}

pub fn paint_error_badge(ui: &Ui, rect: Rect, color: Color32) {
    let p = ui.painter();
    let s = rect.width().min(rect.height());
    let c = rect.center();
    p.circle_stroke(c, s * 0.40, Stroke::new(2.0_f32, color));
    p.line_segment(
        [pos2(c.x, c.y - s * 0.18), pos2(c.x, c.y + s * 0.06)],
        Stroke::new(2.2_f32, color),
    );
    p.circle_filled(pos2(c.x, c.y + s * 0.20), (s * 0.06).max(1.5), color);
}

/// Keeps helper imports tidy even when unused on specific targets.
#[allow(dead_code)]
fn _unused_pos_check(_: Pos2) {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::CATEGORY_EMOJIS;

    #[test]
    fn test_all_category_emojis_decode_as_png() {
        assert_eq!(CATEGORY_EMOJIS.len(), 48);
        for &emoji in CATEGORY_EMOJIS {
            let bytes = emoji_png_bytes(emoji).unwrap_or_else(|| panic!("missing PNG for {emoji}"));
            let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .unwrap_or_else(|e| panic!("invalid PNG for {emoji}: {e}"));
            assert!(img.width() > 0 && img.height() > 0);
        }
    }

    #[test]
    fn test_load_app_icon() {
        let icon = load_app_icon().expect("assets/icon.ico should decode");
        assert_eq!(icon.width, 64);
        assert_eq!(icon.height, 64);
        assert_eq!(icon.rgba.len(), 64 * 64 * 4);
    }
}
