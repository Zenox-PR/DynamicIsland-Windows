//! Shared media geometry: raster and hit-testing use the same DIP rectangles.
use crate::{BACKGROUND, TextRect, distance, mix};
use island_core::media::{Command, Playback, Session};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Previous,
    PlayPause,
    Next,
    Sessions,
}
#[derive(Clone, Copy)]
pub struct Feedback {
    pub focus: Option<Action>,
    pub hover: Option<Action>,
    pub pressed: Option<Action>,
    pub hover_mix: f64,
    pub cover_mix: f64,
}
impl Default for Feedback {
    fn default() -> Self {
        Self {
            focus: None,
            hover: None,
            pressed: None,
            hover_mix: 1.0,
            cover_mix: 1.0,
        }
    }
}
pub const ACTIONS: [Action; 4] = [
    Action::Previous,
    Action::PlayPause,
    Action::Next,
    Action::Sessions,
];
pub fn rect(action: Action) -> [i32; 4] {
    match action {
        Action::Previous => [132, 124, 168, 160],
        Action::PlayPause => [174, 124, 210, 160],
        Action::Next => [216, 124, 252, 160],
        Action::Sessions => [254, 9, 360, 33],
    }
}
pub fn hit(x: f64, y: f64) -> Option<Action> {
    ACTIONS.into_iter().find(|&a| {
        let r = rect(a);
        x >= r[0] as f64 && x < r[2] as f64 && y >= r[1] as f64 && y < r[3] as f64
    })
}
pub fn command(action: Action, session: &Session) -> Option<Command> {
    let c = match action {
        Action::Previous => Command::Previous,
        Action::Next => Command::Next,
        Action::PlayPause => {
            if session.playback == Playback::Playing {
                Command::Pause
            } else {
                Command::Play
            }
        }
        Action::Sessions => return None,
    };
    if session.capabilities.allows(c) {
        Some(c)
    } else if action == Action::PlayPause && session.capabilities.toggle {
        Some(Command::Toggle)
    } else {
        None
    }
}
pub fn text_rect(index: usize, scale: f64) -> TextRect {
    let r = match index {
        0 => [90, 43, 360, 67],
        1 => [90, 69, 360, 89],
        2 => [254, 11, 345, 29],
        3 => [24, 114, 360, 129],
        _ => [24, 160, 360, 168],
    };
    TextRect {
        left: (r[0] as f64 * scale).round() as i32,
        top: (r[1] as f64 * scale).round() as i32,
        right: (r[2] as f64 * scale).round() as i32,
        bottom: (r[3] as f64 * scale).round() as i32,
    }
}
pub fn paint(
    pixels: &mut [u32],
    size: (i32, i32),
    scale: f64,
    session: &Session,
    position: Option<f64>,
    feedback: Feedback,
    busy: bool,
) {
    let (w, h) = size;
    // Only called at settled expanded size. All additions stay inside its opaque body.
    let mut dot = |x: i32, y: i32, color: u32| {
        if x >= 0 && x < w && y >= 0 && y < h {
            pixels[(y * w + x) as usize] = 0xff000000 | color;
        }
    };
    for y in (44.0 * scale).round() as i32..(96.0 * scale).round() as i32 {
        for x in (24.0 * scale).round() as i32..(76.0 * scale).round() as i32 {
            let dx = x as f64 / scale - 24.0;
            let dy = y as f64 / scale - 44.0;
            if distance(dx, dy, 52.0, 52.0, 10.0) > 0.0 {
                continue;
            }
            let mut color = if (dx - 26.0).abs() < 2.0 && (dy - 26.0).abs() < 12.0 {
                0x63656d
            } else {
                0x22242a
            };
            if let Some(art) = session.artwork.as_ref().filter(|a| a.valid()) {
                let ratio = 52.0 / art.width.max(art.height) as f64;
                let (aw, ah) = (art.width as f64 * ratio, art.height as f64 * ratio);
                let (ax, ay) = (
                    (dx - (52.0 - aw) / 2.0) / ratio,
                    (dy - (52.0 - ah) / 2.0) / ratio,
                );
                if ax >= 0.0 && ay >= 0.0 && ax < art.width as f64 && ay < art.height as f64 {
                    let i = (ay as u32 * art.width + ax as u32) as usize * 4;
                    let p = &art.bgra[i..i + 4];
                    let inv = 255 - p[3] as u32;
                    let b = (p[0] as u32 + 0x2a * inv / 255).min(255);
                    let g = (p[1] as u32 + 0x24 * inv / 255).min(255);
                    let r = (p[2] as u32 + 0x22 * inv / 255).min(255);
                    color = mix(color, r << 16 | g << 8 | b, feedback.cover_mix);
                }
            }
            dot(x, y, color);
        }
    }
    if let Some(position) = position
        && let Some(t) = &session.timeline
    {
        let fraction = (position / t.duration).clamp(0.0, 1.0);
        for y in (108.0 * scale).round() as i32..(110.0 * scale).round() as i32 {
            for x in (24.0 * scale).round() as i32..(360.0 * scale).round() as i32 {
                dot(
                    x,
                    y,
                    if (x as f64) < (24.0 + 336.0 * fraction) * scale {
                        0xb9bcc5
                    } else {
                        0x34363d
                    },
                );
            }
        }
    }
    for action in ACTIONS.into_iter().filter(|a| *a != Action::Sessions) {
        let r = rect(action);
        let enabled = command(action, session).is_some() && !busy;
        let color = if enabled { 0xe6e8ed } else { 0x55575f };
        for y in (r[1] as f64 * scale).round() as i32..(r[3] as f64 * scale).round() as i32 {
            for x in (r[0] as f64 * scale).round() as i32..(r[2] as f64 * scale).round() as i32 {
                let dx = x as f64 / scale - (r[0] + r[2]) as f64 / 2.0;
                let dy = y as f64 / scale - (r[1] + r[3]) as f64 / 2.0;
                let d = distance(dx + 15.0, dy + 15.0, 30.0, 30.0, 15.0);
                if d <= 0.0 {
                    dot(
                        x,
                        y,
                        if feedback.focus == Some(action) && d >= -1.5 {
                            0x9a9da8
                        } else if feedback.pressed == Some(action) {
                            crate::theme::PRESSED
                        } else if feedback.hover == Some(action) {
                            mix(BACKGROUND, crate::theme::HOVER, feedback.hover_mix)
                        } else if action == Action::PlayPause {
                            0x202228
                        } else {
                            BACKGROUND
                        },
                    );
                }
                let arrow = |a: f64| (-5.0..=6.0).contains(&a) && dy.abs() <= (6.0 - a) * 0.75;
                let glyph = match action {
                    Action::PlayPause if session.playback == Playback::Playing => {
                        (dx.abs() >= 2.0 && dx.abs() <= 5.0) && dy.abs() <= 7.0
                    }
                    Action::PlayPause => arrow(dx),
                    Action::Previous => arrow(-dx) || ((dx + 8.0).abs() < 1.0 && dy.abs() < 7.0),
                    Action::Next => arrow(dx) || ((dx - 8.0).abs() < 1.0 && dy.abs() < 7.0),
                    _ => false,
                };
                if glyph {
                    dot(x, y, color);
                }
            }
        }
    }
    // Discrete selector affordance, with no animated equalizer.
    if feedback.focus == Some(Action::Sessions) || feedback.hover == Some(Action::Sessions) {
        // Keyboard focus remains visible when Tab reaches the session selector.
        for x in (254.0 * scale).round() as i32..(360.0 * scale).round() as i32 {
            dot(x, (32.0 * scale).round() as i32, 0x9a9da8);
        }
    }
    for step in 0..5 {
        for side in [-1, 1] {
            dot(
                ((354 + side * step) as f64 * scale).round() as i32,
                ((21 - step) as f64 * scale).round() as i32,
                mix(BACKGROUND, 0xa5a7af, 1.0),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_share_geometry_and_respect_capabilities() {
        let mut s = Session::default();
        assert_eq!(command(Action::PlayPause, &s), None);
        s.capabilities.play = true;
        assert_eq!(command(Action::PlayPause, &s), Some(Command::Play));
        s.playback = Playback::Playing;
        assert_eq!(command(Action::PlayPause, &s), None);
        s.capabilities.toggle = true;
        assert_eq!(command(Action::PlayPause, &s), Some(Command::Toggle));
        for action in ACTIONS {
            let r = rect(action);
            assert_eq!(
                hit((r[0] + r[2]) as f64 / 2.0, (r[1] + r[3]) as f64 / 2.0),
                Some(action)
            );
        }
        assert_eq!(hit(10.0, 80.0), None);
    }
}
