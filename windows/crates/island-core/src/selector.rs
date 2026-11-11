//! Stable choices shared by native input and drawing; no platform handles.
use crate::media::{MAX_SESSIONS, Snapshot};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Automatic,
    Session(u64),
    Refresh,
}
pub struct Selector {
    pub choices: Vec<Choice>,
    pub focus: Choice,
    pub pressed: Option<Choice>,
    pub hover: Option<Choice>,
    pub offset: usize,
    pub page: usize,
}
impl Selector {
    pub fn new(snapshot: &Snapshot, page: usize) -> Self {
        let mut s = Self {
            choices: vec![],
            focus: Choice::Automatic,
            pressed: None,
            hover: None,
            offset: 0,
            page: page.max(1),
        };
        s.update(snapshot);
        s
    }
    pub fn update(&mut self, snapshot: &Snapshot) {
        self.choices = std::iter::once(Choice::Automatic)
            .chain(
                snapshot
                    .sessions
                    .iter()
                    .take(MAX_SESSIONS)
                    .map(|s| Choice::Session(s.id)),
            )
            .chain(Some(Choice::Refresh))
            .collect();
        if !self.choices.contains(&self.focus) {
            self.focus = Choice::Automatic;
        }
        self.pressed = self.pressed.filter(|c| self.choices.contains(c));
        self.hover = self.hover.filter(|c| self.choices.contains(c));
        self.reveal();
    }
    pub fn navigate(&mut self, delta: isize) {
        let i = self
            .choices
            .iter()
            .position(|c| *c == self.focus)
            .unwrap_or(0);
        self.focus =
            self.choices[(i as isize + delta).rem_euclid(self.choices.len() as isize) as usize];
        self.hover = None;
        self.pressed = None;
        self.reveal();
    }
    fn reveal(&mut self) {
        let i = self
            .choices
            .iter()
            .position(|c| *c == self.focus)
            .unwrap_or(0);
        if i < self.offset {
            self.offset = i;
        }
        if i >= self.offset + self.page {
            self.offset = i + 1 - self.page;
        }
        self.offset = self
            .offset
            .min(self.choices.len().saturating_sub(self.page));
    }
    pub fn at(&self, row: usize) -> Option<Choice> {
        (row < self.page)
            .then(|| self.choices.get(self.offset + row).copied())
            .flatten()
    }
    pub fn release(&mut self, over: Option<Choice>) -> Option<Choice> {
        self.pressed
            .take()
            .filter(|c| Some(*c) == over && self.choices.contains(c))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::Session;
    #[test]
    fn removal_during_press_never_selects_replacement_row() {
        let mut snap = Snapshot {
            sessions: vec![
                Session {
                    id: 4,
                    ..Default::default()
                },
                Session {
                    id: 9,
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let mut s = Selector::new(&snap, 3);
        s.pressed = Some(Choice::Session(4));
        s.focus = Choice::Session(4);
        snap.sessions.remove(0);
        s.update(&snap);
        assert_eq!(s.release(s.at(1)), None);
        assert_eq!(s.focus, Choice::Automatic);
        for _ in 0..100 {
            s.navigate(1);
            assert!(s.offset + s.page >= s.choices.len());
        }
        s.navigate(-1);
        assert!(s.choices.contains(&s.focus));
    }
}
