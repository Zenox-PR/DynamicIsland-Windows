//! Native shell adapter. All callbacks execute on the single message-loop thread.
mod app_names;
mod concentration;
mod launcher;
mod launcher_editor;
mod launcher_view;
pub mod media;
mod media_view;
mod navigation;
mod placement;
mod popover;
mod preferences;
mod surface;
pub mod system_info;
mod timer_view;

use island_core::{
    Transition, distance,
    placement::{Drag, Placement, fit},
    visibility::{Presentation, Visibility},
};
use island_ui::dimensions;
use std::{
    cell::{Cell, RefCell},
    io,
    mem::size_of,
    ptr::null_mut,
    sync::atomic::{AtomicIsize, Ordering},
    time::Instant,
};
use surface::{Surface, Typography};
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{LibraryLoader::*, Threading::*},
        UI::{
            Accessibility::*, Controls::WM_MOUSELEAVE, HiDpi::*, Input::KeyboardAndMouse::*,
            Shell::*, WindowsAndMessaging::*,
        },
    },
    w,
};

const CLASS: windows_sys::core::PCWSTR = w!("DynamicIsland.Native.Prototype");
const TRAY: u32 = WM_APP + 1;
const ENVIRONMENT: u32 = WM_APP + 2;
const OUTSIDE: u32 = WM_APP + 3;
const SAVE_FAILED: u32 = WM_APP + 5;
const MEDIA_CHANGED: u32 = WM_APP + 6;
const HOTKEY_ID: i32 = 1;
const CONCENTRATION_HOTKEY_ID: i32 = 2;
const FRAME: usize = 1;
const SHELL_RECOVERY: usize = 2;
fn diagnostic(message: &str) {
    use std::io::Write;
    if let Ok(path) = std::env::var("ISLAND_DIAGNOSTICS_PATH")
        && let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
    {
        let _ = writeln!(file, "{message}");
    }
}
static WINDOW: AtomicIsize = AtomicIsize::new(0);
thread_local! { static APP: RefCell<Option<App>> = const { RefCell::new(None) }; }
thread_local! { static ESCAPE_PENDING: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static SHELL_MESSAGE: Cell<u32> = const { Cell::new(0) }; }
thread_local! { static SHELL_PENDING: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static MEDIA_PENDING: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static TIMER_PENDING: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static LAUNCHER_PENDING: Cell<bool> = const { Cell::new(false) }; }
thread_local! { static CAPTURE_LOST: Cell<bool> = const { Cell::new(false) }; }
#[cfg(test)]
static NATIVE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Moving {
    gesture: Drag,
    original: preferences::Settings,
}

struct App {
    launcher: launcher_view::View,
    timer: timer_view::View,
    navigation: island_ui::navigation::Feedback,
    popover: Option<popover::Popup>,
    media_worker: Option<media::Worker>,
    media_view: media_view::View,
    hwnd: HWND,
    transition: Transition,
    epoch: Instant,
    surface: Option<Surface>,
    typography: Option<Typography>,
    scale: f64,
    bounds: RECT,
    radius: f64,
    hidden: bool,
    focused: bool,
    return_focus: HWND,
    mouse: HHOOK,
    events: Vec<HWINEVENTHOOK>,
    tray: NOTIFYICONDATAW,
    icon: HICON,
    settings: preferences::Settings,
    writer: Option<preferences::Writer>,
    visibility: Visibility,
    moving: Option<Moving>,
    move_armed: bool,
    suppress_release: bool,
    hotkey: bool,
    concentration_hotkey: bool,
    render_key: Option<(i32, i32, u64, u64, bool)>,
    shell_retry_pending: bool,
    shell_attempts: u8,
    mode_error: Option<String>,
    error: Option<io::Error>,
}

impl App {
    fn for_window(hwnd: HWND) -> Self {
        Self {
            launcher: launcher_view::View::default(),
            timer: timer_view::View::default(),
            navigation: island_ui::navigation::Feedback::default(),
            popover: None,
            media_worker: None,
            media_view: media_view::View::default(),
            hwnd,
            transition: Transition::default(),
            epoch: Instant::now(),
            surface: None,
            typography: None,
            scale: 1.0,
            bounds: RECT::default(),
            radius: 16.0,
            hidden: false,
            focused: false,
            return_focus: null_mut(),
            mouse: null_mut(),
            events: Vec::new(),
            tray: NOTIFYICONDATAW::default(),
            icon: null_mut(),
            settings: preferences::Settings::default(),
            writer: None,
            visibility: Visibility::default(),
            moving: None,
            move_armed: false,
            suppress_release: false,
            hotkey: false,
            concentration_hotkey: false,
            render_key: None,
            shell_retry_pending: false,
            shell_attempts: 0,
            mode_error: None,
            error: None,
        }
    }

    fn new(hwnd: HWND) -> io::Result<Self> {
        let mut app = Self::for_window(hwnd);
        // SAFETY: all callbacks run out-of-context on this thread; HWND lives until run() cleanup.
        unsafe {
            app.icon = make_icon()?;
            app.tray.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
            app.tray.hWnd = hwnd;
            app.tray.uID = 1;
            app.tray.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
            app.tray.uCallbackMessage = TRAY;
            app.tray.hIcon = app.icon;
            let tip: Vec<u16> = "Dynamic Island — clic pour ouvrir".encode_utf16().collect();
            app.tray.szTip[..tip.len()].copy_from_slice(&tip);
            if Shell_NotifyIconW(NIM_ADD, &app.tray) == 0 {
                return Err(io::Error::other(
                    "Impossible de créer l'icône de notification",
                ));
            }
            let taskbar_message = RegisterWindowMessageW(w!("TaskbarCreated"));
            if taskbar_message == 0 {
                return Err(io::Error::last_os_error());
            }
            SHELL_MESSAGE.set(taskbar_message);
            for event in [
                EVENT_SYSTEM_FOREGROUND,
                EVENT_OBJECT_LOCATIONCHANGE,
                EVENT_SYSTEM_MINIMIZEEND,
            ] {
                let hook = SetWinEventHook(
                    event,
                    event,
                    null_mut(),
                    Some(win_event),
                    0,
                    0,
                    WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
                );
                if hook.is_null() {
                    return Err(io::Error::last_os_error());
                }
                app.events.push(hook);
            }
        }
        let path = preferences::path()?;
        match preferences::load(&path) {
            Ok(settings) => app.settings = settings,
            Err(error) => {
                app.mode_error = Some(format!(
                    "Préférences illisibles : {error}. Position réinitialisée."
                ))
            }
        }
        app.writer = Some(preferences::Writer::new(path, hwnd as isize)?);
        app.timer
            .model
            .configure(app.settings.timer, app.settings.timer_mode);
        app.persist(); // Also creates a documented editable file on first launch.
        if let Some((modifiers, key)) =
            preferences::hotkey(&app.settings.shortcut).map_err(io::Error::other)?
        {
            // SAFETY: registration belongs to this HWND/thread and is released on drop.
            app.hotkey = unsafe { RegisterHotKey(hwnd, HOTKEY_ID, modifiers, key) } != 0;
            if !app.hotkey {
                app.mode_error = Some("Raccourci indisponible : choisissez une autre combinaison dans settings.ini. L'icône système reste utilisable.".into());
            }
        }
        if std::env::args().any(|a| a == "--integrated") {
            diagnostic("--integrated retired; floating only, no reservation");
        }
        app.register_concentration_hotkey()?;
        app.frame()?;
        app.environment();
        if app.settings.modules.media_enabled {
            app.media_worker = Some(media::Worker::new(hwnd as isize)?);
        }
        if app.settings.modules.launcher_visible() {
            app.launcher_load()?;
        }
        Ok(app)
    }

