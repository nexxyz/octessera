use playback_runtime::{NativeRunner, NativeRunnerConfig, RunnerMessage};
use serde_json::Value;
use std::time::{Duration, Instant};

#[path = "../render/hdmi.rs"]
#[allow(dead_code, unused_imports)]
mod hdmi;

fn main() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).expect("native runner");
    for mode in [
        "none",
        "live-grid",
        "plain-grid",
        "active-behavior",
        "cycle-behaviors",
    ] {
        runner
            .apply_config_payload(serde_json::json!({
                "runtimeConfig": { "hdmi": { "mode": mode } }
            }))
            .expect("apply HDMI mode");
        let snapshot = snapshot_from(&mut runner);
        bench_snapshot(&mut runner, mode);
        bench_hdmi_buffer(&snapshot, mode);
    }
}

fn bench_snapshot(runner: &mut NativeRunner, mode: &str) {
    let elapsed = time_loop(1_000, || {
        let _ = snapshot_from(runner);
    });
    println!(
        "snapshot mode={mode} iterations=1000 total_ms={:.3} avg_us={:.3}",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1_000.0
    );
}

fn bench_hdmi_buffer(snapshot: &Value, mode: &str) {
    let elapsed = time_loop(200, || {
        let signature = hdmi::hdmi_signature(snapshot);
        if signature != 0 {
            let _ = hdmi::compose_frame(snapshot, 640, 480, 4);
        }
    });
    println!(
        "hdmi-buffer mode={mode} iterations=200 total_ms={:.3} avg_us={:.3}",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 5_000.0
    );
}

fn snapshot_from(runner: &mut NativeRunner) -> Value {
    runner
        .messages_with_snapshot()
        .expect("snapshot messages")
        .into_iter()
        .find_map(|message| match message {
            RunnerMessage::Snapshot { snapshot } => Some(snapshot),
            _ => None,
        })
        .expect("snapshot")
}

fn time_loop(iterations: usize, mut run: impl FnMut()) -> Duration {
    let start = Instant::now();
    for _ in 0..iterations {
        run();
    }
    start.elapsed()
}
