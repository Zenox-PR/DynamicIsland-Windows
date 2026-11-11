//! Monitor-independent, bounded placement. Fractions survive DPI/resolution changes.
const UNIT: i64 = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub x: u32,
    pub y: u32,
}
impl Default for Placement {
    fn default() -> Self {
        Self { x: 500_000, y: 0 }
    }
}
impl Placement {
    pub fn valid(self) -> bool {
        self.x <= UNIT as u32 && self.y <= UNIT as u32
    }
    pub fn point(self, area: [i32; 4], size: [i32; 2], margin: i32) -> [i32; 2] {
        let [left, top, right, bottom] = area;
        let min_x = left as i64 + size[0] as i64 / 2;
        let min_y = top as i64 + margin as i64;
        let span_x = (right as i64 - left as i64 - size[0] as i64).max(0);
        let span_y = (bottom as i64 - min_y - size[1] as i64).max(0);
        [
            (min_x + span_x * self.x.min(UNIT as u32) as i64 / UNIT) as i32,
            (min_y + span_y * self.y.min(UNIT as u32) as i64 / UNIT) as i32,
        ]
    }
    pub fn from_point(point: [i32; 2], area: [i32; 4], size: [i32; 2], margin: i32) -> Self {
        let origin = Self { x: 0, y: 0 }.point(area, size, margin);
        let end = Self {
            x: UNIT as u32,
            y: UNIT as u32,
        }
        .point(area, size, margin);
        let fraction = |i: usize| {
            let span = end[i] as i64 - origin[i] as i64;
            if span <= 0 {
                0
            } else {
                (((point[i] as i64 - origin[i] as i64) * UNIT / span).clamp(0, UNIT)) as u32
            }
        };
        Self {
            x: fraction(0),
            y: fraction(1),
        }
    }
}

pub fn fit(point: [i32; 2], size: [i32; 2], area: [i32; 4]) -> [i32; 2] {
    [
        (point[0] - size[0] / 2).clamp(area[0], (area[2] - size[0]).max(area[0])),
        point[1].clamp(area[1], (area[3] - size[1]).max(area[1])),
    ]
}

#[derive(Clone, Copy, Debug)]
pub struct Drag {
    pub start: [i32; 2],
    pub anchor: [i32; 2],
    pub moved: bool,
}
impl Drag {
    pub fn update(&mut self, point: [i32; 2], threshold: [i32; 2]) -> Option<[i32; 2]> {
        let delta = [point[0] - self.start[0], point[1] - self.start[1]];
        self.moved |= delta[0].abs() >= threshold[0] || delta[1].abs() >= threshold[1];
        self.moved
            .then_some([self.anchor[0] + delta[0], self.anchor[1] + delta[1]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn placement_roundtrip_negative_monitor_and_dpi() {
        for scale in [1, 2, 3] {
            let area = [-1920 * scale, -300 * scale, 0, 780 * scale];
            let size = [172 * scale, 32 * scale];
            for p in [
                Placement::default(),
                Placement { x: 0, y: 0 },
                Placement {
                    x: 1_000_000,
                    y: 1_000_000,
                },
            ] {
                let point = p.point(area, size, 8 * scale);
                let back = Placement::from_point(point, area, size, 8 * scale);
                let again = back.point(area, size, 8 * scale);
                assert!((point[0] - again[0]).abs() <= 1 && (point[1] - again[1]).abs() <= 1);
                let fitted = fit(point, [384 * scale, 168 * scale], area);
                assert!(fitted[0] >= area[0] && fitted[0] + 384 * scale <= area[2]);
                assert!(fitted[1] >= area[1] && fitted[1] + 168 * scale <= area[3]);
            }
        }
    }
    #[test]
    fn small_screen_and_invalid_position_are_bounded() {
        assert_eq!(fit([99, 99], [384, 168], [0, 0, 100, 50]), [0, 0]);
        assert!(!Placement { x: u32::MAX, y: 0 }.valid());
        assert_eq!(
            Placement::from_point([-999, 9999], [0, 0, 1920, 1080], [172, 32], 8),
            Placement { x: 0, y: 1_000_000 }
        );
    }
    #[test]
    fn explicit_drag_threshold_and_visibility_causes() {
        let mut drag = Drag {
            start: [100, 100],
            anchor: [400, 8],
            moved: false,
        };
        assert_eq!(drag.update([102, 103], [6, 6]), None);
        assert_eq!(drag.update([106, 103], [6, 6]), Some([406, 11]));
        assert_eq!(drag.update([100, 100], [6, 6]), Some([400, 8]));
        let mut v = crate::visibility::Visibility {
            manual: true,
            fullscreen: true,
            ..Default::default()
        };
        v.fullscreen = false;
        assert!(v.hidden());
        v.manual = false;
        assert!(!v.hidden());
        v.fullscreen = true;
        assert!(v.hidden());
    }
}