    fn time(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    fn register_concentration_hotkey(&mut self) -> io::Result<()> {
        if let Some((modifiers, key)) =
            preferences::hotkey(&self.settings.concentration_shortcut).map_err(io::Error::other)?
        {
            // SAFETY: a second independent registration, owned by this HWND until drop.
            self.concentration_hotkey =
                unsafe { RegisterHotKey(self.hwnd, CONCENTRATION_HOTKEY_ID, modifiers, key) } != 0;
            if !self.concentration_hotkey {
                let warning = "Raccourci concentration indisponible : le menu de l'icône système reste utilisable.";
                diagnostic(warning);
                // Non-modal: a conflict must not delay startup or block the UI loop.
                let tip: Vec<u16> =
                    "Dynamic Island — raccourci concentration indisponible ; utiliser le menu"
                        .encode_utf16()
                        .collect();
                self.tray.szTip.fill(0);
                self.tray.szTip[..tip.len()].copy_from_slice(&tip);
                unsafe {
                    Shell_NotifyIconW(NIM_MODIFY, &self.tray);
                }
            }
        }
        Ok(())
    }

    fn persist(&self) {
        if let Some(writer) = &self.writer {
            writer.submit(self.settings.clone());
        }
    }

    fn synchronize_visibility(&mut self) -> io::Result<()> {
        let hidden = self.visibility.hidden();
        if hidden == self.hidden {
            return Ok(());
        }
        self.hidden = hidden;
        if hidden {
            self.close_popover(false);
            self.finish_move(false)?;
            self.move_armed = false;
            self.set_expanded(false)?;
            self.release_keyboard_focus(false);
            // SAFETY: only our live window.
            unsafe {
                ShowWindow(self.hwnd, SW_HIDE);
            }
        } else {
            self.frame()?;
            unsafe {
                ShowWindow(self.hwnd, SW_SHOWNOACTIVATE);
            }
        }
        Ok(())
    }

    fn show_compact(&mut self, reset: bool) -> io::Result<()> {
        self.set_concentration(false)?;
        self.finish_move(false)?;
        self.move_armed = false;
        if reset {
            self.settings.monitor.clear();
            self.settings.position = Placement::default();
            self.persist();
        }
        self.visibility.restore_normal();
        self.environment();
        self.synchronize_visibility()?;
        self.set_expanded(false)?;
        self.release_keyboard_focus(true);
        self.frame()
    }

    fn begin_move(&mut self, point: [i32; 2]) -> io::Result<()> {
        self.close_popover(false);
        if self.hidden || self.concentrating() {
            return Ok(());
        }
        self.focus_for_explicit_open();
        // A gesture must never capture other applications' input if activation was denied.
        if !self.focused {
            self.move_armed = false;
            self.suppress_release = true;
            return Ok(());
        }
        let monitor = placement::find(&self.settings.monitor)?;
        let size = [
            (172.0 * self.scale).round() as i32,
            (32.0 * self.scale).round() as i32,
        ];
        let anchor =
            self.settings
                .position
                .point(monitor.area, size, (8.0 * self.scale).round() as i32);
        self.moving = Some(Moving {
            gesture: Drag {
                start: point,
                anchor,
                moved: false,
            },
            original: self.settings.clone(),
        });
        self.move_armed = false;
        // SAFETY: explicit gesture, foreground HWND, capture released on every termination path.
        unsafe {
            SetCapture(self.hwnd);
        }
        if unsafe { GetCapture() } != self.hwnd {
            self.moving = None;
            self.suppress_release = true;
        }
        Ok(())
    }

    fn move_pointer(&mut self, point: [i32; 2]) -> io::Result<()> {
        let dpi = (self.scale * 96.0).round() as u32;
        // SAFETY: metric queries only, no settings mutation.
        let threshold = unsafe {
            [
                GetSystemMetricsForDpi(SM_CXDRAG, dpi).max(4),
                GetSystemMetricsForDpi(SM_CYDRAG, dpi).max(4),
            ]
        };
        if let Some(moving) = &mut self.moving
            && let Some(anchor) = moving.gesture.update(point, threshold)
        {
            let monitor = placement::at(point)?;
            self.settings.position = Placement::from_point(
                anchor,
                monitor.area,
                [
                    (172.0 * self.scale).round() as i32,
                    (32.0 * self.scale).round() as i32,
                ],
                (8.0 * self.scale).round() as i32,
            );
            self.settings.monitor = monitor.name;
            self.frame()?;
        }
        Ok(())
    }

    fn finish_move(&mut self, commit: bool) -> io::Result<()> {
        if let Some(moving) = self.moving.take() {
            self.suppress_release = true;
            if !commit {
                self.settings = moving.original;
            }
            // Clear state before ReleaseCapture's synchronous WM_CAPTURECHANGED.
            unsafe {
                if GetCapture() == self.hwnd {
                    ReleaseCapture();
                }
            }
            if commit && moving.gesture.moved {
                self.persist();
            }
            self.frame()?;
            if !self.transition.expanded {
                self.release_keyboard_focus(true);
            }
        }
        Ok(())
    }

    fn schedule_shell_recovery(&mut self) {
        if self.shell_retry_pending {
            return;
        }
        self.shell_retry_pending = true;
        self.shell_attempts = 0;
        // Coalesce repeated notifications for our sole HWND.
        // SAFETY: timer is bounded to at most three attempts, killed on all exit paths.
        if unsafe { SetTimer(self.hwnd, SHELL_RECOVERY, 250, None) } == 0 {
            self.shell_retry_pending = false;
            self.mode_error = Some("Restauration de l’icône impossible".into());
        }
    }

    fn restore_shell(&mut self) {
        if !self.shell_retry_pending {
            return;
        } // KillTimer does not discard already queued messages.
        unsafe {
            KillTimer(self.hwnd, SHELL_RECOVERY);
        }
        self.shell_attempts += 1;
        // SAFETY: our tray data and HWND stay valid, no Explorer manipulation.
        let tray_ok = unsafe {
            Shell_NotifyIconW(NIM_ADD, &self.tray) != 0
                || Shell_NotifyIconW(NIM_MODIFY, &self.tray) != 0
        };
        if tray_ok {
            self.shell_retry_pending = false;
        } else if self.shell_attempts < 3
            && unsafe { SetTimer(self.hwnd, SHELL_RECOVERY, 1000, None) } != 0
        {
            return;
        } else {
            self.shell_retry_pending = false;
            self.mode_error = Some(if self.hotkey || self.concentration_hotkey {
                "Restauration de l'icône système impossible. Le raccourci configuré reste disponible.".into()
            } else {
                "Restauration de l'icône système impossible. Aucun raccourci global n'est enregistré.".into()
            });
        }
        let result = self.frame();
        self.record(result);
        self.environment();
    }

    fn remember_foreground(&mut self) {
        // SAFETY: borrowed HWND identity only; never destroyed or dereferenced by this app.
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground != self.hwnd {
                self.return_focus = foreground;
            }
        }
    }

