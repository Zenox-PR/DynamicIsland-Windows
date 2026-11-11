//! UI coordination: copies only, native menus, no COM calls or waits.
use super::*;
use island_core::media::{Playback, Snapshot};
use island_ui::media::Action;
const PROGRESS: usize = 3;
pub(super) const FEEDBACK_TIMER: usize = 4;
#[derive(Default)]
pub(super) struct View {
    pub snapshot: Snapshot,
    pub busy: bool,
    pub focus: Option<Action>,
    pub hover: Option<Action>,
    pub pressed: Option<Action>,
    hover_start: Option<Instant>,
    cover_start: Option<Instant>,
    timer: bool,
}
impl View {
    pub fn feedback(&self) -> island_ui::media::Feedback {
        let mix = |t: Option<Instant>, seconds: f64| {
            t.map_or(1.0, |t| {
                (t.elapsed().as_secs_f64() / seconds).clamp(0.0, 1.0)
            })
        };
        island_ui::media::Feedback {
            focus: self.focus,
            hover: self.hover,
            pressed: self.pressed,
            hover_mix: mix(self.hover_start, 0.08),
            cover_mix: mix(self.cover_start, 0.14),
        }
    }
    fn animating(&self) -> bool {
        self.hover_start
            .is_some_and(|t| t.elapsed().as_millis() < 80)
            || self
                .cover_start
                .is_some_and(|t| t.elapsed().as_millis() < 140)
    }
}
impl App {
    pub(super) fn media_changed(&mut self) -> io::Result<()> {
        if let Some(snapshot) = self.media_worker.as_ref().and_then(|w| w.take()) {
            if !self.settings.modules.media_enabled {
                return Ok(());
            }
            let previous = self.media_view.snapshot.active();
            let next = snapshot.active();
            let changed = previous.map(|s| {
                (
                    s.id,
                    s.generation,
                    s.artwork.as_ref().map(std::sync::Arc::as_ptr),
                )
            }) != next.map(|s| {
                (
                    s.id,
                    s.generation,
                    s.artwork.as_ref().map(std::sync::Arc::as_ptr),
                )
            });
            if changed {
                self.media_view.cover_start = animations_allowed().then(Instant::now);
            }
            self.media_view.snapshot = snapshot;
            if let Some(popup) = &mut self.popover {
                popup.update(&self.media_view.snapshot)?;
            }
            self.media_view.busy = self.media_worker.as_ref().is_some_and(|w| w.busy());
            // Keep every visibility preference. Never expand or take focus for media.
            if self.media_page()
                && !self.hidden
                && !self.concentrating()
                && self.transition.expanded
            {
                self.render_key = None;
                self.frame()?;
            }
        }
        self.media_timer();
        Ok(())
    }
    pub(super) fn media_timer(&mut self) {
        let visible = self.media_page()
            && !self.hidden
            && !self.concentrating()
            && self.transition.expanded
            && !self.transition.active();
        // Finite feedback animation only. No input polling or idle timer.
        unsafe {
            if visible && self.media_view.animating() {
                SetTimer(self.hwnd, FEEDBACK_TIMER, 15, None);
            } else {
                KillTimer(self.hwnd, FEEDBACK_TIMER);
            }
        }
        let active = self.media_page()
            && !self.hidden
            && !self.concentrating()
            && self.transition.expanded
            && !self.transition.active()
            && self.media_view.snapshot.active().is_some_and(|s| {
                s.playback == Playback::Playing
                    && s.timeline.as_ref().is_some_and(|t| {
                        t.position_at(Instant::now(), true)
                            .is_some_and(|p| p < t.duration)
                    })
            });
        if active != self.media_view.timer {
            // SAFETY: one timer on our HWND, exists only while visible progress is useful.
            unsafe {
                if active {
                    self.media_view.timer = SetTimer(self.hwnd, PROGRESS, 1000, None) != 0;
                } else {
                    KillTimer(self.hwnd, PROGRESS);
                    self.media_view.timer = false;
                }
            }
        }
    }
    pub(super) fn media_tick(&mut self) -> io::Result<()> {
        let was_active = self.media_view.timer;
        self.media_timer();
        if was_active && !self.hidden && !self.concentrating() && self.transition.expanded {
            self.render_key = None;
            self.frame()?;
        }
        Ok(())
    }
    pub(super) fn media_click(&mut self, lp: LPARAM) -> bool {
        if self.navigation_click(lp) {
            return true;
        }
        if !self.media_page() {
            return false;
        }
        if !self.transition.expanded || self.transition.active() || self.concentrating() {
            return false;
        }
        let x = (lp as u16 as i16) as f64 / self.scale;
        let y = ((lp >> 16) as u16 as i16) as f64 / self.scale;
        if let Some(action) = island_ui::media::hit(x, y) {
            let pressed = self.media_view.pressed.take();
            if pressed == Some(action) {
                self.media_action(action);
            }
            self.render_key = None;
            let result = self.frame();
            self.record(result);
            true
        } else {
            false
        }
    }
    pub(super) fn media_pointer(&mut self, lp: Option<LPARAM>, down: bool) {
        let navigation_hit = self.navigation_pointer(lp, down);
        if !self.media_page() {
            return;
        }
        if !self.transition.expanded
            || self.transition.active()
            || self.concentrating()
            || self.hidden
        {
            return;
        }
        let hit = lp.filter(|_| !navigation_hit).and_then(|lp| {
            island_ui::media::hit(
                (lp as u16 as i16) as f64 / self.scale,
                ((lp >> 16) as u16 as i16) as f64 / self.scale,
            )
        });
        if hit != self.media_view.hover || down {
            self.media_view.hover = hit;
            self.media_view.hover_start = animations_allowed().then(Instant::now);
            if down {
                self.media_view.pressed = hit;
                if hit.is_some() {
                    self.focus_for_explicit_open();
                }
            } else if lp.is_none() {
                self.media_view.pressed = None;
            }
            self.render_key = None;
            let result = self.frame();
            self.record(result);
        }
        if lp.is_some() {
            unsafe {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: self.hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut t);
            }
        }
    }
    pub(super) fn media_feedback_tick(&mut self) -> io::Result<()> {
        self.media_timer();
        if !self.hidden && !self.concentrating() && self.transition.expanded {
            self.render_key = None;
            self.frame()?;
        }
        Ok(())
    }
    fn media_action(&mut self, action: Action) {
        if action == Action::Sessions {
            self.media_menu();
            return;
        }
        if self.media_view.busy {
            return;
        }
        if let Some(s) = self.media_view.snapshot.active()
            && let Some(command) = island_ui::media::command(action, s)
            && let Some(w) = &self.media_worker
        {
            self.media_view.busy = w.command(s.id, s.generation, command);
            self.render_key = None;
            let result = self.frame();
            self.record(result);
        }
    }
    pub(super) fn media_key(&mut self, key: u16) -> bool {
        if self.navigation_key(key) {
            return true;
        }
        if !self.media_page() {
            return false;
        }
        if !self.transition.expanded || self.transition.active() || self.concentrating() {
            return false;
        }
        if matches!(key, VK_RETURN | VK_SPACE)
            && let Some(action) = self.media_view.focus
        {
            self.media_action(action);
            return true;
        }
        false
    }
    pub(super) fn media_menu(&mut self) {
        if !self.settings.modules.media_enabled {
            return;
        }
        let result = self.open_popover();
        self.record(result);
    }
    pub(super) fn stop_media_timer(&mut self) {
        unsafe {
            KillTimer(self.hwnd, PROGRESS);
            KillTimer(self.hwnd, FEEDBACK_TIMER);
        }
        self.media_view.timer = false;
    }
}
pub(super) fn animations_allowed() -> bool {
    let mut enabled: i32 = 1;
    // SAFETY: read-only BOOL system preference; no global configuration change.
    unsafe {
        SystemParametersInfoW(
            SPI_GETCLIENTAREAANIMATION,
            0,
            (&mut enabled as *mut i32).cast(),
            0,
        );
    }
    enabled != 0
}
pub(super) fn is_tick(wp: WPARAM) -> bool {
    wp == PROGRESS
}

