use rodio_engine_source::{event_queue, EngineEvent, EngineSource};
use std::time::Instant;

const SAMPLE_RATE: u32 = 44_100;
const SECONDS: usize = 20;

fn main() {
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: (SECONDS as u32 + 1) * 1000,
    })
    .expect("send note on");

    let mut source = EngineSource::with_load_status_tx(rx, SAMPLE_RATE, None);
    let samples = SAMPLE_RATE as usize * SECONDS * 2;
    let started = Instant::now();
    let mut checksum = 0.0_f32;
    for _ in 0..samples {
        checksum += source.next().unwrap_or(0.0).abs();
    }
    let elapsed = started.elapsed().as_secs_f32();
    let rendered_seconds = SECONDS as f32;
    let realtime_ratio = rendered_seconds / elapsed;

    println!("rendered_seconds={SECONDS}");
    println!("samples={samples}");
    println!("elapsed_seconds={elapsed:.4}");
    println!("realtime_ratio={realtime_ratio:.2}");
    println!("checksum={checksum:.6}");
}