    fn focus_for_explicit_open(&mut self) {
        // SAFETY: HWND belongs to this UI thread. Called only for a click/menu/key action.
        // Activation sends synchronous messages: their default processing must still run
        // when APP is already borrowed. Never attach input queues or retry a denied request.
        unsafe {
            if self.hidden || self.concentrating() || IsWindowVisible(self.hwnd) == 0 {
                return;
            }
            self.remember_foreground();
            SetForegroundWindow(self.hwnd);
            if GetForegroundWindow() == self.hwnd {
                SetFocus(self.hwnd);
            }
            self.focused = GetForegroundWindow() == self.hwnd && GetFocus() == self.hwnd;
            if !self.focused {
                diagnostic("explicit keyboard focus request not granted");
            }
        }
    }

    fn release_keyboard_focus(&mut self, restore_previous: bool) {
        let previous = std::mem::replace(&mut self.return_focus, null_mut());
        // SAFETY: borrowed HWND is validated before use. Do not reactivate anything after
        // an outside click/Alt+Tab, or if another application is already in the foreground.
        unsafe {
            if restore_previous
                && GetForegroundWindow() == self.hwnd
                && !previous.is_null()
                && IsWindow(previous) != 0
                && IsWindowVisible(previous) != 0
                && IsIconic(previous) == 0
                && SetForegroundWindow(previous) == 0
            {
                diagnostic("previous foreground restoration not granted");
            }
            self.focused = GetForegroundWindow() == self.hwnd && GetFocus() == self.hwnd;
        }
    }

    fn toggle_explicit(&mut self) -> io::Result<()> {
        if self.concentrating() {
            return Ok(());
        }
        let expanded = !self.transition.expanded;
        if expanded {
            self.focus_for_explicit_open();
        } else {
            self.release_keyboard_focus(true);
        }
        self.set_expanded(expanded)
    }

