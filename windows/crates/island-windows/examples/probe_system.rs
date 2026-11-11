//! Bounded read-only probe latency check. No desktop window or permanent sampling.
use island_core::system::Selection;
use island_windows::system_info::sample;
use std::time::Instant;

fn main() {
    for (name, selection) in [
        ("disabled", Selection::default()),
        (
            "cpu",
            Selection {
                cpu: true,
                ..Default::default()
            },
        ),
        (
            "memory",
            Selection {
                memory: true,
                ..Default::default()
            },
        ),
        (
            "battery",
            Selection {
                battery: true,
                ..Default::default()
            },
        ),
    ] {
        let first = sample(selection);
        println!("{name}: first={first:?}");
        let started = Instant::now();
        let mut available = 0;
        for _ in 0..100 {
            let result = std::hint::black_box(sample(selection));
            available += usize::from(
                !selection.any()
                    || result.cpu.is_some()
                    || result.memory.is_some()
                    || result.battery.is_some(),
            );
        }
        println!(
            "{name}: calls=100 available={available} total_us={} (wall time, not process CPU)",
            started.elapsed().as_micros()
        );
    }
}
