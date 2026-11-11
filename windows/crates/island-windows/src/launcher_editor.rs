//! Modeless native editor. Own controls/message routing; no global keyboard interception.
use super::*;
use island_core::launcher::{Entry, Kind};
use windows::{
    Win32::{
        Foundation::HWND as ComHwnd,
        System::Com::*,
        UI::Shell::{
            FOS_DONTADDTORECENT, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM, FOS_NOCHANGEDIR,
            FOS_NODEREFERENCELINKS, FOS_PATHMUSTEXIST, FOS_PICKFOLDERS, FileOpenDialog,
            IFileOpenDialog, SIGDN_FILESYSPATH,
        },
    },
    core::HSTRING,
};
use windows_sys::Win32::UI::Controls::EM_SETLIMITTEXT;
pub(super) const SUBMIT: u32 = WM_APP + 9;
const NAME: i32 = 100;
const TYPE: i32 = 101;
const TARGET: i32 = 102;
const FAVORITE: i32 = 103;
const BROWSE: i32 = 104;
const STATUS: i32 = 105;
struct Form {
    hwnd: HWND,
    parent: HWND,
    id: Option<u32>,
    font: HFONT,
    brush: HBRUSH,
}
thread_local! {
    static FORM: RefCell<Option<Form>> = const { RefCell::new(None) };
    static RESULT: RefCell<Option<(Option<u32>, Entry)>> = const { RefCell::new(None) };
}
pub(super) fn take() -> Option<(Option<u32>, Entry)> {
    RESULT.with(|s| s.borrow_mut().take())
}
fn window() -> HWND {
    FORM.with(|s| s.borrow().as_ref().map_or(null_mut(), |s| s.hwnd))
}
pub(super) fn close() {
    let hwnd = window();
    if !hwnd.is_null() {
        unsafe {
            DestroyWindow(hwnd);
        }
    }
    RESULT.with(|s| s.borrow_mut().take());
}
pub(super) fn route(message: &MSG) -> bool {
    let hwnd = window();
    !hwnd.is_null() && unsafe { IsDialogMessageW(hwnd, message) != 0 }
}
pub(super) fn status(text: &str, busy: bool) {
    let hwnd = window();
    if hwnd.is_null() {
        return;
    }
    let text = wide(text);
    unsafe {
        SetWindowTextW(GetDlgItem(hwnd, STATUS), text.as_ptr());
        EnableWindow(GetDlgItem(hwnd, IDOK), i32::from(!busy));
        for id in [NAME, TYPE, TARGET, FAVORITE, BROWSE] {
            EnableWindow(GetDlgItem(hwnd, id), i32::from(!busy));
        }
    }
}
fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn text(hwnd: HWND, id: i32) -> String {
    let mut buffer = [0u16; 2049];
    let count = unsafe {
        GetWindowTextW(
            GetDlgItem(hwnd, id),
            buffer.as_mut_ptr(),
            buffer.len() as i32,
        )
    };
    String::from_utf16_lossy(&buffer[..count.max(0) as usize])
}
pub(super) fn open(parent: HWND, entry: Option<&Entry>) -> io::Result<()> {
    create(parent, entry, true)
}
fn create(parent: HWND, entry: Option<&Entry>, visible: bool) -> io::Result<()> {
    let existing = window();
    if !existing.is_null() {
        unsafe {
            SetForegroundWindow(existing);
        }
        return Ok(());
    }
    // SAFETY: all child controls and graphics resources live until WM_NCDESTROY on this thread.
    unsafe {
        let instance = GetModuleHandleW(null_mut());
        let class = w!("DynamicIsland.ShortcutEditor");
        let wc = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: class,
            hCursor: LoadCursorW(null_mut(), IDC_ARROW),
            ..Default::default()
        };
        if RegisterClassW(&wc) == 0 && GetLastError() != ERROR_CLASS_ALREADY_EXISTS {
            return Err(io::Error::last_os_error());
        }
        let hwnd = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class,
            w!("Dynamic Island — Raccourci"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            560,
            360,
            // Independent normal window: an owned window would inherit the
            // island's topmost status and could cover another app/fullscreen.
            null_mut(),
            null_mut(),
            instance,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err(io::Error::last_os_error());
        }
        let brush = CreateSolidBrush(0x161211);
        let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
        let font = CreateFontW(
            -(14.0 * scale) as i32,
            0,
            0,
            0,
            400,
            0,
            0,
            0,
            DEFAULT_CHARSET as u32,
            0,
            0,
            ANTIALIASED_QUALITY as u32,
            0,
            w!("Segoe UI"),
        );
        FORM.with(|s| {
            *s.borrow_mut() = Some(Form {
                hwnd,
                parent,
                id: entry.map(|e| e.id),
                font,
                brush,
            })
        });
        if font.is_null() || brush.is_null() {
            close();
            return Err(io::Error::last_os_error());
        }
        let controls = [
            ("STATIC", "Nom", 200, 0),
            (
                "EDIT",
                "",
                NAME,
                WS_TABSTOP | ES_AUTOHSCROLL as u32 | WS_BORDER,
            ),
            ("STATIC", "Type", 201, 0),
            (
                "COMBOBOX",
                "",
                TYPE,
                WS_TABSTOP | CBS_DROPDOWNLIST as u32 | WS_VSCROLL,
            ),
            (
                "STATIC",
                "Cible : chemin, URL, AUMID ou identifiant d'outil",
                202,
                0,
            ),
            (
                "EDIT",
                "",
                TARGET,
                WS_TABSTOP | ES_AUTOHSCROLL as u32 | WS_BORDER,
            ),
            (
                "BUTTON",
                "Parcourir…",
                BROWSE,
                WS_TABSTOP | BS_PUSHBUTTON as u32,
            ),
            (
                "BUTTON",
                "Épingler aux favoris",
                FAVORITE,
                WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            ),
            (
                "STATIC",
                "Outils : codex, vscode, terminal, powershell, explorer",
                203,
                0,
            ),
            (
                "STATIC",
                "Aucune commande ni argument libre. Rien n'est ouvert à l'enregistrement.",
                STATUS,
                0,
            ),
            (
                "BUTTON",
                "Enregistrer",
                IDOK,
                WS_TABSTOP | BS_DEFPUSHBUTTON as u32,
            ),
            (
                "BUTTON",
                "Annuler",
                IDCANCEL,
                WS_TABSTOP | BS_PUSHBUTTON as u32,
            ),
        ];
        for (class, label, id, flags) in controls {
            let c = wide(class);
            let label = wide(label);
            let control = CreateWindowExW(
                0,
                c.as_ptr(),
                label.as_ptr(),
                WS_CHILD | WS_VISIBLE | flags,
                0,
                0,
                10,
                10,
                hwnd,
                id as usize as _,
                instance,
                std::ptr::null(),
            );
            if control.is_null() {
                close();
                return Err(io::Error::last_os_error());
            }
            SendMessageW(control, WM_SETFONT, font as usize, 1);
        }
        let combo = GetDlgItem(hwnd, TYPE);
        for k in Kind::ALL {
            let s = wide(k.label());
            SendMessageW(combo, CB_ADDSTRING, 0, s.as_ptr() as isize);
        }
        SendMessageW(GetDlgItem(hwnd, NAME), EM_SETLIMITTEXT, 48, 0);
        SendMessageW(GetDlgItem(hwnd, TARGET), EM_SETLIMITTEXT, 2048, 0);
        if let Some(e) = entry {
            SetWindowTextW(GetDlgItem(hwnd, NAME), wide(&e.name).as_ptr());
            SetWindowTextW(GetDlgItem(hwnd, TARGET), wide(&e.target).as_ptr());
            SendMessageW(
                combo,
                CB_SETCURSEL,
                Kind::ALL.iter().position(|k| *k == e.kind).unwrap(),
                0,
            );
            SendMessageW(
                GetDlgItem(hwnd, FAVORITE),
                BM_SETCHECK,
                usize::from(e.favorite),
                0,
            );
        } else {
            SendMessageW(combo, CB_SETCURSEL, 0, 0);
        }
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: (520.0 * scale) as i32,
            bottom: (320.0 * scale) as i32,
        };
        AdjustWindowRectExForDpi(
            &mut rect,
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            0,
            WS_EX_CONTROLPARENT,
            GetDpiForWindow(hwnd),
        );
        SetWindowPos(
            hwnd,
            null_mut(),
            0,
            0,
            rect.right - rect.left,
            rect.bottom - rect.top,
            SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        layout(hwnd);
        if visible {
            ShowWindow(hwnd, SW_SHOWNORMAL);
            SetForegroundWindow(hwnd);
            SetFocus(GetDlgItem(hwnd, NAME));
        }
    }
    Ok(())
}
unsafe fn layout(hwnd: HWND) {
    let scale = unsafe { GetDpiForWindow(hwnd).max(96) } as f64 / 96.0;
    for (id, x, y, w, h) in [
        (200, 20, 12, 480, 20),
        (NAME, 20, 34, 480, 26),
        (201, 20, 68, 480, 20),
        (TYPE, 20, 90, 480, 180),
        (202, 20, 124, 480, 20),
        (TARGET, 20, 146, 360, 26),
        (BROWSE, 390, 146, 110, 26),
        (FAVORITE, 20, 182, 480, 24),
        (203, 20, 214, 480, 20),
        (STATUS, 20, 240, 480, 40),
        (IDOK, 276, 286, 108, 28),
        (IDCANCEL, 392, 286, 108, 28),
    ] {
        unsafe {
            SetWindowPos(
                GetDlgItem(hwnd, id),
                null_mut(),
                (x as f64 * scale) as i32,
                (y as f64 * scale) as i32,
                (w as f64 * scale) as i32,
                (h as f64 * scale) as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}
fn browse(hwnd: HWND) -> Result<(), windows::core::Error> {
    let selected = unsafe { SendMessageW(GetDlgItem(hwnd, TYPE), CB_GETCURSEL, 0, 0) };
    if !matches!(selected, 0 | 1) {
        status(
            "Parcourir est disponible pour une application .exe ou un dossier.",
            false,
        );
        return Ok(());
    }
    // This dialog is explicit and modeless relative to App; its nested loop still services timers.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
        struct Com;
        impl Drop for Com {
            fn drop(&mut self) {
                unsafe {
                    CoUninitialize();
                }
            }
        }
        let _com = Com;
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        let mut options =
            FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR | FOS_DONTADDTORECENT | FOS_PATHMUSTEXIST;
        options |= if selected == 1 {
            FOS_PICKFOLDERS
        } else {
            FOS_FILEMUSTEXIST | FOS_NODEREFERENCELINKS
        };
        dialog.SetOptions(options)?;
        dialog.SetTitle(&HSTRING::from(if selected == 1 {
            "Choisir le dossier"
        } else {
            "Choisir une application .exe"
        }))?;
        if let Err(e) = dialog.Show(Some(ComHwnd(hwnd))) {
            if e.code().0 as u32 == 0x800704c7 {
                return Ok(());
            }
            return Err(e);
        }
        let item = dialog.GetResult()?;
        let raw = item.GetDisplayName(SIGDN_FILESYSPATH)?;
        let path = raw.to_string();
        CoTaskMemFree(Some(raw.0.cast()));
        SetWindowTextW(GetDlgItem(hwnd, TARGET), wide(&path?).as_ptr());
    }
    Ok(())
}
unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    // SAFETY: own HWND controls only. No pointer payload crosses into App or a worker.
    unsafe {
        match msg {
            WM_CLOSE => {
                DestroyWindow(hwnd);
                return 0;
            }
            WM_NCDESTROY => {
                let form = FORM.with(|s| s.borrow_mut().take());
                if let Some(form) = form {
                    DeleteObject(form.font);
                    DeleteObject(form.brush);
                }
                RESULT.with(|s| s.borrow_mut().take());
            }
            WM_DPICHANGED => {
                let r = &*(lp as *const RECT);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                layout(hwnd);
                let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.0;
                let font = CreateFontW(
                    -(14.0 * scale) as i32,
                    0,
                    0,
                    0,
                    400,
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    ANTIALIASED_QUALITY as u32,
                    0,
                    w!("Segoe UI"),
                );
                if !font.is_null() {
                    let old = FORM.with(|s| {
                        s.borrow_mut()
                            .as_mut()
                            .map(|s| std::mem::replace(&mut s.font, font))
                    });
                    for id in [
                        200, 201, 202, 203, NAME, TYPE, TARGET, FAVORITE, BROWSE, STATUS, IDOK,
                        IDCANCEL,
                    ] {
                        SendMessageW(GetDlgItem(hwnd, id), WM_SETFONT, font as usize, 1);
                    }
                    if let Some(old) = old {
                        DeleteObject(old);
                    } else {
                        DeleteObject(font);
                    }
                }
                return 0;
            }
            WM_SIZE => {
                layout(hwnd);
                return 0;
            }
            WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT => {
                SetTextColor(wp as _, 0xe8e8e8);
                SetBkColor(wp as _, 0x161211);
                return FORM.with(|s| s.borrow().as_ref().map_or(0, |s| s.brush as isize));
            }
            WM_ERASEBKGND => {
                let mut rect = RECT::default();
                GetClientRect(hwnd, &mut rect);
                FORM.with(|s| {
                    if let Some(s) = s.borrow().as_ref() {
                        FillRect(wp as _, &rect, s.brush);
                    }
                });
                return 1;
            }
            WM_COMMAND if wp >> 16 == BN_CLICKED as usize => {
                match wp as u16 as i32 {
                    IDCANCEL => {
                        DestroyWindow(hwnd);
                    }
                    BROWSE => {
                        if let Err(e) = browse(hwnd) {
                            status(&format!("Sélection indisponible : {e}"), false);
                        }
                    }
                    IDOK => {
                        if IsWindowEnabled(GetDlgItem(hwnd, IDOK)) == 0 {
                            return 0;
                        }
                        // SetWindowText (including the picker) can bypass EM_SETLIMITTEXT.
                        // Reject before reading into the bounded buffer, never truncate a target.
                        if GetWindowTextLengthW(GetDlgItem(hwnd, NAME)) > 48
                            || GetWindowTextLengthW(GetDlgItem(hwnd, TARGET)) > 2048
                        {
                            status("Nom ou cible trop long ; raccourcissez le texte.", false);
                            return 0;
                        }
                        let kind = SendMessageW(GetDlgItem(hwnd, TYPE), CB_GETCURSEL, 0, 0);
                        if let Some(kind) = Kind::ALL.get(kind as usize) {
                            let entry = Entry {
                                id: 0,
                                name: text(hwnd, NAME).trim().into(),
                                kind: *kind,
                                target: text(hwnd, TARGET).trim().into(),
                                favorite: SendMessageW(
                                    GetDlgItem(hwnd, FAVORITE),
                                    BM_GETCHECK,
                                    0,
                                    0,
                                ) != 0,
                            };
                            if let Err(e) = entry.validate() {
                                status(e, false);
                                return 0;
                            }
                            let destination =
                                FORM.with(|s| s.borrow().as_ref().map(|s| (s.parent, s.id)));
                            if let Some((parent, id)) = destination {
                                RESULT.with(|s| *s.borrow_mut() = Some((id, entry)));
                                status("Vérification et enregistrement…", true);
                                if PostMessageW(parent, SUBMIT, 0, 0) == 0 {
                                    status(
                                        "L'application ne peut pas recevoir la modification.",
                                        false,
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
                return 0;
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wp, lp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_editor_validates_and_releases_its_controls_and_fonts() {
        let _guard = NATIVE_TEST_LOCK.lock().unwrap();
        let entry = Entry {
            id: 7,
            name: "Essai".into(),
            kind: Kind::Application,
            target: r"C:\Projet\app.exe".into(),
            favorite: true,
        };
        // No foreground request or displayed window; genuine Win32 child controls.
        create(null_mut(), Some(&entry), false).unwrap();
        close();
        let resources = || unsafe { GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS) };
        let before = resources();
        for _ in 0..20 {
            create(null_mut(), Some(&entry), false).unwrap();
            let hwnd = window();
            assert!(unsafe { GetWindow(hwnd, GW_OWNER) }.is_null());
            assert_eq!(
                unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST,
                0
            );
            assert_eq!(text(hwnd, NAME), "Essai");
            unsafe {
                let oversized = format!("https://example.com/{}", "a".repeat(2048));
                SetWindowTextW(GetDlgItem(hwnd, TARGET), wide(&oversized).as_ptr());
                SendMessageW(hwnd, WM_COMMAND, IDOK as usize, 0);
                assert!(take().is_none());
                assert!(text(hwnd, STATUS).contains("trop long"));
                SetWindowTextW(
                    GetDlgItem(hwnd, TARGET),
                    wide(r"C:\Projet\script.ps1").as_ptr(),
                );
                SendMessageW(hwnd, WM_COMMAND, IDOK as usize, 0);
                assert!(take().is_none());
                assert!(text(hwnd, STATUS).contains(".exe"));
                SetWindowTextW(GetDlgItem(hwnd, TARGET), wide(&entry.target).as_ptr());
                SendMessageW(hwnd, WM_COMMAND, IDOK as usize, 0);
            }
            // NULL parent deliberately cannot receive a message; submitted data is still typed.
            let (id, saved) = take().unwrap();
            assert_eq!(id, Some(7));
            assert_eq!(saved.target, entry.target);
            close();
            assert!(window().is_null());
        }
        assert_eq!(resources(), before);
    }
}