    fn set_expanded(&mut self, expanded: bool) -> io::Result<()> {
        // Never carry a previously focused activation toggle across panel sessions.
        self.navigation = island_ui::navigation::Feedback::default();
        if !expanded {
            self.close_popover(false);
            self.media_view.focus = None;
            self.media_view.hover = None;
            self.media_view.pressed = None;
        }
        let expanded = expanded && !self.concentrating();
        // SAFETY: reads a system preference; bool buffer is a Win32 BOOL.
        unsafe {
            let mut animations: i32 = 1;
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                (&mut animations as *mut i32).cast(),
                0,
            );
            self.transition.set(
                expanded,
                self.time(),
                animations != 0 && !self.hidden && !self.concentrating(),
            );
            if expanded && self.mouse.is_null() {
                self.mouse = SetWindowsHookExW(
                    WH_MOUSE_LL,
                    Some(mouse_hook),
                    GetModuleHandleW(null_mut()),
                    0,
                );
                if self.mouse.is_null() {
                    return Err(io::Error::last_os_error());
                }
            } else if !expanded && !self.mouse.is_null() {
                UnhookWindowsHookEx(self.mouse);
                self.mouse = null_mut();
            }
            self.frame()?;
            if self.transition.active() && SetTimer(self.hwnd, FRAME, 15, None) == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn frame(&mut self) -> io::Result<()> {
        self.reconcile_timer();
        // SAFETY: live HWND and initialized monitor info, no retained pointers to stack data.
        unsafe {
            let monitor = placement::find(&self.settings.monitor)?;
            if self.settings.monitor != monitor.name {
                if !self.settings.monitor.is_empty() {
                    self.settings.position = Placement::default();
                }
                self.settings.monitor = monitor.name.clone();
            }
            self.scale = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.0;
            let progress = self.transition.sample(self.time());
            let concentration = self.concentrating();
            let (width, height, radius) = if concentration {
                let (w, h, _) = island_ui::concentration_dimensions(self.scale);
                (w, h, h as f64 / 2.0)
            } else {
                dimensions(progress, self.scale)
            };
            let mut anchor = self.settings.position.point(
                monitor.area,
                [
                    (172.0 * self.scale).round() as i32,
                    (32.0 * self.scale).round() as i32,
                ],
                (island_ui::TOP_MARGIN * self.scale).round() as i32,
            );
            if concentration {
                anchor[1] += island_ui::concentration_dimensions(self.scale).2;
            }
            let [x, y] = fit(anchor, [width, height], monitor.area);
            let old_bounds = self.bounds;
            self.bounds = RECT {
                left: x,
                top: y,
                right: x + width,
                bottom: y + height,
            };
            self.radius = radius;
            if self.hidden {
                self.timer_schedule()?;
                self.stop_media_timer();
                self.render_key = None;
                KillTimer(self.hwnd, FRAME);
                return Ok(());
            }
            if self
                .typography
                .as_ref()
                .is_none_or(|cache| cache.scale != self.scale)
            {
                self.typography = Some(Typography::new(self.scale)?);
            }
            if self
                .surface
                .as_ref()
                .is_none_or(|s| s.size() != (width, height))
            {
                self.surface = Some(Surface::new(width, height)?);
            }
            let key = (
                width,
                height,
                progress.to_bits(),
                self.scale.to_bits(),
                self.focused,
            );
            if self.render_key != Some(key) {
                if let (Some(surface), Some(typography)) = (&mut self.surface, &self.typography) {
                    if concentration {
                        surface.draw_concentration();
                    } else if progress >= 1.0 {
                        if self.settings.modules.media_visible() {
                            surface.draw_media(
                                radius,
                                self.scale,
                                self.focused,
                                typography,
                                &self.media_view,
                            )?;
                        } else if self.settings.modules.launcher_visible() {
                            surface.draw_launcher_shell(
                                radius,
                                self.scale,
                                self.focused,
                                typography,
                            )?;
                        } else if self.settings.modules.timer_visible() {
                            surface.draw_timer(
                                radius,
                                self.scale,
                                self.focused,
                                typography,
                                &self.timer.model,
                                timer_view::now(),
                            )?;
                        } else {
                            surface.draw_home(
                                radius,
                                self.scale,
                                self.focused,
                                typography,
                                self.settings.modules,
                            )?;
                        }
                        surface.draw_navigation(
                            self.scale,
                            typography,
                            self.settings.modules,
                            &self.navigation,
                            &self.timer.model,
                        )?;
                        if self.settings.modules.launcher_visible() {
                            surface.draw_launcher_content(
                                self.scale,
                                typography,
                                &self.launcher,
                            )?;
                        }
                    } else {
                        // Animate the shell only: legacy placeholder labels must not flash
                        // before the media content becomes available at settled geometry.
                        surface.draw(radius, self.scale, 0.0, self.focused, typography)?;
                    }
                    surface.present(self.hwnd, x, y)?;
                    self.render_key = Some(key);
                }
            } else if (old_bounds.left != x || old_bounds.top != y)
                && SetWindowPos(
                    self.hwnd,
                    null_mut(),
                    x,
                    y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            if !self.transition.active() {
                KillTimer(self.hwnd, FRAME);
            }
            self.timer_schedule()?;
            self.media_timer();
        }
        Ok(())
    }

    fn environment(&mut self) {
        // SAFETY: handle queries only, no foreign pointers are dereferenced.
        unsafe {
            let foreground = GetForegroundWindow();
            if foreground == self.hwnd {
                return;
            }
            let primary = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
            let mut rect = RECT::default();
            let mut info = MONITORINFO {
                cbSize: size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let mut name = [0u16; 64];
            GetClassNameW(foreground, name.as_mut_ptr(), 64);
            let class =
                String::from_utf16_lossy(&name[..name.iter().position(|c| *c == 0).unwrap_or(64)]);
            let shell = matches!(
                class.as_str(),
                "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd"
            );
            let fullscreen = !foreground.is_null()
                && !shell
                && IsIconic(foreground) == 0
                && IsZoomed(foreground) == 0
                && MonitorFromWindow(foreground, MONITOR_DEFAULTTONEAREST) == primary
                && GetWindowRect(foreground, &mut rect) != 0
                && GetMonitorInfoW(primary, &mut info) != 0
                && rect.left <= info.rcMonitor.left
                && rect.top <= info.rcMonitor.top
                && rect.right >= info.rcMonitor.right
                && rect.bottom >= info.rcMonitor.bottom
                && (GetWindowLongW(foreground, GWL_STYLE) as u32 & WS_CAPTION == 0);
            self.visibility.fullscreen = fullscreen;
            let result = self.synchronize_visibility();
            self.record(result);
        }
    }

    fn record(&mut self, result: io::Result<()>) {
        if let Err(error) = result {
            self.error = Some(error);
            unsafe {
                PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
            }
        }
    }

    fn menu(&mut self) {
        self.close_popover(false);
        // SAFETY: menu is created/destroyed locally; nested messages use try_borrow_mut.
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return;
            }
            AppendMenuW(menu, MF_STRING, 1, w!("Ouvrir / Replier (clavier)"));
            AppendMenuW(menu, MF_STRING, 5, w!("Afficher l'encoche"));
            AppendMenuW(menu, MF_STRING, 6, w!("Masquer temporairement"));
            AppendMenuW(menu, MF_STRING, 7, w!("Recentrer et afficher"));
            AppendMenuW(menu, MF_STRING, 11, w!("Accueil / Modules"));
            AppendMenuW(
                menu,
                MF_STRING
                    | if self.settings.modules.launcher_enabled {
                        0
                    } else {
                        MF_GRAYED
                    },
                16,
                w!("Page Raccourcis"),
            );
            AppendMenuW(
                menu,
                MF_STRING,
                17,
                if self.settings.modules.launcher_enabled {
                    w!("Désactiver les raccourcis")
                } else {
                    w!("Activer les raccourcis")
                },
            );
            AppendMenuW(
                menu,
                MF_STRING
                    | if self.settings.modules.timer_enabled {
                        0
                    } else {
                        MF_GRAYED
                    },
                14,
                w!("Page Minuteurs / Pomodoro"),
            );
            AppendMenuW(
                menu,
                MF_STRING,
                15,
                if self.settings.modules.timer_enabled {
                    w!("Désactiver le module minuteurs (pause)")
                } else {
                    w!("Activer le module minuteurs")
                },
            );
            AppendMenuW(
                menu,
                MF_STRING
                    | if self.settings.modules.media_enabled {
                        0
                    } else {
                        MF_GRAYED
                    },
                12,
                w!("Page Médias"),
            );
            AppendMenuW(
                menu,
                MF_STRING
                    | if self.settings.modules.media_enabled {
                        0
                    } else {
                        MF_GRAYED
                    },
                10,
                w!("Sessions multimédias…"),
            );
            AppendMenuW(
                menu,
                MF_STRING,
                13,
                if self.settings.modules.media_enabled {
                    w!("Désactiver le module multimédia")
                } else {
                    w!("Activer le module multimédia")
                },
            );
            AppendMenuW(
                menu,
                MF_STRING,
                9,
                if self.concentrating() {
                    w!("Désactiver le mode concentration")
                } else {
                    w!("Activer le mode concentration")
                },
            );
            AppendMenuW(
                menu,
                MF_STRING,
                8,
                w!("Déplacer l'encoche — puis glisser à la souris"),
            );
            AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(menu, MF_STRING, 2, w!("Quitter"));
            let mut point = POINT::default();
            GetCursorPos(&mut point);
            self.remember_foreground();
            SetForegroundWindow(self.hwnd);
            let command = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON,
                point.x,
                point.y,
                0,
                self.hwnd,
                std::ptr::null(),
            );
            DestroyMenu(menu);
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            if !self.command(command as usize) && !self.transition.expanded {
                self.release_keyboard_focus(true);
            }
        }
    }

    fn command(&mut self, command: usize) -> bool {
        let result = match command {
            1 => {
                if self.concentrating() {
                    let result = self.show_compact(false).and_then(|_| {
                        if self.hidden {
                            Ok(())
                        } else {
                            self.toggle_explicit()
                        }
                    });
                    self.record(result);
                    return true;
                }
                if self.visibility.manual {
                    self.visibility.manual = false;
                    self.environment();
                }
                self.synchronize_visibility().and_then(|_| {
                    if self.hidden {
                        Ok(())
                    } else {
                        self.toggle_explicit()
                    }
                })
            }
            2 => {
                unsafe {
                    PostMessageW(self.hwnd, WM_CLOSE, 0, 0);
                }
                Ok(())
            }
            5 => self.show_compact(false),
            6 => {
                self.release_keyboard_focus(true);
                self.visibility.manual = true;
                self.synchronize_visibility()
            }
            7 => self.show_compact(true),
            8 => self.show_compact(false).map(|_| {
                if !self.hidden {
                    self.focus_for_explicit_open();
                    self.move_armed = true;
                }
            }),
            9 => self.set_concentration(!self.concentrating()),
            10 => {
                self.media_menu();
                Ok(())
            }
            11 | 12 | 14 | 16 => {
                let action = if command == 11 {
                    island_ui::navigation::Action::Home
                } else if command == 12 {
                    island_ui::navigation::Action::Media
                } else if command == 14 {
                    island_ui::navigation::Action::Timer
                } else {
                    island_ui::navigation::Action::OpenLauncher
                };
                let result = self.navigation_action(action);
                self.record(result);
                if !self.transition.expanded && !self.concentrating() && !self.hidden {
                    self.command(1);
                }
                return true;
            }
            13 => self.navigation_action(island_ui::navigation::Action::ToggleMedia),
            15 => self.navigation_action(island_ui::navigation::Action::ToggleTimer),
            17 => self.navigation_action(island_ui::navigation::Action::ToggleLauncher),
            _ => return false,
        };
        self.record(result);
        true
    }
}

