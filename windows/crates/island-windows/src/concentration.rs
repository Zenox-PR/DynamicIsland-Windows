//! Native click-through presentation. No polling, extra HWND, or input hook.
use super::*;

impl App {
    pub(super) fn concentrating(&self) -> bool {
        self.visibility.presentation == Presentation::Concentration
    }

    pub(super) fn set_concentration(&mut self, enabled: bool) -> io::Result<()> {
        if enabled == self.concentrating() {
            return Ok(());
        }
        self.close_popover(false);
        // Restore any in-flight drag's original placement before changing presentation.
        self.finish_move(false)?;
        self.move_armed = false;
        self.suppress_release = true;
        self.release_keyboard_focus(true);
        // This mode switch is immediate. In-flight/queued animation ticks cannot reopen it.
        self.transition.set(false, self.time(), false);
        // SAFETY: all operations target this UI thread's own HWND/capture/hook only.
        unsafe {
            if GetFocus() == self.hwnd {
                SetFocus(null_mut());
            }
            self.focused = false;
            KillTimer(self.hwnd, FRAME);
            if GetCapture() == self.hwnd && ReleaseCapture() == 0 {
                return Err(io::Error::last_os_error());
            }
            if !self.mouse.is_null() {
                if UnhookWindowsHookEx(self.mouse) == 0 {
                    return Err(io::Error::last_os_error());
                }
                self.mouse = null_mut();
            }
            // SAFETY: UI-thread-owned live HWND, preserve every other extended style.
            // Layered + TRANSPARENT routes input to windows in other processes as well.
            SetLastError(0);
            let old = GetWindowLongPtrW(self.hwnd, GWL_EXSTYLE);
            if old == 0 && GetLastError() != 0 {
                return Err(io::Error::last_os_error());
            }
            let new = if enabled {
                old | WS_EX_TRANSPARENT as isize
            } else {
                old & !(WS_EX_TRANSPARENT as isize)
            };
            SetLastError(0);
            if SetWindowLongPtrW(self.hwnd, GWL_EXSTYLE, new) == 0 && GetLastError() != 0 {
                return Err(io::Error::last_os_error());
            }
            if SetWindowPos(
                self.hwnd,
                null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        self.visibility.presentation = if enabled {
            Presentation::Concentration
        } else {
            Presentation::Normal
        };
        self.render_key = None;
        self.frame()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_mode_cancels_gesture_animation_and_restores_styles() {
        let _guard = NATIVE_TEST_LOCK.lock().unwrap();
        // Hidden owned HWND: no desktop input or foreground manipulation. Real style/hook APIs.
        unsafe {
            let instance = GetModuleHandleW(null_mut());
            let class = w!("DynamicIsland.Test.Concentration");
            let descriptor = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&descriptor), 0);
            let styles = WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE;
            let hwnd = CreateWindowExW(
                styles,
                class,
                class,
                WS_POPUP,
                0,
                0,
                172,
                32,
                null_mut(),
                null_mut(),
                instance,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let mut app = App::for_window(hwnd);
            app.hidden = true;
            app.visibility.manual = true;
            // Resolve monitor once, then verify exact preferences through all transitions.
            app.frame().unwrap();
            let original = app.settings.clone();
            APP.with(|cell| *cell.borrow_mut() = Some(app));
            let outcome = std::panic::catch_unwind(|| {
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    app.set_expanded(true).unwrap();
                    assert!(!app.mouse.is_null());
                    app.transition.set(false, 0.0, false);
                    app.transition.set(true, 0.0, true);
                    app.transition.sample(0.05);
                    app.moving = Some(Moving {
                        original: original.clone(),
                        gesture: Drag {
                            start: [0, 0],
                            anchor: [200, 10],
                            moved: true,
                        },
                    });
                    app.settings.position = Placement { x: 0, y: 800_000 };
                    app.move_armed = true;
                    SetCapture(hwnd);
                    app.set_concentration(true).unwrap();
                    assert!(app.mouse.is_null() && GetCapture() != hwnd);
                    assert!(app.moving.is_none() && !app.move_armed);
                    assert!(!app.transition.active() && !app.transition.expanded);
                    assert_eq!(app.settings, original);
                    assert_eq!(
                        GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32,
                        styles | WS_EX_TRANSPARENT
                    );
                });
                // Queued ticks/clicks must never reopen a click-through surface.
                SendMessageW(hwnd, WM_TIMER, FRAME, 0);
                SendMessageW(hwnd, WM_LBUTTONDOWN, 0, 0);
                SendMessageW(hwnd, WM_LBUTTONUP, 0, 0);
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    assert!(app.concentrating() && !app.transition.expanded && app.mouse.is_null());
                    for _ in 0..100 {
                        app.set_concentration(false).unwrap();
                        assert_eq!(GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32, styles);
                        app.set_concentration(true).unwrap();
                        assert_eq!(app.settings, original);
                    }
                    // Fullscreen/manual visibility are not cleared by a mode toggle.
                    app.visibility.fullscreen = true;
                    app.set_concentration(false).unwrap();
                    assert!(app.visibility.manual && app.visibility.fullscreen);
                    app.set_expanded(true).unwrap();
                    assert!(app.transition.expanded && !app.mouse.is_null());
                    app.set_expanded(false).unwrap();
                    assert!(app.mouse.is_null());
                    assert!(app.error.is_none());
                    // Reserve a free combination ourselves, then exercise non-fatal collision.
                    let modifiers = MOD_CONTROL | MOD_ALT | MOD_SHIFT | MOD_NOREPEAT;
                    let key = (VK_F1 as u32..=VK_F11 as u32)
                        .find(|key| RegisterHotKey(hwnd, 71, modifiers, *key) != 0)
                        .expect("No free test combination; no foreign registration modified");
                    app.settings.concentration_shortcut =
                        format!("Ctrl+Alt+Shift+F{}", key - VK_F1 as u32 + 1);
                    app.register_concentration_hotkey().unwrap();
                    assert!(!app.concentration_hotkey);
                    assert!(app.error.is_none() && app.mode_error.is_none());
                    assert_ne!(UnregisterHotKey(hwnd, 71), 0);
                    app.register_concentration_hotkey().unwrap();
                    assert!(app.concentration_hotkey);
                    // Clean registration released here; drop tests the same owned path below.
                    assert_ne!(UnregisterHotKey(hwnd, CONCENTRATION_HOTKEY_ID), 0);
                    app.concentration_hotkey = false;
                });
            });
            APP.with(|cell| drop(cell.borrow_mut().take()));
            CAPTURE_LOST.set(false);
            ESCAPE_PENDING.set(false);
            DestroyWindow(hwnd);
            UnregisterClassW(class, instance);
            if let Err(panic) = outcome {
                std::panic::resume_unwind(panic);
            }
        }
    }

    #[test]
    fn invalid_window_style_switch_reports_error() {
        let mut app = App::for_window(null_mut());
        assert!(app.set_concentration(true).is_err());
        assert!(!app.concentrating());
    }
}
