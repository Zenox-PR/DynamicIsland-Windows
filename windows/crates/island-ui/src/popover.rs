use crate::{TextRect, theme};
use island_core::selector::{Choice, Selector};
pub const WIDTH: f64 = 360.0;
pub const TOP: f64 = 46.0;
pub const ROW: f64 = 52.0;
pub fn height(rows: usize) -> f64 {
    TOP + ROW * rows as f64 + 24.0
}
pub fn hit(x: f64, y: f64, s: &Selector) -> Option<Choice> {
    if !(12.0..WIDTH - 12.0).contains(&x) || y < TOP {
        return None;
    }
    s.at(((y - TOP) / ROW) as usize)
}
pub fn text_rect(row: usize, secondary: bool, scale: f64) -> TextRect {
    let top = TOP + row as f64 * ROW + if secondary { 27.0 } else { 5.0 };
    TextRect {
        left: (30.0 * scale).round() as i32,
        top: (top * scale).round() as i32,
        right: ((WIDTH - 58.0) * scale).round() as i32,
        bottom: ((top + 18.0) * scale).round() as i32,
    }
}
pub fn paint(
    pixels: &mut [u32],
    size: (i32, i32),
    scale: f64,
    s: &Selector,
    checked: Option<Choice>,
) {
    pixels.fill(0);
    theme::rounded(
        pixels,
        size,
        [0.0, 0.0, size.0 as f64, size.1 as f64],
        20.0 * scale,
        theme::SURFACE,
    );
    for row in 0..s.page {
        let Some(c) = s.at(row) else { break };
        let y = TOP + ROW * row as f64;
        let color = if s.pressed == Some(c) {
            theme::PRESSED
        } else if s.hover == Some(c) {
            theme::HOVER
        } else {
            theme::SURFACE
        };
        let rect = [
            12.0 * scale,
            y * scale,
            (WIDTH - 12.0) * scale,
            (y + ROW - 3.0) * scale,
        ];
        theme::rounded(
            pixels,
            size,
            rect,
            11.0 * scale,
            if s.focus == c { crate::BORDER } else { color },
        );
        if s.focus == c {
            theme::rounded(
                pixels,
                size,
                [
                    rect[0] + scale,
                    rect[1] + scale,
                    rect[2] - scale,
                    rect[3] - scale,
                ],
                10.0 * scale,
                color,
            );
        }
        if checked == Some(c) {
            for i in 0..12 {
                let (x, y) = if i < 4 {
                    (322 + i, y + 24.0 + i as f64)
                } else {
                    (322 + i, y + 31.0 - (i - 3) as f64)
                };
                theme::rounded(
                    pixels,
                    size,
                    [
                        x as f64 * scale,
                        y * scale,
                        (x as f64 + 2.0) * scale,
                        (y + 2.0) * scale,
                    ],
                    scale,
                    theme::ACCENT,
                );
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounded_corners_and_hit_regions_agree_at_each_dpi() {
        let s = Selector::new(&Default::default(), 4);
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let size = (
                (WIDTH * scale).round() as i32,
                (height(2) * scale).round() as i32,
            );
            let mut pixels = vec![0; (size.0 * size.1) as usize];
            paint(&mut pixels, size, scale, &s, Some(Choice::Automatic));
            assert_eq!(pixels[0], 0);
            assert_eq!(pixels[pixels.len() - 1], 0);
            assert!(pixels.iter().all(|p| p & 255 <= p >> 24
                && (p >> 8) & 255 <= p >> 24
                && (p >> 16) & 255 <= p >> 24));
        }
        assert_eq!(hit(20.0, TOP + 10.0, &s), Some(Choice::Automatic));
        assert_eq!(hit(4.0, TOP + 10.0, &s), None);
    }
}
