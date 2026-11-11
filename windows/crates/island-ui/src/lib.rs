//! Noir satiné: pure geometry, layout and premultiplied BGRA rasterization.
use island_core::distance;
pub mod launcher;
pub mod media;
pub mod navigation;
pub mod popover;
pub mod theme;

pub const COMPACT: (i32, i32) = (172, 32);
pub const EXPANDED: (i32, i32) = (384, 168);
pub const CONCENTRATION: (i32, i32) = (96, 4);

/// Same center as the remembered compact; only the rendered bounds change.
pub fn concentration_dimensions(scale: f64) -> (i32, i32, i32) {
    (
        (CONCENTRATION.0 as f64 * scale).round() as i32,
        (CONCENTRATION.1 as f64 * scale).round() as i32,
        ((COMPACT.1 - CONCENTRATION.1) as f64 * scale / 2.0).round() as i32,
    )
}

pub fn paint_concentration(pixels: &mut [u32], size: (i32, i32)) {
    let (width, height) = size;
    assert_eq!(pixels.len(), width as usize * height as usize);
    for y in 0..height {
        for x in 0..width {
            let d = distance(
                x as f64 + 0.5,
                y as f64 + 0.5,
                width as f64,
                height as f64,
                height as f64 / 2.0,
            );
            let coverage = (0.5 - d).clamp(0.0, 1.0);
            // Restrained satin edge remains visible on both light and dark backgrounds.
            let color = mix(BACKGROUND, 0x63656d, (1.0 + d).clamp(0.0, 1.0));
            let alpha = coverage * 0.85;
            pixels[(y * width + x) as usize] =
                ((alpha * 255.0) as u32) << 24 | mix(0, color, alpha);
        }
    }
}
pub const TOP_MARGIN: f64 = 8.0;
pub const BACKGROUND: u32 = 0x0b0c0f;
pub const BORDER: u32 = 0x2a2c32;
pub const TEXT: [u32; 4] = [0xf3f4f6, 0xa5a7af, 0xa5a7af, 0xc6c9d1];
pub const FONT_SIZES: [f64; 4] = [18.0, 12.0, 11.0, 10.0];
pub const LABELS: [&str; 4] = [
    "Dynamic Island",
    "Un espace calme, à portée de clic.",
    "Cliquer pour replier",
    "Échap",
];

pub fn dimensions(progress: f64, scale: f64) -> (i32, i32, f64) {
    let p = progress.clamp(0.0, 1.0);
    (
        ((COMPACT.0 as f64 + (EXPANDED.0 - COMPACT.0) as f64 * p) * scale).round() as i32,
        ((COMPACT.1 as f64 + (EXPANDED.1 - COMPACT.1) as f64 * p) * scale).round() as i32,
        (16.0 + 10.0 * p) * scale,
    )
}

#[derive(Clone, Copy, Debug)]
pub struct TextRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Header, description, footer hint and key label: reserved visual regions, no modules.
pub fn text_layout(width: i32, height: i32, scale: f64) -> [TextRect; 4] {
    let px = |v: f64| (v * scale).round() as i32;
    let left = px(24.0);
    let right = width - left;
    let footer = height - px(34.0);
    [
        TextRect {
            left,
            top: px(42.0),
            right,
            bottom: px(69.0),
        },
        TextRect {
            left,
            top: px(74.0),
            right,
            bottom: px(94.0),
        },
        TextRect {
            left,
            top: footer,
            right: right - px(52.0),
            bottom: footer + px(18.0),
        },
        TextRect {
            left: right - px(40.0),
            top: footer,
            right,
            bottom: footer + px(18.0),
        },
    ]
}

pub fn content_opacity(progress: f64) -> f64 {
    let p = ((progress - 0.82) / 0.18).clamp(0.0, 1.0);
    p * p * (3.0 - 2.0 * p)
}

pub fn mix(background: u32, foreground: u32, amount: f64) -> u32 {
    let amount = amount.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let base = ((background >> shift) & 255) as f64;
        (base + (((foreground >> shift) & 255) as f64 - base) * amount).round() as u32
    };
    channel(16) << 16 | channel(8) << 8 | channel(0)
}

