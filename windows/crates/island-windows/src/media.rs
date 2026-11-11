//! GSMTC owner thread. Callbacks signal a bounded mailbox; UI never owns WinRT objects.
use island_core::media::*;
use std::{
    collections::BTreeMap,
    io,
    sync::{
        Arc, Condvar, Mutex, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use windows::{
    Foundation::TypedEventHandler,
    Graphics::Imaging::*,
    Media::{Control::*, MediaPlaybackType},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::{Error, HRESULT, Result, RuntimeType},
};
use windows_future::{AsyncOperationCompletedHandler, IAsyncOperation};

const META: u8 = 1;
const STATE: u8 = 2;
const ALL: u8 = META | STATE;
const ABORT: HRESULT = HRESULT(0x80004004u32 as i32);
#[derive(Clone, Copy)]
struct PendingCommand {
    id: u64,
    generation: u64,
    action: Command,
    epoch: u64,
}
#[derive(Default)]
struct Input {
    disabled: bool,
    stop: bool,
    topology: bool,
    dirty: BTreeMap<u64, u8>,
    select: Option<Option<u64>>,
    command: Option<PendingCommand>,
    refresh: bool,
    latest: Option<Snapshot>,
}
struct Shared {
    input: Mutex<Input>,
    wake: Condvar,
    epoch: AtomicU64,
    busy: AtomicBool,
    hwnd: isize,
}
impl Shared {
    fn current(&self, epoch: u64) -> bool {
        let input = self.input.lock().unwrap();
        !input.stop && !input.disabled && self.epoch.load(Ordering::Acquire) == epoch
    }
    /// Recheck immediately before dispatch; callbacks can invalidate work after the
    /// worker drains its mailbox. Never hold the mailbox lock across a COM call.
    fn command_current(&self, command: PendingCommand, selected: Option<u64>) -> bool {
        let input = self.input.lock().unwrap();
        !input.stop
            && !input.disabled
            && selected == Some(command.id)
            && self.epoch.load(Ordering::Acquire) == command.epoch
            && !input.topology
            && !input.refresh
            && input.select.is_none()
            && input.dirty.get(&command.id).copied().unwrap_or(0) & META == 0
    }
    fn signal(&self, id: u64, flags: u8) {
        let mut input = self.input.lock().unwrap();
        if input.stop || input.disabled {
            return;
        }
        if id == 0 {
            input.topology = true;
        } else if input.dirty.len() < MAX_SESSIONS || input.dirty.contains_key(&id) {
            *input.dirty.entry(id).or_default() |= flags;
        }
        if id == 0 || flags & META != 0 {
            self.epoch.fetch_add(1, Ordering::Release);
        }
        self.wake.notify_one();
    }
    fn publish(&self, snapshot: Snapshot) {
        let mut input = self.input.lock().unwrap();
        if input.stop {
            return;
        }
        let post = input.latest.is_none();
        input.latest = Some(snapshot);
        if post {
            // SAFETY: owner joins worker before destroying HWND. No pointer in message.
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW(
                    self.hwnd as _,
                    super::MEDIA_CHANGED,
                    0,
                    0,
                );
            }
        }
    }
}
pub struct Worker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}
impl Worker {
    /// Coalesced lifecycle request; native subscriptions/resources are released on
    /// their existing MTA. The UI never joins or waits when toggling a module.
    pub fn set_enabled(&self, enabled: bool) {
        let mut i = self.shared.input.lock().unwrap();
        if i.disabled == !enabled {
            return;
        }
        i.disabled = !enabled;
        i.topology = true;
        i.dirty.clear();
        i.select = None;
        i.command = None;
        i.latest = None;
        self.shared.epoch.fetch_add(1, Ordering::Release);
        self.shared.wake.notify_one();
    }
    pub fn new(hwnd: isize) -> io::Result<Self> {
        let shared = Arc::new(Shared {
            input: Mutex::new(Input {
                topology: true,
                ..Default::default()
            }),
            wake: Condvar::new(),
            epoch: AtomicU64::new(1),
            busy: AtomicBool::new(false),
            hwnd,
        });
        let copy = shared.clone();
        let thread = std::thread::Builder::new()
            .name("island-media".into())
            .spawn(move || {
                // SAFETY: balanced initialization on this dedicated worker; no UI apartment change.
                if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                    copy.publish(Snapshot {
                        notice: Some(format!("Multimédia indisponible ({})", error.code())),
                        ..Default::default()
                    });
                    return;
                }
                run(&copy);
                unsafe {
                    RoUninitialize();
                }
            })?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }
    pub fn take(&self) -> Option<Snapshot> {
        self.shared.input.lock().unwrap().latest.take()
    }
    pub fn busy(&self) -> bool {
        self.shared.busy.load(Ordering::Acquire)
    }
    pub fn select(&self, id: Option<u64>) {
        let mut i = self.shared.input.lock().unwrap();
        if i.disabled || i.stop {
            return;
        }
        i.select = Some(id);
        // A new explicit selection should cancel the old cover's async work immediately.
        self.shared.epoch.fetch_add(1, Ordering::Release);
        self.shared.wake.notify_one();
    }
    pub fn refresh(&self) {
        let mut i = self.shared.input.lock().unwrap();
        if i.disabled || i.stop {
            return;
        }
        i.refresh = true;
        self.shared.epoch.fetch_add(1, Ordering::Release);
        self.shared.wake.notify_one();
    }
    pub fn command(&self, id: u64, generation: u64, command: Command) -> bool {
        let mut i = self.shared.input.lock().unwrap();
        if i.command.is_some()
            || i.stop
            || i.disabled
            || self.shared.busy.swap(true, Ordering::AcqRel)
        {
            return false;
        }
        i.command = Some(PendingCommand {
            id,
            generation,
            action: command,
            epoch: self.shared.epoch.load(Ordering::Acquire),
        });
        self.shared.wake.notify_one();
        true
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        {
            let mut i = self.shared.input.lock().unwrap();
            i.stop = true;
            self.shared.wake.notify_all();
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Wait only while a specific async operation is in flight, max 5 seconds.
/// Stop or a newer metadata generation cancels it. Idle uses an indefinite Condvar wait.
fn complete<T: RuntimeType + 'static>(
    op: IAsyncOperation<T>,
    shared: &Arc<Shared>,
    epoch: Option<u64>,
) -> Result<T> {
    let done = Arc::new(AtomicBool::new(false));
    let flag = done.clone();
    let weak = Arc::downgrade(shared);
    op.SetCompleted(&AsyncOperationCompletedHandler::new(move |_, _| {
        flag.store(true, Ordering::Release);
        if let Some(s) = weak.upgrade() {
            let _guard = s.input.lock().unwrap();
            s.wake.notify_all();
        }
        Ok(())
    }))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut input = shared.input.lock().unwrap();
    loop {
        if input.stop
            || input.disabled
            || epoch.is_some_and(|e| shared.epoch.load(Ordering::Acquire) != e)
            || Instant::now() >= deadline
        {
            drop(input);
            let _ = op.Cancel();
            return Err(Error::from_hresult(ABORT));
        }
        if done.load(Ordering::Acquire) {
            break;
        }
        input = shared
            .wake
            .wait_timeout(input, deadline.saturating_duration_since(Instant::now()))
            .unwrap()
            .0;
    }
    drop(input);
    let result = op.GetResults()?;
    // GetResults is itself a COM call: a newer event may arrive while it returns.
    if epoch.is_some_and(|e| !shared.current(e)) {
        return Err(Error::from_hresult(ABORT));
    }
    Ok(result)
}
fn wake(weak: &Weak<Shared>, id: u64, flags: u8) {
    if let Some(s) = weak.upgrade() {
        s.signal(id, flags);
    }
}
struct Entry {
    native: GlobalSystemMediaTransportControlsSession,
    data: Session,
    tokens: [Option<i64>; 3],
    art_loaded: bool,
}
impl Entry {
    fn new(
        native: GlobalSystemMediaTransportControlsSession,
        id: u64,
        shared: &Arc<Shared>,
    ) -> Result<Self> {
        let mut entry = Self {
            native,
            data: Session {
                id,
                ..Default::default()
            },
            tokens: [None; 3],
            art_loaded: false,
        };
        let weak = Arc::downgrade(shared);
        entry.tokens[0] = Some(
            entry
                .native
                .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    wake(&weak, id, META);
                    Ok(())
                }))?,
        );
        let weak = Arc::downgrade(shared);
        entry.tokens[1] = Some(entry.native.PlaybackInfoChanged(&TypedEventHandler::new(
            move |_, _| {
                wake(&weak, id, STATE);
                Ok(())
            },
        ))?);
        let weak = Arc::downgrade(shared);
        entry.tokens[2] = Some(
            entry
                .native
                .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                    wake(&weak, id, STATE);
                    Ok(())
                }))?,
        );
        entry.data.source =
            bounded_text(&entry.native.SourceAppUserModelId()?.to_string_lossy(), 256);
        Ok(entry)
    }
    fn playback(&mut self) -> Result<()> {
        let p = self.native.GetPlaybackInfo()?;
        self.data.playback = match p.PlaybackStatus()? {
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing => Playback::Playing,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Paused => Playback::Paused,
            GlobalSystemMediaTransportControlsSessionPlaybackStatus::Stopped => Playback::Stopped,
            _ => Playback::Unknown,
        };
        let c = p.Controls()?;
        self.data.capabilities = Capabilities {
            play: c.IsPlayEnabled()?,
            pause: c.IsPauseEnabled()?,
            toggle: c.IsPlayPauseToggleEnabled()?,
            previous: c.IsPreviousEnabled()?,
            next: c.IsNextEnabled()?,
        };
        self.data.timeline = self.native.GetTimelineProperties().ok().and_then(|t| {
            let start = t.StartTime().ok()?.Duration as f64 / 1e7;
            let end = t.EndTime().ok()?.Duration as f64 / 1e7;
            let position = t.Position().ok()?.Duration as f64 / 1e7;
            let updated = t.LastUpdatedTime().ok()?.UniversalTime;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs_f64()
                + 11644473600.0;
            let age = (now - updated as f64 / 1e7).max(0.0);
            if updated <= 0
                || age > 86400.0
                || start < 0.0
                || end <= start
                || position < start
                || position > end
            {
                return None;
            }
            Some(Timeline {
                position: position - start,
                duration: end - start,
                sampled: Instant::now().checked_sub(Duration::from_secs_f64(age))?,
                rate: p
                    .PlaybackRate()
                    .ok()
                    .and_then(|v| v.Value().ok())
                    .unwrap_or(1.0),
            })
        });
        Ok(())
    }
    fn metadata(&mut self, shared: &Arc<Shared>) -> Result<()> {
        let epoch = shared.epoch.load(Ordering::Acquire);
        let p = complete(
            self.native.TryGetMediaPropertiesAsync()?,
            shared,
            Some(epoch),
        )?;
        let title = bounded_text(&p.Title()?.to_string_lossy(), 512);
        let artist = bounded_text(&p.Artist()?.to_string_lossy(), 256);
        let album = bounded_text(&p.AlbumTitle()?.to_string_lossy(), 256);
        let kind = p
            .PlaybackType()
            .ok()
            .and_then(|v| v.Value().ok())
            .and_then(|t| match t {
                MediaPlaybackType::Music => Some("Musique".into()),
                MediaPlaybackType::Video => Some("Vidéo".into()),
                _ => None,
            });
        // Publish one coherent set of fields, never a partly read new title.
        if !shared.current(epoch) {
            return Err(Error::from_hresult(ABORT));
        }
        self.data.title = title;
        self.data.artist = artist;
        self.data.album = album;
        self.data.kind = kind;
        Ok(())
    }
    fn artwork(&mut self, shared: &Arc<Shared>) -> Result<()> {
        self.art_loaded = true;
        let epoch = shared.epoch.load(Ordering::Acquire);
        let props = complete(
            self.native.TryGetMediaPropertiesAsync()?,
            shared,
            Some(epoch),
        )?;
        // Verify the identity again: never pair a later thumbnail with an earlier title.
        if bounded_text(&props.Title()?.to_string_lossy(), 512) != self.data.title
            || bounded_text(&props.Artist()?.to_string_lossy(), 256) != self.data.artist
            || bounded_text(&props.AlbumTitle()?.to_string_lossy(), 256) != self.data.album
        {
            shared.signal(self.data.id, META);
            return Err(Error::from_hresult(ABORT));
        }
        let stream = complete(props.Thumbnail()?.OpenReadAsync()?, shared, Some(epoch))?;
        let size = stream.Size()?;
        if size == 0 || size > 8 * 1024 * 1024 {
            return Err(Error::from_hresult(ABORT));
        }
        let decoder = complete(BitmapDecoder::CreateAsync(&stream)?, shared, Some(epoch))?;
        let (w, h) = (decoder.PixelWidth()?, decoder.PixelHeight()?);
        if w == 0 || h == 0 || w > 4096 || h > 4096 || w as u64 * h as u64 > 4_000_000 {
            return Err(Error::from_hresult(ABORT));
        }
        let ratio = (ART_SIDE as f64 / w.max(h) as f64).min(1.0);
        let width = (w as f64 * ratio).round().max(1.0) as u32;
        let height = (h as f64 * ratio).round().max(1.0) as u32;
        let transform = BitmapTransform::new()?;
        transform.SetScaledWidth(width)?;
        transform.SetScaledHeight(height)?;
        let pixels = complete(
            decoder.GetPixelDataTransformedAsync(
                BitmapPixelFormat::Bgra8,
                BitmapAlphaMode::Premultiplied,
                &transform,
                ExifOrientationMode::IgnoreExifOrientation,
                ColorManagementMode::DoNotColorManage,
            )?,
            shared,
            Some(epoch),
        )?
        .DetachPixelData()?;
        let image = Artwork {
            width,
            height,
            bgra: pixels.to_vec(),
        };
        if image.valid() && shared.current(epoch) {
            self.data.artwork = Some(Arc::new(image));
        }
        Ok(())
    }
}
impl Drop for Entry {
    fn drop(&mut self) {
        if let Some(t) = self.tokens[0] {
            let _ = self.native.RemoveMediaPropertiesChanged(t);
        }
        if let Some(t) = self.tokens[1] {
            let _ = self.native.RemovePlaybackInfoChanged(t);
        }
        if let Some(t) = self.tokens[2] {
            let _ = self.native.RemoveTimelinePropertiesChanged(t);
        }
    }
}
struct Manager {
    native: GlobalSystemMediaTransportControlsSessionManager,
    tokens: [Option<i64>; 2],
}
impl Manager {
    fn new(shared: &Arc<Shared>) -> Result<Self> {
        let native = complete(
            GlobalSystemMediaTransportControlsSessionManager::RequestAsync()?,
            shared,
            None,
        )?;
        let mut manager = Self {
            native,
            tokens: [None; 2],
        };
        let weak = Arc::downgrade(shared);
        manager.tokens[0] = Some(manager.native.SessionsChanged(&TypedEventHandler::new(
            move |_, _| {
                wake(&weak, 0, ALL);
                Ok(())
            },
        ))?);
        let weak = Arc::downgrade(shared);
        manager.tokens[1] = Some(
            manager
                .native
                .CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                    wake(&weak, 0, STATE);
                    Ok(())
                }))?,
        );
        Ok(manager)
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        if let Some(t) = self.tokens[0] {
            let _ = self.native.RemoveSessionsChanged(t);
        }
        if let Some(t) = self.tokens[1] {
            let _ = self.native.RemoveCurrentSessionChanged(t);
        }
    }
}
fn run(shared: &Arc<Shared>) {
    let mut names = crate::app_names::Names::default();
    let mut manager: Option<Manager> = None;
    let mut entries: Vec<Entry> = Vec::new();
    let (mut next_id, mut selected, mut manual) = (1u64, None, None);
    loop {
        let (disabled, topology, mut dirty, selection, command, refresh) = {
            let mut i = shared.input.lock().unwrap();
            while !i.stop
                && !i.topology
                && i.dirty.is_empty()
                && i.select.is_none()
                && i.command.is_none()
                && !i.refresh
            {
                i = shared.wake.wait(i).unwrap();
            }
            if i.stop {
                break;
            }
            (
                i.disabled,
                std::mem::take(&mut i.topology),
                std::mem::take(&mut i.dirty),
                i.select.take(),
                i.command.take(),
                std::mem::take(&mut i.refresh),
            )
        };
        if command.is_none() {
            shared.busy.store(false, Ordering::Release);
        }
        if disabled {
            entries.clear(); // revoke session handlers before releasing the manager
            manager = None;
            names = crate::app_names::Names::default();
            selected = None;
            manual = None;
            shared.publish(Snapshot::default());
            continue; // no deadline: sleep on Condvar until enable or stop
        }
        let mut notice = None;
        if manager.is_none() {
            match Manager::new(shared) {
                Ok(m) => manager = Some(m),
                Err(e) => {
                    shared.busy.store(false, Ordering::Release);
                    shared.publish(Snapshot {
                        notice: Some(format!(
                            "Médias Windows indisponibles ({}) — Réessayer via le menu",
                            e.code()
                        )),
                        ..Default::default()
                    });
                    continue;
                }
            }
        }
        let m = manager.as_ref().unwrap();
        if topology || refresh {
            match m.native.GetSessions() {
                Ok(list) => {
                    let natives: Vec<_> = (0..list.Size().unwrap_or(0).min(MAX_SESSIONS as u32))
                        .filter_map(|i| list.GetAt(i).ok())
                        .filter_map(|n| {
                            n.SourceAppUserModelId()
                                .ok()
                                .map(|s| (n, bounded_text(&s.to_string_lossy(), 256)))
                        })
                        .collect();
                    let mut rebuilt = Vec::with_capacity(natives.len());
                    for (native, source) in &natives {
                        // GSMTC returns fresh COM wrappers on topology changes. AUMID is
                        // our stable *source* key only when unique in both snapshots.
                        // Ambiguous duplicate sources get new IDs, never guessed bindings.
                        let previous = entries.iter().find(|e| e.native == *native).or_else(|| {
                            if !source.is_empty()
                                && natives.iter().filter(|(_, s)| s == source).count() == 1
                                && entries.iter().filter(|e| e.data.source == *source).count() == 1
                            {
                                entries.iter().find(|e| e.data.source == *source)
                            } else {
                                None
                            }
                        });
                        let id = previous.map(|e| e.data.id).unwrap_or_else(|| {
                            let id = next_id;
                            next_id = next_id.wrapping_add(1);
                            id
                        });
                        match Entry::new(native.clone(), id, shared) {
                            Ok(mut e) => {
                                e.data.source_name = names.get(&e.data.source);
                                // Rebind subscriptions to the current wrapper and invalidate
                                // commands captured before this topology/identity refresh.
                                e.data.generation = previous.map_or(0, |e| e.data.generation);
                                dirty.insert(id, ALL);
                                rebuilt.push(e);
                            }
                            Err(_) => {
                                notice =
                                    Some("Une session ne permet pas le suivi événementiel".into())
                            }
                        }
                    }
                    entries = rebuilt; // revoke all replaced subscriptions on the MTA
                }
                Err(_) => {
                    entries.clear();
                    notice =
                        Some("Sessions Windows temporairement indisponibles — Réessayer".into());
                }
            }
        }
        if let Some(selection) = selection {
            manual = selection;
        }
        if !entries.iter().any(|e| Some(e.data.id) == manual) {
            manual = None;
        }
        let mut metadata_pending = false;
        for entry in &mut entries {
            let flags = dirty.get(&entry.data.id).copied().unwrap_or(0);
            if flags & META != 0 || entry.data.generation == 0 || refresh {
                entry.data.invalidate_metadata();
                entry.art_loaded = false;
                dirty
                    .entry(entry.data.id)
                    .and_modify(|f| *f |= META)
                    .or_insert(META);
                metadata_pending = true;
            }
        }
        if metadata_pending || topology || refresh || selection.is_some() {
            // Publish cleared content before asynchronous reads (up to 5s each),
            // preserving the previous valid selection until playback policy is known.
            selected =
                manual.or_else(|| selected.filter(|id| entries.iter().any(|e| e.data.id == *id)));
            shared.publish(Snapshot {
                sessions: entries.iter().map(|e| e.data.clone()).collect(),
                selected,
                manual,
                notice: notice.clone(),
            });
        }
        for entry in &mut entries {
            if shared.input.lock().unwrap().stop {
                return;
            }
            let flags = dirty.get(&entry.data.id).copied().unwrap_or(0);
            if flags & META != 0 || entry.data.generation == 0 || refresh {
                let epoch = shared.epoch.load(Ordering::Acquire);
                if entry.metadata(shared).is_err() {
                    notice = Some("Métadonnées temporairement indisponibles".into());
                    if shared.epoch.load(Ordering::Acquire) != epoch {
                        shared.signal(entry.data.id, META);
                    }
                }
            }
            if entry.playback().is_err() {
                entry.data.playback = Playback::Unknown;
                entry.data.capabilities = Capabilities::default();
                entry.data.timeline = None;
            }
        }
        let current = m.native.GetCurrentSession().ok().and_then(|s| {
            entries
                .iter()
                .find(|e| e.native == s)
                .map(|e| e.data.id)
                .or_else(|| {
                    let source =
                        bounded_text(&s.SourceAppUserModelId().ok()?.to_string_lossy(), 256);
                    let mut found = entries
                        .iter()
                        .filter(|e| !source.is_empty() && e.data.source == source);
                    let first = found.next()?;
                    found.next().is_none().then_some(first.data.id)
                })
        });
        let data: Vec<_> = entries.iter().map(|e| e.data.clone()).collect();
        selected = select(&data, manual, current, selected);
        if let Some(command) = command {
            let result = entries
                .iter_mut()
                .find(|e| e.data.id == command.id && e.data.generation == command.generation)
                .ok_or_else(|| Error::from_hresult(ABORT))
                .and_then(|e| {
                    e.playback()?;
                    if !e.data.capabilities.allows(command.action)
                        || !shared.command_current(command, selected)
                    {
                        return Ok(false);
                    }
                    let op = match command.action {
                        Command::Play => e.native.TryPlayAsync(),
                        Command::Pause => e.native.TryPauseAsync(),
                        Command::Toggle => e.native.TryTogglePlayPauseAsync(),
                        Command::Previous => e.native.TrySkipPreviousAsync(),
                        Command::Next => e.native.TrySkipNextAsync(),
                    }?;
                    let result = complete(op, shared, None);
                    let _ = e.playback();
                    result
                });
            if !matches!(result, Ok(true)) {
                notice = Some("Commande refusée ou session fermée".into());
            }
            shared.busy.store(false, Ordering::Release);
        }
        // Only one decoded cover is cached. Unselected images are released immediately.
        for e in &mut entries {
            if Some(e.data.id) != selected {
                e.data.artwork = None;
                e.art_loaded = false;
            }
        }
        shared.publish(Snapshot {
            sessions: entries.iter().map(|e| e.data.clone()).collect(),
            selected,
            manual,
            notice: notice.clone(),
        });
        if let Some(e) = entries
            .iter_mut()
            .find(|e| Some(e.data.id) == selected && !e.art_loaded)
        {
            let epoch = shared.epoch.load(Ordering::Acquire);
            if e.artwork(shared).is_err() && shared.epoch.load(Ordering::Acquire) != epoch {
                // Cancellation means unfinished, not "no cover". A selection can
                // cancel the current image even when the selected source is unchanged.
                e.art_loaded = false;
                shared.signal(e.data.id, STATE);
            }
            if e.data.artwork.is_some() {
                shared.publish(Snapshot {
                    sessions: entries.iter().map(|e| e.data.clone()).collect(),
                    selected,
                    manual,
                    notice,
                });
            }
        }
    }
    // Entries and manager (and their callbacks) drop before RoUninitialize on this thread.
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shared() -> Arc<Shared> {
        Arc::new(Shared {
            input: Mutex::new(Input::default()),
            wake: Condvar::new(),
            epoch: AtomicU64::new(1),
            busy: AtomicBool::new(false),
            hwnd: 0,
        })
    }
    #[test]
    fn bounded_mailbox_and_single_pending_command() {
        let s = shared();
        for i in 1..1000 {
            s.signal(i, META);
        }
        assert_eq!(s.input.lock().unwrap().dirty.len(), MAX_SESSIONS);
        let w = Worker {
            shared: s.clone(),
            thread: None,
        };
        assert!(w.command(1, 2, Command::Play));
        assert!(!w.command(2, 3, Command::Pause));
        for id in 1..1000 {
            s.publish(Snapshot {
                selected: Some(id),
                ..Default::default()
            });
        }
        assert_eq!(w.take().unwrap().selected, Some(999));
        assert!(w.take().is_none());
    }
    #[test]
    fn disabled_module_ignores_callbacks_and_rejects_commands() {
        let s = shared();
        let worker = Worker {
            shared: s.clone(),
            thread: None,
        };
        worker.set_enabled(false);
        let epoch = s.epoch.load(Ordering::Acquire);
        assert!(!worker.command(1, 1, Command::Play));
        worker.select(Some(9));
        worker.refresh();
        s.signal(1, META);
        s.signal(0, ALL);
        let i = s.input.lock().unwrap();
        assert!(i.disabled && i.dirty.is_empty() && i.select.is_none() && i.command.is_none());
        assert_eq!(s.epoch.load(Ordering::Acquire), epoch);
        drop(i);
        assert!(complete(IAsyncOperation::ready(Ok(true)), &s, None).is_err());
        worker.set_enabled(true);
        assert!(!s.input.lock().unwrap().disabled);
        assert!(worker.command(1, 1, Command::Play));
    }
    #[test]
    fn async_results_failures_and_stale_generation() {
        let s = shared();
        assert!(complete(IAsyncOperation::ready(Ok(true)), &s, Some(1)).unwrap());
        assert!(
            complete(
                IAsyncOperation::<bool>::ready(Err(Error::from_hresult(ABORT))),
                &s,
                None
            )
            .is_err()
        );
        s.epoch.store(2, Ordering::Release);
        assert!(complete(IAsyncOperation::ready(Ok(true)), &s, Some(1)).is_err());
        s.input.lock().unwrap().stop = true;
        assert!(complete(IAsyncOperation::ready(Ok(true)), &s, None).is_err());
    }
    #[test]
    fn pending_command_rejects_events_received_after_mailbox_drain() {
        let s = shared();
        let w = Worker {
            shared: s.clone(),
            thread: None,
        };
        assert!(w.command(7, 3, Command::Pause));
        let ticket = s.input.lock().unwrap().command.take().unwrap();
        assert!(s.command_current(ticket, Some(7)));
        s.signal(7, STATE); // Timeline/playback notification alone is not a new track.
        assert!(s.command_current(ticket, Some(7)));
        assert!(!s.command_current(ticket, Some(8)));
        s.signal(7, META); // Arrives after the worker's current batch was captured.
        assert!(!s.command_current(ticket, Some(7)));
        // A click on an old UI snapshot after the event must also be rejected, even
        // though its epoch is already current: pending metadata is not processed yet.
        let current_ticket = PendingCommand {
            epoch: s.epoch.load(Ordering::Acquire),
            ..ticket
        };
        assert!(!s.command_current(current_ticket, Some(7)));
        s.input.lock().unwrap().dirty.clear();
        assert!(s.command_current(current_ticket, Some(7)));
        w.select(None);
        assert!(!s.command_current(current_ticket, Some(7)));
        s.input.lock().unwrap().select.take();
        let latest = PendingCommand {
            epoch: s.epoch.load(Ordering::Acquire),
            ..ticket
        };
        w.refresh();
        assert!(!s.command_current(latest, Some(7)));
        s.input.lock().unwrap().refresh = false;
        let latest = PendingCommand {
            epoch: s.epoch.load(Ordering::Acquire),
            ..ticket
        };
        s.signal(0, ALL);
        assert!(!s.command_current(latest, Some(7)));
    }
    #[test]
    fn metadata_change_interrupts_async_without_publishing_old_result() {
        let s = shared();
        let (release, gate) = std::sync::mpsc::channel();
        let (finished, done) = std::sync::mpsc::channel();
        let op = IAsyncOperation::spawn(move || {
            gate.recv().unwrap();
            finished.send(()).unwrap();
            Ok(true)
        });
        let copy = s.clone();
        let join = std::thread::spawn(move || complete(op, &copy, Some(1)));
        s.signal(7, META);
        let result = join.join().unwrap();
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(
            result.is_err(),
            "old asynchronous result must not reach metadata assignment"
        );
        assert!(!s.current(1));
        assert!(s.current(2));
    }
    #[test]
    fn stop_interrupts_pending_async_without_waiting_for_provider() {
        let s = shared();
        let (release, gate) = std::sync::mpsc::channel();
        let (finished, done) = std::sync::mpsc::channel();
        let op = IAsyncOperation::spawn(move || {
            gate.recv().unwrap();
            finished.send(()).unwrap();
            Ok(true)
        });
        let copy = s.clone();
        let join = std::thread::spawn(move || complete(op, &copy, None));
        let start = Instant::now();
        {
            s.input.lock().unwrap().stop = true;
            s.wake.notify_all();
        }
        let result = join.join().unwrap();
        // Release even if the assertion below fails: no blocked fixture thread.
        release.send(()).unwrap();
        done.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(result.is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
    }
    #[test]
    fn media_notification_survives_native_menu_reentrancy() {
        use windows_sys::{
            Win32::{System::LibraryLoader::GetModuleHandleW, UI::WindowsAndMessaging::*},
            w,
        };
        let _guard = crate::NATIVE_TEST_LOCK.lock().unwrap();
        // Hidden owned HWND only; proves the coalesced data survives a reentrant notification.
        unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class = w!("DynamicIsland.Test.MediaPending");
            let descriptor = WNDCLASSW {
                lpfnWndProc: Some(crate::window_proc),
                hInstance: instance,
                lpszClassName: class,
                ..Default::default()
            };
            assert_ne!(RegisterClassW(&descriptor), 0);
            let hwnd = CreateWindowExW(
                0,
                class,
                class,
                WS_POPUP,
                0,
                0,
                1,
                1,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                instance,
                std::ptr::null(),
            );
            assert!(!hwnd.is_null());
            let s = shared();
            s.input.lock().unwrap().latest = Some(Snapshot {
                selected: Some(7),
                sessions: vec![Session {
                    id: 7,
                    ..Default::default()
                }],
                ..Default::default()
            });
            let mut app = crate::App::for_window(hwnd);
            app.hidden = true;
            app.media_worker = Some(Worker {
                shared: s,
                thread: None,
            });
            crate::APP.with(|cell| *cell.borrow_mut() = Some(app));
            let result = std::panic::catch_unwind(|| {
                crate::APP.with(|cell| {
                    let _state = cell.borrow_mut();
                    SendMessageW(hwnd, crate::MEDIA_CHANGED, 0, 0);
                });
                SendMessageW(hwnd, WM_NULL, 0, 0);
                crate::APP.with(|cell| {
                    assert_eq!(
                        cell.borrow().as_ref().unwrap().media_view.snapshot.selected,
                        Some(7)
                    )
                });
            });
            crate::APP.with(|cell| drop(cell.borrow_mut().take()));
            crate::MEDIA_PENDING.set(false);
            DestroyWindow(hwnd);
            UnregisterClassW(class, instance);
            if let Err(p) = result {
                std::panic::resume_unwind(p);
            }
        }
    }
}
