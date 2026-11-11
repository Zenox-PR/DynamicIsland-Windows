//! Development harness restricted to our window class, not a general UI automation tool.
use windows_sys::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    },
    w,
};
fn main() {
    // SAFETY: only queries our own prototype window and posts its standard close message.
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hwnd = FindWindowW(w!("DynamicIsland.Native.Prototype"), std::ptr::null());
        println!(
            "monitors={} window_dpi={} primary={}x{}",
            GetSystemMetrics(SM_CMONITORS),
            GetDpiForWindow(hwnd),
            GetSystemMetrics(SM_CXSCREEN),
            GetSystemMetrics(SM_CYSCREEN)
        );
        let mut r = RECT::default();
        GetWindowRect(hwnd, &mut r);
        println!(
            "hwnd={hwnd:?} visible={} rect={},{},{},{}",
            IsWindowVisible(hwnd),
            r.left,
            r.top,
            r.right,
            r.bottom
        );
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if GetMonitorInfoW(
            MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY),
            &mut info,
        ) != 0
        {
            println!(
                "work={},{},{},{}",
                info.rcWork.left, info.rcWork.top, info.rcWork.right, info.rcWork.bottom
            );
        }
        let reservation = FindWindowW(w!("DynamicIsland.WorkspaceReservation"), std::ptr::null());
        if !reservation.is_null() {
            GetWindowRect(reservation, &mut r);
            println!(
                "appbar={reservation:?} dpi={} rect={},{},{},{}",
                GetDpiForWindow(reservation),
                r.left,
                r.top,
                r.right,
                r.bottom
            );
        } else {
            println!("appbar=none");
        }
        let fg = GetForegroundWindow();
        GetWindowRect(fg, &mut r);
        println!(
            "foreground={fg:?} zoomed={} style={:x} rect={},{},{},{}",
            IsZoomed(fg),
            GetWindowLongW(fg, GWL_STYLE),
            r.left,
            r.top,
            r.right,
            r.bottom
        );
        if std::env::args().any(|a| a == "close") && !hwnd.is_null() {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        if !hwnd.is_null() {
            for arg in std::env::args().skip(1) {
                let command = match arg.as_str() {
                    "toggle" => 1,
                    "show" => 5,
                    "hide" => 6,
                    "reset" => 7,
                    "move" => 8,
                    "concentration" => 9,
                    _ => continue,
                };
                PostMessageW(hwnd, WM_COMMAND, command, 0);
            }
        }
    }
}