impl Drop for App {
    fn drop(&mut self) {
        launcher_editor::close();
        drop(self.launcher.worker.take());
        self.close_popover(false);
        // SAFETY: hooks removed before state/window is released; all resources owned here.
        unsafe {
            KillTimer(self.hwnd, FRAME);
            KillTimer(self.hwnd, timer_view::WAKE);
            self.stop_media_timer();
            drop(self.media_worker.take());
            KillTimer(self.hwnd, SHELL_RECOVERY);
            drop(self.writer.take());
            if self.hotkey {
                UnregisterHotKey(self.hwnd, HOTKEY_ID);
            }
            if self.concentration_hotkey {
                UnregisterHotKey(self.hwnd, CONCENTRATION_HOTKEY_ID);
            }
            if GetCapture() == self.hwnd {
                ReleaseCapture();
            }
            if !self.mouse.is_null() {
                UnhookWindowsHookEx(self.mouse);
            }
            for hook in self.events.drain(..) {
                UnhookWinEvent(hook);
            }
            if !self.tray.hWnd.is_null() {
                Shell_NotifyIconW(NIM_DELETE, &self.tray);
            }
            if !self.icon.is_null() {
                DestroyIcon(self.icon);
            }
        }
    }
}

unsafe extern "system" fn mouse_hook(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: MSLLHOOKSTRUCT is valid only during HC_ACTION callback. Never consume input.
    unsafe {
        if code == HC_ACTION as i32
            && matches!(
                wp as u32,
                WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN
            )
        {
            let point = (*(lp as *const MSLLHOOKSTRUCT)).pt;
            APP.with(|cell| {
                if let Ok(state) = cell.try_borrow()
                    && let Some(app) = state.as_ref()
                    && distance(
                        (point.x - app.bounds.left) as f64,
                        (point.y - app.bounds.top) as f64,
                        (app.bounds.right - app.bounds.left) as f64,
                        (app.bounds.bottom - app.bounds.top) as f64,
                        app.radius,
                    ) > 0.0
                    && app.moving.is_none()
                    && !app.popup_contains(point)
                {
                    PostMessageW(app.hwnd, OUTSIDE, 0, 0);
                }
            });
        }
        CallNextHookEx(null_mut(), code, wp, lp)
    }
}

unsafe extern "system" fn win_event(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    // SAFETY: post only; no synchronous cross-process work inside the callback.
    unsafe {
        if event != EVENT_OBJECT_LOCATIONCHANGE
            || (object == OBJID_WINDOW && hwnd == GetForegroundWindow())
        {
            let target = WINDOW.load(Ordering::Relaxed) as HWND;
            if !target.is_null() {
                PostMessageW(target, ENVIRONMENT, 0, 0);
            }
        }
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: called by Windows for this class on the UI thread; RefCell prevents reentrant aliasing.
    unsafe {
        if matches!(msg, launcher::CHANGED | launcher_editor::SUBMIT) {
            LAUNCHER_PENDING.set(true);
            apply_pending_launcher();
            return 0;
        }
        if msg == WM_POWERBROADCAST || (msg == WM_TIMER && wp == timer_view::WAKE) {
            TIMER_PENDING.set(true);
            apply_pending_timer();
            return if msg == WM_POWERBROADCAST { 1 } else { 0 };
        }
        if msg == MEDIA_CHANGED {
            MEDIA_PENDING.set(true);
            apply_pending_media();
            return 0;
        }
        if msg != 0 && msg == SHELL_MESSAGE.get() {
            SHELL_PENDING.set(true);
            apply_pending_shell();
            return 0;
        }
        match msg {
            WM_CAPTURECHANGED | WM_CANCELMODE => {
                CAPTURE_LOST.set(true);
                apply_capture_lost();
                return 0;
            }
            WM_KEYDOWN if wp == VK_ESCAPE as usize && GetFocus() == hwnd => {
                // Accept only input delivered to this focused window. A native call can
                // reenter while APP is borrowed: one coalesced cancellation survives it.
                ESCAPE_PENDING.set(true);
                apply_pending_escape();
                return 0;
            }
            WM_MOUSEACTIVATE => return MA_NOACTIVATE as isize,
            WM_ERASEBKGND => return 1,
            WM_PAINT => {
                let mut paint = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut paint);
                EndPaint(hwnd, &paint);
                return 0;
            }
            WM_CLOSE => {
                PostQuitMessage(0);
                return 0;
            }
            WM_QUERYENDSESSION => return 1,
            WM_ENDSESSION if wp != 0 => {
                PostQuitMessage(0);
                return 0;
            }
            WM_DISPLAYCHANGE | WM_DPICHANGED | WM_SETTINGCHANGE => {
                PostMessageW(hwnd, ENVIRONMENT, 1, 0);
                return 0;
            }
            _ => {}
        }
        let handled = APP.with(|cell| {
            let Ok(mut state) = cell.try_borrow_mut() else {
                return false;
            };
            let Some(app) = state.as_mut() else {
                return false;
            };
            match msg {
                m if popover::is_event(m)=>{let result=app.popup_event(m,wp as HWND,lp);app.record(result);}
                WM_COMMAND if lp == 0 && wp >> 16 == 0 => return app.command(wp),
                WM_LBUTTONDOWN => {
                    app.suppress_release=false;
                    if app.popover.is_some() {app.close_popover(false);app.suppress_release=true;}
                    else if app.move_armed || GetKeyState(VK_CONTROL as i32)<0 {
                        let mut point=POINT::default();
                        if GetCursorPos(&mut point)!=0 { let result=app.begin_move([point.x,point.y]); app.record(result); }
                    } else {app.media_pointer(Some(lp),true);}
                }
                WM_MOUSEMOVE if app.moving.is_some() => {
                    let mut point=POINT::default();
                    if GetCursorPos(&mut point)!=0 {let result=app.move_pointer([point.x,point.y]);app.record(result);}
                }
                WM_SETCURSOR if app.move_armed || app.moving.is_some() => {SetCursor(LoadCursorW(null_mut(),IDC_SIZEALL));return true;}
                WM_MOUSEMOVE=>app.media_pointer(Some(lp),false),
                WM_MOUSELEAVE=>app.media_pointer(None,false),
                WM_LBUTTONUP => {
                    if app.moving.is_some() { let result=app.finish_move(true);app.record(result); }
                    else if !app.suppress_release && !app.media_click(lp) { let result=app.toggle_explicit();app.record(result); }
                    app.suppress_release=false;
                }
                OUTSIDE if app.moving.is_none() => {
                    app.move_armed=false;
                    app.release_keyboard_focus(false);
                    let result=app.set_expanded(false); app.record(result);
                }
                WM_HOTKEY if wp==HOTKEY_ID as usize => {app.command(if app.visibility.manual {5}else{6});}
                WM_HOTKEY if wp==CONCENTRATION_HOTKEY_ID as usize && app.concentration_hotkey => {app.command(9);}
                SAVE_FAILED => {app.mode_error=Some("Impossible de sauvegarder la position. Elle reste utilisable pour cette session.".into());}
                WM_TIMER if media_view::is_tick(wp) => {let result=app.media_tick();app.record(result);}
                WM_TIMER if wp==media_view::FEEDBACK_TIMER=>{let result=app.media_feedback_tick();app.record(result);}
                WM_KEYDOWN if GetFocus()==hwnd && app.settings.modules.launcher_visible() && matches!(wp as u16, VK_RETURN | VK_SPACE) && lp & (1 << 30) != 0 => {}
                WM_KEYDOWN if GetFocus()==hwnd && app.media_key(wp as u16) => {}
                WM_KEYDOWN if GetFocus() == hwnd => match wp as u16 {
                    VK_RETURN | VK_SPACE => {
                        let result = app.toggle_explicit();
                        app.record(result);
                    }
                    _ => return false,
                },
                WM_ACTIVATE => {
                    if wp as u16 == WA_INACTIVE as u16 {
                        app.return_focus = null_mut();
                        app.move_armed=false;
                        CAPTURE_LOST.set(true);
                    }
                    // DefWindowProc sets keyboard focus on activation. Do not swallow it.
                    return false;
                }
                WM_SETFOCUS | WM_KILLFOCUS => {
                    app.focused = msg == WM_SETFOCUS;
                    let result = app.frame();
                    app.record(result);
                }
                WM_TIMER if wp == FRAME => {
                    let result = app.frame();
                    app.record(result);
                }
                WM_TIMER if wp == SHELL_RECOVERY => app.restore_shell(),
                ENVIRONMENT => {
                    if wp == 1 {
                        app.close_popover(false);
                        let result = app.frame();
                        app.record(result);
                    }
                    app.environment();
                }
                WM_CONTEXTMENU => app.menu(),
                TRAY => match lp as u32 {
                    WM_RBUTTONUP | WM_CONTEXTMENU => app.menu(),
                    WM_LBUTTONUP => {
                        app.command(5);
                    }
                    _ => {}
                },
                _ => return false,
            }
            true
        });
        apply_pending_escape();
        apply_capture_lost();
        apply_pending_shell();
        apply_pending_media();
        apply_pending_timer();
        apply_pending_launcher();
        if handled {
            0
        } else {
            DefWindowProcW(hwnd, msg, wp, lp)
        }
    }
}

