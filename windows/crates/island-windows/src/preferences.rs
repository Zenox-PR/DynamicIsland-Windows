//! Small versioned local file. One sleeping writer coalesces saves; no polling.
use island_core::placement::Placement;
use std::{
    io,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};
use windows_sys::Win32::{
    Storage::FileSystem::{MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW},
    UI::Input::KeyboardAndMouse::*,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    pub timer: island_core::timer::Config,
    pub timer_mode: island_core::timer::Mode,
    pub modules: island_core::modules::Modules,
    pub monitor: String,
    pub position: Placement,
    pub shortcut: String,
    pub concentration_shortcut: String,
}
pub fn hotkey(text: &str) -> Result<Option<(u32, u32)>, &'static str> {
    if text.is_empty() || text == "none" {
        return Ok(None);
    }
    let mut parts: Vec<&str> = text.split('+').collect();
    let key = parts.pop().ok_or("Raccourci vide")?;
    let mut modifiers = 0;
    for p in parts {
        modifiers |= match p {
            "Ctrl" => MOD_CONTROL,
            "Alt" => MOD_ALT,
            "Shift" => MOD_SHIFT,
            _ => return Err("Modificateur invalide (Ctrl, Alt, Shift)"),
        };
    }
    if modifiers & (MOD_CONTROL | MOD_ALT) == 0 {
        return Err("Ctrl ou Alt requis");
    }
    let vk = if key.len() == 1 && key.as_bytes()[0].is_ascii_alphanumeric() {
        key.as_bytes()[0].to_ascii_uppercase() as u32
    } else if let Some(n) = key
        .strip_prefix('F')
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|n| (1..=11).contains(n))
    {
        VK_F1 as u32 + n - 1
    } else {
        return Err("Touche invalide (A-Z, 0-9, F1-F11)");
    };
    Ok(Some((modifiers | MOD_NOREPEAT, vk)))
}
impl Settings {
    pub fn parse(text: &str) -> io::Result<Self> {
        let invalid = || io::Error::new(io::ErrorKind::InvalidData, "Préférences invalides");
        let mut result = Self::default();
        let mut seen = std::collections::HashSet::new();
        let mut version = 0;
        for line in text.lines().filter(|l| !l.is_empty()) {
            let (key, value) = line.split_once('=').ok_or_else(invalid)?;
            if !seen.insert(key) {
                return Err(invalid());
            }
            match key {
                "version" if matches!(value, "1" | "2" | "3" | "4" | "5") => {
                    version = value.parse().unwrap()
                }
                "timer_enabled" => {
                    result.modules.timer_enabled = value.parse().map_err(|_| invalid())?
                }
                "launcher_enabled" => {
                    result.modules.launcher_enabled = value.parse().map_err(|_| invalid())?
                }
                "timer_minutes" => result.timer.minutes = value.parse().map_err(|_| invalid())?,
                "work_minutes" => result.timer.work = value.parse().map_err(|_| invalid())?,
                "short_minutes" => {
                    result.timer.short_break = value.parse().map_err(|_| invalid())?
                }
                "long_minutes" => result.timer.long_break = value.parse().map_err(|_| invalid())?,
                "timer_mode" => {
                    result.timer_mode = match value {
                        "timer" => island_core::timer::Mode::Timer,
                        "pomodoro" => island_core::timer::Mode::Pomodoro,
                        _ => return Err(invalid()),
                    }
                }
                "media_enabled" => {
                    result.modules.media_enabled = match value {
                        "true" => true,
                        "false" => false,
                        _ => return Err(invalid()),
                    }
                }
                "active_page" => {
                    result.modules.page = match value {
                        "home" => island_core::modules::Page::Home,
                        "media" => island_core::modules::Page::Media,
                        "timer" => island_core::modules::Page::Timer,
                        "launcher" => island_core::modules::Page::Launcher,
                        _ => return Err(invalid()),
                    }
                }
                "monitor" if value.len() <= 128 && !value.chars().any(char::is_control) => {
                    result.monitor = value.into()
                }
                "x" => result.position.x = value.parse().map_err(|_| invalid())?,
                "y" => result.position.y = value.parse().map_err(|_| invalid())?,
                "hotkey" => {
                    result.shortcut = if value == "none" {
                        String::new()
                    } else {
                        value.into()
                    }
                }
                "concentration_hotkey" => {
                    result.concentration_shortcut = if value == "none" {
                        String::new()
                    } else {
                        value.into()
                    };
                }
                _ => return Err(invalid()),
            }
        }
        let schema_valid = match version {
            1 => seen.len() == 5 && !seen.contains("concentration_hotkey"),
            2 => seen.len() == 6 && seen.contains("concentration_hotkey"),
            3 => {
                seen.len() == 8
                    && seen.contains("concentration_hotkey")
                    && seen.contains("media_enabled")
                    && seen.contains("active_page")
            }
            4 | 5 => {
                seen.len() == if version == 5 { 15 } else { 14 }
                    && (version != 5 || seen.contains("launcher_enabled"))
                    && [
                        "concentration_hotkey",
                        "media_enabled",
                        "active_page",
                        "timer_enabled",
                        "timer_mode",
                        "timer_minutes",
                        "work_minutes",
                        "short_minutes",
                        "long_minutes",
                    ]
                    .into_iter()
                    .all(|key| seen.contains(key))
            }
            _ => false,
        };
        if !schema_valid
            || !["version", "monitor", "x", "y", "hotkey"]
                .into_iter()
                .all(|key| seen.contains(key))
            || !result.timer.valid()
            || !result.position.valid()
            || hotkey(&result.shortcut).is_err()
            || hotkey(&result.concentration_shortcut).is_err()
        {
            return Err(invalid());
        }
        if version < 3 && (seen.contains("media_enabled") || seen.contains("active_page")) {
            return Err(invalid());
        }
        if version < 5
            && (seen.contains("launcher_enabled")
                || result.modules.page == island_core::modules::Page::Launcher)
        {
            return Err(invalid());
        }
        if version < 4
            && (seen.iter().any(|k| {
                k.starts_with("timer_")
                    || matches!(*k, "work_minutes" | "short_minutes" | "long_minutes")
            }) || result.modules.page == island_core::modules::Page::Timer)
        {
            return Err(invalid());
        }
        if (!result.modules.media_enabled
            && result.modules.page == island_core::modules::Page::Media)
            || (!result.modules.timer_enabled
                && result.modules.page == island_core::modules::Page::Timer)
            || (!result.modules.launcher_enabled
                && result.modules.page == island_core::modules::Page::Launcher)
        {
            result.modules.page = island_core::modules::Page::Home;
        }
        Ok(result)
    }
    fn text(&self) -> String {
        // Keep the reference version's format while no new shortcut is configured.
        // This makes rollback safe for the default case without rewriting user placement.
        let version = if !self.modules.launcher_enabled
            || self.modules.page == island_core::modules::Page::Launcher
        {
            5
        } else if self.timer != island_core::timer::Config::default()
            || self.timer_mode != island_core::timer::Mode::Timer
            || !self.modules.timer_enabled
            || self.modules.page == island_core::modules::Page::Timer
        {
            4
        } else if self.modules != island_core::modules::Modules::default() {
            3
        } else if self.concentration_shortcut.is_empty() {
            1
        } else {
            2
        };
        let mut text = format!(
            "version={version}\nmonitor={}\nx={}\ny={}\nhotkey={}\n",
            self.monitor,
            self.position.x,
            self.position.y,
            if self.shortcut.is_empty() {
                "none"
            } else {
                &self.shortcut
            }
        );
        if version >= 2 {
            text.push_str(&format!(
                "concentration_hotkey={}\n",
                if self.concentration_shortcut.is_empty() {
                    "none"
                } else {
                    &self.concentration_shortcut
                }
            ));
        }
        if version >= 3 {
            text.push_str(&format!(
                "media_enabled={}\nactive_page={}\n",
                self.modules.media_enabled,
                match self.modules.page {
                    island_core::modules::Page::Home => "home",
                    island_core::modules::Page::Media => "media",
                    island_core::modules::Page::Timer => "timer",
                    island_core::modules::Page::Launcher => "launcher",
                }
            ));
        }
        if version >= 4 {
            text.push_str(&format!("timer_enabled={}\ntimer_mode={}\ntimer_minutes={}\nwork_minutes={}\nshort_minutes={}\nlong_minutes={}\n", self.modules.timer_enabled, if self.timer_mode == island_core::timer::Mode::Timer { "timer" } else { "pomodoro" }, self.timer.minutes, self.timer.work, self.timer.short_break, self.timer.long_break));
        }
        if version == 5 {
            text.push_str(&format!(
                "launcher_enabled={}\n",
                self.modules.launcher_enabled
            ));
        }
        text
    }
}
pub fn path() -> io::Result<PathBuf> {
    if let Some(p) = std::env::var_os("ISLAND_SETTINGS_PATH") {
        return Ok(p.into());
    }
    std::env::var_os("LOCALAPPDATA")
        .map(|p| PathBuf::from(p).join("DynamicIsland/settings.ini"))
        .ok_or_else(|| io::Error::other("LOCALAPPDATA absent"))
}
pub fn load(path: &std::path::Path) -> io::Result<Settings> {
    match std::fs::metadata(path) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Settings::default()),
        Err(e) => return Err(e),
        Ok(m) if m.len() > 4096 => return Err(io::Error::other("Préférences trop volumineuses")),
        _ => {}
    }
    Settings::parse(&std::fs::read_to_string(path)?)
}
fn save(path: &std::path::Path, settings: &Settings) -> io::Result<()> {
    use std::io::Write;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("tmp");
    let mut file = std::fs::File::create(&temp)?;
    file.write_all(settings.text().as_bytes())?;
    file.sync_all()?;
    drop(file);
    use std::os::windows::ffi::OsStrExt;
    let from: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
    let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: NUL-terminated paths live through synchronous replacement.
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
#[derive(Default)]
struct Pending {
    value: Option<Settings>,
    stop: bool,
}
pub struct Writer {
    state: Arc<(Mutex<Pending>, Condvar)>,
    thread: Option<JoinHandle<()>>,
}
impl Writer {
    pub fn new(path: PathBuf, hwnd: isize) -> io::Result<Self> {
        let state = Arc::new((Mutex::new(Pending::default()), Condvar::new()));
        let copy = state.clone();
        let thread = std::thread::Builder::new()
            .name("island-settings".into())
            .spawn(move || {
                loop {
                    let (lock, wake) = &*copy;
                    let mut pending = lock.lock().unwrap();
                    while pending.value.is_none() && !pending.stop {
                        pending = wake.wait(pending).unwrap();
                    }
                    let value = pending.value.take();
                    let stop = pending.stop;
                    drop(pending);
                    if let Some(value) = value
                        && let Err(error) = save(&path, &value)
                    {
                        super::diagnostic(&format!("settings save failed: {error}"));
                        // SAFETY: owner joins this worker before destroying HWND; post carries no pointer.
                        unsafe {
                            windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                                hwnd as _,
                                super::SAVE_FAILED,
                                0,
                                0,
                            );
                        }
                    }
                    if stop {
                        break;
                    }
                }
            })?;
        Ok(Self {
            state,
            thread: Some(thread),
        })
    }
    pub fn submit(&self, settings: Settings) {
        let (lock, wake) = &*self.state;
        lock.lock().unwrap().value = Some(settings);
        wake.notify_one();
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        let (lock, wake) = &*self.state;
        lock.lock().unwrap().stop = true;
        wake.notify_one();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_preferences_and_shortcuts() {
        let s = Settings {
            monitor: "\\\\.\\DISPLAY2".into(),
            position: Placement {
                x: 123456,
                y: 789000,
            },
            shortcut: "Ctrl+Shift+F10".into(),
            concentration_shortcut: "Ctrl+Shift+F9".into(),
            ..Default::default()
        };
        assert_eq!(Settings::parse(&s.text()).unwrap(), s);
        for text in [
            "version=2",
            "version=1\nmonitor=\nx=1000001\ny=0\nhotkey=none",
            "version=1\nmonitor=\nx=0\ny=0\nhotkey=Escape",
            "version=1\nmonitor=\nx=0\ny=0\nhotkey=none\nx=1",
        ] {
            assert!(Settings::parse(text).is_err());
        }
        assert!(hotkey("Ctrl+F12").is_err());
        assert!(hotkey("Win+D").is_err());
        assert!(hotkey("Shift+A").is_err());
        assert_eq!(hotkey("none").unwrap(), None);
        let migrated = Settings::parse(
            "version=1\nmonitor=DISPLAY1\nx=123456\ny=789000\nhotkey=Ctrl+Shift+F10",
        )
        .unwrap();
        assert_eq!(migrated.position, s.position);
        assert_eq!(migrated.shortcut, s.shortcut);
        assert!(migrated.concentration_shortcut.is_empty());
        assert_eq!(Settings::parse(&migrated.text()).unwrap(), migrated);
        assert!(Settings::parse("version=2\nmonitor=\nx=0\ny=0\nhotkey=none").is_err());
    }
    #[test]
    fn modules_v3_migrate_and_roundtrip_without_changing_placement() {
        let original=Settings::parse("version=2\nmonitor=DISPLAY1\nx=123\ny=456\nhotkey=none\nconcentration_hotkey=Ctrl+F9\n").unwrap();
        assert_eq!(original.modules, island_core::modules::Modules::default());
        let mut settings = original.clone();
        settings
            .modules
            .set_enabled(island_core::modules::ModuleId::Media, false);
        let serialized = settings.text();
        assert!(serialized.starts_with("version=3\n"));
        assert_eq!(Settings::parse(&serialized).unwrap(), settings);
        assert_eq!(settings.position, original.position);
        assert_eq!(
            settings.concentration_shortcut,
            original.concentration_shortcut
        );
        assert_eq!(
            Settings::parse(&serialized.replace("active_page=home", "active_page=media"))
                .unwrap()
                .modules
                .page,
            island_core::modules::Page::Home
        );
        assert!(
            Settings::parse(&serialized.replace("media_enabled=false", "media_enabled=maybe"))
                .is_err()
        );
        assert!(
            Settings::parse(&serialized.replace("active_page=home", "active_page=unknown"))
                .is_err()
        );
        assert!(Settings::parse(&serialized.replace("version=3", "version=2")).is_err());
        settings.modules = island_core::modules::Modules::default();
        assert_eq!(settings.text(), original.text());
    }
    #[test]
    fn writer_flushes_latest_on_shutdown() {
        let path =
            std::env::temp_dir().join(format!("island-settings-test-{}.ini", std::process::id()));
        let writer = Writer::new(path.clone(), 0).unwrap();
        let mut last = Settings::default();
        for i in 0..100 {
            last.position.x = i;
            writer.submit(last.clone());
        }
        drop(writer);
        assert_eq!(load(&path).unwrap(), last);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn timer_v4_roundtrip_bounds_and_old_schema_migration() {
        use island_core::{modules::Page, timer::Mode};
        let mut s = Settings::parse("version=3\nmonitor=DISPLAY1\nx=123\ny=456\nhotkey=Ctrl+F8\nconcentration_hotkey=none\nmedia_enabled=false\nactive_page=home\n").unwrap();
        assert!(s.modules.timer_enabled);
        s.modules.page = Page::Timer;
        s.timer_mode = Mode::Pomodoro;
        s.timer.minutes = 240;
        s.timer.short_break = 1;
        let text = s.text();
        assert_eq!(Settings::parse(&text).unwrap(), s);
        assert_eq!(Settings::parse(&text).unwrap().modules.page, Page::Timer);
        assert_eq!(
            Settings::parse(&text.replace("timer_enabled=true", "timer_enabled=false"))
                .unwrap()
                .modules
                .page,
            Page::Home
        );
        for invalid in [
            text.replace("timer_minutes=240", "timer_minutes=241"),
            text.replace("short_minutes=1", "short_minutes=0"),
            text.replace("version=4", "version=3"),
            text.replace("timer_mode=pomodoro", "timer_mode=unknown"),
            text.replace("work_minutes=25\n", ""),
            text.replace("monitor=DISPLAY1\n", ""),
        ] {
            assert!(Settings::parse(&invalid).is_err());
        }
    }
    #[test]
    fn launcher_v5_preserves_previous_modules_and_position() {
        let mut s = Settings::default();
        s.modules.page = island_core::modules::Page::Launcher;
        s.timer.work = 42;
        s.position.x = 12345;
        let text = s.text();
        assert!(text.starts_with("version=5\n"));
        assert_eq!(Settings::parse(&text).unwrap(), s);
        assert!(Settings::parse(&text.replace("version=5", "version=4")).is_err());
        assert_eq!(
            Settings::parse(&text.replace("launcher_enabled=true", "launcher_enabled=false"))
                .unwrap()
                .modules
                .page,
            island_core::modules::Page::Home
        );
    }
}
