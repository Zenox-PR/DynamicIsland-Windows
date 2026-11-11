//! One on-demand STA worker for local catalog I/O and explicit launches. No discovery polling.
use island_core::launcher::{Catalog, Entry, Kind, MAX_FILE, local_path};
use std::{
    io::{self, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
    },
    thread::JoinHandle,
};
use windows::{
    Win32::{
        System::Com::*,
        UI::Shell::{AO_NOERRORUI, ApplicationActivationManager, IApplicationActivationManager},
    },
    core::{HSTRING, PCWSTR},
};
use windows_sys::Win32::{
    Foundation::*,
    Storage::FileSystem::*,
    System::Threading::*,
    UI::{Shell::*, WindowsAndMessaging::*},
};

pub(super) const CHANGED: u32 = WM_APP + 8;
pub(super) enum Job {
    Load,
    Save(Catalog),
    Edit(Catalog, Option<u32>, Entry),
    Launch(Entry),
}
pub(super) enum Event {
    Loaded(Catalog),
    Saved(Catalog),
    Launched,
    Error(String),
}
pub(super) struct Worker {
    tx: SyncSender<Option<Job>>,
    output: Arc<Mutex<Option<Event>>>,
    enabled: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    pub fn new(hwnd: isize) -> io::Result<Self> {
        let path = if let Some(p) = std::env::var_os("ISLAND_SHORTCUTS_PATH") {
            PathBuf::from(p)
        } else {
            super::preferences::path()?.with_file_name("shortcuts.ini")
        };
        let (tx, rx) = mpsc::sync_channel(1);
        let output = Arc::new(Mutex::new(None));
        let out = output.clone();
        let enabled = Arc::new(AtomicBool::new(true));
        let active = enabled.clone();
        let thread = std::thread::Builder::new()
            .name("island-launcher".into())
            .spawn(move || {
                // SAFETY: this worker alone owns its STA and every temporary COM object.
                let com = unsafe {
                    CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()
                };
                while let Ok(Some(job)) = rx.recv() {
                    let result = match job {
                        Job::Load => load(&path).map(Event::Loaded),
                        Job::Save(catalog) => save(&path, &catalog).map(|_| Event::Saved(catalog)),
                        Job::Edit(mut catalog, id, entry) => resolve(&entry).and_then(|_| {
                            catalog.upsert(id, entry).map_err(io::Error::other)?;
                            save(&path, &catalog)?;
                            Ok(Event::Saved(catalog))
                        }),
                        Job::Launch(entry) => {
                            if let Err(e) = &com {
                                Err(io::Error::other(format!(
                                    "Initialisation Windows indisponible : {e}"
                                )))
                            } else {
                                resolve(&entry).and_then(|target| {
                                    if !active.load(Ordering::Acquire) {
                                        return Err(io::Error::other(
                                            "Module désactivé ; ouverture annulée.",
                                        ));
                                    }
                                    execute(&target).map(|_| Event::Launched)
                                })
                            }
                        }
                    };
                    *out.lock().unwrap() = Some(result.unwrap_or_else(|e| {
                        Event::Error(e.to_string().chars().take(240).collect())
                    }));
                    // SAFETY: notification has no payload pointer; owner joins before HWND destruction.
                    unsafe {
                        PostMessageW(hwnd as _, CHANGED, 0, 0);
                    }
                }
                if com.is_ok() {
                    unsafe {
                        CoUninitialize();
                    }
                }
            })?;
        Ok(Self {
            tx,
            output,
            enabled,
            thread: Some(thread),
        })
    }
    pub fn submit(&self, job: Job) -> bool {
        self.tx.try_send(Some(job)).is_ok()
    }
    pub fn take(&self) -> Option<Event> {
        self.output.lock().unwrap().take()
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::Release);
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.enabled.store(false, Ordering::Release);
        let _ = self.tx.send(None); // flush an accepted save; never leave a detached thread
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn load(path: &Path) -> io::Result<Catalog> {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Catalog::default()),
        Err(e) => return Err(e),
    };
    let mut text = String::new();
    file.take(MAX_FILE as u64 + 1).read_to_string(&mut text)?;
    Catalog::parse(&text).map_err(io::Error::other)
}
fn save(path: &Path, catalog: &Catalog) -> io::Result<()> {
    catalog.validate().map_err(io::Error::other)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(catalog.text().as_bytes())?;
    file.sync_all()?;
    drop(file);
    use std::os::windows::ffi::OsStrExt;
    let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: owned NUL-terminated buffers; only our explicit catalog path is replaced.
    if unsafe {
        MoveFileExW(
            from.as_ptr(),
            to.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    } == 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
#[derive(Debug)]
enum Target {
    Exe(String, Option<&'static str>),
    Shell(String),
    App(String),
}
fn local_existing(target: &str, directory: bool) -> io::Result<String> {
    if !local_path(target) {
        return Err(io::Error::other("Chemin local absolu requis."));
    }
    let path = std::fs::canonicalize(target).map_err(|_| {
        io::Error::other("Cible introuvable ou inaccessible ; modifiez ce raccourci.")
    })?;
    let value = path
        .to_str()
        .ok_or_else(|| io::Error::other("Chemin Unicode non pris en charge."))?;
    let value = value.strip_prefix(r"\\?\").unwrap_or(value);
    if !local_path(value) {
        return Err(io::Error::other(
            "Une redirection vers le réseau ou un périphérique est refusée.",
        ));
    }
    let metadata = std::fs::metadata(&path)?;
    if (directory && !metadata.is_dir()) || (!directory && !metadata.is_file()) {
        return Err(io::Error::other("Le type de la cible a changé."));
    }
    if !directory && !value.to_ascii_lowercase().ends_with(".exe") {
        return Err(io::Error::other(
            "La cible résolue n'est pas une application .exe.",
        ));
    }
    Ok(value.into())
}
fn resolve(entry: &Entry) -> io::Result<Target> {
    entry.validate().map_err(io::Error::other)?;
    match entry.kind {
        Kind::Application => Ok(Target::Exe(local_existing(&entry.target, false)?, None)),
        Kind::Folder => Ok(Target::Shell(local_existing(&entry.target, true)?)),
        Kind::Https => Ok(Target::Shell(entry.target.clone())),
        Kind::Packaged => Ok(Target::App(entry.target.clone())),
        Kind::Tool => match entry.target.as_str() {
            "terminal" => Ok(Target::App(
                "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App".into(),
            )),
            "explorer" | "powershell" => {
                let mut system = [0u16; 32768];
                let n = unsafe {
                    windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW(
                        system.as_mut_ptr(),
                        system.len() as u32,
                    )
                } as usize;
                if n == 0 || n >= system.len() {
                    return Err(io::Error::last_os_error());
                }
                let system = PathBuf::from(String::from_utf16_lossy(&system[..n]));
                if entry.target == "powershell" {
                    Ok(Target::Exe(
                        local_existing(
                            &system
                                .join(r"WindowsPowerShell\v1.0\powershell.exe")
                                .to_string_lossy(),
                            false,
                        )?,
                        Some("-NoLogo -NoProfile"),
                    ))
                } else {
                    let path = system
                        .parent()
                        .ok_or_else(|| io::Error::other("Dossier Windows indisponible."))?
                        .join("explorer.exe");
                    Ok(Target::Exe(
                        local_existing(&path.to_string_lossy(), false)?,
                        None,
                    ))
                }
            }
            "vscode" | "codex" => {
                let relative = if entry.target == "vscode" {
                    r"Microsoft VS Code\Code.exe"
                } else {
                    r"Codex\Codex.exe"
                };
                let roots = [
                    std::env::var_os("LOCALAPPDATA").map(|p| PathBuf::from(p).join("Programs")),
                    std::env::var_os("ProgramFiles").map(PathBuf::from),
                ];
                for root in roots.into_iter().flatten() {
                    if let Ok(path) = local_existing(&root.join(relative).to_string_lossy(), false)
                    {
                        return Ok(Target::Exe(path, None));
                    }
                }
                Err(io::Error::other(
                    "Application non trouvée à son emplacement standard. Modifier : choisir son .exe ou son AUMID Windows.",
                ))
            }
            _ => Err(io::Error::other("Outil inconnu.")),
        },
    }
}
fn execute(target: &Target) -> io::Result<()> {
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    // SAFETY: all calls run on our initialized STA; buffers outlive synchronous APIs.
    // No user-supplied command-line, no shell interpreter, no hidden launch or elevation verb.
    unsafe {
        match target {
            Target::Exe(path, args) => {
                let application = wide(path);
                let mut command = wide(&format!(
                    "\"{path}\"{}",
                    args.map(|a| format!(" {a}")).unwrap_or_default()
                ));
                let directory = wide(
                    Path::new(path)
                        .parent()
                        .and_then(Path::to_str)
                        .ok_or_else(|| {
                            io::Error::other("Dossier de l'application indisponible.")
                        })?,
                );
                let startup = STARTUPINFOW {
                    cb: std::mem::size_of::<STARTUPINFOW>() as u32,
                    dwFlags: STARTF_USESHOWWINDOW,
                    wShowWindow: SW_SHOWNORMAL as u16,
                    ..Default::default()
                };
                let mut process = PROCESS_INFORMATION::default();
                if CreateProcessW(
                    application.as_ptr(),
                    command.as_mut_ptr(),
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    0,
                    std::ptr::null(),
                    directory.as_ptr(),
                    &startup,
                    &mut process,
                ) == 0
                {
                    return Err(io::Error::last_os_error());
                }
                CloseHandle(process.hThread);
                CloseHandle(process.hProcess);
            }
            Target::Shell(path) => {
                let target = wide(path);
                let mut info = SHELLEXECUTEINFOW {
                    cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
                    fMask: SEE_MASK_NOASYNC | SEE_MASK_FLAG_NO_UI,
                    lpVerb: windows_sys::w!("open"),
                    lpFile: target.as_ptr(),
                    nShow: SW_SHOWNORMAL,
                    ..Default::default()
                };
                if ShellExecuteExW(&mut info) == 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            Target::App(id) => {
                let manager: IApplicationActivationManager =
                    CoCreateInstance(&ApplicationActivationManager, None, CLSCTX_LOCAL_SERVER)
                        .map_err(io::Error::other)?;
                manager
                    .ActivateApplication(&HSTRING::from(id), PCWSTR::null(), AO_NOERRORUI)
                    .map_err(|e| {
                        io::Error::other(format!(
                            "Application Windows absente ou activation refusée : {e}"
                        ))
                    })?;
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn missing_target_never_falls_back_to_path_or_a_shell() {
        let e = Entry {
            id: 1,
            name: "Test".into(),
            kind: Kind::Application,
            target: r"C:\DynamicIslandMissingTest\missing.exe".into(),
            favorite: false,
        };
        assert!(resolve(&e).is_err());
        let mut script = e;
        script.target = r"C:\test.ps1".into();
        assert!(resolve(&script).is_err());
        assert!(local_existing(r"\\server\share", true).is_err());
    }
    #[test]
    fn catalog_atomic_roundtrip_and_invalid_file_preserved() {
        let dir = std::env::temp_dir().join(format!("island-launcher-test-{}", std::process::id()));
        let path = dir.join("shortcuts.ini");
        let catalog = Catalog::default();
        save(&path, &catalog).unwrap();
        assert_eq!(load(&path).unwrap(), catalog);
        std::fs::write(&path, "invalid").unwrap();
        assert!(load(&path).is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "invalid");
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();
    }
}
