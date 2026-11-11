//! Panel navigation and module lifecycle coordination. Visibility policy remains in App.
use super::*;
use island_core::modules::{ModuleId, Page};
use island_ui::{
    media::Action as MediaAction,
    navigation::{self as layout, Action},
};

impl App {
    pub(super) fn media_page(&self) -> bool {
        self.settings.modules.media_visible()
    }
    pub(super) fn panel_ready(&self) -> bool {
        !self.hidden
            && !self.concentrating()
            && self.transition.expanded
            && !self.transition.active()
    }
    pub(super) fn navigation_action(&mut self, action: Action) -> io::Result<()> {
        let before = self.settings.modules;
        match action {
            Action::Launcher | Action::OpenLauncher => {
                self.open_launcher_page()?;
            }
            Action::ToggleLauncher => {
                let enabled = !self.settings.modules.launcher_enabled;
                if let Some(worker) = &self.launcher.worker {
                    worker.set_enabled(enabled);
                }
                if !enabled {
                    launcher_editor::close();
                    self.launcher.editing = false;
                }
                self.settings
                    .modules
                    .set_enabled(ModuleId::Launcher, enabled);
            }
            Action::AddShortcut
            | Action::FavoriteFilter
            | Action::PreviousShortcuts
            | Action::NextShortcuts
            | Action::Launch(_)
            | Action::Pin(_)
            | Action::EditShortcut(_) => return self.launcher_action(action),
            Action::Timer => {
                self.settings.modules.navigate(Page::Timer);
            }
            Action::ToggleTimer => {
                self.reconcile_timer();
                self.timer.model.pause(timer_view::now());
                self.settings
                    .modules
                    .set_enabled(ModuleId::Timer, !self.settings.modules.timer_enabled);
            }
            Action::Countdown
            | Action::Pomodoro
            | Action::TimerPrimary
            | Action::TimerReset
            | Action::TimerSettings => {
                return self.timer_action(action);
            }
            Action::Home => {
                self.settings.modules.navigate(Page::Home);
            }
            Action::Media => {
                self.settings.modules.navigate(Page::Media);
            }
            Action::ToggleMedia => {
                let enabled = !self.settings.modules.media_enabled;
                // Allocate only on the first activation; later disables retain a
                // sleeping worker, with all native subscriptions removed on its MTA.
                if enabled && self.media_worker.is_none() {
                    self.media_worker = Some(media::Worker::new(self.hwnd as isize)?);
                }
                if let Some(worker) = &self.media_worker {
                    worker.set_enabled(enabled);
                }
                self.settings.modules.set_enabled(ModuleId::Media, enabled);
                self.media_view = media_view::View::default();
            }
        }
        if self.settings.modules != before {
            self.close_popover(false);
            self.media_view.focus = None;
            self.media_view.hover = None;
            self.media_view.pressed = None;
            self.navigation = layout::Feedback::default();
            self.persist();
            self.stop_media_timer();
            self.render_key = None;
            self.frame()?;
        }
        Ok(())
    }
    pub(super) fn navigation_pointer(&mut self, lp: Option<LPARAM>, down: bool) -> bool {
        if !self.panel_ready() {
            return false;
        }
        let hit = lp.and_then(|p| {
            layout::hit(
                (p as u16 as i16) as f64 / self.scale,
                ((p >> 16) as u16 as i16) as f64 / self.scale,
                self.settings.modules,
            )
        });
        let changed = hit != self.navigation.hover || down;
        self.navigation.hover = hit;
        if down {
            self.navigation.pressed = hit;
            if hit.is_some() {
                self.navigation.focus = hit;
                self.media_view.focus = None;
                self.focus_for_explicit_open();
            } else {
                self.navigation.focus = None;
            }
        } else if lp.is_none() {
            self.navigation.pressed = None;
        }
        if changed {
            self.render_key = None;
            let result = self.frame();
            self.record(result);
        }
        if hit.is_some() {
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
        hit.is_some()
    }
    pub(super) fn navigation_click(&mut self, lp: LPARAM) -> bool {
        if !self.panel_ready() {
            return false;
        }
        let hit = layout::hit(
            (lp as u16 as i16) as f64 / self.scale,
            ((lp >> 16) as u16 as i16) as f64 / self.scale,
            self.settings.modules,
        );
        let pressed = self.navigation.pressed.take();
        if let Some(action) = hit {
            if pressed == Some(action) {
                let result = self.navigation_action(action);
                self.record(result);
            }
            self.render_key = None;
            let result = self.frame();
            self.record(result);
            return true;
        }
        false
    }
    pub(super) fn navigation_key(&mut self, key: u16) -> bool {
        if !self.panel_ready() {
            return false;
        }
        if key == VK_TAB {
            let mut order: Vec<(Option<Action>, Option<MediaAction>)> =
                layout::actions(self.settings.modules)
                    .into_iter()
                    .map(|a| (Some(a), None))
                    .collect();
            if self.media_page() {
                order.extend(
                    island_ui::media::ACTIONS
                        .into_iter()
                        .map(|a| (None, Some(a))),
                );
            }
            let index = order
                .iter()
                .position(|v| *v == (self.navigation.focus, self.media_view.focus));
            let backwards = unsafe { GetKeyState(VK_SHIFT as i32) < 0 };
            let next = match index {
                None => {
                    if backwards {
                        order.len() - 1
                    } else {
                        0
                    }
                }
                Some(i) => {
                    if backwards {
                        (i + order.len() - 1) % order.len()
                    } else {
                        (i + 1) % order.len()
                    }
                }
            };
            (self.navigation.focus, self.media_view.focus) = order[next];
            self.render_key = None;
            let result = self.frame();
            self.record(result);
            return true;
        }
        if matches!(key, VK_RETURN | VK_SPACE)
            && let Some(action) = self.navigation.focus
        {
            let result = self.navigation_action(action);
            self.record(result);
            return true;
        }
        false
    }
}
