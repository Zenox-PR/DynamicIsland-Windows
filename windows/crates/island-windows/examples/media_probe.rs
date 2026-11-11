//! Bounded read-only GSMTC diagnostic; never controls third-party sessions.
fn main() {
    let worker = island_windows::media::Worker::new(0).expect("media worker");
    let start = std::time::Instant::now();
    while start.elapsed() < std::time::Duration::from_secs(8) {
        if let Some(snapshot) = worker.take() {
            println!(
                "sessions={} selected={:?} notice={:?}",
                snapshot.sessions.len(),
                snapshot.selected,
                snapshot.notice
            );
            for s in snapshot.sessions {
                println!(
                    "id={} source={} name={} title_present={} artwork={} playback={:?} timeline={} controls={:?}",
                    s.id,
                    s.source,
                    s.source_name,
                    !s.title.is_empty(),
                    s.artwork.is_some(),
                    s.playback,
                    s.timeline.is_some(),
                    s.capabilities
                );
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let stop = std::time::Instant::now();
    drop(worker);
    println!("shutdown_ms={}", stop.elapsed().as_millis());
}
