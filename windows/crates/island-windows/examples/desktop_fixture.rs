//! Local test window: real outside clicks and deterministic fullscreen transitions.
#![windows_subsystem = "windows"]
use std::{
    ptr::null_mut,
    sync::atomic::{AtomicU32, Ordering},
};
use windows_sys::{
    Win32::{
        Foundation::*, Graphics::Gdi::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*,
    },
    w,
};
static CLICKS: AtomicU32 = AtomicU32::new(0);
// Optional test setup, never used by the application executable.
static MEASURE_STATE: AtomicU32 = AtomicU32::new(0);
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: HWND and message data belong to this test window; no external process mutation.
    unsafe {
        match msg {
            WM_APP if wp == 7 => {
                // Test-only handshake: grant activation only to the known prototype
                // HWND's process, never ASFW_ANY and never from the shipped product.
                let island = FindWindowW(w!("DynamicIsland.Native.Prototype"), std::ptr::null());
                let mut pid = 0;
                if !island.is_null() {
                    GetWindowThreadProcessId(island, &mut pid);
                    if pid != 0 {
                        AllowSetForegroundWindow(pid);
                    }
                }
                0
            }
            WM_TIMER if wp == 2 => {
                KillTimer(hwnd, 2);
                let island = FindWindowW(w!("DynamicIsland.Native.Prototype"), std::ptr::null());
                let desired = MEASURE_STATE.load(Ordering::Relaxed);
                let mut bounds = RECT::default();
                if !island.is_null()
                    && IsWindowVisible(island) != 0
                    && GetWindowRect(island, &mut bounds) != 0
                {
                    let expanded = (bounds.bottom - bounds.top) * 3 > bounds.right - bounds.left;
                    if expanded != (desired == 1) {
                        PostMessageW(island, WM_LBUTTONUP, 0, 0);
                    }
                }
                0
            }
            WM_LBUTTONDOWN => {
                CLICKS.fetch_add(1, Ordering::Relaxed);
                InvalidateRect(hwnd, std::ptr::null(), 1);
                0
            }
            WM_KEYDOWN if wp == 0x7a => {
                let full = GetWindowLongW(hwnd, GWL_STYLE) as u32 & WS_CAPTION == 0;
                SetWindowLongW(
                    hwnd,
                    GWL_STYLE,
                    if full {
                        WS_OVERLAPPEDWINDOW | WS_VISIBLE
                    } else {
                        WS_POPUP | WS_VISIBLE
                    } as i32,
                );
                let (x, y, w, h) = if full {
                    (120, 120, 900, 550)
                } else {
                    (
                        0,
                        0,
                        GetSystemMetrics(SM_CXSCREEN),
                        GetSystemMetrics(SM_CYSCREEN),
                    )
                };
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    x,
                    y,
                    w,
                    h,
                    SWP_FRAMECHANGED | SWP_NOZORDER,
                );
                0
            }
            WM_KEYDOWN if wp == 27 => {
                DestroyWindow(hwnd);
                0
            }
            WM_PAINT => {
                let mut ps = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut ps);
                let mut r = RECT::default();
                GetClientRect(hwnd, &mut r);
                let s = format!(
                    "Banc de test local\n\nClics recus : {}\n\nF11 : plein ecran / fenetre\nEchap : fermer ce banc de test",
                    CLICKS.load(Ordering::Relaxed)
                );
                let text: Vec<u16> = s.encode_utf16().collect();
                r.top += 70;
                SetBkMode(dc, TRANSPARENT as i32);
                DrawTextW(dc, text.as_ptr(), text.len() as i32, &mut r, DT_CENTER);
                EndPaint(hwnd, &ps);
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wp, lp),
        }
    }
}
fn main() {
    // SAFETY: self-contained test process with native message loop and class lifetime.
    unsafe {
        let instance = GetModuleHandleW(null_mut());
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: w!("IslandTestFixture"),
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            ..Default::default()
        };
        RegisterClassW(&class);
        let hwnd = CreateWindowExW(
            0,
            class.lpszClassName,
            w!("Banc de test Dynamic Island"),
            WS_OVERLAPPEDWINDOW,
            120,
            120,
            900,
            550,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        ShowWindow(
            hwnd,
            if std::env::args().any(|a| a == "--maximized") {
                SW_SHOWMAXIMIZED
            } else {
                SW_SHOWNORMAL
            },
        );
        SetForegroundWindow(hwnd);
        if std::env::args().any(|a| a == "--fullscreen") {
            PostMessageW(hwnd, WM_KEYDOWN, 0x7a, 0);
        }
        // Programmatic setup of our own prototype only, no SendInput or global hooks.
        let desired = if std::env::args().any(|a| a == "--expanded") {
            1
        } else if std::env::args().any(|a| a == "--compact") {
            2
        } else {
            0
        };
        MEASURE_STATE.store(desired, Ordering::Relaxed);
        if desired != 0 {
            SetTimer(hwnd, 2, 700, None);
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        UnregisterClassW(class.lpszClassName, instance);
    }
}
