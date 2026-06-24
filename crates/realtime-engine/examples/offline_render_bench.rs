use realtime_engine::synth::{SynthEngine, DEFAULT_AUDIO_SAMPLE_RATE};
use std::env;
use std::time::Instant;

#[path = "offline_render_bench/config.rs"]
mod config;
#[path = "offline_render_bench/sample_data.rs"]
mod sample_data;
#[path = "offline_render_bench/scenario.rs"]
mod scenario;

use scenario::Scenario;

const SAMPLE_RATE: u32 = DEFAULT_AUDIO_SAMPLE_RATE;
pub(crate) const SECONDS: usize = 20;

fn main() {
    let scenario = match Scenario::from_args(env::args().nth(1)) {
        Ok(scenario) => scenario,
        Err(message) => {
            eprintln!("{message}");
            std::process::exit(1);
        }
    };
    let mut engine = SynthEngine::new(SAMPLE_RATE);
    engine.set_instruments(config::bench_config(scenario));
    scenario.setup(&mut engine);

    let frames = SAMPLE_RATE as usize * SECONDS;
    let start = Instant::now();
    let mut checksum = 0.0_f32;

    for frame in 0..frames {
        scenario.schedule(&mut engine, frame);
        let (left, right) = engine.next_stereo_sample();
        checksum += (left * 0.5 + right * 0.5).abs();
    }

    print_report(
        scenario,
        frames,
        start.elapsed().as_secs_f64(),
        checksum,
        &engine,
    );
}

fn print_report(
    scenario: Scenario,
    frames: usize,
    elapsed: f64,
    checksum: f32,
    engine: &SynthEngine,
) {
    let realtime = SECONDS as f64 / elapsed;
    let frames_per_second = frames as f64 / elapsed;
    let profile = engine.profile_snapshot();
    println!("scenario={}", scenario.name());
    println!("sample_rate={SAMPLE_RATE}");
    println!("rendered_seconds={SECONDS}");
    println!("frames={frames}");
    println!("elapsed_seconds={elapsed:.4}");
    println!("realtime_ratio={realtime:.2}");
    println!("frames_per_second={frames_per_second:.0}");
    println!("checksum={checksum:.6}");
    println!("active_synth_voices={}", profile.active_synth_voices);
    println!("active_sample_voices={}", profile.active_sample_voices);
    println!(
        "active_preview_sample_voices={}",
        profile.active_preview_sample_voices
    );
    println!(
        "cumulative_voice_steals={}",
        profile.cumulative_voice_steals
    );
}
