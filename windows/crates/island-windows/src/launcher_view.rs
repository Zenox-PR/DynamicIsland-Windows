//! Shortcut page coordinator. File access and launch APIs never execute on the UI thread.
use super::*;
use island_core::{
    launcher::{Catalog, Entry},
    modules::Page,
};
use island_ui::navigation::Action;
pub(super) struct View {
    pub catalog: Catalog,
    pub worker: Option<launcher::Worker>,
    pub busy: bool,
    pub loaded: bool,
    pub favorites: bool,
    pub offset: usize,
    pub notice: String,
    pub editing: bool,
}
impl Default for View {
    fn default() -> Self {
        Self {
            catalog: Catalog {
                entries: Vec::new(),
            },
            worker: None,
            busy: false,
            loaded: false,
            favorites: false,
            offset: 0,
            notice: "Raccourcis".into(),
            editing: false,
        }
    }
}
impl View {
    pub fn row(&self, index: u8) -> Option<&Entry> {
        self.catalog
            .visible(self.favorites)
            .get(self.offset + usize::from(index))
            .copied()
    }
    fn clamp(&mut self) {
        let count = self.catalog.visible(self.favorites).len();
        self.offset = self.offset.min(count.saturating_sub(1) / 3 * 3);
    }
}
impl App {
    pub(super) fn launcher_load(&mut self) -> io::Result<()> {
        if !self.settings.modules.launcher_enabled {
            return Ok(());
        }
        if self.launcher.worker.is_none() {
            self.launcher.worker = Some(launcher::Worker::new(self.hwnd as isize)?);
        }
        if !self.launcher.loaded && !self.launcher.busy {
            self.launcher_submit(launcher::Job::Load);
        }
        Ok(())
    }
    fn launcher_submit(&mut self, job: launcher::Job) {
        if self.launcher.busy {
            return;
        }
        self.launcher.busy = self.launcher.worker.as_ref().is_some_and(|w| w.submit(job));
        self.launcher.notice = if self.launcher.busy {
            "Opération en cours…"
        } else {
            "Indisponible ; réouvrez la page pour réessayer."
        }
        .into();
    }
    pub(super) fn launcher_changed(&mut self) -> io::Result<()> {
        if let Some(event) = self.launcher.worker.as_ref().and_then(|w| w.take()) {
            self.launcher.busy = false;
            match event {
                launcher::Event::Loaded(c) | launcher::Event::Saved(c) => {
                    diagnostic(&format!(
                        "launcher catalog available: {} entries",
                        c.entries.len()
                    ));
                    // Discard a press recorded before a catalog change: the same row
                    // may now represent a different application after delete/reorder.
                    self.navigation = Default::default();
                    self.launcher.catalog = c;
                    self.launcher.loaded = true;
                    self.launcher.clamp();
                    self.launcher.notice = "Raccourcis".into();
                    if self.launcher.editing {
                        launcher_editor::close();
                        self.launcher.editing = false;
                    }
                }
                launcher::Event::Launched => {
                    diagnostic("launcher explicit open succeeded");
                    self.launcher.notice = "Ouverture demandée".into();
                }
                launcher::Event::Error(error) => {
                    diagnostic(&format!("launcher operation failed: {error}"));
                    self.launcher.notice = error.clone();
                    if self.launcher.editing {
                        launcher_editor::status(&error, false);
                        self.launcher.editing = false;
                    }
                }
            }
            // A late result updates data only. Never expand, restore or focus the panel.
            if self.settings.modules.launcher_visible() && self.panel_ready() {
                self.render_key = None;
                self.frame()?;
            }
        }
        Ok(())
    }
    pub(super) fn launcher_save_editor(&mut self) -> io::Result<()> {
        if let Some((id, entry)) = launcher_editor::take() {
            if !self.settings.modules.launcher_enabled || self.launcher.busy {
                launcher_editor::status(
                    "Module indisponible ; réessayez après l'opération en cours.",
                    false,
                );
            } else {
                self.launcher.editing = true;
                self.launcher_submit(launcher::Job::Edit(
                    self.launcher.catalog.clone(),
                    id,
                    entry,
                ));
                if !self.launcher.busy {
                    self.launcher.editing = false;
                    launcher_editor::status(&self.launcher.notice, false);
                }
            }
        }
        Ok(())
    }
    pub(super) fn launcher_action(&mut self, action: Action) -> io::Result<()> {
        if !self.settings.modules.launcher_visible() || self.launcher.busy {
            return Ok(());
        }
        if !self.launcher.loaded {
            self.launcher_load()?;
            return Ok(());
        }
        match action {
            Action::AddShortcut => {
                self.set_expanded(false)?;
                launcher_editor::open(self.hwnd, None)?;
            }
            Action::FavoriteFilter => {
                self.launcher.favorites = !self.launcher.favorites;
                self.launcher.offset = 0;
            }
            Action::PreviousShortcuts => {
                self.launcher.offset = self.launcher.offset.saturating_sub(3)
            }
            Action::NextShortcuts => {
                self.launcher.offset += 3;
                self.launcher.clamp();
            }
            Action::Launch(row) => {
                if let Some(e) = self.launcher.row(row).cloned() {
                    self.launcher_submit(launcher::Job::Launch(e));
                }
            }
            Action::Pin(row) => {
                if let Some(e) = self.launcher.row(row) {
                    let id = e.id;
                    let mut c = self.launcher.catalog.clone();
                    if let Some(e) = c.entries.iter_mut().find(|e| e.id == id) {
                        e.favorite = !e.favorite;
                    }
                    self.launcher_submit(launcher::Job::Save(c));
                }
            }
            Action::EditShortcut(row) => {
                if let Some(e) = self.launcher.row(row).cloned() {
                    self.shortcut_menu(e)?;
                }
            }
            _ => {}
        }
        self.render_key = None;
        self.frame()
    }
    fn shortcut_menu(&mut self, entry: Entry) -> io::Result<()> {
        // SAFETY: owned synchronous menu with fixed commands; stable entry ID survives ordering.
        unsafe {
            let menu = CreatePopupMenu();
            if menu.is_null() {
                return Err(io::Error::last_os_error());
            }
            let items = [
                (1, w!("Modifier…")),
                (2, w!("Monter")),
                (3, w!("Descendre")),
                (4, w!("Supprimer le raccourci")),
            ];
            for (id, text) in items {
                if AppendMenuW(menu, MF_STRING, id, text) == 0 {
                    DestroyMenu(menu);
                    return Err(io::Error::last_os_error());
                }
            }
            let mut point = POINT::default();
            GetCursorPos(&mut point);
            let choice = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_NONOTIFY,
                point.x,
                point.y,
                0,
                self.hwnd,
                std::ptr::null(),
            );
            DestroyMenu(menu);
            PostMessageW(self.hwnd, WM_NULL, 0, 0);
            match choice {
                1 => {
                    self.set_expanded(false)?;
                    launcher_editor::open(self.hwnd, Some(&entry))?;
                }
                2..=4 => {
                    let mut c = self.launcher.catalog.clone();
                    if choice == 4 {
                        c.remove(entry.id);
                    } else {
                        c.move_by(entry.id, if choice == 2 { -1 } else { 1 });
                    }
                    self.launcher_submit(launcher::Job::Save(c));
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub(super) fn open_launcher_page(&mut self) -> io::Result<()> {
        self.settings.modules.navigate(Page::Launcher);
        self.launcher_load()
    }
}
