//! Bounded shortcut catalog. Data only: no shell commands, arguments, I/O or implicit launches.
pub const MAX_ENTRIES: usize = 32;
pub const MAX_FILE: usize = 96 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Application,
    Folder,
    Https,
    Packaged,
    Tool,
}
impl Kind {
    pub const ALL: [Self; 5] = [
        Self::Application,
        Self::Folder,
        Self::Https,
        Self::Packaged,
        Self::Tool,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Application => "Application (.exe)",
            Self::Folder => "Dossier local",
            Self::Https => "Lien HTTPS",
            Self::Packaged => "Application Windows (AUMID)",
            Self::Tool => "Outil connu",
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Self::Application => "exe",
            Self::Folder => "folder",
            Self::Https => "https",
            Self::Packaged => "app",
            Self::Tool => "tool",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: u32,
    pub name: String,
    pub kind: Kind,
    pub target: String,
    pub favorite: bool,
}
impl Entry {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty()
            || self.name.chars().count() > 48
            || self.name.chars().any(char::is_control)
        {
            return Err("Nom requis, 48 caractères maximum.");
        }
        if self.target.is_empty()
            || self.target.len() > 2048
            || self
                .target
                .chars()
                .any(|c| c.is_control() || matches!(c, '"' | '\u{202e}' | '\u{202d}'))
        {
            return Err("Cible invalide ou trop longue.");
        }
        match self.kind {
            Kind::Application | Kind::Folder => {
                if !local_path(&self.target) {
                    return Err(
                        "Choisissez un chemin local absolu, sans arguments ni flux alternatif.",
                    );
                }
                if self.kind == Kind::Application
                    && !self.target.to_ascii_lowercase().ends_with(".exe")
                {
                    return Err(
                        "Seules les applications .exe sont autorisées ; aucun script ni .lnk.",
                    );
                }
            }
            Kind::Https => {
                let Some(rest) = self.target.strip_prefix("https://") else {
                    return Err("Seuls les liens https:// sont autorisés.");
                };
                let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
                if host.is_empty()
                    || !host
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b':'))
                    || !host.bytes().any(|c| c.is_ascii_alphanumeric())
                    || self.target.chars().any(char::is_whitespace)
                    || self.target.contains('\\')
                {
                    return Err("Lien HTTPS invalide (sans identifiants intégrés).");
                }
            }
            Kind::Packaged => {
                let parts: Vec<_> = self.target.split('!').collect();
                if parts.len() != 2
                    || parts.iter().any(|p| p.is_empty())
                    || self.target.len() > 128
                    || !self.target.bytes().all(|c| {
                        c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'!')
                    })
                {
                    return Err("AUMID invalide : famille_de_package!Application attendu.");
                }
            }
            Kind::Tool => {
                if !["codex", "vscode", "terminal", "powershell", "explorer"]
                    .contains(&self.target.as_str())
                {
                    return Err("Outil connu : codex, vscode, terminal, powershell ou explorer.");
                }
            }
        }
        Ok(())
    }
}
pub fn local_path(path: &str) -> bool {
    let b = path.as_bytes();
    b.len() >= 3
        && b[0].is_ascii_alphabetic()
        && b[1] == b':'
        && b[2] == b'\\'
        && !path[2..].contains([':', '/', '"', '*', '?', '|', '<', '>'])
        && !path.chars().any(char::is_control)
        && path[3..]
            .split('\\')
            .all(|c| c != "." && c != ".." && !c.ends_with(['.', ' ']))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Catalog {
    pub entries: Vec<Entry>,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            entries: [
                ("Codex", Kind::Tool, "codex"),
                ("Visual Studio Code", Kind::Tool, "vscode"),
                ("GitHub", Kind::Https, "https://github.com"),
                ("Terminal Windows", Kind::Tool, "terminal"),
                ("PowerShell", Kind::Tool, "powershell"),
                ("Explorateur", Kind::Tool, "explorer"),
            ]
            .into_iter()
            .enumerate()
            .map(|(i, (name, kind, target))| Entry {
                id: i as u32 + 1,
                name: name.into(),
                kind,
                target: target.into(),
                favorite: i < 3,
            })
            .collect(),
        }
    }
}
impl Catalog {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.entries.len() > MAX_ENTRIES {
            return Err("32 raccourcis maximum.");
        }
        let mut ids = std::collections::HashSet::new();
        for e in &self.entries {
            e.validate()?;
            if e.id == 0 || !ids.insert(e.id) {
                return Err("Identifiant de raccourci invalide.");
            }
        }
        if self.text().len() > MAX_FILE {
            return Err("Catalogue trop volumineux.");
        }
        Ok(())
    }
    pub fn upsert(&mut self, id: Option<u32>, mut entry: Entry) -> Result<u32, &'static str> {
        entry.validate()?;
        if let Some(id) = id {
            let old = self
                .entries
                .iter_mut()
                .find(|e| e.id == id)
                .ok_or("Raccourci supprimé.")?;
            entry.id = id;
            *old = entry;
            Ok(id)
        } else {
            if self.entries.len() >= MAX_ENTRIES {
                return Err("32 raccourcis maximum.");
            }
            entry.id = self
                .entries
                .iter()
                .map(|e| e.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("Identifiants épuisés.")?;
            let id = entry.id;
            self.entries.push(entry);
            Ok(id)
        }
    }
    pub fn remove(&mut self, id: u32) {
        self.entries.retain(|e| e.id != id);
    }
    pub fn move_by(&mut self, id: u32, delta: isize) {
        if let Some(i) = self.entries.iter().position(|e| e.id == id) {
            let next = i.saturating_add_signed(delta).min(self.entries.len() - 1);
            self.entries.swap(i, next);
        }
    }
    pub fn visible(&self, favorites: bool) -> Vec<&Entry> {
        self.entries
            .iter()
            .filter(|e| !favorites || e.favorite)
            .collect()
    }
    pub fn text(&self) -> String {
        let hex = |s: &str| {
            s.as_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        };
        let mut text = "version=1\n".to_owned();
        for e in &self.entries {
            text.push_str(&format!(
                "{}|{}|{}|{}|{}\n",
                e.id,
                e.kind.key(),
                u8::from(e.favorite),
                hex(&e.name),
                hex(&e.target)
            ));
        }
        text
    }
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        if text.len() > MAX_FILE {
            return Err("Catalogue trop volumineux.");
        }
        let mut lines = text.lines();
        if lines.next() != Some("version=1") {
            return Err("Version de catalogue inconnue.");
        }
        let unhex = |s: &str| -> Result<String, &'static str> {
            if !s.len().is_multiple_of(2) || !s.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err("Encodage de catalogue invalide.");
            }
            let bytes = (0..s.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
                .collect();
            String::from_utf8(bytes).map_err(|_| "Texte de catalogue invalide.")
        };
        let mut result = Self {
            entries: Vec::new(),
        };
        for line in lines {
            let p: Vec<_> = line.split('|').collect();
            if p.len() != 5 || result.entries.len() == MAX_ENTRIES {
                return Err("Catalogue invalide ou plein.");
            }
            result.entries.push(Entry {
                id: p[0].parse().map_err(|_| "Identifiant invalide.")?,
                kind: Kind::ALL
                    .into_iter()
                    .find(|k| k.key() == p[1])
                    .ok_or("Type inconnu.")?,
                favorite: match p[2] {
                    "0" => false,
                    "1" => true,
                    _ => return Err("Favori invalide."),
                },
                name: unhex(p[3])?,
                target: unhex(p[4])?,
            });
        }
        result.validate()?;
        Ok(result)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn entry(kind: Kind, target: &str) -> Entry {
        Entry {
            id: 1,
            name: "Projet été | 工作".into(),
            kind,
            target: target.into(),
            favorite: true,
        }
    }
    #[test]
    fn rejects_scripts_arguments_unc_ads_and_protocols() {
        for target in [
            r"C:\run.ps1",
            r"C:\run.cmd",
            r"C:\run.bat",
            r"C:\run.lnk",
            "C:\\tool.exe --run",
            "\"C:\\tool.exe\"",
            r"\server\tool.exe",
            r"C:\a.exe:payload.exe",
            r"C:\..\tool.exe",
            r"C:\dir.\tool.exe",
            "tool.exe",
        ] {
            assert!(
                entry(Kind::Application, target).validate().is_err(),
                "{target}"
            );
        }
        assert!(
            entry(Kind::Application, r"C:\Program Files\Outil\outil.exe")
                .validate()
                .is_ok()
        );
        for target in [
            "file:///C:/x",
            "javascript:alert(1)",
            "https://u:p@example.org",
            "https://",
            "https://x\nrun",
            "https://x\\evil",
            "http://github.com",
        ] {
            assert!(entry(Kind::Https, target).validate().is_err());
        }
        assert!(
            entry(Kind::Https, "https://github.com/user/repo?tab=readme")
                .validate()
                .is_ok()
        );
        assert!(
            entry(
                Kind::Packaged,
                "Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"
            )
            .validate()
            .is_ok()
        );
        assert!(
            entry(Kind::Packaged, "shell:AppsFolder")
                .validate()
                .is_err()
        );
    }
    #[test]
    fn crud_order_favorites_and_encoding_roundtrip() {
        let mut c = Catalog::default();
        let id = c
            .upsert(None, entry(Kind::Folder, r"C:\Projets\Été"))
            .unwrap();
        c.move_by(id, -100);
        assert_eq!(c.entries[0].id, id);
        let mut e = c.entries[0].clone();
        e.name = "Modifié".into();
        e.favorite = false;
        c.upsert(Some(id), e).unwrap();
        assert!(!c.visible(true).iter().any(|e| e.id == id));
        assert_eq!(Catalog::parse(&c.text()).unwrap(), c);
        c.remove(id);
        assert!(
            c.upsert(Some(id), entry(Kind::Folder, r"C:\Projet"))
                .is_err()
        );
        let mut dup = Catalog::default();
        dup.entries.push(dup.entries[0].clone());
        assert!(Catalog::parse(&dup.text()).is_err());
        assert!(Catalog::parse("version=2\n").is_err());
        assert!(Catalog::parse("version=1\n1|exe|1|zz|ff\n").is_err());
        while c.entries.len() < MAX_ENTRIES {
            c.upsert(None, entry(Kind::Folder, r"C:\Projet")).unwrap();
        }
        assert!(c.upsert(None, entry(Kind::Folder, r"C:\Projet")).is_err());
    }
}