fn apply_pending_shell() {
    if !SHELL_PENDING.get() {
        return;
    }
    APP.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else {
            return;
        };
        if let Some(app) = state.as_mut() {
            SHELL_PENDING.set(false);
            app.schedule_shell_recovery();
        }
    });
}

fn apply_pending_media() {
    if !MEDIA_PENDING.get() {
        return;
    }
    APP.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut()
            && let Some(app) = state.as_mut()
        {
            MEDIA_PENDING.set(false);
            let result = app.media_changed();
            app.record(result);
        }
    });
}
fn apply_pending_launcher() {
    if !LAUNCHER_PENDING.get() {
        return;
    }
    APP.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut()
            && let Some(app) = state.as_mut()
        {
            LAUNCHER_PENDING.set(false);
            let result = app
                .launcher_save_editor()
                .and_then(|_| app.launcher_changed());
            app.record(result);
        }
    });
}

fn apply_pending_timer() {
    if !TIMER_PENDING.get() {
        return;
    }
    APP.with(|cell| {
        if let Ok(mut state) = cell.try_borrow_mut()
            && let Some(app) = state.as_mut()
        {
            // Reentrant native menus may dispatch power/timer messages while App is
            // borrowed. Coalesce one wake, then reconcile after the outer dispatch.
            TIMER_PENDING.set(false);
            let result = app.timer_tick();
            app.record(result);
        }
    });
}

fn apply_capture_lost() {
    if !CAPTURE_LOST.get() {
        return;
    }
    APP.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else {
            return;
        };
        CAPTURE_LOST.set(false);
        if let Some(app) = state.as_mut() {
            let result = app.finish_move(false);
            app.record(result);
        }
    });
}

fn show_pending_mode_error() {
    let message = APP.with(|cell| {
        cell.borrow_mut()
            .as_mut()
            .and_then(|app| app.mode_error.take())
    });
    if let Some(message) = message {
        show_error(&message);
    }
}

fn apply_pending_escape() {
    if !ESCAPE_PENDING.get() {
        return;
    }
    APP.with(|cell| {
        let Ok(mut state) = cell.try_borrow_mut() else {
            return; // The outer callback drains it after releasing its borrow.
        };
        ESCAPE_PENDING.set(false);
        if let Some(app) = state.as_mut() {
            if app.moving.is_some() || app.move_armed {
                app.move_armed = false;
                let result = app.finish_move(false);
                app.record(result);
                if !app.transition.expanded {
                    app.release_keyboard_focus(true);
                }
                return;
            }
            // Commit the reversal before an activation call can reenter or wait on
            // another window. Returning focus must not postpone the close request.
            let result = app.set_expanded(false);
            app.record(result);
            let was_focused = app.focused;
            app.release_keyboard_focus(true);
            if was_focused != app.focused && !app.transition.active() {
                let result = app.frame();
                app.record(result);
            }
        }
    });
}

unsafe fn make_icon() -> io::Result<HICON> {
    // SAFETY: 32x32 monochrome mask and BGRA pixels live through CreateIcon, which copies them.
    unsafe {
        let mask = [0u8; 128];
        let mut pixels = [0u32; 1024];
        for y in 0..32 {
            for x in 0..32 {
                if distance(x as f64, y as f64 - 8.0, 32.0, 16.0, 8.0) <= 0.0 {
                    pixels[y * 32 + x] = 0xffaeb1bc;
                }
            }
        }
        let icon = CreateIcon(
            GetModuleHandleW(null_mut()),
            32,
            32,
            1,
            32,
            mask.as_ptr(),
            pixels.as_ptr().cast(),
        );
        if icon.is_null() {
            Err(io::Error::last_os_error())
        } else {
            Ok(icon)
        }
    }
}

