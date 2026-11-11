//! One-shot native probes, intended for an on-demand worker. No timer or polling here.
use island_core::system::{Battery, CpuTimes, Memory, Selection};
use windows_sys::Win32::{
    Foundation::FILETIME,
    System::{Power::*, SystemInformation::*, Threading::*},
};

#[derive(Debug, Default)]
pub struct Snapshot {
    pub cpu: Option<CpuTimes>,
    pub memory: Option<Memory>,
    pub battery: Option<Battery>,
    pub ac_online: Option<bool>,
    /// Multi-group systems are not represented as whole-machine CPU by GetSystemTimes.
    pub cpu_scope_unsupported: bool,
}
pub fn sample(selection: Selection) -> Snapshot {
    let mut result = Snapshot::default();
    // SAFETY: stack output structures are initialized and live across each synchronous call.
    // No retained pointers, subscriptions, acquired handles or system mutations.
    unsafe {
        if selection.cpu {
            let groups = GetActiveProcessorGroupCount();
            result.cpu_scope_unsupported = groups > 1;
            if groups == 1 {
                let mut idle = FILETIME::default();
                let mut kernel = FILETIME::default();
                let mut user = FILETIME::default();
                if GetSystemTimes(&mut idle, &mut kernel, &mut user) != 0 {
                    let ticks = |v: FILETIME| {
                        (u64::from(v.dwHighDateTime) << 32) | u64::from(v.dwLowDateTime)
                    };
                    result.cpu = Some(CpuTimes {
                        idle: ticks(idle),
                        kernel: ticks(kernel),
                        user: ticks(user),
                    });
                }
            }
        }
        if selection.memory {
            let mut memory = MEMORYSTATUSEX {
                dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };
            if GlobalMemoryStatusEx(&mut memory) != 0 {
                result.memory = Memory::new(memory.ullTotalPhys, memory.ullAvailPhys);
            }
        }
        if selection.battery {
            let mut power = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut power) != 0 {
                result.battery = Some(Battery::from_flags(
                    power.BatteryFlag,
                    power.BatteryLifePercent,
                ));
                result.ac_online = match power.ACLineStatus {
                    0 => Some(false),
                    1 => Some(true),
                    _ => None,
                };
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disabled_probes_return_no_data() {
        let s = sample(Selection::default());
        assert!(s.cpu.is_none() && s.memory.is_none() && s.battery.is_none());
        assert!(s.ac_online.is_none() && !s.cpu_scope_unsupported);
    }
    #[test]
    fn selected_native_probes_are_independent_and_bounded() {
        let s = sample(Selection {
            memory: true,
            ..Default::default()
        });
        let memory = s.memory.expect("GlobalMemoryStatusEx unavailable");
        assert!(memory.used_percent() >= 0.0 && memory.used_percent() <= 100.0);
        assert!(s.cpu.is_none() && s.battery.is_none());
        let s = sample(Selection {
            cpu: true,
            battery: true,
            ..Default::default()
        });
        assert!(s.cpu.is_some() || s.cpu_scope_unsupported);
        assert!(s.memory.is_none());
        assert!(s.battery.is_some());
    }
}
