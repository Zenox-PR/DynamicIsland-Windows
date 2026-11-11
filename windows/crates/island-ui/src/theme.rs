//! Shared satin tokens. Accent is a rendering parameter, ready for future settings.
pub const SURFACE: u32 = 0x111216;
pub const HOVER: u32 = 0x252830;
pub const PRESSED: u32 = 0x343945;
pub const ACCENT: u32 = 0xc6d3e8;
pub fn rounded(pixels: &mut [u32], size: (i32, i32), rect: [f64; 4], radius: f64, color: u32) {
    let (w, h) = size;
    for y in (rect[1].floor() as i32).max(0)..(rect[3].ceil() as i32).min(h) {
        for x in (rect[0].floor() as i32).max(0)..(rect[2].ceil() as i32).min(w) {
            let coverage = (0.5
                - crate::distance(
                    x as f64 + 0.5 - rect[0],
                    y as f64 + 0.5 - rect[1],
                    rect[2] - rect[0],
                    rect[3] - rect[1],
                    radius,
                ))
            .clamp(0.0, 1.0);
            if coverage > 0.0 {
                let old = pixels[(y * w + x) as usize];
                let alpha =
                    (coverage * 255.0 + (old >> 24) as f64 * (1.0 - coverage)).round() as u32;
                pixels[(y * w + x) as usize] =
                    alpha << 24 | crate::mix(old & 0xffffff, color, coverage);
            }
        }
    }
}