pub fn run() -> io::Result<()> {
    diagnostic("run entered");
    // SAFETY: entry point owns all windows/hooks on this thread. Cleanup precedes return.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let mutex = CreateMutexW(
            std::ptr::null(),
            0,
            w!("Local\\DynamicIsland.Native.Prototype"),
        );
        let mutex_error = GetLastError();
        diagnostic(&format!("mutex={mutex:?} last_error={mutex_error}"));
        if mutex.is_null() {
            return Err(io::Error::last_os_error());
        }
        if mutex_error == ERROR_ALREADY_EXISTS {
            CloseHandle(mutex);
            return Ok(());
        }
        let instance = GetModuleHandleW(null_mut());
        let class = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            lpszClassName: CLASS,
            hCursor: LoadCursorW(null_mut(), IDC_HAND),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            let e = io::Error::last_os_error();
            CloseHandle(mutex);
            return Err(e);
        }
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_TOPMOST,
            CLASS,
            w!("Dynamic Island — ouvrir ou replier"),
            WS_POPUP,
            0,
            0,
            island_ui::COMPACT.0,
            island_ui::COMPACT.1,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            let e = io::Error::last_os_error();
            UnregisterClassW(CLASS, instance);
            CloseHandle(mutex);
            return Err(e);
        }
        WINDOW.store(hwnd as isize, Ordering::Relaxed);
        diagnostic(&format!("window={hwnd:?}"));
        let result = match App::new(hwnd) {
            Ok(app) => {
                diagnostic("app created");
                let hidden = app.hidden;
                APP.with(|cell| *cell.borrow_mut() = Some(app));
                if !hidden {
                    ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
                apply_pending_shell();
                show_pending_mode_error();
                let mut message = MSG::default();
                let mut result = Ok(());
                loop {
                    let status = GetMessageW(&mut message, null_mut(), 0, 0);
                    if status == 0 {
                        diagnostic("WM_QUIT received");
                        break;
                    }
                    if status == -1 {
                        result = Err(io::Error::last_os_error());
                        break;
                    }
                    if launcher_editor::route(&message) {
                        continue;
                    }
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                    apply_pending_escape();
                    apply_capture_lost();
                    apply_pending_shell();
                    apply_pending_media();
                    show_pending_mode_error();
                }
                APP.with(|cell| {
                    if let Some(mut app) = cell.borrow_mut().take()
                        && let Some(error) = app.error.take()
                    {
                        result = Err(error);
                    }
                });
                result
            }
            Err(e) => Err(e),
        };
        diagnostic(&format!("run result={result:?}"));
        WINDOW.store(0, Ordering::Relaxed);
        ESCAPE_PENDING.set(false);
        CAPTURE_LOST.set(false);
        SHELL_PENDING.set(false);
        MEDIA_PENDING.set(false);
        SHELL_MESSAGE.set(0);
        DestroyWindow(hwnd);
        UnregisterClassW(CLASS, instance);
        CloseHandle(mutex);
        result
    }
}

