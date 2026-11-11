//! Bounded, silent SMTC provider and end-to-end test. Never controls other players.
use island_core::media::{Command, Playback, Snapshot};
use island_windows::media::Worker;
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use windows::{
    Foundation::{TimeSpan, TypedEventHandler},
    Media::{Playback::MediaPlayer, *},
    Storage::Streams::{DataWriter, InMemoryRandomAccessStream, RandomAccessStreamReference},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::{HSTRING, Result},
};
const PREFIX: &str = "DynamicIsland TEST ONLY ";
struct Fixture {
    player: MediaPlayer,
    controls: SystemMediaTransportControls,
    token: i64,
    events: mpsc::Receiver<SystemMediaTransportControlsButton>,
}
impl Fixture {
    fn new(name: &str, video: bool) -> Result<Self> {
        let player = MediaPlayer::new()?;
        player.SetIsMuted(true)?;
        player.CommandManager()?.SetIsEnabled(false)?;
        let controls = player.SystemMediaTransportControls()?;
        controls.SetIsEnabled(true)?;
        controls.SetIsPlayEnabled(true)?;
        controls.SetIsPauseEnabled(true)?;
        controls.SetIsNextEnabled(true)?;
        controls.SetIsPreviousEnabled(false)?;
        controls.SetPlaybackRate(1.0)?;
        let (tx, events) = mpsc::channel();
        let token = controls.ButtonPressed(&TypedEventHandler::<
            SystemMediaTransportControls,
            SystemMediaTransportControlsButtonPressedEventArgs,
        >::new(move |_, args| {
            if let Some(args) = args.as_ref() {
                let _ = tx.send(args.Button()?);
            }
            Ok(())
        }))?;
        let fixture = Self {
            player,
            controls,
            token,
            events,
        };
        fixture.title(name, video)?;
        fixture.timeline(5)?;
        fixture
            .controls
            .SetPlaybackStatus(MediaPlaybackStatus::Playing)?;
        Ok(fixture)
    }
    fn title(&self, name: &str, video: bool) -> Result<()> {
        let u = self.controls.DisplayUpdater()?;
        let title = HSTRING::from(format!("{PREFIX}{name}"));
        if video {
            u.SetType(MediaPlaybackType::Video)?;
            u.VideoProperties()?.SetTitle(&title)?;
        } else {
            u.SetType(MediaPlaybackType::Music)?;
            u.MusicProperties()?.SetTitle(&title)?;
            u.MusicProperties()?
                .SetArtist(&HSTRING::from("Données synthétiques, aucun son"))?;
        }
        // Original solid-color BMP, entirely in memory; exercises Windows' real decoder.
        let mut bmp = vec![0u8; 54 + 64 * 64 * 4];
        bmp[0..2].copy_from_slice(b"BM");
        let length = bmp.len() as u32;
        bmp[2..6].copy_from_slice(&length.to_le_bytes());
        bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
        bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
        bmp[18..22].copy_from_slice(&64i32.to_le_bytes());
        bmp[22..26].copy_from_slice(&64i32.to_le_bytes());
        bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
        bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
        for p in bmp[54..].as_chunks_mut::<4>().0 {
            p.copy_from_slice(&[80, 60, 30, 255]);
        }
        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(&bmp)?;
        writer.StoreAsync()?.join()?;
        writer.DetachStream()?;
        stream.Seek(0)?;
        u.SetThumbnail(&RandomAccessStreamReference::CreateFromStream(&stream)?)?;
        u.Update()
    }
    fn timeline(&self, seconds: i64) -> Result<()> {
        let t = SystemMediaTransportControlsTimelineProperties::new()?;
        t.SetStartTime(TimeSpan { Duration: 0 })?;
        t.SetEndTime(TimeSpan {
            Duration: 3600 * 10_000_000,
        })?;
        t.SetPosition(TimeSpan {
            Duration: seconds * 10_000_000,
        })?;
        t.SetMinSeekTime(TimeSpan { Duration: 0 })?;
        t.SetMaxSeekTime(TimeSpan {
            Duration: 3600 * 10_000_000,
        })?;
        self.controls.UpdateTimelineProperties(&t)
    }
    fn dispatch(&self) -> Result<()> {
        for b in self.events.try_iter() {
            let status = if b == SystemMediaTransportControlsButton::Pause {
                MediaPlaybackStatus::Paused
            } else {
                MediaPlaybackStatus::Playing
            };
            self.controls.SetPlaybackStatus(status)?;
            if b == SystemMediaTransportControlsButton::Next {
                self.title("NEXT", false)?;
            }
        }
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.controls.RemoveButtonPressed(self.token);
        let _ = self.controls.SetIsEnabled(false);
        let _ = self.player.Close();
    }
}
fn wait(worker: &Worker, fixture: &Fixture, check: impl Fn(&Snapshot) -> bool) -> Snapshot {
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        fixture.dispatch().expect("fixture event");
        if let Some(s) = worker.take() {
            if std::env::var_os("ISLAND_FIXTURE_TRACE").is_some() {
                println!(
                    "snapshot selected={:?} manual={:?} own={:?}",
                    s.selected,
                    s.manual,
                    s.sessions
                        .iter()
                        .filter(|s| s.title.starts_with(PREFIX))
                        .map(|s| (s.id, s.title.as_str()))
                        .collect::<Vec<_>>()
                );
            }
            if check(&s) {
                return s;
            }
        }
        std::thread::sleep(Duration::from_millis(20)); // Bounded test only; product has no polling.
    }
    panic!("GSMTC fixture condition timed out");
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let name = args
        .iter()
        .position(|s| s == "--name")
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
        .unwrap_or("A");
    let app_id: Vec<u16> = format!("DynamicIsland.Test.Media.{name}")
        .encode_utf16()
        .chain(Some(0))
        .collect();
    // SAFETY: per-process test identity only, not a shortcut or a Windows setting.
    let hr = unsafe {
        windows_sys::Win32::UI::Shell::SetCurrentProcessExplicitAppUserModelID(app_id.as_ptr())
    };
    assert!(hr >= 0, "test AUMID");
    let video = args.iter().any(|s| s == "--video");
    let fixture = Fixture::new(name, video)?;
    if let Some(i) = args.iter().position(|s| s == "--hold") {
        let duration = args
            .get(i + 1)
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(60)
            .clamp(1, 900);
        println!("Synthetic silent session, hold_seconds={duration} video={video}");
        let end = Instant::now() + Duration::from_secs(duration);
        while Instant::now() < end {
            fixture.dispatch()?;
            std::thread::sleep(Duration::from_millis(100));
        }
        return Ok(());
    }
    let worker = Worker::new(0).expect("worker");
    let s = wait(&worker, &fixture, |s| {
        s.sessions.iter().any(|s| s.title == format!("{PREFIX}A"))
    });
    let id = s
        .sessions
        .iter()
        .find(|s| s.title == format!("{PREFIX}A"))
        .unwrap()
        .id;
    worker.select(Some(id));
    let s = wait(&worker, &fixture, |s| {
        s.manual == Some(id)
            && s.active()
                .is_some_and(|s| s.artwork.is_some() && s.timeline.is_some())
    });
    println!("PASS real GSMTC detection, manual selection, timeline, thumbnail decode");
    let own = s.active().unwrap();
    assert!(own.title.starts_with(PREFIX));
    assert!(!own.capabilities.previous && own.capabilities.next);
    let first_generation = own.generation;
    assert!(worker.command(id, own.generation, Command::Pause));
    let s = wait(&worker, &fixture, |s| {
        s.active()
            .is_some_and(|s| s.id == id && s.playback == Playback::Paused)
    });
    assert!(worker.command(id, s.active().unwrap().generation, Command::Play));
    let s = wait(&worker, &fixture, |s| {
        s.active()
            .is_some_and(|s| s.id == id && s.playback == Playback::Playing)
    });
    println!("PASS real Pause/Play routed only to fixture");
    assert!(worker.command(id, s.active().unwrap().generation, Command::Next));
    wait(&worker, &fixture, |s| {
        s.active()
            .is_some_and(|s| s.id == id && s.title == format!("{PREFIX}NEXT"))
    });
    println!("PASS Next and metadata notification");
    for i in 0..30 {
        fixture.title(&format!("RAPID{i}"), false)?;
    }
    wait(&worker, &fixture, |s| {
        s.active().is_some_and(|s| {
            s.id == id && s.title == format!("{PREFIX}RAPID29") && s.artwork.is_some()
        })
    });
    println!("PASS 30 rapid metadata changes settle with matching artwork");
    assert!(worker.command(id, first_generation, Command::Pause));
    wait(&worker, &fixture, |s| {
        s.notice.as_deref() == Some("Commande refusée ou session fermée")
    });
    assert!(fixture.events.try_recv().is_err());
    println!("PASS stale-generation command rejected before provider dispatch");
    let updater = fixture.controls.DisplayUpdater()?;
    updater.ClearAll()?;
    updater.SetType(MediaPlaybackType::Music)?;
    updater
        .MusicProperties()?
        .SetTitle(&HSTRING::from(format!("{PREFIX}NO_COVER")))?;
    updater.Update()?;
    wait(&worker, &fixture, |s| {
        s.active().is_some_and(|s| {
            s.id == id
                && s.title == format!("{PREFIX}NO_COVER")
                && s.artwork.is_none()
                && s.artist.is_empty()
        })
    });
    println!("PASS missing cover and artist clear previous metadata");
    fixture.controls.SetIsNextEnabled(false)?;
    let s = wait(&worker, &fixture, |s| {
        s.active()
            .is_some_and(|s| s.id == id && !s.capabilities.next)
    });
    assert!(worker.command(id, s.active().unwrap().generation, Command::Next));
    wait(&worker, &fixture, |s| {
        s.notice.as_deref() == Some("Commande refusée ou session fermée")
    });
    assert!(fixture.events.try_recv().is_err());
    println!("PASS unavailable Next rejected before provider dispatch");
    use std::os::windows::process::CommandExt;
    let mut second = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--name", "SECOND", "--video", "--hold", "6"])
        .creation_flags(0x08000000) // CREATE_NO_WINDOW: no console or foreground change.
        .spawn()
        .expect("owned bounded second fixture");
    wait(&worker, &fixture, |s| {
        s.manual == Some(id)
            && s.selected == Some(id)
            && s.sessions
                .iter()
                .any(|s| s.title == format!("{PREFIX}SECOND"))
    });
    println!("PASS two real SMTC fixture sessions, video and music, manual pin retained");
    assert!(second.wait().unwrap().success());
    fixture.controls.SetIsEnabled(false)?;
    wait(&worker, &fixture, |s| {
        !s.sessions.iter().any(|s| s.id == id) && s.manual.is_none()
    });
    println!("PASS closed session removed and manual pin cleared");
    assert!(worker.command(id, first_generation, Command::Play));
    wait(&worker, &fixture, |s| {
        s.notice.as_deref() == Some("Commande refusée ou session fermée")
    });
    println!("PASS command addressed to removed session rejected; no fallback player controlled");
    fixture.title("RETURNED", false)?;
    fixture.controls.SetIsEnabled(true)?;
    let s = wait(&worker, &fixture, |s| {
        s.sessions
            .iter()
            .any(|s| s.title == format!("{PREFIX}RETURNED"))
    });
    let returned = s
        .sessions
        .iter()
        .find(|s| s.title == format!("{PREFIX}RETURNED"))
        .unwrap();
    assert_ne!(
        returned.id, id,
        "observed removed session must not retain its retired ID"
    );
    assert!(s.manual.is_none());
    worker.select(Some(returned.id));
    wait(&worker, &fixture, |s| {
        s.manual == Some(returned.id) && s.selected == Some(returned.id)
    });
    println!("PASS renewed source gets fresh identity and can be selected again");
    worker.set_enabled(false);
    wait(&worker, &fixture, |s| {
        s.sessions.is_empty() && s.selected.is_none()
    });
    assert!(!worker.command(returned.id, returned.generation, Command::Play));
    fixture.title("DISABLED", false)?;
    std::thread::sleep(Duration::from_millis(200));
    assert!(
        worker.take().is_none(),
        "disabled module must not publish provider callbacks"
    );
    worker.set_enabled(true);
    wait(&worker, &fixture, |s| {
        s.sessions
            .iter()
            .any(|s| s.title == format!("{PREFIX}DISABLED"))
    });
    println!(
        "PASS disabled module clears sessions, rejects commands, sleeps and reacquires on enable"
    );
    let start = Instant::now();
    drop(worker);
    println!("PASS shutdown_ms={}", start.elapsed().as_millis());
    Ok(())
}
fn main() -> Result<()> {
    // SAFETY: this test owns an MTA; all WinRT values drop before uninitialization.
    unsafe {
        RoInitialize(RO_INIT_MULTITHREADED)?;
    }
    let result = run();
    unsafe {
        RoUninitialize();
    }
    result
}
