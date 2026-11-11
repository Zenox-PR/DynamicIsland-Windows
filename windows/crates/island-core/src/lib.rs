//! Pure state and geometry; no operating system dependencies.
pub mod launcher;
pub mod media;
pub mod modules;
pub mod placement;
pub mod selector;
pub mod system;
pub mod timer;
pub mod visibility;

pub const OPEN_DURATION: f64 = 0.22;
pub const CLOSE_DURATION: f64 = 0.18;

#[derive(Debug, Default)]
pub struct Transition {
    pub expanded: bool,
    from: f64,
    start: f64,
    duration: f64,
    value: f64,
}

impl Transition {
    pub fn sample(&mut self, now: f64) -> f64 {
        let t = if self.duration > 0.0 {
            ((now - self.start) / self.duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        let smooth = t * t * (3.0 - 2.0 * t);
        let target = if self.expanded { 1.0 } else { 0.0 };
        self.value = self.from + (target - self.from) * smooth;
        self.value
    }

    pub fn set(&mut self, expanded: bool, now: f64, animate: bool) {
        self.sample(now);
        if animate && self.expanded == expanded {
            return; // Repeated close requests must not restart an in-flight animation.
        }
        self.from = self.value;
        self.start = now;
        self.expanded = expanded;
        let target = if expanded { 1.0 } else { 0.0 };
        let full_duration = if expanded {
            OPEN_DURATION
        } else {
            CLOSE_DURATION
        };
        self.duration = full_duration * (target - self.from).abs();
        if !animate {
            self.duration = 0.0;
            self.from = target;
            self.value = self.from;
        }
    }

    pub fn active(&self) -> bool {
        (self.value - if self.expanded { 1.0 } else { 0.0 }).abs() > 0.00001
    }
}

/// Signed distance to a rounded rectangle. Negative inside.
pub fn distance(x: f64, y: f64, w: f64, h: f64, radius: f64) -> f64 {
    let r = radius.min(w / 2.0).min(h / 2.0);
    let qx = (x - w / 2.0).abs() - (w / 2.0 - r);
    let qy = (y - h / 2.0).abs() - (h / 2.0 - r);
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settles_without_frames_after_deadline() {
        let mut t = Transition::default();
        t.set(true, 0.0, true);
        assert_eq!(t.sample(1.0), 1.0);
        assert!(!t.active());
        t.set(false, 2.0, true);
        assert_eq!(t.sample(3.0), 0.0);
        assert!(!t.active());
    }
    #[test]
    fn reversal_is_continuous() {
        let mut t = Transition::default();
        t.set(true, 0.0, true);
        let before = t.sample(0.1);
        t.set(false, 0.1, true);
        assert_eq!(t.sample(0.1), before);
        assert_eq!(t.sample(0.5), 0.0);
    }
    #[test]
    fn reduced_motion_is_immediate() {
        let mut t = Transition::default();
        t.set(true, 0.0, false);
        assert_eq!(t.sample(0.0), 1.0);
        assert!(!t.active());
    }
    #[test]
    fn geometry_excludes_corners_and_scales() {
        assert!(distance(0.0, 0.0, 176.0, 34.0, 17.0) > 0.0);
        assert!(distance(88.0, 17.0, 176.0, 34.0, 17.0) < 0.0);
    }

    #[test]
    fn early_close_uses_remaining_distance_and_never_jumps() {
        for elapsed in [0.0, 0.001, 0.015, 0.060, 0.120, 0.239] {
            let mut t = Transition::default();
            t.set(true, 0.0, true);
            let before = t.sample(elapsed);
            t.set(false, elapsed, true);
            assert!((t.sample(elapsed) - before).abs() < 1e-12);
            let mut previous = before;
            for step in 1..=20 {
                let value = t.sample(elapsed + CLOSE_DURATION * before * step as f64 / 20.0);
                assert!(value <= previous + 1e-12 && value >= 0.0);
                previous = value;
            }
            assert!(t.sample(elapsed + CLOSE_DURATION * before + 1e-9) < 1e-12);
            assert!(!t.active());
        }
    }

    #[test]
    fn repeated_escape_does_not_extend_closing_deadline() {
        let mut t = Transition::default();
        t.set(true, 0.0, false);
        t.set(false, 1.0, true);
        for step in 1..18 {
            t.set(false, 1.0 + step as f64 * 0.01, true);
        }
        assert_eq!(t.sample(1.0 + CLOSE_DURATION + 1e-9), 0.0);
        assert!(!t.active());
    }

    #[test]
    fn rapid_alternating_requests_stay_bounded_and_settle() {
        let mut t = Transition::default();
        for step in 0..100 {
            let now = step as f64 * 0.017;
            let before = t.sample(now);
            t.set(step % 2 == 0, now, true);
            assert!((before - t.sample(now)).abs() < 1e-12);
            assert!((0.0..=1.0).contains(&before));
        }
        assert_eq!(t.sample(2.0), 0.0);
        assert!(!t.active());
    }

    #[test]
    fn full_motion_uses_the_two_deadlines_without_overshoot() {
        let mut t = Transition::default();
        t.set(true, 0.0, true);
        assert!(t.sample(OPEN_DURATION - 0.001) < 1.0);
        assert_eq!(t.sample(OPEN_DURATION), 1.0);
        t.set(false, 1.0, true);
        assert!(t.sample(1.0 + CLOSE_DURATION - 0.001) > 0.0);
        assert_eq!(t.sample(1.0 + CLOSE_DURATION + 1e-9), 0.0);
        assert!(!t.active());
    }
}
