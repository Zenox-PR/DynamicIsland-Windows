//! Countdown adapter: one HWND timer, no worker, polling, high-resolution timer or wake lock.
use super::*;
use island_core::timer::{Mode, Timer};
use island_ui::navigation::Action;

pub(super) const WAKE: usize = 5;
pub(super) fn now() -> u64 {
    // SAFETY: parameterless monotonic system counter, including sleep/hibernation.
    unsafe { windows_sys::Win32::System::SystemInformation::GetTickCount64() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use island_core::{modules::Page, timer::State};
    #[test]
    fn native_deadline_survives_navigation_and_stops_on_disable_and_drop() {
        let _guard = NATIVE_TEST_LOCK.lock().unwrap();
        // Hidden owned window, no tray registration, foreground changes or input injection.
        unsafe {
            let hwnd = CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Island.Timer.Test"),
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
            let outcome = std::panic::catch_unwind(|| {
                let mut app = App::for_window(hwnd);
                app.hidden = true;
                app.settings.modules.page = Page::Timer;
                app.timer_action(Action::TimerPrimary).unwrap();
                let deadline = app.timer.armed.unwrap();
                assert!(deadline > now() + 1_400_000);
                app.navigation_action(Action::Home).unwrap();
                assert_eq!(app.timer.armed, Some(deadline));
                app.navigation_action(Action::ToggleTimer).unwrap();
                assert!(matches!(app.timer.model.state, State::Paused { .. }));
                assert_eq!(app.timer.armed, None);
                let paused = app.timer.model.remaining(now());
                app.timer_tick().unwrap(); // already queued WM_TIMER must not restart
                app.navigation_action(Action::ToggleTimer).unwrap();
                assert_eq!(app.timer.model.remaining(now()), paused);
                assert_eq!(app.timer.armed, None);
                app.timer_action(Action::TimerPrimary).unwrap();
                assert!(app.timer.armed.is_some());
                // Simulate overdue wake/resume, no real sleep or global clock adjustment.
                app.timer.model.state = State::Running {
                    deadline: now().saturating_sub(1),
                };
                app.timer_tick().unwrap();
                assert_eq!(app.timer.model.state, State::Finished);
                assert_eq!(app.timer.armed, None);
                app.timer_tick().unwrap();
                assert_eq!(app.timer.model.state, State::Finished);
                app.timer.model.state = State::Running {
                    deadline: now().saturating_sub(1),
                };
                app.timer_action(Action::TimerPrimary).unwrap();
                assert_eq!(
                    app.timer.model.state,
                    State::Finished,
                    "late Pause click does not restart"
                );
                app.timer_action(Action::TimerPrimary).unwrap();
                assert!(app.timer.armed.is_some());
                drop(app);
                assert_eq!(KillTimer(hwnd, WAKE), 0, "owned timer released at shutdown");
            });
            DestroyWindow(hwnd);
            if let Err(e) = outcome {
                std::panic::resume_unwind(e);
            }
        }
    }
}
#[derive(Default)]
pub(super) struct View {
    pub model: Timer,
    pub armed: Option<u64>,
}
impl App {
    pub(super) fn reconcile_timer(&mut self) {
        if let Some(phase) = self.timer.model.settle(now()) {
            diagnostic(&format!("timer completed: {}", phase.label()));
            self.render_key = None;
            // Copy: NIF_INFO must not remain in the tray registration/recovery state.
            // Windows may suppress the notification. Finished state remains visible in the page.
            if !self.tray.hWnd.is_null() {
                let mut info = self.tray;
                info.uFlags = NIF_INFO | NIF_REALTIME;
                info.dwInfoFlags = NIIF_INFO | NIIF_NOSOUND | NIIF_RESPECT_QUIET_TIME;
                info.szInfo.fill(0);
                info.szInfoTitle.fill(0);
                let title: Vec<u16> = "Dynamic Island — terminé".encode_utf16().collect();
                info.szInfoTitle[..title.len()].copy_from_slice(&title);
                let text: Vec<u16> = format!(
                    "{} terminé. Retrouvez le minuteur dans l'encoche.",
                    phase.label()
                )
                .encode_utf16()
                .collect();
                info.szInfo[..text.len()].copy_from_slice(&text);
                // SAFETY: copied initialized tray identity, bounded NUL-terminated arrays.
                if unsafe { Shell_NotifyIconW(NIM_MODIFY, &info) } == 0 {
                    diagnostic("timer notification unavailable; completion retained in panel");
                }
            }
        }
    }
    pub(super) fn timer_schedule(&mut self) -> io::Result<()> {
        let current = now();
        let visible = self.settings.modules.timer_visible() && self.panel_ready();
        let due = self.timer.model.next_wake(current, visible);
        if due != self.timer.armed {
            // SAFETY: private ID bound to the owned HWND. Late messages recheck the model.
            unsafe {
                KillTimer(self.hwnd, WAKE);
            }
            self.timer.armed = None;
            if let Some(deadline) = due {
                let delay = deadline.saturating_sub(current).clamp(10, 0x7fff_ffff) as u32;
                if unsafe { SetTimer(self.hwnd, WAKE, delay, None) } == 0 {
                    return Err(io::Error::last_os_error());
                }
                self.timer.armed = due;
            }
        }
        Ok(())
    }
    pub(super) fn timer_tick(&mut self) -> io::Result<()> {
        // Stop periodic Win32 semantics, then select exactly the next useful deadline.
        unsafe {
            KillTimer(self.hwnd, WAKE);
        }
        self.timer.armed = None;
        self.reconcile_timer();
        if self.settings.modules.timer_visible() && self.panel_ready() {
            self.render_key = None;
            self.frame()?;
        } else {
            self.timer_schedule()?;
        }
        Ok(())
    }
    pub(super) fn timer_action(&mut self, action: Action) -> io::Result<()> {
        if !self.settings.modules.timer_enabled {
            return Ok(());
        }
        let was_running = matches!(
            self.timer.model.state,
            island_core::timer::State::Running { .. }
        );
        self.reconcile_timer();
        // A press intended as Pause must not start the next phase if its deadline
        // elapsed between the last displayed frame and the delivered click.
        if action == Action::TimerPrimary
            && was_running
            && self.timer.model.state == island_core::timer::State::Finished
        {
            return self.frame();
        }
        match action {
            Action::TimerPrimary => self.timer.model.primary(now()),
            Action::TimerReset => self.timer.model.reset(),
            Action::Countdown | Action::Pomodoro => {
                let mode = if action == Action::Countdown {
                    Mode::Timer
                } else {
                    Mode::Pomodoro
                };
                if mode != self.timer.model.mode
                    && self.timer.model.configure(self.settings.timer, mode)
                {
                    self.settings.timer_mode = mode;
                    self.persist();
                }
            }
            Action::TimerSettings if self.timer.model.configurable() => self.timer_settings()?,
            _ => {}
        }
        self.render_key = None;
        diagnostic(&format!(
            "timer action: {action:?}; phase={:?}; state={:?}",
            self.timer.model.phase, self.timer.model.state
        ));
        self.frame()
    }
    fn timer_settings(&mut self) -> io::Result<()> {
        // Owned native menu, modal only for the explicit user action. No global settings.
        struct Menu(HMENU);
        impl Drop for Menu {
            fn drop(&mut self) {
                unsafe {
                    DestroyMenu(self.0);
                }
            }
        }
        unsafe fn append(menu: HMENU, flags: u32, id: usize, label: &str) -> io::Result<()> {
            let text: Vec<u16> = label.encode_utf16().chain(Some(0)).collect();
            if unsafe { AppendMenuW(menu, flags, id, text.as_ptr()) } == 0 {
                Err(io::Error::last_os_error())
            } else {
                Ok(())
            }
        }
        // SAFETY: menus and UTF-16 buffers are owned until the synchronous APIs return.
        unsafe {
            let root = Menu(CreatePopupMenu());
            if root.0.is_null() {
                return Err(io::Error::last_os_error());
            }
            let values = [
                self.settings.timer.minutes,
                self.settings.timer.work,
                self.settings.timer.short_break,
                self.settings.timer.long_break,
            ];
            for (i, label) in ["Minuteur", "Travail", "Pause courte", "Pause longue"]
                .into_iter()
                .enumerate()
            {
                let submenu = CreatePopupMenu();
                if submenu.is_null() {
                    return Err(io::Error::last_os_error());
                }
                // Transfer ownership to root immediately; destroy explicitly if transfer fails.
                if let Err(e) = append(
                    root.0,
                    MF_POPUP,
                    submenu as usize,
                    &format!("{label} : {} min", values[i]),
                ) {
                    DestroyMenu(submenu);
                    return Err(e);
                }
                let base = 100 + i * 20;
                for (j, (label, allowed)) in [
                    ("− 1 minute", values[i] > 1),
                    ("+ 1 minute", values[i] < 240),
                    ("− 5 minutes", values[i] > 1),
                    ("+ 5 minutes", values[i] < 240),
                ]
                .into_iter()
                .enumerate()
                {
                    append(
                        submenu,
                        MF_STRING | if allowed { 0 } else { MF_GRAYED },
                        base + j,
                        label,
                    )?;
                }
                for (j, value) in [1, 5, 10, 15, 25, 30, 45, 60, 90, 120, 240]
                    .into_iter()
                    .enumerate()
                {
                    append(
                        submenu,
                        MF_STRING | if values[i] == value { MF_CHECKED } else { 0 },
                        base + 4 + j,
                        &format!("{value} minutes"),
                    )?;
                }
            }
            let point = POINT {
                x: self.bounds.left + (268.0 * self.scale) as i32,
                y: self.bounds.top + (151.0 * self.scale) as i32,
            };
            let selected = TrackPopupMenu(
                root.0,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                self.hwnd,
                std::ptr::null(),
            ) as usize;
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            if (100..180).contains(&selected) {
                let i = (selected - 100) / 20;
                let entry = (selected - 100) % 20;
                let value = match entry {
                    0 => values[i].saturating_sub(1).max(1),
                    1 => (values[i] + 1).min(240),
                    2 => values[i].saturating_sub(5).max(1),
                    3 => (values[i] + 5).min(240),
                    4..=14 => [1, 5, 10, 15, 25, 30, 45, 60, 90, 120, 240][entry - 4],
                    _ => return Ok(()),
                };
                let mut config = self.settings.timer;
                *match i {
                    0 => &mut config.minutes,
                    1 => &mut config.work,
                    2 => &mut config.short_break,
                    _ => &mut config.long_break,
                } = value;
                if self.timer.model.configure(config, self.timer.model.mode) {
                    self.settings.timer = config;
                    self.persist();
                }
            }
        }
        Ok(())
    }
}
