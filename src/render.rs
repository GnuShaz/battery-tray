//! Рисует иконки трея в памяти: кольцо с числом и значок устройства с полоской.
use crate::model::{Kind, Level, MiniStyle};
use resvg::usvg;
use tiny_skia::{LineCap, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

pub const SIZE: u32 = 32; // трей на HiDPI берёт 32x32, на 100% масштабе сам уменьшит

/// Шрифт 3x5 пикселя, чтобы не тащить в проект файл шрифта.
const DIGITS: [[u8; 5]; 10] = [
    [0b111, 0b101, 0b101, 0b101, 0b111], [0b010, 0b110, 0b010, 0b010, 0b111],
    [0b111, 0b001, 0b111, 0b100, 0b111], [0b111, 0b001, 0b111, 0b001, 0b111],
    [0b101, 0b101, 0b111, 0b001, 0b001], [0b111, 0b100, 0b111, 0b001, 0b111],
    [0b111, 0b100, 0b111, 0b101, 0b111], [0b111, 0b001, 0b001, 0b001, 0b001],
    [0b111, 0b101, 0b111, 0b101, 0b111], [0b111, 0b101, 0b111, 0b001, 0b111],
];

fn paint(c: [u8; 3], a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color_rgba8(c[0], c[1], c[2], a);
    p.anti_alias = true;
    p
}

fn rect(pm: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, c: [u8; 3], a: u8) {
    if let Some(r) = Rect::from_xywh(x, y, w, h) {
        pm.fill_rect(r, &paint(c, a), Transform::identity(), None);
    }
}

fn digits(pm: &mut Pixmap, value: u8, small: bool) {
    let text = value.min(100).to_string();
    let k = if small || text.len() >= 3 { 2 } else { 3 };
    let n = text.len() as i32;
    let width = n * 3 * k + (n - 1) * k;
    let mut x = (SIZE as i32 - width) / 2;
    let y = if small { 21 } else { (SIZE as i32 - 5 * k) / 2 };
    for ch in text.bytes() {
        for (ry, row) in DIGITS[(ch - b'0') as usize].iter().enumerate() {
            for cx in 0..3 {
                if (row >> (2 - cx)) & 1 == 1 {
                    let (px, py) = (x + cx * k, y + ry as i32 * k);
                    rect(pm, px as f32, py as f32, k as f32, k as f32, [255; 3], 255);
                }
            }
        }
        x += 4 * k;
    }
}

/// Кольцо: дуга по часовой от верхней точки, как в дизайне.
pub fn summary(percent: u8, level: Level) -> Pixmap {
    let mut pm = Pixmap::new(SIZE, SIZE).unwrap();
    let (c, r) = (SIZE as f32 / 2.0, 13.0_f32);
    let stroke = Stroke { width: 3.5, line_cap: LineCap::Round, ..Stroke::default() };
    let track = PathBuilder::from_circle(c, c, r).unwrap();
    pm.stroke_path(&track, &paint([255; 3], 56), &stroke, Transform::identity(), None);
    if percent > 0 {
        let sweep = percent.min(100) as f32 / 100.0 * std::f32::consts::TAU;
        let steps = ((sweep.to_degrees() / 3.0).ceil() as usize).max(2);
        let mut pb = PathBuilder::new();
        for i in 0..=steps {
            let a = -std::f32::consts::FRAC_PI_2 + sweep * i as f32 / steps as f32;
            let (x, y) = (c + r * a.cos(), c + r * a.sin());
            if i == 0 { pb.move_to(x, y) } else { pb.line_to(x, y) }
        }
        if let Some(arc) = pb.finish() {
            pm.stroke_path(&arc, &paint(level.rgb(), 255), &stroke, Transform::identity(), None);
        }
    }
    digits(&mut pm, percent, false);
    pm
}

fn glyph_svg(kind: Kind) -> &'static str {
    match kind {
        Kind::Mouse => include_str!("../assets/mouse.svg"),
        Kind::Keyboard => include_str!("../assets/keyboard.svg"),
        Kind::Headset => include_str!("../assets/headphones.svg"),
        Kind::Gamepad => include_str!("../assets/gamepad-2.svg"),
        Kind::Other => include_str!("../assets/bluetooth.svg"),
    }
}

/// Мини-иконка закреплённого устройства: значок и полоска заряда под ним.
pub fn device(kind: Kind, percent: u8, level: Level, style: MiniStyle) -> Pixmap {
    let mut pm = Pixmap::new(SIZE, SIZE).unwrap();
    let svg = glyph_svg(kind).replace("currentColor", &level.hex());
    let tree = usvg::Tree::from_str(&svg, &usvg::Options::default()).expect("встроенный SVG");
    let (px, x) = match style { MiniStyle::Bar => (22.0, 5.0), MiniStyle::Percent => (18.0, 7.0) };
    let s = px / tree.size().width();
    resvg::render(&tree, Transform::from_row(s, 0.0, 0.0, s, x, 1.0), &mut pm.as_mut());
    match style {
        MiniStyle::Bar => {
            rect(&mut pm, 5.0, 26.0, 22.0, 4.0, [255; 3], 56);
            rect(&mut pm, 5.0, 26.0, 22.0 * percent.min(100) as f32 / 100.0, 4.0, level.rgb(), 255);
        }
        MiniStyle::Percent => digits(&mut pm, percent, true),
    }
    pm
}

/// Прямой RGBA, какой ждёт `tray_icon::Icon::from_rgba` (tiny-skia хранит premultiplied).
pub fn to_rgba(pm: &Pixmap) -> Vec<u8> {
    pm.pixels().iter().flat_map(|p| {
        let c = p.demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Кладёт PNG в target/preview, чтобы глазами проверить иконки.
    #[test]
    fn render_preview() {
        std::fs::create_dir_all("target/preview").unwrap();
        for (i, (p, l)) in [(82, Level::Normal), (18, Level::Low), (47, Level::Charging), (100, Level::Normal)].iter().enumerate() {
            let pm = summary(*p, *l);
            assert!(pm.pixels().iter().any(|px| px.alpha() > 0));
            pm.save_png(format!("target/preview/ring{i}.png")).unwrap();
        }
        for (k, n) in [(Kind::Mouse, "mouse"), (Kind::Keyboard, "keyboard"), (Kind::Headset, "headset"), (Kind::Gamepad, "gamepad")] {
            device(k, 64, Level::Normal, MiniStyle::Bar).save_png(format!("target/preview/{n}.png")).unwrap();
            device(k, 64, Level::Normal, MiniStyle::Percent).save_png(format!("target/preview/{n}-pct.png")).unwrap();
        }
    }
}
