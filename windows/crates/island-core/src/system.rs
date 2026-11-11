//! Validated system snapshots and differences. No clocks, I/O or background activity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CpuTimes {
    pub idle: u64,
    /// Windows kernel time includes idle time.
    pub kernel: u64,
    pub user: u64,
}
#[derive(Default)]
pub struct CpuMeter(Option<CpuTimes>);
impl CpuMeter {
    pub fn reset(&mut self) {
        self.0 = None;
    }
    /// None means warming up, reset, API failure or inconsistent counters, never 0%.
    pub fn sample(&mut self, current: Option<CpuTimes>) -> Option<f64> {
        let previous = std::mem::replace(&mut self.0, current)?;
        let current = current?;
        let idle = current.idle.checked_sub(previous.idle)?;
        let kernel = current.kernel.checked_sub(previous.kernel)?;
        let user = current.user.checked_sub(previous.user)?;
        let total = kernel.checked_add(user)?;
        let busy = total.checked_sub(idle)?;
        (total > 0).then(|| 100.0 * busy as f64 / total as f64)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Memory {
    pub total: u64,
    pub available: u64,
}
impl Memory {
    pub fn new(total: u64, available: u64) -> Option<Self> {
        (total > 0 && available <= total).then_some(Self { total, available })
    }
    pub fn used_percent(self) -> f64 {
        100.0 * (self.total - self.available) as f64 / self.total as f64
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Battery {
    Unknown,
    Absent,
    Present { percent: Option<u8>, charging: bool },
}
impl Battery {
    pub fn from_flags(flags: u8, percent: u8) -> Self {
        if flags == 255 {
            Self::Unknown
        } else if flags & 128 != 0 {
            Self::Absent
        } else {
            Self::Present {
                percent: (percent <= 100).then_some(percent),
                charging: flags & 8 != 0,
            }
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Selection {
    pub cpu: bool,
    pub memory: bool,
    pub battery: bool,
}
impl Selection {
    pub fn any(self) -> bool {
        self.cpu || self.memory || self.battery
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn times(idle: u64, kernel: u64, user: u64) -> Option<CpuTimes> {
        Some(CpuTimes { idle, kernel, user })
    }
    #[test]
    fn cpu_includes_idle_and_uses_differences() {
        let mut m = CpuMeter::default();
        assert_eq!(m.sample(times(80, 100, 100)), None);
        assert_eq!(m.sample(times(160, 200, 200)), Some(60.0));
        m.reset();
        assert_eq!(m.sample(times(160, 200, 200)), None);
        assert_eq!(m.sample(times(160, 200, 200)), None);
    }
    #[test]
    fn invalid_or_missing_cpu_never_fabricates_zero() {
        let mut m = CpuMeter::default();
        m.sample(times(0, 100, 100));
        assert_eq!(m.sample(times(1, 0, 0)), None);
        assert_eq!(m.sample(None), None);
        assert_eq!(m.sample(times(0, 0, 0)), None);
        assert_eq!(m.sample(times(10, 1, 1)), None);
        m.reset();
        m.sample(times(0, 0, 0));
        assert_eq!(m.sample(times(0, u64::MAX, 1)), None);
    }
    #[test]
    fn memory_and_battery_unknown_states_are_explicit() {
        assert_eq!(Memory::new(100, 25).unwrap().used_percent(), 75.0);
        assert!(Memory::new(0, 0).is_none());
        assert!(Memory::new(100, 101).is_none());
        assert_eq!(Battery::from_flags(255, 255), Battery::Unknown);
        assert_eq!(Battery::from_flags(128, 100), Battery::Absent);
        assert_eq!(
            Battery::from_flags(8, 255),
            Battery::Present {
                percent: None,
                charging: true
            }
        );
        assert!(!Selection::default().any());
    }
}
