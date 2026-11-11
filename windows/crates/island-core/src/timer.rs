//! Deterministic countdowns. `now` is monotonic milliseconds including system sleep.
//! No OS resources, tick accumulation, automatic phase skipping or wall-clock dependence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    pub minutes: u16,
    pub work: u16,
    pub short_break: u16,
    pub long_break: u16,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            minutes: 25,
            work: 25,
            short_break: 5,
            long_break: 15,
        }
    }
}
impl Config {
    pub fn valid(self) -> bool {
        [self.minutes, self.work, self.short_break, self.long_break]
            .into_iter()
            .all(|n| (1..=240).contains(&n))
    }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Timer,
    Pomodoro,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Countdown,
    Work,
    ShortBreak,
    LongBreak,
}
impl Phase {
    pub fn label(self) -> &'static str {
        match self {
            Self::Countdown => "Minuteur",
            Self::Work => "Travail",
            Self::ShortBreak => "Pause courte",
            Self::LongBreak => "Pause longue",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Ready,
    Running { deadline: u64 },
    Paused { remaining: u64 },
    Finished,
}
#[derive(Clone, Debug)]
pub struct Timer {
    config: Config,
    pub mode: Mode,
    pub phase: Phase,
    pub state: State,
    /// Completed work sessions within the current four-session cycle.
    pub completed: u8,
}
impl Default for Timer {
    fn default() -> Self {
        Self::new(Config::default())
    }
}
impl Timer {
    pub fn new(config: Config) -> Self {
        assert!(config.valid());
        Self {
            config,
            mode: Mode::Timer,
            phase: Phase::Countdown,
            state: State::Ready,
            completed: 0,
        }
    }
    pub fn configurable(&self) -> bool {
        matches!(self.state, State::Ready | State::Finished)
    }
    pub fn configure(&mut self, config: Config, mode: Mode) -> bool {
        if !config.valid() || !self.configurable() {
            return false;
        }
        self.config = config;
        self.mode = mode;
        self.reset();
        true
    }
    pub fn reset(&mut self) {
        self.phase = if self.mode == Mode::Timer {
            Phase::Countdown
        } else {
            Phase::Work
        };
        self.state = State::Ready;
        self.completed = 0;
    }
    pub fn duration(&self) -> u64 {
        let minutes = match self.phase {
            Phase::Countdown => self.config.minutes,
            Phase::Work => self.config.work,
            Phase::ShortBreak => self.config.short_break,
            Phase::LongBreak => self.config.long_break,
        };
        u64::from(minutes) * 60_000
    }
    pub fn remaining(&self, now: u64) -> u64 {
        match self.state {
            State::Ready => self.duration(),
            State::Finished => 0,
            State::Running { deadline } => deadline.saturating_sub(now),
            State::Paused { remaining } => remaining,
        }
    }
    /// A delayed/duplicate callback emits at most one completion, never a chain of breaks.
    pub fn settle(&mut self, now: u64) -> Option<Phase> {
        if matches!(self.state, State::Running { deadline } if now >= deadline) {
            self.state = State::Finished;
            if self.phase == Phase::Work {
                self.completed += 1;
            }
            Some(self.phase)
        } else {
            None
        }
    }
    /// Call settle first, so UI coordination can deliver completion before changing state.
    pub fn primary(&mut self, now: u64) {
        match self.state {
            State::Running { .. } => {
                self.state = State::Paused {
                    remaining: self.remaining(now),
                };
            }
            State::Paused { remaining } => {
                self.state = State::Running {
                    deadline: now.saturating_add(remaining),
                };
            }
            State::Ready | State::Finished => {
                if self.state == State::Finished {
                    self.phase = match self.phase {
                        Phase::Countdown => Phase::Countdown,
                        Phase::Work if self.completed == 4 => Phase::LongBreak,
                        Phase::Work => Phase::ShortBreak,
                        Phase::ShortBreak => Phase::Work,
                        Phase::LongBreak => {
                            self.completed = 0;
                            Phase::Work
                        }
                    };
                }
                self.state = State::Running {
                    deadline: now.saturating_add(self.duration()),
                };
            }
        }
    }
    pub fn pause(&mut self, now: u64) {
        if matches!(self.state, State::Running { .. }) {
            self.primary(now);
        }
    }
    pub fn primary_label(&self) -> &'static str {
        match self.state {
            State::Ready => "Démarrer",
            State::Running { .. } => "Pause",
            State::Paused { .. } => "Reprendre",
            State::Finished => match self.phase {
                Phase::Countdown => "Recommencer",
                Phase::Work if self.completed == 4 => "Pause longue",
                Phase::Work => "Pause courte",
                _ => "Travail suivant",
            },
        }
    }
    pub fn next_wake(&self, now: u64, visible: bool) -> Option<u64> {
        if let State::Running { deadline } = self.state {
            // Exact deadline while hidden. Visible seconds use ceiling, avoiding 00:00 early.
            let remaining = deadline.saturating_sub(now);
            Some(if visible && remaining > 0 {
                now.saturating_add((remaining - 1) % 1000 + 1)
            } else {
                deadline
            })
        } else {
            None
        }
    }
}
pub fn clock(ms: u64) -> String {
    let seconds = ms.div_ceil(1000);
    format!("{:02}:{:02}", seconds / 60, seconds % 60)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pause_resume_is_independent_of_refresh_and_disabled_time() {
        let mut t = Timer::default();
        t.primary(10_000);
        assert_eq!(t.remaining(11_234), 1_498_766);
        t.pause(11_234);
        assert_eq!(t.remaining(9_000_000), 1_498_766);
        assert_eq!(t.next_wake(9_000_000, true), None);
        t.primary(9_000_000);
        assert_eq!(t.remaining(9_000_001), 1_498_765);
        assert!(!t.configure(Config::default(), Mode::Pomodoro));
        t.reset();
        assert_eq!(t.state, State::Ready);
    }
    #[test]
    fn sleep_jump_completes_once_and_does_not_auto_start_another_phase() {
        let mut t = Timer::default();
        t.configure(Config::default(), Mode::Pomodoro);
        t.primary(0);
        assert_eq!(t.settle(86_400_000), Some(Phase::Work));
        assert_eq!(t.settle(86_400_001), None);
        assert_eq!(t.completed, 1);
        assert_eq!(t.next_wake(86_400_001, true), None);
        t.primary(86_400_001);
        assert_eq!(t.phase, Phase::ShortBreak);
        assert_eq!(t.remaining(86_400_001), 300_000);
    }
    #[test]
    fn four_work_sessions_then_long_break_and_new_cycle() {
        let mut t = Timer::default();
        t.configure(Config::default(), Mode::Pomodoro);
        let mut now = 0;
        for round in 1..=4 {
            t.primary(now);
            assert_eq!(t.phase, Phase::Work);
            now += t.duration();
            assert_eq!(t.settle(now), Some(Phase::Work));
            assert_eq!(t.completed, round);
            t.primary(now);
            assert_eq!(
                t.phase,
                if round == 4 {
                    Phase::LongBreak
                } else {
                    Phase::ShortBreak
                }
            );
            now += t.duration();
            assert!(t.settle(now).is_some());
        }
        t.primary(now);
        assert_eq!((t.phase, t.completed), (Phase::Work, 0));
    }
    #[test]
    fn scheduler_has_no_idle_wakes_and_only_one_deadline_when_invisible() {
        let mut t = Timer::default();
        assert_eq!(t.next_wake(0, false), None);
        t.primary(0);
        assert_eq!(t.next_wake(1234, false), Some(1_500_000));
        assert_eq!(t.next_wake(1234, true), Some(2000));
        assert_eq!(clock(1), "00:01");
        assert_eq!(clock(0), "00:00");
        assert_eq!(t.settle(1_499_999), None);
        assert_eq!(t.settle(1_500_000), Some(Phase::Countdown));
        assert_eq!(t.next_wake(1_500_000, true), None);
        let mut c = Config {
            minutes: 0,
            ..Default::default()
        };
        assert!(!t.configure(c, Mode::Timer));
        c.minutes = 241;
        assert!(!t.configure(c, Mode::Timer));
    }
}
