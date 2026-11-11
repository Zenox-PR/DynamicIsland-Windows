use std::{io, mem::size_of};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*};
use windows_sys::{Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY, core::BOOL};

#[derive(Clone, Debug)]
pub struct Monitor {
    pub name: String,
    pub area: [i32; 4],
    pub primary: bool,
}

unsafe fn info(handle: HMONITOR) -> io::Result<Monitor> {
    let mut info = MONITORINFOEXW {
        monitorInfo: MONITORINFO {
            cbSize: size_of::<MONITORINFOEXW>() as u32,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: correctly sized writable structure, API doesn't retain the pointer.
    if unsafe { GetMonitorInfoW(handle, (&mut info as *mut MONITORINFOEXW).cast()) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let rect = |r: RECT| [r.left, r.top, r.right, r.bottom];
    Ok(Monitor {
        name: String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|v| *v == 0).unwrap_or(32)],
        ),
        area: rect(info.monitorInfo.rcWork),
        primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}
pub fn at(point: [i32; 2]) -> io::Result<Monitor> {
    // SAFETY: value-only monitor query.
    unsafe {
        info(MonitorFromPoint(
            POINT {
                x: point[0],
                y: point[1],
            },
            MONITOR_DEFAULTTONEAREST,
        ))
    }
}
pub fn find(name: &str) -> io::Result<Monitor> {
    let mut monitors = Vec::<Monitor>::new();
    unsafe extern "system" fn collect(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: EnumDisplayMonitors invokes synchronously; Vec outlives all calls.
        unsafe {
            if let Ok(m) = info(m) {
                (&mut *(data as *mut Vec<Monitor>)).push(m);
            }
        }
        1
    }
    // SAFETY: synchronous enumeration, no pointer retained.
    unsafe {
        EnumDisplayMonitors(
            std::ptr::null_mut(),
            std::ptr::null(),
            Some(collect),
            (&mut monitors as *mut Vec<Monitor>) as isize,
        );
    }
    monitors
        .iter()
        .find(|m| m.name == name)
        .or_else(|| monitors.iter().find(|m| m.primary))
        .cloned()
        .ok_or_else(|| io::Error::other("Aucun écran disponible"))
}
