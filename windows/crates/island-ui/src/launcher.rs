//! Small original vector pictograms. Never load third-party executable icon handlers.
use crate::theme;
use island_core::launcher::Kind;
pub fn icon(pixels: &mut [u32], size: (i32, i32), scale: f64, x: f64, y: f64, kind: Kind) {
    let mut shape = |r: [f64; 4], radius: f64, color| {
        theme::rounded(
            pixels,
            size,
            [
                (x + r[0]) * scale,
                (y + r[1]) * scale,
                (x + r[2]) * scale,
                (y + r[3]) * scale,
            ],
            radius * scale,
            color,
        )
    };
    match kind {
        Kind::Folder => {
            shape([0.0, 1.0, 7.0, 5.0], 1.0, theme::ACCENT);
            shape([0.0, 4.0, 16.0, 14.0], 2.0, theme::ACCENT);
            shape([2.0, 6.0, 14.0, 12.0], 1.0, theme::SURFACE);
        }
        Kind::Https => {
            shape([0.0, 0.0, 16.0, 16.0], 8.0, theme::ACCENT);
            shape([1.5, 1.5, 14.5, 14.5], 6.5, theme::SURFACE);
            shape([7.0, 0.0, 9.0, 16.0], 1.0, theme::ACCENT);
            shape([0.0, 7.0, 16.0, 9.0], 1.0, theme::ACCENT);
        }
        Kind::Packaged => {
            for (x, y) in [(0.0, 0.0), (9.0, 0.0), (0.0, 9.0), (9.0, 9.0)] {
                shape([x, y, x + 7.0, y + 7.0], 1.0, theme::ACCENT);
            }
        }
        Kind::Application | Kind::Tool => {
            shape([0.0, 1.0, 16.0, 15.0], 2.0, theme::ACCENT);
            shape([1.5, 5.0, 14.5, 13.5], 1.0, theme::SURFACE);
            shape([4.0, 10.0, 11.0, 11.5], 0.5, theme::ACCENT);
        }
    }
}
