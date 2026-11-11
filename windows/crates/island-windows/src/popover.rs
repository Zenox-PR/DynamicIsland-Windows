//! Modeless owned layered popup. Input is posted to the existing UI loop, never polled.
use super::*;
use island_core::{
    media::Snapshot,
    selector::{Choice, Selector},
};
use island_ui::popover as layout;
const CLASS_NAME: windows_sys::core::PCWSTR = w!("DynamicIsland.MediaPopover");
pub(super) const EVENT: u32 = WM_APP + 20;
const MOVE: u32 = EVENT;
const DOWN: u32 = EVENT + 1;
const UP: u32 = EVENT + 2;
const KEY: u32 = EVENT + 3;
const LEAVE: u32 = EVENT + 4;
const CLOSE: u32 = EVENT + 5;
const TICK: u32 = EVENT + 6;
const WHEEL: u32 = EVENT + 7;
pub(super) fn is_event(message: u32) -> bool {
    (EVENT..=WHEEL).contains(&message)
}
pub(super) struct Popup {
    pub hwnd: HWND,
    pub bounds: RECT,
    pub return_focus: HWND,
    scale: f64,
    anchor: RECT,
    work: [i32; 4],
    model: Selector,
    snapshot: Snapshot,
    surface: Surface,
    fonts: Typography,
    fade: Option<Instant>,
}
impl Popup {
    fn new(owner: HWND, anchor: RECT, scale: f64, snapshot: &Snapshot) -> io::Result<Self> {
        let work = placement::at([anchor.left, anchor.top])?.area;
        let page = (((work[3] - work[1]) as f64 / scale - 70.0) / layout::ROW)
            .floor()
            .clamp(1.0, 6.0) as usize;
        let model = Selector::new(snapshot, page);
        let rows = model.choices.len().min(page);
        let surface = Surface::new(
            (layout::WIDTH * scale).round() as i32,
            (layout::height(rows) * scale).round() as i32,
        )?;
        let fonts = Typography::new(scale)?;
        // SAFETY: all resources belong to this UI thread. No state pointers stored in HWND.
        unsafe {
            let instance = GetModuleHandleW(null_mut());
            let wc = WNDCLASSW {
                lpfnWndProc: Some(proc),
                hInstance: instance,
                lpszClassName: CLASS_NAME,
                hCursor: LoadCursorW(null_mut(), IDC_ARROW),
                ..Default::default()
            };
            if RegisterClassW(&wc) == 0 {
                return Err(io::Error::last_os_error());
            }
            let hwnd = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                CLASS_NAME,
                w!("Dynamic Island — lecteurs"),
                WS_POPUP,
                anchor.left,
                anchor.top,
                1,
                1,
                owner,
                null_mut(),
                instance,
                std::ptr::null(),
            );
            if hwnd.is_null() {
                let error = io::Error::last_os_error();
                UnregisterClassW(CLASS_NAME, instance);
                return Err(error);
            }
            let mut popup = Self {
                hwnd,
                bounds: RECT::default(),
                return_focus: GetForegroundWindow(),
                scale,
                anchor,
                work,
                model,
                snapshot: snapshot.clone(),
                surface,
                fonts,
                fade: None,
            };
            popup.draw()?;
            popup.fade = media_view::animations_allowed().then(Instant::now);
            Ok(popup)
        }
    }
    pub fn update(&mut self, snapshot: &Snapshot) -> io::Result<()> {
        self.snapshot = snapshot.clone();
        self.model.update(snapshot);
        self.draw()
    }
    fn draw(&mut self) -> io::Result<()> {
        let size = (
            (layout::WIDTH * self.scale).round() as i32,
            (layout::height(self.model.choices.len().min(self.model.page)) * self.scale).round()
                as i32,
        );
        if self.surface.size() != size {
            self.surface = Surface::new(size.0, size.1)?;
        }
        let x = (self.anchor.left + (self.anchor.right - self.anchor.left - size.0) / 2)
            .clamp(self.work[0], (self.work[2] - size.0).max(self.work[0]));
        let below = self.anchor.bottom + (8.0 * self.scale).round() as i32;
        let y = if below + size.1 <= self.work[3] {
            below
        } else {
            self.anchor.top - size.1 - (8.0 * self.scale).round() as i32
        }
        .clamp(self.work[1], (self.work[3] - size.1).max(self.work[1]));
        self.bounds = RECT {
            left: x,
            top: y,
            right: x + size.0,
            bottom: y + size.1,
        };
        self.surface
            .draw_popover(self.scale, &self.fonts, &self.snapshot, &self.model)?;
        self.present()
    }
    fn present(&mut self) -> io::Result<()> {
        let t = self
            .fade
            .map_or(1.0, |t| (t.elapsed().as_secs_f64() / 0.12).clamp(0.0, 1.0));
        self.surface.present_alpha(
            self.hwnd,
            self.bounds.left,
            self.bounds.top,
            (255.0 * (t * t * (3.0 - 2.0 * t))).round() as u8,
        )?;
        if t >= 1.0 {
            self.fade = None;
            unsafe {
                KillTimer(self.hwnd, 1);
            }
        }
        Ok(())
    }
    fn hit(&self, lp: LPARAM) -> Option<Choice> {
        layout::hit(
            (lp as u16 as i16) as f64 / self.scale,
            ((lp >> 16) as u16 as i16) as f64 / self.scale,
            &self.model,
        )
    }
}
impl Drop for Popup {
    fn drop(&mut self) {
        // SAFETY: own HWND. Destruction messages only post a tagged event; no reentrant borrow.
        unsafe {
            KillTimer(self.hwnd, 1);
            if GetCapture() == self.hwnd {
                let _ = ReleaseCapture();
            }
            DestroyWindow(self.hwnd);
            UnregisterClassW(CLASS_NAME, GetModuleHandleW(null_mut()));
        }
    }
}
impl App {
    pub(super) fn open_popover(&mut self) -> io::Result<()> {
        self.close_popover(false);
        if self.hidden {
            diagnostic("media popover skipped: main window hidden");
            return Ok(());
        }
        let popup = Popup::new(
            self.hwnd,
            self.bounds,
            self.scale,
            &self.media_view.snapshot,
        )?;
        let hwnd = popup.hwnd;
        let animate = popup.fade.is_some();
        self.popover = Some(popup);
        // Explicit menu/button action only; never called for hover or a media event.
        unsafe {
            if let Some(p) = &mut self.popover {
                p.present()?;
            }
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
            SetFocus(hwnd);
            if GetForegroundWindow() != hwnd {
                diagnostic("media popover foreground request not granted");
                self.close_popover(false);
                return Ok(());
            }
            if animate
                && SetTimer(hwnd, 1, 15, None) == 0
                && let Some(p) = &mut self.popover
            {
                p.fade = None;
                p.present()?;
            }
        }
        Ok(())
    }
    pub(super) fn close_popover(&mut self, restore: bool) {
        if let Some(popup) = self.popover.take() {
            let previous = if popup.return_focus == self.hwnd && !self.transition.expanded {
                self.return_focus
            } else {
                popup.return_focus
            };
            let foreground = unsafe { GetForegroundWindow() == popup.hwnd };
            drop(popup);
            if restore && foreground && !previous.is_null() && unsafe { IsWindow(previous) != 0 } {
                unsafe {
                    SetForegroundWindow(previous);
                    if previous == self.hwnd {
                        SetFocus(previous);
                    }
                }
            }
        }
    }
    pub(super) fn popup_contains(&self, point: POINT) -> bool {
        self.popover.as_ref().is_some_and(|p| {
            distance(
                (point.x - p.bounds.left) as f64,
                (point.y - p.bounds.top) as f64,
                (p.bounds.right - p.bounds.left) as f64,
                (p.bounds.bottom - p.bounds.top) as f64,
                20.0 * p.scale,
            ) <= 0.0
        })
    }
    pub(super) fn popup_event(&mut self, msg: u32, source: HWND, lp: LPARAM) -> io::Result<()> {
        let Some(p) = &mut self.popover else {
            return Ok(());
        };
        if p.hwnd != source {
            return Ok(());
        }
        if msg == CLOSE {
            self.close_popover(lp != 0);
            return Ok(());
        }
        let mut selected = None;
        match msg {
            MOVE => {
                let hit = p.hit(lp);
                if hit == p.model.hover {
                    return Ok(());
                };
                p.model.hover = hit;
            }
            DOWN => {
                p.model.pressed = p.hit(lp);
                unsafe {
                    SetCapture(p.hwnd);
                }
            }
            UP => {
                selected = p.model.release(p.hit(lp));
                unsafe {
                    if GetCapture() == p.hwnd {
                        ReleaseCapture();
                    }
                }
            }
            LEAVE => {
                p.model.hover = None;
                p.model.pressed = None;
            }
            WHEEL => p.model.navigate(if lp > 0 { -1 } else { 1 }),
            KEY => match lp as u16 {
                VK_ESCAPE => {
                    self.close_popover(true);
                    return Ok(());
                }
                VK_UP => p.model.navigate(-1),
                VK_DOWN => p.model.navigate(1),
                VK_TAB => p.model.navigate(if lp & 0x10000 != 0 { -1 } else { 1 }),
                VK_HOME => {
                    p.model.focus = Choice::Automatic;
                    p.model.update(&p.snapshot);
                }
                VK_END => {
                    p.model.focus = Choice::Refresh;
                    p.model.update(&p.snapshot);
                }
                VK_PRIOR => p.model.navigate(-(p.model.page as isize)),
                VK_NEXT => p.model.navigate(p.model.page as isize),
                VK_RETURN | VK_SPACE => selected = Some(p.model.focus),
                _ => return Ok(()),
            },
            TICK => return p.present(),
            _ => return Ok(()),
        }
        p.draw()?;
        if let Some(choice) = selected {
            if let Some(worker) = &self.media_worker {
                match choice {
                    Choice::Automatic => worker.select(None),
                    Choice::Refresh => worker.refresh(),
                    Choice::Session(id) => {
                        if self.media_view.snapshot.sessions.iter().any(|s| s.id == id) {
                            worker.select(Some(id));
                        }
                    }
                }
            }
            if choice != Choice::Refresh {
                self.close_popover(true);
            }
        }
        Ok(())
    }
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: forwards scalar events only to our owner HWND; no borrowed Rust state.
    unsafe {
        let event = match msg {
            WM_MOUSEMOVE => {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                TrackMouseEvent(&mut t);
                Some((MOVE, lp))
            }
            WM_LBUTTONDOWN => Some((DOWN, lp)),
            WM_LBUTTONUP => Some((UP, lp)),
            WM_KEYDOWN if GetFocus() == hwnd => Some((
                KEY,
                wp as isize
                    | if GetKeyState(VK_SHIFT as i32) < 0 {
                        0x10000
                    } else {
                        0
                    },
            )),
            WM_MOUSELEAVE | WM_CAPTURECHANGED => Some((LEAVE, 0)),
            WM_CANCELMODE => {
                if GetCapture() == hwnd {
                    ReleaseCapture();
                }
                Some((CLOSE, 0))
            }
            WM_MOUSEWHEEL => Some((WHEEL, ((wp >> 16) as u16 as i16) as isize)),
            WM_ACTIVATE if wp as u16 == WA_INACTIVE as u16 => Some((CLOSE, 0)),
            WM_CLOSE => Some((CLOSE, 1)),
            WM_DPICHANGED | WM_DISPLAYCHANGE => Some((CLOSE, 0)),
            WM_TIMER => Some((TICK, 0)),
            WM_ERASEBKGND => return 1,
            WM_PAINT => {
                let mut p = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut p);
                EndPaint(hwnd, &p);
                return 0;
            }
            _ => None,
        };
        if let Some((message, data)) = event {
            let owner = GetWindow(hwnd, GW_OWNER);
            PostMessageW(owner, message, hwnd as usize, data);
            if msg != WM_ACTIVATE {
                return 0;
            }
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use island_core::media::{Playback, Session};
    fn snapshot() -> Snapshot {
        Snapshot {
            sessions: (1..=12)
                .map(|id| Session {
                    id,
                    source_name: format!("Lecteur de test {id}"),
                    title:
                        "Un titre volontairement très long pour vérifier les ellipses du sélecteur"
                            .into(),
                    playback: if id == 2 {
                        Playback::Playing
                    } else {
                        Playback::Paused
                    },
                    ..Default::default()
                })
                .collect(),
            selected: Some(2),
            ..Default::default()
        }
    }
    fn counts() -> (u32, u32) {
        unsafe {
            (
                GetGuiResources(GetCurrentProcess(), 0),
                GetGuiResources(GetCurrentProcess(), 1),
            )
        }
    }
    #[test]
    fn native_popup_lifetime_selection_removal_and_modes() {
        let _guard = NATIVE_TEST_LOCK.lock().unwrap();
        // Hidden test windows; no activation or global input generated by this test.
        unsafe {
            let owner = CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW,
                w!("STATIC"),
                w!("Popover test"),
                WS_POPUP,
                100,
                100,
                200,
                40,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null_mut()),
                std::ptr::null(),
            );
            assert!(!owner.is_null());
            let anchor = RECT {
                left: 100,
                top: 100,
                right: 300,
                bottom: 140,
            };
            let mut app = App::for_window(owner);
            app.hidden = true;
            app.media_view.snapshot = snapshot();
            let make = |s: &Snapshot| Popup::new(owner, anchor, 1.25, s).unwrap();
            drop(make(&app.media_view.snapshot)); // warm system caches once
            let before = counts();
            for _ in 0..100 {
                drop(make(&app.media_view.snapshot));
            }
            assert_eq!(
                counts(),
                before,
                "popup HWND/fonts/DIB/DC must all be released"
            );
            app.popover = Some(make(&app.media_view.snapshot));
            let h = app.popover.as_ref().unwrap().hwnd;
            let lp = ((130i32 as isize) << 16) | 40; // second row at 125%, source ID 1
            app.popup_event(DOWN, h, lp).unwrap();
            assert_eq!(
                app.popover.as_ref().unwrap().model.pressed,
                Some(Choice::Session(1))
            );
            app.media_view.snapshot.sessions.remove(0);
            app.popover
                .as_mut()
                .unwrap()
                .update(&app.media_view.snapshot)
                .unwrap();
            app.popup_event(UP, h, lp).unwrap();
            assert!(
                app.popover.is_some(),
                "removed row must never select its replacement"
            );
            app.popup_event(KEY, h, VK_END as isize).unwrap();
            assert_eq!(app.popover.as_ref().unwrap().model.focus, Choice::Refresh);
            assert!(app.popover.as_ref().unwrap().model.offset > 0);
            app.popup_event(KEY, h, VK_ESCAPE as isize).unwrap();
            assert!(app.popover.is_none());
            assert_eq!(IsWindow(h), 0);
            app.popover = Some(make(&app.media_view.snapshot));
            app.set_concentration(true).unwrap();
            assert!(app.popover.is_none());
            assert!(app.concentrating());
            drop(app);
            DestroyWindow(owner);
        }
    }
    #[test]
    fn popup_raster_scales_and_finite_fade() {
        let _guard = NATIVE_TEST_LOCK.lock().unwrap();
        // Hidden owned popup snapshots, no desktop interaction.
        unsafe {
            let owner = CreateWindowExW(
                0,
                w!("STATIC"),
                w!("Popup raster"),
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
            assert!(!owner.is_null());
            for scale in [1.0, 1.25, 1.5, 2.0] {
                let mut popup = Popup::new(
                    owner,
                    RECT {
                        left: 1700,
                        top: 900,
                        right: 1900,
                        bottom: 950,
                    },
                    scale,
                    &snapshot(),
                )
                .unwrap();
                assert!(popup.bounds.left >= popup.work[0] && popup.bounds.right <= popup.work[2]);
                assert!(popup.bounds.top >= popup.work[1] && popup.bounds.bottom <= popup.work[3]);
                popup.fade = Some(Instant::now() - std::time::Duration::from_millis(200));
                popup.present().unwrap();
                assert!(popup.fade.is_none());
                if scale == 1.25
                    && let Some(dir) = std::env::var_os("ISLAND_PREVIEW_DIR")
                {
                    std::fs::create_dir_all(&dir).unwrap();
                    popup
                        .surface
                        .save_preview(
                            &std::path::PathBuf::from(dir).join("popover-125.bmp"),
                            0xe9edf2,
                        )
                        .unwrap();
                }
            }
            DestroyWindow(owner);
        }
    }
}