pub fn paint(
    pixels: &mut [u32],
    size: (i32, i32),
    radius: f64,
    scale: f64,
    focused: bool,
    progress: f64,
) {
    let (width, height) = size;
    assert_eq!(pixels.len(), width as usize * height as usize);
    let border = if focused { 0x787c86 } else { BORDER };
    for y in 0..height {
        for x in 0..width {
            let d = distance(
                x as f64 + 0.5,
                y as f64 + 0.5,
                width as f64,
                height as f64,
                radius,
            );
            let alpha = (0.5 - d).clamp(0.0, 1.0);
            let edge = (scale + d).clamp(0.0, 1.0);
            let color = mix(BACKGROUND, border, edge);
            pixels[(y * width + x) as usize] =
                ((alpha * 255.0) as u32) << 24 | mix(0, color, alpha);
        }
    }
    let cx = width as f64 / 2.0;
    let cy = 16.0 * scale;
    for y in
        ((cy - 2.0 * scale).floor() as i32).max(0)..((cy + 2.0 * scale).ceil() as i32).min(height)
    {
        for x in ((cx - 15.0 * scale).floor() as i32).max(0)
            ..((cx + 15.0 * scale).ceil() as i32).min(width)
        {
            let d = distance(
                x as f64 + 0.5 - cx + 14.0 * scale,
                y as f64 + 0.5 - cy + 1.5 * scale,
                28.0 * scale,
                3.0 * scale,
                1.5 * scale,
            );
            let amount = (0.5 - d).clamp(0.0, 1.0);
            let pixel = &mut pixels[(y * width + x) as usize];
            *pixel = 0xff000000 | mix(*pixel, 0x45474d, amount);
        }
    }
    let fade = content_opacity(progress);
    if fade > 0.0 {
        let layout = text_layout(width, height, scale);
        let line_y = height - (44.0 * scale).round() as i32;
        let line_color = 0xff000000 | mix(BACKGROUND, BORDER, fade);
        for x in layout[0].left..layout[0].right {
            pixels[(line_y * width + x) as usize] = line_color;
        }
        let key = layout[3];
        for y in key.top..key.bottom {
            for x in key.left..key.right {
                let d = distance(
                    (x - key.left) as f64 + 0.5,
                    (y - key.top) as f64 + 0.5,
                    (key.right - key.left) as f64,
                    (key.bottom - key.top) as f64,
                    5.0 * scale,
                );
                let amount = (0.5 - d).clamp(0.0, 1.0) * (scale + d).clamp(0.0, 1.0) * fade;
                pixels[(y * width + x) as usize] = 0xff000000 | mix(BACKGROUND, 0x3e4149, amount);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concentration_scales_and_premultiplies_without_changing_compact() {
        for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
            let (w, h, offset) = concentration_dimensions(scale);
            assert!((offset as f64 + h as f64 / 2.0 - 16.0 * scale).abs() <= 1.0);
            let mut pixels = vec![0; (w * h) as usize];
            paint_concentration(&mut pixels, (w, h));
            assert!(pixels[(w * h / 2) as usize + w as usize / 2] >> 24 > 0);
            for p in pixels {
                assert!(
                    (p & 255) <= p >> 24
                        && ((p >> 8) & 255) <= p >> 24
                        && ((p >> 16) & 255) <= p >> 24
                );
            }
        }
        assert_eq!(concentration_dimensions(1.25), (120, 5, 18));
    }
    #[test]
    fn all_scales_keep_alpha_corners_and_text_inside_the_shape() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for p in [0.0, 0.5, 0.821, 0.9, 1.0] {
                let (w, h, r) = dimensions(p, scale);
                let mut pixels = vec![0; (w * h) as usize];
                paint(&mut pixels, (w, h), r, scale, false, p);
                assert_eq!(pixels[0], 0);
                assert_eq!(pixels[(w * h - 1) as usize], 0);
                for &pixel in &pixels {
                    let a = pixel >> 24;
                    assert!(
                        (pixel & 255) <= a
                            && ((pixel >> 8) & 255) <= a
                            && ((pixel >> 16) & 255) <= a
                    );
                }
                if content_opacity(p) > 0.0 {
                    let layout = text_layout(w, h, scale);
                    assert!(layout[1].bottom < layout[2].top);
                    for rect in layout {
                        for y in [rect.top, rect.bottom - 1] {
                            for x in [rect.left, rect.right - 1] {
                                assert!(
                                    distance(x as f64 + 0.5, y as f64 + 0.5, w as f64, h as f64, r)
                                        < -1.0
                                );
                                assert_eq!(pixels[(y * w + x) as usize] >> 24, 255);
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(dimensions(0.0, 1.25), (215, 40, 20.0));
        assert_eq!(dimensions(1.0, 1.25), (480, 210, 32.5));
    }
}
