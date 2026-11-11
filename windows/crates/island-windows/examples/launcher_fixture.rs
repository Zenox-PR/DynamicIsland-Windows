//! Explicit-launch fixture only. Opens an owned window and closes it normally after 500 ms.
#![windows_subsystem = "windows"]
use std::ptr::null_mut;
use windows_sys::{
    Win32::{Foundation::*, System::LibraryLoader::*, UI::WindowsAndMessaging::*},
    w,
};
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                KillTimer(hwnd, 1);
                DestroyWindow(hwnd);
                0
            }
            WM_CLOSE => {
                DestroyWindow(hwnd);
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
fn mark(value: &str) {
    if let Some(path) = std::env::var_os("ISLAND_LAUNCH_FIXTURE_LOG") {
        use std::io::Write;
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = writeln!(file, "{value}");
        }
    }
}
fn main() {
    unsafe {
        let instance = GetModuleHandleW(null_mut());
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: w!("IslandLauncherFixture"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return;
        }
        let hwnd = CreateWindowExW(
            0,
            class.lpszClassName,
            w!("Test local du lanceur — fermeture automatique"),
            WS_OVERLAPPEDWINDOW,
            150,
            150,
            480,
            180,
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return;
        }
        ShowWindow(hwnd, SW_SHOWNORMAL);
        mark("started-visible");
        if SetTimer(hwnd, 1, 500, None) == 0 {
            DestroyWindow(hwnd);
        }
        let mut message = MSG::default();
        while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
            TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        UnregisterClassW(class.lpszClassName, instance);
        mark("normal-exit");
    }
}
