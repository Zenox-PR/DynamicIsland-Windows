//! Compile-time module catalog and navigation; no native resources or visibility changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModuleId {
    Media,
    Timer,
    Launcher,
}
pub struct Descriptor {
    pub id: ModuleId,
    pub label: &'static str,
}
pub const CATALOG: &[Descriptor] = &[
    Descriptor {
        id: ModuleId::Media,
        label: "Multimédia",
    },
    Descriptor {
        id: ModuleId::Timer,
        label: "Pomodoro / Minuteur",
    },
    Descriptor {
        id: ModuleId::Launcher,
        label: "Raccourcis",
    },
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    Home,
    #[default]
    Media,
    Timer,
    Launcher,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modules {
    pub media_enabled: bool,
    pub timer_enabled: bool,
    pub launcher_enabled: bool,
    pub page: Page,
}
impl Default for Modules {
    fn default() -> Self {
        Self {
            media_enabled: true,
            timer_enabled: true,
            launcher_enabled: true,
            page: Page::Media,
        }
    }
}
impl Modules {
    pub fn navigate(&mut self, page: Page) -> bool {
        if (page == Page::Media && !self.media_enabled)
            || (page == Page::Timer && !self.timer_enabled)
            || (page == Page::Launcher && !self.launcher_enabled)
        {
            return false;
        }
        let changed = self.page != page;
        self.page = page;
        changed
    }
    pub fn set_enabled(&mut self, id: ModuleId, enabled: bool) {
        match id {
            ModuleId::Media => self.media_enabled = enabled,
            ModuleId::Timer => self.timer_enabled = enabled,
            ModuleId::Launcher => self.launcher_enabled = enabled,
        }
        if (!self.media_enabled && self.page == Page::Media)
            || (!self.timer_enabled && self.page == Page::Timer)
            || (!self.launcher_enabled && self.page == Page::Launcher)
        {
            self.page = Page::Home;
        }
    }
    pub fn media_visible(&self) -> bool {
        self.media_enabled && self.page == Page::Media
    }
    pub fn timer_visible(&self) -> bool {
        self.timer_enabled && self.page == Page::Timer
    }
    pub fn launcher_visible(&self) -> bool {
        self.launcher_enabled && self.page == Page::Launcher
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disable_falls_back_without_selecting_an_unavailable_module() {
        let mut modules = Modules::default();
        assert!(modules.media_visible());
        modules.set_enabled(ModuleId::Media, false);
        assert_eq!(modules.page, Page::Home);
        assert!(!modules.navigate(Page::Media));
        modules.set_enabled(ModuleId::Media, true);
        assert_eq!(modules.page, Page::Home); // activation never opens a module implicitly
        assert!(modules.navigate(Page::Media));
        assert_eq!(CATALOG.len(), 3); // only implemented modules are advertised
        assert!(modules.navigate(Page::Timer));
        modules.set_enabled(ModuleId::Media, false);
        assert_eq!(modules.page, Page::Timer);
        modules.set_enabled(ModuleId::Timer, false);
        assert_eq!(modules.page, Page::Home);
        assert!(!modules.navigate(Page::Timer));
    }
}
