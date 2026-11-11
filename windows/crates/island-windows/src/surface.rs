use std::{io, mem::size_of, ptr::null_mut};
use windows_sys::Win32::{Foundation::*, Graphics::Gdi::*, UI::WindowsAndMessaging::*};

/// Cached per DPI, independent of the DIB resized during an animation.
pub struct Typography {
    pub scale: f64,
    fonts: [HFONT; 4],
    text: [Vec<u16>; 4],
}
impl Typography {
    pub fn new(scale: f64) -> io::Result<Self> {
        let mut cache = Self {
            scale,
            fonts: [null_mut(); 4],
            text: island_ui::LABELS.map(|s| s.encode_utf16().collect()),
        };
        // SAFETY: created fonts are owned by this cache, never retained selected in a DC.
        unsafe {
            for (index, size) in island_ui::FONT_SIZES.into_iter().enumerate() {
                cache.fonts[index] = CreateFontW(
                    -(size * scale).round() as i32,
                    0,
                    0,
                    0,
                    if index == 0 { 600 } else { 400 },
                    0,
                    0,
                    0,
                    DEFAULT_CHARSET as u32,
                    0,
                    0,
                    ANTIALIASED_QUALITY as u32,
                    0,
                    windows_sys::w!("Segoe UI"),
                );
                if cache.fonts[index].is_null() {
                    return Err(io::Error::last_os_error());
                }
            }
        }
        Ok(cache)
    }
}
impl Drop for Typography {
    fn drop(&mut self) {
        // SAFETY: all draws restore the original font before returning; cache owns these.
        unsafe {
            for font in self.fonts {
                if !font.is_null() {
                    DeleteObject(font);
                }
            }
        }
    }
}

/// Owns a memory DC and its selected DIB. Not Send: used only by the UI thread.
pub struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    pixels: *mut u32,
    width: i32,
    height: i32,
}