#[cfg(test)]
mod tests {
    use super::*;
    use island_core::media::{Session, Timeline};
    #[test]
    fn progress_timer_only_for_visible_settled_playback() {
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        // SAFETY: hidden test-owned HWND, no foreground or input manipulation.
        unsafe {
            let hwnd = CreateWindowExW(
                0,
                w!("STATIC"),
                w!("MediaTimerTest"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null_mut()),
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let mut app = App::for_window(hwnd);
            app.media_view.snapshot = Snapshot {
                selected: Some(1),
                sessions: vec![Session {
                    id: 1,
                    playback: Playback::Playing,
                    timeline: Some(Timeline {
                        position: 5.0,
                        duration: 100.0,
                        rate: 1.0,
                        sampled: Instant::now(),
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            };
            app.hidden = false;
            app.media_timer();
            assert!(!app.media_view.timer);
            app.transition.set(true, 0.0, true);
            app.media_timer();
            assert!(!app.media_view.timer);
            app.transition.sample(1.0);
            app.media_timer();
            assert!(app.media_view.timer);
            app.visibility.presentation = Presentation::Concentration;
            app.media_timer();
            assert!(!app.media_view.timer);
            app.visibility.presentation = Presentation::Normal;
            app.media_timer();
            assert!(app.media_view.timer);
            app.settings.modules.page = island_core::modules::Page::Home;
            app.media_timer();
            assert!(!app.media_view.timer);
            app.settings.modules.page = island_core::modules::Page::Media;
            app.media_timer();
            assert!(app.media_view.timer);
            app.hidden = true;
            app.media_timer();
            assert!(!app.media_view.timer);
            app.hidden = false;
            app.media_view.snapshot.sessions[0].playback = Playback::Paused;
            app.media_timer();
            assert!(!app.media_view.timer);
            app.media_view.snapshot.sessions[0].playback = Playback::Playing;
            app.media_view.snapshot.sessions[0].timeline = None;
            app.media_timer();
            assert!(!app.media_view.timer);
            drop(app);
            DestroyWindow(hwnd);
        }
    }
}