pub fn show_error(message: &str) {
    let text: Vec<u16> = message.encode_utf16().chain(Some(0)).collect();
    // SAFETY: null terminated strings valid for synchronous MessageBoxW.
    unsafe {
        MessageBoxW(
            null_mut(),
            text.as_ptr(),
            w!("Dynamic Island — erreur"),
            MB_OK | MB_ICONERROR,
        );
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::*;

    #[test]
    fn tray_recovery_survives_reentrancy_and_stops_retrying() {
        let _guard = NATIVE_TEST_LOCK.lock().unwrap();
        // Only our hidden test HWND and temporary tray icon. No broadcast or Explorer restart.
        unsafe {
            let instance = GetModuleHandleW(null_mut());
            let class = w!("DynamicIsland.Test.TrayRecovery");
            let descriptor = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&descriptor), 0);
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
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
            let message = RegisterWindowMessageW(w!("TaskbarCreated"));
            assert_ne!(message, 0);
            let mut app = App::for_window(hwnd);
            app.hidden = true;
            app.visibility.manual = true;
            SHELL_MESSAGE.set(message);
            APP.with(|cell| *cell.borrow_mut() = Some(app));
            let outcome = std::panic::catch_unwind(|| {
                APP.with(|cell| {
                    let _borrow = cell.borrow_mut();
                    SendMessageW(hwnd, message, 0, 0);
                });
                SendMessageW(hwnd, WM_NULL, 0, 0);
                APP.with(|cell| {
                    assert!(
                        cell.borrow().as_ref().unwrap().shell_retry_pending,
                        "reentrant TaskbarCreated was lost"
                    )
                });
                // A second notification must coalesce rather than reset the recovery.
                SendMessageW(hwnd, message, 0, 0);
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    app.icon = make_icon().unwrap();
                    app.tray.cbSize = size_of::<NOTIFYICONDATAW>() as u32;
                    app.tray.hWnd = hwnd;
                    app.tray.uID = 73;
                    app.tray.uFlags = NIF_ICON | NIF_MESSAGE;
                    app.tray.hIcon = app.icon;
                    app.tray.uCallbackMessage = TRAY;
                    assert_ne!(Shell_NotifyIconW(NIM_ADD, &app.tray), 0);
                    assert_ne!(Shell_NotifyIconW(NIM_DELETE, &app.tray), 0);
                    assert_eq!(Shell_NotifyIconW(NIM_MODIFY, &app.tray), 0);
                });
                SendMessageW(hwnd, WM_TIMER, SHELL_RECOVERY, 0);
                APP.with(|cell| {
                    let state = cell.borrow();
                    let app = state.as_ref().unwrap();
                    assert!(!app.shell_retry_pending && app.mode_error.is_none());
                    assert_eq!(app.shell_attempts, 1);
                    assert_ne!(
                        Shell_NotifyIconW(NIM_MODIFY, &app.tray),
                        0,
                        "our removed icon must be restored"
                    );
                });
                SendMessageW(hwnd, WM_TIMER, SHELL_RECOVERY, 0);
                APP.with(|cell| assert_eq!(cell.borrow().as_ref().unwrap().shell_attempts, 1));
                // Simulate API rejection with invalid data, leaving Explorer untouched.
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    assert_ne!(Shell_NotifyIconW(NIM_DELETE, &app.tray), 0);
                    // cbSize=0 is accepted by some Shell versions; use a verified
                    // invalid owner HWND instead of relying on size validation.
                    app.tray.hWnd = 1usize as HWND;
                    assert_eq!(IsWindow(app.tray.hWnd), 0);
                    assert_eq!(Shell_NotifyIconW(NIM_ADD, &app.tray), 0);
                });
                SendMessageW(hwnd, message, 0, 0);
                for _ in 0..3 {
                    SendMessageW(hwnd, WM_TIMER, SHELL_RECOVERY, 0);
                }
                APP.with(|cell| {
                    let state = cell.borrow();
                    let app = state.as_ref().unwrap();
                    assert!(!app.shell_retry_pending && app.mode_error.is_some());
                    assert_eq!(app.shell_attempts, 3);
                    assert!(app.mouse.is_null());
                });
                // The existing outside-click hook must disappear on every collapse.
                // Hidden test HWND: no paint/input injection; hook exists only within this block.
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    for _ in 0..10 {
                        app.set_expanded(true).unwrap();
                        assert!(!app.mouse.is_null());
                        app.set_expanded(false).unwrap();
                        assert!(app.mouse.is_null());
                    }
                });
                SendMessageW(hwnd, WM_TIMER, SHELL_RECOVERY, 0);
                APP.with(|cell| assert_eq!(cell.borrow().as_ref().unwrap().shell_attempts, 3));
            });
            APP.with(|cell| drop(cell.borrow_mut().take()));
            SHELL_MESSAGE.set(0);
            SHELL_PENDING.set(false);
            DestroyWindow(hwnd);
            UnregisterClassW(class, instance);
            if let Err(panic) = outcome {
                std::panic::resume_unwind(panic);
            }
        }
    }

    #[test]
    fn native_activation_focus_and_local_escape() {
        let _guard = NATIVE_TEST_LOCK.lock().unwrap();
        // Hidden windows on this test thread only: no foreground request, global hook,
        // input injection, tray icon or user-window activation. Exercise the real wndproc.
        unsafe {
            let instance = GetModuleHandleW(null_mut());
            let class = w!("DynamicIsland.Test.KeyboardFocus");
            let descriptor = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&descriptor), 0);
            let hwnd = CreateWindowExW(
                WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW,
                class,
                class,
                WS_POPUP,
                0,
                0,
                176,
                34,
                null_mut(),
                null_mut(),
                instance,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let mut app = App::for_window(hwnd);
            app.hidden = true; // No drawing/timer/hook for this keyboard dispatch test.
            app.transition.set(true, 0.0, false);
            APP.with(|cell| *cell.borrow_mut() = Some(app));

            let outcome = std::panic::catch_unwind(|| {
                assert_eq!(
                    SendMessageW(hwnd, WM_MOUSEACTIVATE, 0, 0),
                    MA_NOACTIVATE as isize
                );
                SetFocus(null_mut());
                SendMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0);
                APP.with(|cell| assert!(cell.borrow().as_ref().unwrap().transition.expanded));

                // Power/wake messages dispatched by a nested native menu must be
                // retained, just like local Escape, rather than lost to RefCell.
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    app.timer
                        .model
                        .configure(Default::default(), island_core::timer::Mode::Pomodoro);
                    app.timer.model.state = island_core::timer::State::Running { deadline: 0 };
                    SendMessageW(hwnd, WM_POWERBROADCAST, 18, 0);
                    assert!(TIMER_PENDING.get());
                });
                SendMessageW(hwnd, WM_NULL, 0, 0);
                APP.with(|cell| {
                    let state = cell.borrow();
                    let app = state.as_ref().unwrap();
                    assert_eq!(app.timer.model.state, island_core::timer::State::Finished);
                    assert_eq!(app.timer.model.completed, 1);
                    assert!(app.timer.armed.is_none());
                });
                SendMessageW(hwnd, WM_TIMER, timer_view::WAKE, 0);
                APP.with(|cell| {
                    assert_eq!(cell.borrow().as_ref().unwrap().timer.model.completed, 1)
                });
                assert!(!TIMER_PENDING.get());

                // The previous handler swallowed this message: GetFocus remained NULL.
                SendMessageW(hwnd, WM_ACTIVATE, WA_ACTIVE as usize, 0);
                assert_eq!(
                    GetFocus(),
                    hwnd,
                    "activation must run default focus handling"
                );
                // Reproduce a key delivered by a nested Win32 callback while rendering
                // owns APP. It must survive until the outer callback can process it.
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    app.transition.set(false, 0.0, false);
                    app.transition.set(true, app.time(), true);
                    app.navigation.focus = Some(island_ui::navigation::Action::ToggleMedia);
                    SendMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0);
                });
                SendMessageW(hwnd, WM_TIMER, FRAME, 0);
                APP.with(|cell| {
                    assert!(
                        !cell.borrow().as_ref().unwrap().transition.expanded,
                        "Escape during a native callback must not be lost"
                    );
                    cell.borrow_mut()
                        .as_mut()
                        .unwrap()
                        .transition
                        .set(true, 0.0, false);
                });
                SendMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0);
                APP.with(|cell| {
                    let state = cell.borrow();
                    let app = state.as_ref().unwrap();
                    assert!(!app.transition.expanded, "focused Escape must collapse");
                    assert!(app.navigation.focus.is_none());
                    assert!(app.mouse.is_null());
                    assert!(app.error.is_none());
                });
                SetFocus(null_mut());
                APP.with(|cell| {
                    cell.borrow_mut()
                        .as_mut()
                        .unwrap()
                        .transition
                        .set(true, 0.0, false)
                });
                SendMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0);
                APP.with(|cell| assert!(cell.borrow().as_ref().unwrap().transition.expanded));
                SendMessageW(hwnd, WM_LBUTTONUP, 0, 0);
                APP.with(|cell| assert!(!cell.borrow().as_ref().unwrap().transition.expanded));
                // A stale recovery timer must not create a tray icon or reserve work area.
                SendMessageW(hwnd, WM_TIMER, SHELL_RECOVERY, 0);
                APP.with(|cell| {
                    let state = cell.borrow();
                    let app = state.as_ref().unwrap();
                    assert_eq!(app.shell_attempts, 0);
                    assert!(app.mode_error.is_none());
                });
                // Model an explicit drag without injecting input or activating any user window.
                SetFocus(hwnd);
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    let original = app.settings.clone();
                    app.moving = Some(Moving {
                        gesture: Drag {
                            start: [10, 10],
                            anchor: [300, 8],
                            moved: true,
                        },
                        original,
                    });
                    app.settings.position = Placement {
                        x: 900000,
                        y: 900000,
                    };
                });
                SendMessageW(hwnd, WM_KEYDOWN, VK_ESCAPE as usize, 0);
                APP.with(|cell| {
                    let state = cell.borrow();
                    let app = state.as_ref().unwrap();
                    assert!(app.moving.is_none());
                    assert_eq!(app.settings.position, Placement::default());
                    assert!(app.suppress_release);
                });
                SendMessageW(hwnd, WM_LBUTTONUP, 0, 0);
                APP.with(|cell| assert!(!cell.borrow().as_ref().unwrap().transition.expanded));
                APP.with(|cell| {
                    let mut state = cell.borrow_mut();
                    let app = state.as_mut().unwrap();
                    app.moving = Some(Moving {
                        gesture: Drag {
                            start: [10, 10],
                            anchor: [300, 8],
                            moved: true,
                        },
                        original: app.settings.clone(),
                    });
                    app.settings.position = Placement {
                        x: 900000,
                        y: 900000,
                    };
                    SendMessageW(hwnd, WM_CAPTURECHANGED, 0, 0); // Reentrant capture loss must survive.
                });
                apply_capture_lost();
                APP.with(|cell| {
                    let state = cell.borrow();
                    let app = state.as_ref().unwrap();
                    assert!(app.moving.is_none());
                    assert_eq!(app.settings.position, Placement::default());
                });
            });
            APP.with(|cell| drop(cell.borrow_mut().take()));
            DestroyWindow(hwnd);
            UnregisterClassW(class, instance);
            if let Err(panic) = outcome {
                std::panic::resume_unwind(panic);
            }
        }
    }
}
