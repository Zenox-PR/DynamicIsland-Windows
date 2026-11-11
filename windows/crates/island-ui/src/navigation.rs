use crate::{TextRect, theme};
use island_core::modules::{Modules, Page};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Home,
    Media,
    ToggleMedia,
    Timer,
    ToggleTimer,
    Countdown,
    Pomodoro,
    TimerPrimary,
    TimerReset,
    TimerSettings,
    Launcher,
    OpenLauncher,
    ToggleLauncher,
    AddShortcut,
    FavoriteFilter,
    PreviousShortcuts,
    NextShortcuts,
    Launch(u8),
    Pin(u8),
    EditShortcut(u8),
}
#[derive(Default)]
pub struct Feedback {
    pub hover: Option<Action>,
    pub pressed: Option<Action>,
    pub focus: Option<Action>,
}
pub fn rect(a: Action) -> [f64; 4] {
    match a {
        Action::Home => [16.0, 8.0, 86.0, 34.0],
        Action::Media => [90.0, 8.0, 160.0, 34.0],
        Action::ToggleMedia => [234.0, 76.0, 360.0, 103.0],
        Action::Timer => [164.0, 8.0, 244.0, 34.0],
        Action::ToggleTimer => [234.0, 106.0, 360.0, 133.0],
        Action::Countdown => [24.0, 42.0, 116.0, 68.0],
        Action::Pomodoro => [122.0, 42.0, 230.0, 68.0],
        Action::TimerPrimary => [24.0, 123.0, 148.0, 151.0],
        Action::TimerReset => [156.0, 123.0, 260.0, 151.0],
        Action::TimerSettings => [268.0, 123.0, 360.0, 151.0],
        Action::Launcher => [164.0, 8.0, 244.0, 34.0],
        Action::OpenLauncher => [24.0, 44.0, 222.0, 71.0],
        Action::ToggleLauncher => [234.0, 44.0, 360.0, 71.0],
        Action::AddShortcut => [24.0, 55.0, 110.0, 78.0],
        Action::FavoriteFilter => [118.0, 55.0, 232.0, 78.0],
        Action::PreviousShortcuts => [270.0, 55.0, 310.0, 78.0],
        Action::NextShortcuts => [318.0, 55.0, 360.0, 78.0],
        Action::Launch(row) => [
            24.0,
            84.0 + f64::from(row.min(2)) * 24.0,
            264.0,
            105.0 + f64::from(row.min(2)) * 24.0,
        ],
        Action::Pin(row) => [
            272.0,
            84.0 + f64::from(row.min(2)) * 24.0,
            310.0,
            105.0 + f64::from(row.min(2)) * 24.0,
        ],
        Action::EditShortcut(row) => [
            318.0,
            84.0 + f64::from(row.min(2)) * 24.0,
            360.0,
            105.0 + f64::from(row.min(2)) * 24.0,
        ],
    }
}
pub fn actions(modules: Modules) -> Vec<Action> {
    let mut a = vec![Action::Home];
    if modules.media_enabled {
        a.push(Action::Media);
    }
    if modules.launcher_visible() {
        a.push(Action::Launcher);
    } else if modules.timer_enabled {
        a.push(Action::Timer);
    }
    if modules.page == Page::Home {
        a.push(Action::ToggleMedia);
        a.push(Action::ToggleTimer);
        a.push(Action::OpenLauncher);
        a.push(Action::ToggleLauncher);
    }
    if modules.launcher_visible() {
        a.extend([
            Action::AddShortcut,
            Action::FavoriteFilter,
            Action::PreviousShortcuts,
            Action::NextShortcuts,
        ]);
        for row in 0..3 {
            a.extend([
                Action::Launch(row),
                Action::Pin(row),
                Action::EditShortcut(row),
            ]);
        }
    }
    if modules.timer_visible() {
        a.extend([
            Action::Countdown,
            Action::Pomodoro,
            Action::TimerPrimary,
            Action::TimerReset,
            Action::TimerSettings,
        ]);
    }
    a
}
pub fn hit(x: f64, y: f64, modules: Modules) -> Option<Action> {
    // Consume a disabled tab's click, rather than collapsing as background input.
    let mut available = actions(modules);
    if !modules.media_enabled {
        available.push(Action::Media);
    }
    if !modules.timer_enabled && !modules.launcher_visible() {
        available.push(Action::Timer);
    }
    available.into_iter().find(|a| {
        let r = rect(*a);
        x >= r[0] && x < r[2] && y >= r[1] && y < r[3]
    })
}
pub fn text_rect(r: [f64; 4], scale: f64) -> TextRect {
    TextRect {
        left: (r[0] * scale).round() as i32,
        top: (r[1] * scale).round() as i32,
        right: (r[2] * scale).round() as i32,
        bottom: (r[3] * scale).round() as i32,
    }
}
pub fn paint(pixels: &mut [u32], size: (i32, i32), scale: f64, modules: Modules, f: &Feedback) {
    for a in actions(modules) {
        let selected = matches!(
            (modules.page, a),
            (Page::Home, Action::Home)
                | (Page::Media, Action::Media)
                | (Page::Timer, Action::Timer)
                | (Page::Launcher, Action::Launcher)
        );
        let r = rect(a).map(|n| n * scale);
        let color = if f.pressed == Some(a) {
            theme::PRESSED
        } else if f.hover == Some(a) || selected {
            theme::HOVER
        } else {
            theme::SURFACE
        };
        theme::rounded(
            pixels,
            size,
            r,
            9.0 * scale,
            if f.focus == Some(a) {
                theme::ACCENT
            } else {
                color
            },
        );
        if f.focus == Some(a) {
            theme::rounded(
                pixels,
                size,
                [r[0] + scale, r[1] + scale, r[2] - scale, r[3] - scale],
                8.0 * scale,
                color,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_never_hits_disabled_or_overlapping_media_controls() {
        let mut m = Modules::default();
        assert_eq!(hit(115.0, 20.0, m), Some(Action::Media));
        assert_eq!(hit(220.0, 20.0, m), Some(Action::Timer));
        assert!(hit(270.0, 20.0, m).is_none());
        assert!(hit(250.0, 90.0, m).is_none());
        m.set_enabled(island_core::modules::ModuleId::Media, false);
        assert_eq!(hit(115.0, 20.0, m), Some(Action::Media));
        assert!(!m.navigate(Page::Media));
        assert_eq!(hit(250.0, 90.0, m), Some(Action::ToggleMedia));
    }
    #[test]
    fn timer_actions_do_not_overlap_navigation_or_media_source() {
        let m = Modules {
            page: Page::Timer,
            ..Default::default()
        };
        for a in actions(m) {
            let r = rect(a);
            let center = [(r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0];
            assert_eq!(hit(center[0], center[1], m), Some(a));
            for b in actions(m).into_iter().filter(|b| *b != a) {
                let s = rect(b);
                assert!(r[2] <= s[0] || s[2] <= r[0] || r[3] <= s[1] || s[3] <= r[1]);
            }
        }
        assert!(crate::media::hit(200.0, 20.0).is_none());
        assert_eq!(
            crate::media::hit(280.0, 20.0),
            Some(crate::media::Action::Sessions)
        );
    }
    #[test]
    fn launcher_slots_and_context_actions_are_disjoint() {
        let modules = Modules {
            page: Page::Launcher,
            ..Default::default()
        };
        let all = actions(modules);
        assert!(!all.contains(&Action::Timer));
        for a in &all {
            let r = rect(*a);
            assert_eq!(
                hit((r[0] + r[2]) / 2.0, (r[1] + r[3]) / 2.0, modules),
                Some(*a)
            );
            for b in all.iter().filter(|b| *b != a) {
                let s = rect(*b);
                assert!(r[2] <= s[0] || s[2] <= r[0] || r[3] <= s[1] || s[3] <= r[1]);
            }
        }
    }
}
