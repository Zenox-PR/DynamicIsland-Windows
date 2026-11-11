//! Shell application labels, resolved only on the existing COM-initialized media worker.
use island_core::media::{MAX_SESSIONS, bounded_text};
use std::collections::BTreeMap;
use windows::{
    Win32::{System::Com::CoTaskMemFree, UI::Shell::*},
    core::HSTRING,
};

#[derive(Default)]
pub(crate) struct Names(BTreeMap<String, String>);
impl Names {
    pub fn get(&mut self, source: &str) -> String {
        if let Some(name) = self.0.get(source) {
            return name.clone();
        }
        let name = shell_name(source).unwrap_or_else(|| fallback(source).into());
        if self.0.len() == MAX_SESSIONS {
            self.0.pop_first();
        }
        self.0.insert(source.into(), name.clone());
        name
    }
}
fn shell_name(source: &str) -> Option<String> {
    if source.is_empty() || source.contains('\0') {
        return None;
    }
    // SAFETY: called on the worker MTA; Shell owns the COM item. The returned
    // display string is copied and freed with the documented COM allocator.
    unsafe {
        let item: IShellItem = SHCreateItemInKnownFolder(
            &FOLDERID_AppsFolder,
            KF_FLAG_DEFAULT,
            &HSTRING::from(source),
        )
        .ok()?;
        let ptr = item.GetDisplayName(SIGDN_NORMALDISPLAY).ok()?;
        if ptr.is_null() {
            return None;
        }
        let value = ptr.to_string().ok();
        CoTaskMemFree(Some(ptr.0.cast()));
        let name = bounded_text(&value?, 96);
        (!name.is_empty() && name != source && !name.contains('!') && !name.contains('\\'))
            .then_some(name)
    }
}
fn fallback(source: &str) -> &'static str {
    // Recognized exact provider families only; no guessed executable paths or domains.
    if source
        .strip_prefix("OperaSoftware.OperaGXWebBrowser.")
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
    {
        return "Opera GX";
    }
    if source
        .strip_prefix("OperaSoftware.OperaStable.")
        .is_some_and(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
    {
        return "Opera";
    }
    match source {
        "Spotify.exe" => "Spotify",
        "Chrome" => "Google Chrome",
        "MSEdge" => "Microsoft Edge",
        _ => "Lecteur Windows",
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_identity_is_not_invented() {
        assert_eq!(
            fallback("OperaSoftware.OperaGXWebBrowser.1745167474"),
            "Opera GX"
        );
        assert_eq!(
            fallback("OperaSoftware.OperaGXWebBrowser.fake"),
            "Lecteur Windows"
        );
        assert_eq!(fallback("unknown.package!App"), "Lecteur Windows");
    }
}
