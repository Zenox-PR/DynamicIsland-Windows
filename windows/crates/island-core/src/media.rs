//! Media data and selection policy: no Windows objects, URLs, or external services.
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub const MAX_SESSIONS: usize = 32;
pub const ART_SIDE: u32 = 160;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Playback {
    Playing,
    Paused,
    Stopped,
    #[default]
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Play,
    Pause,
    Toggle,
    Previous,
    Next,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    pub play: bool,
    pub pause: bool,
    pub toggle: bool,
    pub previous: bool,
    pub next: bool,
}
impl Capabilities {
    pub fn allows(self, command: Command) -> bool {
        match command {
            Command::Play => self.play,
            Command::Pause => self.pause,
            Command::Toggle => self.toggle,
            Command::Previous => self.previous,
            Command::Next => self.next,
        }
    }
}
#[derive(Clone, Debug)]
pub struct Timeline {
    pub position: f64,
    pub duration: f64,
    pub sampled: Instant,
    pub rate: f64,
}
impl Timeline {
    pub fn position_at(&self, now: Instant, playing: bool) -> Option<f64> {
        let age = now.saturating_duration_since(self.sampled);
        // Do not extrapolate indefinitely when a provider stops updating its timeline.
        if age > Duration::from_secs(86400)
            || !self.duration.is_finite()
            || self.duration <= 0.0
            || !self.position.is_finite()
            || self.position < 0.0
        {
            return None;
        }
        let rate = if self.rate.is_finite() && (0.0..=8.0).contains(&self.rate) {
            self.rate
        } else {
            0.0
        };
        Some(
            (self.position
                + if playing {
                    age.as_secs_f64() * rate
                } else {
                    0.0
                })
            .clamp(0.0, self.duration),
        )
    }
}
#[derive(Clone, Debug)]
pub struct Artwork {
    pub width: u32,
    pub height: u32,
    pub bgra: Vec<u8>,
}
impl Artwork {
    pub fn valid(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.width <= ART_SIDE
            && self.height <= ART_SIDE
            && self.bgra.len() == self.width as usize * self.height as usize * 4
    }
}
#[derive(Clone, Debug, Default)]
pub struct Session {
    pub id: u64,
    pub generation: u64,
    pub source: String,
    /// Human label only. `source` remains the full identity used by GSMTC.
    pub source_name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub kind: Option<String>,
    pub playback: Playback,
    pub capabilities: Capabilities,
    pub timeline: Option<Timeline>,
    pub artwork: Option<Arc<Artwork>>,
}
impl Session {
    /// A changed track is pending: keep source identity, invalidate old content and
    /// commands before any asynchronous provider call can delay the next snapshot.
    pub fn invalidate_metadata(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.title.clear();
        self.artist.clear();
        self.album.clear();
        self.kind = None;
        self.artwork = None;
        self.timeline = None;
        self.capabilities = Capabilities::default();
        self.playback = Playback::Unknown;
    }
}
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub sessions: Vec<Session>,
    pub selected: Option<u64>,
    pub manual: Option<u64>,
    pub notice: Option<String>,
}
impl Snapshot {
    pub fn active(&self) -> Option<&Session> {
        self.sessions.iter().find(|s| Some(s.id) == self.selected)
    }
}
/// Manual pin > Windows' playing session > previously selected playing > first playing
/// > Windows current > previous valid > first. Stable IDs preserve enumeration ties.
pub fn select(
    sessions: &[Session],
    manual: Option<u64>,
    current: Option<u64>,
    previous: Option<u64>,
) -> Option<u64> {
    let exists = |id: Option<u64>| id.filter(|id| sessions.iter().any(|s| s.id == *id));
    let playing = |id: Option<u64>| {
        id.filter(|id| {
            sessions
                .iter()
                .any(|s| s.id == *id && s.playback == Playback::Playing)
        })
    };
    exists(manual)
        .or_else(|| playing(current))
        .or_else(|| playing(previous))
        .or_else(|| {
            sessions
                .iter()
                .find(|s| s.playback == Playback::Playing)
                .map(|s| s.id)
        })
        .or_else(|| exists(current))
        .or_else(|| exists(previous))
        .or_else(|| sessions.first().map(|s| s.id))
}
pub fn bounded_text(value: &str, max: usize) -> String {
    value
        .chars()
        .filter(|c| !c.is_control())
        .take(max)
        .collect()
}
pub fn clock(seconds: f64) -> String {
    let n = seconds.clamp(0.0, 359999.0) as u64;
    if n >= 3600 {
        format!("{}:{:02}:{:02}", n / 3600, n / 60 % 60, n % 60)
    } else {
        format!("{}:{:02}", n / 60, n % 60)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sessions() -> Vec<Session> {
        (1..=3)
            .map(|id| Session {
                id,
                playback: if id == 2 {
                    Playback::Playing
                } else {
                    Playback::Paused
                },
                ..Default::default()
            })
            .collect()
    }
    #[test]
    fn selection_pin_playback_current_and_disappearance() {
        let mut s = sessions();
        assert_eq!(select(&[], None, None, None), None);
        assert_eq!(select(&s, None, Some(1), Some(3)), Some(2));
        assert_eq!(select(&s, Some(1), Some(2), None), Some(1));
        assert_eq!(select(&s, Some(99), Some(1), None), Some(2));
        s[2].playback = Playback::Playing;
        assert_eq!(select(&s, None, None, Some(3)), Some(3));
        assert_eq!(select(&s, None, Some(2), Some(3)), Some(2));
        for entry in &mut s {
            entry.playback = Playback::Paused;
        }
        assert_eq!(select(&s, None, Some(1), Some(3)), Some(1));
    }
    #[test]
    fn timeline_pause_end_unknown_and_staleness() {
        let now = Instant::now();
        let mut t = Timeline {
            position: 5.0,
            duration: 10.0,
            sampled: now,
            rate: 1.0,
        };
        assert_eq!(t.position_at(now + Duration::from_secs(3), true), Some(8.0));
        assert_eq!(
            t.position_at(now + Duration::from_secs(3), false),
            Some(5.0)
        );
        assert_eq!(
            t.position_at(now + Duration::from_secs(10), true),
            Some(10.0)
        );
        assert_eq!(t.position_at(now + Duration::from_secs(86401), true), None);
        t.duration = 0.0;
        assert_eq!(t.position_at(now, false), None);
    }
    #[test]
    fn metadata_and_images_are_bounded() {
        assert_eq!(bounded_text("a\nb\0cdef", 4), "abcd");
        assert!(
            !Artwork {
                width: 10000,
                height: 1,
                bgra: vec![]
            }
            .valid()
        );
        assert!(!Capabilities::default().allows(Command::Play));
        assert_eq!(clock(3661.0), "1:01:01");
    }
    #[test]
    fn changed_track_clears_content_before_async_reload_but_keeps_identity() {
        let mut session = Session {
            id: 7,
            generation: 4,
            source: "source-id".into(),
            source_name: "Lecteur".into(),
            title: "Ancien titre".into(),
            artist: "Ancien artiste".into(),
            album: "Ancien album".into(),
            kind: Some("Musique".into()),
            playback: Playback::Playing,
            capabilities: Capabilities {
                next: true,
                ..Default::default()
            },
            artwork: Some(Arc::new(Artwork {
                width: 1,
                height: 1,
                bgra: vec![0, 0, 0, 255],
            })),
            timeline: Some(Timeline {
                position: 5.0,
                duration: 10.0,
                sampled: Instant::now(),
                rate: 1.0,
            }),
        };
        session.invalidate_metadata();
        assert_eq!((session.id, session.generation), (7, 5));
        assert_eq!(session.source, "source-id");
        assert_eq!(session.source_name, "Lecteur");
        assert!(session.title.is_empty() && session.artist.is_empty() && session.album.is_empty());
        assert!(session.kind.is_none() && session.artwork.is_none() && session.timeline.is_none());
        assert_eq!(session.playback, Playback::Unknown);
        assert_eq!(session.capabilities, Capabilities::default());
    }
}