impl Surface {
    pub fn new(width: i32, height: i32) -> io::Result<Self> {
        // SAFETY: all pointers refer to initialized structures; ownership is retained here.
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            if dc.is_null() {
                return Err(io::Error::last_os_error());
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width,
                    biHeight: -height,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, null_mut(), 0);
            if bitmap.is_null() {
                let error = io::Error::last_os_error();
                DeleteDC(dc);
                return Err(error);
            }
            let previous = SelectObject(dc, bitmap);
            if previous.is_null() {
                let error = io::Error::last_os_error();
                DeleteObject(bitmap);
                DeleteDC(dc);
                return Err(error);
            }
            Ok(Self {
                dc,
                bitmap,
                previous,
                pixels: bits.cast(),
                width,
                height,
            })
        }
    }

    pub fn draw_concentration(&mut self) {
        // SAFETY: this owned DIB contains exactly width*height u32 pixels; no GDI text draw.
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize)
        };
        island_ui::paint_concentration(pixels, self.size());
    }

    pub fn draw_media(
        &mut self,
        radius: f64,
        scale: f64,
        focused: bool,
        typography: &Typography,
        view: &super::media_view::View,
    ) -> io::Result<()> {
        use island_core::media::{Playback, clock};
        let empty = island_core::media::Session::default();
        let available = view.snapshot.active().is_some();
        let session = view.snapshot.active().unwrap_or(&empty);
        let position = session.timeline.as_ref().and_then(|t| {
            t.position_at(
                std::time::Instant::now(),
                session.playback == Playback::Playing,
            )
        });
        // SAFETY: owned DIB; no pending GDI accesses between CPU raster passes.
        unsafe {
            let pixels =
                std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize);
            island_ui::paint(pixels, self.size(), radius, scale, focused, 0.0);
            // Replace the normal compact handle with the media source header.
            for y in (10.0 * scale) as i32..(22.0 * scale).ceil() as i32 {
                for x in (150.0 * scale) as i32..(234.0 * scale).ceil() as i32 {
                    pixels[(y * self.width + x) as usize] = 0xff000000 | island_ui::BACKGROUND;
                }
            }
            island_ui::media::paint(
                pixels,
                self.size(),
                scale,
                session,
                position,
                view.feedback(),
                view.busy,
            );
        }
        let title = if !available {
            "Aucun média actif"
        } else if session.title.is_empty() {
            "Titre indisponible"
        } else {
            &session.title
        };
        let secondary = if !available {
            "Lancez un lecteur compatible"
        } else if !session.artist.is_empty() {
            session.artist.as_str()
        } else if !session.album.is_empty() {
            session.album.as_str()
        } else {
            session.kind.as_deref().unwrap_or("Média Windows")
        };
        let source = if session.source_name.is_empty() {
            "Médias Windows"
        } else {
            &session.source_name
        };
        let status = if view.busy {
            "Commande en cours…".to_string()
        } else if let Some(notice) = &view.snapshot.notice {
            notice.clone()
        } else if !available {
            "Échap ou clic pour replier".into()
        } else if let (Some(pos), Some(t)) = (position, &session.timeline) {
            format!("{}  /  {}", clock(pos), clock(t.duration))
        } else {
            match session.playback {
                Playback::Playing => "En lecture · durée indisponible",
                Playback::Paused => "En pause",
                Playback::Stopped => "Arrêté",
                Playback::Unknown => "État indisponible",
            }
            .into()
        };
        for (index, text) in [title, secondary, source, &status].into_iter().enumerate() {
            self.label(
                typography,
                index,
                text,
                island_ui::media::text_rect(index, scale),
            )?;
        }
        Ok(())
    }

    pub(super) fn draw_home(
        &mut self,
        radius: f64,
        scale: f64,
        focused: bool,
        fonts: &Typography,
        modules: island_core::modules::Modules,
    ) -> io::Result<()> {
        self.draw(radius, scale, 0.0, focused, fonts)?;
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize)
        };
        // The normal compact handle is replaced by the navigation in expanded pages.
        for y in (10.0 * scale) as i32..(22.0 * scale).ceil() as i32 {
            for x in (150.0 * scale) as i32..(234.0 * scale).ceil() as i32 {
                pixels[(y * self.width + x) as usize] = 0xff000000 | island_ui::BACKGROUND;
            }
        }
        island_ui::theme::rounded(
            pixels,
            self.size(),
            [16.0, 76.0, 368.0, 103.0].map(|n| n * scale),
            12.0 * scale,
            island_ui::theme::SURFACE,
        );
        island_ui::theme::rounded(
            pixels,
            self.size(),
            [16.0, 106.0, 368.0, 133.0].map(|n| n * scale),
            12.0 * scale,
            island_ui::theme::SURFACE,
        );
        let rect = |r| island_ui::navigation::text_rect(r, scale);
        self.label(
            fonts,
            if modules.media_enabled { 1 } else { 3 },
            island_core::modules::CATALOG[0].label,
            rect([26.0, 79.0, 224.0, 100.0]),
        )?;
        self.label(
            fonts,
            if modules.timer_enabled { 1 } else { 3 },
            island_core::modules::CATALOG[1].label,
            rect([26.0, 109.0, 224.0, 130.0]),
        )?;
        self.label(
            fonts,
            3,
            "Choisissez les modules à garder actifs.",
            rect([24.0, 138.0, 360.0, 156.0]),
        )
    }
    pub(super) fn draw_navigation(
        &mut self,
        scale: f64,
        fonts: &Typography,
        modules: island_core::modules::Modules,
        feedback: &island_ui::navigation::Feedback,
        timer: &island_core::timer::Timer,
    ) -> io::Result<()> {
        use island_ui::navigation::{self as nav, Action};
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize)
        };
        nav::paint(pixels, self.size(), scale, modules, feedback);
        if modules.timer_visible() {
            let r = nav::rect(if timer.mode == island_core::timer::Mode::Timer {
                Action::Countdown
            } else {
                Action::Pomodoro
            });
            island_ui::theme::rounded(
                pixels,
                self.size(),
                [r[0] + 10.0, r[3] - 2.0, r[2] - 10.0, r[3]].map(|n| n * scale),
                scale,
                island_ui::theme::ACCENT,
            );
        }
        for (action, text) in [
            (Action::Home, "Accueil"),
            (Action::Media, "Médias"),
            (Action::Timer, "Minuteurs"),
            (Action::Launcher, "Raccourcis"),
            (Action::OpenLauncher, "Raccourcis  ›"),
            (
                Action::ToggleLauncher,
                if modules.launcher_enabled {
                    "Désactiver"
                } else {
                    "Activer"
                },
            ),
            (
                Action::ToggleTimer,
                if modules.timer_enabled {
                    "Désactiver"
                } else {
                    "Activer"
                },
            ),
            (Action::Countdown, "Minuteur"),
            (Action::Pomodoro, "Pomodoro"),
            (Action::TimerPrimary, timer.primary_label()),
            (Action::TimerReset, "Réinitialiser"),
            (Action::TimerSettings, "Durées…"),
            (
                Action::ToggleMedia,
                if modules.media_enabled {
                    "Désactiver"
                } else {
                    "Activer"
                },
            ),
        ] {
            if action == Action::Timer && modules.launcher_visible() {
                continue;
            }
            if !nav::actions(modules).contains(&action)
                && !matches!(action, Action::Home | Action::Media | Action::Timer)
            {
                continue;
            }
            let r = nav::rect(action);
            self.label(
                fonts,
                if (action == Action::Media && !modules.media_enabled)
                    || (action == Action::Timer && !modules.timer_enabled)
                    || (action == Action::OpenLauncher && !modules.launcher_enabled)
                    || (matches!(
                        action,
                        Action::Countdown | Action::Pomodoro | Action::TimerSettings
                    ) && !timer.configurable())
                {
                    3
                } else {
                    2
                },
                text,
                nav::text_rect([r[0] + 10.0, r[1] + 3.0, r[2] - 6.0, r[3] - 3.0], scale),
            )?;
        }
        Ok(())
    }
    pub(super) fn draw_launcher_shell(
        &mut self,
        radius: f64,
        scale: f64,
        focused: bool,
        fonts: &Typography,
    ) -> io::Result<()> {
        self.draw(radius, scale, 0.0, focused, fonts)?;
        // SAFETY: owned and flushed DIB. Clear the compact handle under the expanded tabs.
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize)
        };
        for y in (10.0 * scale) as i32..(22.0 * scale).ceil() as i32 {
            for x in (150.0 * scale) as i32..(234.0 * scale).ceil() as i32 {
                pixels[(y * self.width + x) as usize] = 0xff000000 | island_ui::BACKGROUND;
            }
        }
        Ok(())
    }
    pub(super) fn draw_launcher_content(
        &mut self,
        scale: f64,
        fonts: &Typography,
        view: &super::launcher_view::View,
    ) -> io::Result<()> {
        use island_ui::navigation::{self as nav, Action};
        let count = view.catalog.visible(view.favorites).len();
        let title = if view.notice == "Raccourcis" {
            format!(
                "{} raccourci(s) · page {}/{}",
                count,
                view.offset / 3 + 1,
                count.div_ceil(3).max(1)
            )
        } else {
            view.notice.clone()
        };
        self.label(
            fonts,
            3,
            &title,
            nav::text_rect([24.0, 36.0, 360.0, 53.0], scale),
        )?;
        for (a, text) in [
            (Action::AddShortcut, "+ Ajouter"),
            (
                Action::FavoriteFilter,
                if view.favorites {
                    "★ Favoris"
                } else {
                    "Tous"
                },
            ),
            (Action::PreviousShortcuts, "‹"),
            (Action::NextShortcuts, "›"),
        ] {
            let r = nav::rect(a);
            self.label(
                fonts,
                if view.busy { 3 } else { 2 },
                text,
                nav::text_rect([r[0] + 8.0, r[1] + 1.0, r[2] - 4.0, r[3] - 1.0], scale),
            )?;
        }
        for row in 0..3 {
            let Some(entry) = view.row(row) else {
                continue;
            };
            let r = nav::rect(Action::Launch(row));
            let pixels = unsafe {
                std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize)
            };
            island_ui::launcher::icon(pixels, self.size(), scale, 29.0, r[1] + 2.5, entry.kind);
            self.label(
                fonts,
                if view.busy { 3 } else { 2 },
                &entry.name,
                nav::text_rect([51.0, r[1] + 1.0, 258.0, r[3] - 1.0], scale),
            )?;
            for (a, text) in [
                (Action::Pin(row), if entry.favorite { "★" } else { "☆" }),
                (Action::EditShortcut(row), "…"),
            ] {
                let r = nav::rect(a);
                self.label(
                    fonts,
                    2,
                    text,
                    nav::text_rect([r[0] + 12.0, r[1] + 1.0, r[2] - 5.0, r[3] - 1.0], scale),
                )?;
            }
        }
        if count == 0 && !view.busy {
            self.label(
                fonts,
                2,
                "Aucun raccourci dans cette vue.",
                nav::text_rect([32.0, 87.0, 258.0, 104.0], scale),
            )?;
        }
        Ok(())
    }
    pub(super) fn draw_timer(
        &mut self,
        radius: f64,
        scale: f64,
        focused: bool,
        fonts: &Typography,
        timer: &island_core::timer::Timer,
        now: u64,
    ) -> io::Result<()> {
        use island_core::timer::{Mode, State, clock};
        self.draw(radius, scale, 0.0, focused, fonts)?;
        // SAFETY: owned DIB, CPU raster follows the flushed GDI draw above.
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize)
        };
        for y in (10.0 * scale) as i32..(22.0 * scale).ceil() as i32 {
            for x in (150.0 * scale) as i32..(234.0 * scale).ceil() as i32 {
                pixels[(y * self.width + x) as usize] = 0xff000000 | island_ui::BACKGROUND;
            }
        }
        let remaining = timer.remaining(now);
        let fill = 1.0 - remaining as f64 / timer.duration() as f64;
        island_ui::theme::rounded(
            pixels,
            self.size(),
            [24.0, 109.0, 360.0, 113.0].map(|n| n * scale),
            2.0 * scale,
            island_ui::theme::SURFACE,
        );
        if fill > 0.0 {
            island_ui::theme::rounded(
                pixels,
                self.size(),
                [24.0, 109.0, 24.0 + 336.0 * fill, 113.0].map(|n| n * scale),
                2.0 * scale,
                island_ui::theme::ACCENT,
            );
        }
        let rect = |r| island_ui::navigation::text_rect(r, scale);
        self.label(
            fonts,
            0,
            &clock(remaining),
            rect([26.0, 76.0, 142.0, 104.0]),
        )?;
        let state = match timer.state {
            State::Ready => "Prêt",
            State::Running { .. } => "En cours",
            State::Paused { .. } => "En pause",
            State::Finished => "Terminé",
        };
        let status = if timer.mode == Mode::Pomodoro {
            format!(
                "{} · {} · {}/4",
                timer.phase.label(),
                state,
                timer.completed
            )
        } else {
            format!("Minuteur · {state}")
        };
        self.label(fonts, 2, &status, rect([148.0, 78.0, 360.0, 103.0]))
    }
    pub(super) fn draw_popover(
        &mut self,
        scale: f64,
        typography: &Typography,
        snapshot: &island_core::media::Snapshot,
        selector: &island_core::selector::Selector,
    ) -> io::Result<()> {
        use island_core::selector::Choice;
        let checked = Some(if snapshot.manual.is_none() {
            Choice::Automatic
        } else {
            Choice::Session(snapshot.selected.unwrap_or(0))
        });
        // SAFETY: the DIB owns this exact pixel count; every text rect is inset.
        let pixels = unsafe {
            std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize)
        };
        island_ui::popover::paint(pixels, self.size(), scale, selector, checked);
        let area = |r: [f64; 4]| island_ui::TextRect {
            left: (r[0] * scale).round() as i32,
            top: (r[1] * scale).round() as i32,
            right: (r[2] * scale).round() as i32,
            bottom: (r[3] * scale).round() as i32,
        };
        self.label(typography, 0, "Lecteurs", area([24.0, 12.0, 330.0, 37.0]))?;
        for row in 0..selector.page {
            let Some(choice) = selector.at(row) else {
                break;
            };
            let (name, detail) = match choice {
                Choice::Automatic => ("Sélection automatique", "Suivre le lecteur actif"),
                Choice::Refresh => (
                    "Actualiser",
                    snapshot
                        .notice
                        .as_deref()
                        .unwrap_or("Rechercher les lecteurs Windows"),
                ),
                Choice::Session(id) => {
                    let Some(s) = snapshot.sessions.iter().find(|s| s.id == id) else {
                        continue;
                    };
                    (
                        if s.source_name.is_empty() {
                            "Lecteur Windows"
                        } else {
                            &s.source_name
                        },
                        if s.title.is_empty() {
                            "Titre indisponible"
                        } else {
                            &s.title
                        },
                    )
                }
            };
            self.label(
                typography,
                1,
                name,
                island_ui::popover::text_rect(row, false, scale),
            )?;
            self.label(
                typography,
                2,
                detail,
                island_ui::popover::text_rect(row, true, scale),
            )?;
            if let Choice::Session(id) = choice
                && snapshot
                    .sessions
                    .iter()
                    .any(|s| s.id == id && s.playback == island_core::media::Playback::Playing)
            {
                let y = island_ui::popover::TOP + row as f64 * island_ui::popover::ROW + 26.0;
                self.label(typography, 3, "▶", area([312.0, y, 338.0, y + 18.0]))?;
            }
        }
        if selector.choices.len() > selector.page {
            let count = format!(
                "{}–{} / {}  ·  Molette ou flèches",
                selector.offset + 1,
                (selector.offset + selector.page).min(selector.choices.len()),
                selector.choices.len()
            );
            let h = self.height as f64 / scale;
            self.label(
                typography,
                3,
                &count,
                area([24.0, h - 22.0, 330.0, h - 7.0]),
            )?;
        }
        Ok(())
    }

    fn label(
        &mut self,
        cache: &Typography,
        index: usize,
        text: &str,
        area: island_ui::TextRect,
    ) -> io::Result<()> {
        if text.is_empty() {
            return Ok(());
        }
        let text: Vec<u16> = text.encode_utf16().collect();
        // SAFETY: local text and font outlive DrawText; selected object restored on all paths.
        unsafe {
            let old = SelectObject(self.dc, cache.fonts[index]);
            if old.is_null() {
                return Err(io::Error::last_os_error());
            }
            let rgb = island_ui::TEXT[index];
            SetBkMode(self.dc, TRANSPARENT as i32);
            SetTextColor(self.dc, (rgb & 255) << 16 | (rgb & 0xff00) | (rgb >> 16));
            let mut rect = RECT {
                left: area.left,
                top: area.top,
                right: area.right,
                bottom: area.bottom,
            };
            let drawn = DrawTextW(
                self.dc,
                text.as_ptr(),
                text.len() as i32,
                &mut rect,
                DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS | DT_LEFT | DT_VCENTER,
            );
            SelectObject(self.dc, old);
            GdiFlush();
            for y in area.top..area.bottom {
                for x in area.left..area.right {
                    *self.pixels.add((y * self.width + x) as usize) |= 0xff000000;
                }
            }
            if drawn == 0 {
                return Err(io::Error::other("Texte multimédia indisponible"));
            }
        }
        Ok(())
    }

    pub fn draw(
        &mut self,
        radius: f64,
        scale: f64,
        progress: f64,
        focused: bool,
        typography: &Typography,
    ) -> io::Result<()> {
        debug_assert_eq!(scale, typography.scale);
        // SAFETY: DIB owns width*height pixels. GDI writes are flushed before CPU access.
        unsafe {
            let pixels =
                std::slice::from_raw_parts_mut(self.pixels, (self.width * self.height) as usize);
            island_ui::paint(
                pixels,
                (self.width, self.height),
                radius,
                scale,
                focused,
                progress,
            );
            let fade = island_ui::content_opacity(progress);
            if fade > 0.0 {
                let layout = island_ui::text_layout(self.width, self.height, scale);
                let mut result = Ok(());
                for (index, rect) in layout.into_iter().enumerate() {
                    if let Err(error) = self.text(typography, index, rect, fade) {
                        result = Err(error);
                        break;
                    }
                }
                GdiFlush();
                // Every text rectangle lies strictly inside the opaque body (DPI tests).
                // Restore alpha only there, without recomputing distances for the whole DIB.
                for rect in layout {
                    for y in rect.top..rect.bottom {
                        for x in rect.left..rect.right {
                            *self.pixels.add((y * self.width + x) as usize) |= 0xff000000;
                        }
                    }
                }
                result?;
            }
        }
        Ok(())
    }

    unsafe fn text(
        &self,
        cache: &Typography,
        index: usize,
        area: island_ui::TextRect,
        fade: f64,
    ) -> io::Result<()> {
        // SAFETY: fonts outlive this call; old font restored even if DrawText fails.
        unsafe {
            let old = SelectObject(self.dc, cache.fonts[index]);
            if old.is_null() {
                return Err(io::Error::last_os_error());
            }
            let rgb = island_ui::mix(island_ui::BACKGROUND, island_ui::TEXT[index], fade);
            let color = (rgb & 255) << 16 | (rgb & 0xff00) | (rgb >> 16);
            SetBkMode(self.dc, TRANSPARENT as i32);
            SetTextColor(self.dc, color);
            let mut rect = RECT {
                left: area.left,
                top: area.top,
                right: area.right,
                bottom: area.bottom,
            };
            let flags = DT_SINGLELINE
                | DT_NOPREFIX
                | DT_END_ELLIPSIS
                | if index == 3 {
                    DT_CENTER | DT_VCENTER
                } else {
                    DT_LEFT
                };
            let drawn = DrawTextW(
                self.dc,
                cache.text[index].as_ptr(),
                cache.text[index].len() as i32,
                &mut rect,
                flags,
            );
            SelectObject(self.dc, old);
            if drawn == 0 {
                return Err(io::Error::other("Impossible de dessiner le texte"));
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn save_preview(&self, path: &std::path::Path, background: u32) -> io::Result<()> {
        use std::io::Write;
        // Test-only BMP export of the actual native raster, flattened onto a chosen backdrop.
        let mut file = std::fs::File::create(path)?;
        let bytes = (self.width * self.height * 4) as u32;
        file.write_all(b"BM")?;
        file.write_all(&(54 + bytes).to_le_bytes())?;
        file.write_all(&[0; 4])?;
        file.write_all(&54u32.to_le_bytes())?;
        file.write_all(&40u32.to_le_bytes())?;
        file.write_all(&self.width.to_le_bytes())?;
        file.write_all(&(-self.height).to_le_bytes())?;
        file.write_all(&1u16.to_le_bytes())?;
        file.write_all(&32u16.to_le_bytes())?;
        file.write_all(&[0; 24])?;
        // SAFETY: no GDI write remains pending after draw(); initialized DIB dimensions.
        for &pixel in
            unsafe { std::slice::from_raw_parts(self.pixels, (self.width * self.height) as usize) }
        {
            let inv = 255 - (pixel >> 24);
            let channel = |shift: u32| {
                ((pixel >> shift) & 255u32) + (((background >> shift) & 255) * inv / 255)
            };
            let flattened: u32 = 0xff000000 | channel(16) << 16 | channel(8) << 8 | channel(0);
            file.write_all(&flattened.to_le_bytes())?;
        }
        Ok(())
    }

    pub fn present(&self, hwnd: HWND, x: i32, y: i32) -> io::Result<()> {
        self.present_alpha(hwnd, x, y, 255)
    }
    pub fn present_alpha(&self, hwnd: HWND, x: i32, y: i32, alpha: u8) -> io::Result<()> {
        // SAFETY: hwnd/DC are live on the owning UI thread; descriptors are stack values.
        unsafe {
            let blend = BLENDFUNCTION {
                BlendOp: AC_SRC_OVER as u8,
                BlendFlags: 0,
                SourceConstantAlpha: alpha,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };
            let ok = UpdateLayeredWindow(
                hwnd,
                null_mut(),
                &POINT { x, y },
                &SIZE {
                    cx: self.width,
                    cy: self.height,
                },
                self.dc,
                &POINT { x: 0, y: 0 },
                0,
                &blend,
                ULW_ALPHA,
            );
            if ok == 0 {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    pub fn size(&self) -> (i32, i32) {
        (self.width, self.height)
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        // SAFETY: restore the original selected bitmap before releasing owned GDI resources.
        unsafe {
            SelectObject(self.dc, self.previous);
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn counts() -> (u32, u32) {
        // SAFETY: read-only resource counters for this test process.
        unsafe {
            (
                GetGuiResources(GetCurrentProcess(), GR_GDIOBJECTS),
                GetGuiResources(GetCurrentProcess(), GR_USEROBJECTS),
            )
        }
    }
    #[test]
    fn navigation_pages_keep_transparent_corners_at_all_scales() {
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let fonts = Typography::new(scale).unwrap();
            let (w, h, r) = island_ui::dimensions(1.0, scale);
            let mut surface = Surface::new(w, h).unwrap();
            for enabled in [true, false] {
                let modules = island_core::modules::Modules {
                    media_enabled: enabled,
                    page: island_core::modules::Page::Home,
                    ..Default::default()
                };
                surface.draw_home(r, scale, false, &fonts, modules).unwrap();
                surface
                    .draw_navigation(
                        scale,
                        &fonts,
                        modules,
                        &Default::default(),
                        &Default::default(),
                    )
                    .unwrap();
                let pixels =
                    unsafe { std::slice::from_raw_parts(surface.pixels, (w * h) as usize) };
                for i in [
                    0,
                    w as usize - 1,
                    (w * (h - 1)) as usize,
                    (w * h - 1) as usize,
                ] {
                    assert_eq!(pixels[i] >> 24, 0);
                }
                if scale == 1.25
                    && let Some(dir) = std::env::var_os("ISLAND_PREVIEW_DIR")
                {
                    std::fs::create_dir_all(&dir).unwrap();
                    surface
                        .save_preview(
                            &std::path::PathBuf::from(dir).join(format!("home-{enabled}.bmp")),
                            0xe9edf2,
                        )
                        .unwrap();
                }
            }
        }
    }
    #[test]
    fn launcher_raster_is_bounded_and_keeps_rounded_transparency() {
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        let view = crate::launcher_view::View {
            catalog: Default::default(),
            loaded: true,
            ..Default::default()
        };
        let modules = island_core::modules::Modules {
            page: island_core::modules::Page::Launcher,
            ..Default::default()
        };
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let fonts = Typography::new(scale).unwrap();
            let (w, h, r) = island_ui::dimensions(1.0, scale);
            let mut surface = Surface::new(w, h).unwrap();
            surface
                .draw_launcher_shell(r, scale, false, &fonts)
                .unwrap();
            surface
                .draw_navigation(
                    scale,
                    &fonts,
                    modules,
                    &Default::default(),
                    &Default::default(),
                )
                .unwrap();
            surface.draw_launcher_content(scale, &fonts, &view).unwrap();
            let pixels = unsafe { std::slice::from_raw_parts(surface.pixels, (w * h) as usize) };
            for i in [
                0,
                w as usize - 1,
                (w * (h - 1)) as usize,
                (w * h - 1) as usize,
            ] {
                assert_eq!(pixels[i] >> 24, 0);
            }
            if scale == 1.25
                && let Some(dir) = std::env::var_os("ISLAND_PREVIEW_DIR")
            {
                std::fs::create_dir_all(&dir).unwrap();
                surface
                    .save_preview(
                        &std::path::PathBuf::from(dir).join("launcher.bmp"),
                        0xe9edf2,
                    )
                    .unwrap();
            }
        }
    }
    #[test]
    fn timer_raster_preserves_alpha_at_all_scales_and_states() {
        use island_core::{
            modules::{Modules, Page},
            timer::{Mode, State, Timer},
        };
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let fonts = Typography::new(scale).unwrap();
            let (w, h, r) = island_ui::dimensions(1.0, scale);
            let mut surface = Surface::new(w, h).unwrap();
            let mut timer = Timer::default();
            timer.configure(Default::default(), Mode::Pomodoro);
            for (i, state) in [
                State::Ready,
                State::Running { deadline: 700_000 },
                State::Paused { remaining: 123_000 },
                State::Finished,
            ]
            .into_iter()
            .enumerate()
            {
                timer.state = state;
                surface
                    .draw_timer(r, scale, false, &fonts, &timer, 0)
                    .unwrap();
                surface
                    .draw_navigation(
                        scale,
                        &fonts,
                        Modules {
                            page: Page::Timer,
                            ..Default::default()
                        },
                        &Default::default(),
                        &timer,
                    )
                    .unwrap();
                let pixels =
                    unsafe { std::slice::from_raw_parts(surface.pixels, (w * h) as usize) };
                for index in [
                    0,
                    w as usize - 1,
                    (w * (h - 1)) as usize,
                    (w * h - 1) as usize,
                ] {
                    assert_eq!(pixels[index] >> 24, 0);
                }
                if scale == 1.25
                    && let Some(dir) = std::env::var_os("ISLAND_PREVIEW_DIR")
                {
                    std::fs::create_dir_all(&dir).unwrap();
                    surface
                        .save_preview(
                            &std::path::PathBuf::from(dir).join(format!("timer-{i}.bmp")),
                            0xe9edf2,
                        )
                        .unwrap();
                }
            }
        }
    }
    #[test]
    fn media_raster_is_bounded_at_all_scales() {
        use island_core::media::*;
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let typography = Typography::new(scale).unwrap();
            let (w, h, r) = island_ui::dimensions(1.0, scale);
            let mut surface = Surface::new(w, h).unwrap();
            let mut view = crate::media_view::View::default();
            view.snapshot = Snapshot {
                sessions: vec![Session {
                    id: 1,
                    title: "Exemple de test — titre long avec ellipse".into(),
                    artist: "Données de test".into(),
                    source: "Lecteur de test".into(),
                    playback: Playback::Playing,
                    capabilities: Capabilities {
                        play: true,
                        pause: true,
                        toggle: true,
                        previous: true,
                        next: true,
                    },
                    timeline: Some(Timeline {
                        position: 83.0,
                        duration: 246.0,
                        sampled: std::time::Instant::now(),
                        rate: 1.0,
                    }),
                    ..Default::default()
                }],
                selected: Some(1),
                ..Default::default()
            };
            for image in [false, true] {
                if image {
                    view.snapshot.sessions[0].artwork = Some(std::sync::Arc::new(Artwork {
                        width: 160,
                        height: 90,
                        bgra: [0x70, 0x55, 0x35, 0xff].repeat(160 * 90),
                    }));
                }
                surface
                    .draw_media(r, scale, false, &typography, &view)
                    .unwrap();
                // SAFETY: draw_media flushes GDI; only reads the owned DIB.
                let pixels =
                    unsafe { std::slice::from_raw_parts(surface.pixels, (w * h) as usize) };
                assert_eq!(pixels[0], 0);
                assert_eq!(pixels[(w * h - 1) as usize], 0);
                assert!(pixels.iter().all(|p| (p & 255) <= p >> 24
                    && ((p >> 8) & 255) <= p >> 24
                    && ((p >> 16) & 255) <= p >> 24));
                if scale == 1.25
                    && let Some(dir) = std::env::var_os("ISLAND_PREVIEW_DIR")
                {
                    let dir = std::path::PathBuf::from(dir);
                    std::fs::create_dir_all(&dir).unwrap();
                    surface
                        .save_preview(
                            &dir.join(if image {
                                "media-art.bmp"
                            } else {
                                "media-fallback.bmp"
                            }),
                            0xe9edf2,
                        )
                        .unwrap();
                }
            }
            view.snapshot.sessions[0].title.clear();
            view.snapshot.sessions[0].artist.clear();
            view.snapshot.sessions[0].timeline = None;
            view.snapshot.sessions[0].capabilities = Capabilities::default();
            surface
                .draw_media(r, scale, false, &typography, &view)
                .unwrap();
            let baseline = counts();
            for _ in 0..100 {
                surface
                    .draw_media(r, scale, false, &typography, &view)
                    .unwrap();
            }
            assert_eq!(
                counts(),
                baseline,
                "media redraw must not retain GDI/USER objects"
            );
            view.snapshot = Snapshot::default();
            surface
                .draw_media(r, scale, false, &typography, &view)
                .unwrap();
            if scale == 1.25
                && let Some(dir) = std::env::var_os("ISLAND_PREVIEW_DIR")
            {
                surface
                    .save_preview(
                        &std::path::PathBuf::from(dir).join("media-empty.bmp"),
                        0xe9edf2,
                    )
                    .unwrap();
            }
        }
    }
    use windows_sys::Win32::System::Threading::{
        GR_GDIOBJECTS, GR_USEROBJECTS, GetCurrentProcess, GetGuiResources,
    };

    #[test]
    fn native_text_does_not_damage_transparent_corners() {
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        let output = std::env::var_os("ISLAND_PREVIEW_DIR").map(std::path::PathBuf::from);
        if let Some(path) = &output {
            std::fs::create_dir_all(path).unwrap();
        }
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let fonts = Typography::new(scale).unwrap();
            for progress in [0.0, 0.83, 1.0] {
                let (w, h, r) = island_ui::dimensions(progress, scale);
                let mut surface = Surface::new(w, h).unwrap();
                surface.draw(r, scale, progress, false, &fonts).unwrap();
                // SAFETY: draw() flushed GDI, surface owns all pixels through this loop.
                let pixels =
                    unsafe { std::slice::from_raw_parts(surface.pixels, (w * h) as usize) };
                assert_eq!(pixels[0], 0);
                assert_eq!(pixels[(w * h - 1) as usize], 0);
                for &pixel in pixels {
                    let alpha = pixel >> 24;
                    assert!(
                        (pixel & 255) <= alpha
                            && ((pixel >> 8) & 255) <= alpha
                            && ((pixel >> 16) & 255) <= alpha
                    );
                }
                if let Some(path) = &output {
                    let name = format!(
                        "satin-{}-{}.bmp",
                        (scale * 100.0) as i32,
                        (progress * 100.0) as i32
                    );
                    surface.save_preview(&path.join(name), 0xe9edf2).unwrap();
                }
            }
        }
    }

    #[test]
    fn repeated_animation_surfaces_release_gdi_resources() {
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        let typography = Typography::new(1.25).expect("font cache");
        // No window, input injection or global setting: exercise only the real raster resources.
        let cycle = |samples: &mut Vec<f64>| {
            for step in (0..=16).chain((0..16).rev()) {
                let frame_started = std::time::Instant::now();
                let progress = step as f64 / 16.0;
                let (width, height, radius) = island_ui::dimensions(progress, 1.25);
                let mut surface = Surface::new(width, height).expect("DIB allocation");
                surface
                    .draw(radius, 1.25, progress, false, &typography)
                    .expect("native draw");
                drop(surface);
                samples.push(frame_started.elapsed().as_secs_f64() * 1000.0);
            }
        };
        cycle(&mut Vec::new()); // Warm up GDI/font caches before establishing the baseline.
        // SAFETY: pseudo-handle is valid for querying this test process; no ownership transfer.
        let counts = || unsafe {
            let process = GetCurrentProcess();
            (
                GetGuiResources(process, GR_GDIOBJECTS),
                GetGuiResources(process, GR_USEROBJECTS),
            )
        };
        let baseline = counts();
        assert!(baseline.0 > 0, "GDI resource counters unavailable");
        let started = std::time::Instant::now();
        let mut samples = Vec::with_capacity(3300);
        for checkpoint in 1..=4 {
            for _ in 0..25 {
                cycle(&mut samples);
            }
            let current = counts();
            println!(
                "raster cycles={} baseline={baseline:?} current={current:?}",
                checkpoint * 25
            );
            assert_eq!(current, baseline, "GDI/USER resources leaked");
        }
        println!(
            "100 raster cycles elapsed={:?}; no window presentation measured",
            started.elapsed()
        );
        samples.sort_by(f64::total_cmp);
        println!(
            "raster allocation/draw/drop samples={} median_ms={:.3} p95_ms={:.3} max_ms={:.3}",
            samples.len(),
            samples[samples.len() / 2],
            samples[samples.len() * 95 / 100],
            samples[samples.len() - 1]
        );
    }
}
