mod report;
mod samples;
mod scenarios;
mod telemetry;
mod timing;

use report::{emit_system_row, emit_timed_row, print_csv_header, TimedRow};
use scenarios::{profile_scenarios, runtime_step_scenarios, ProfileMode};
use timing::{profile_block_frames, profile_sample_rate};

const PROFILE_BLOCKS: usize = 48;
const SOAK_BLOCKS: usize = 3_750;
const FX_LIMIT_BLOCKS: usize = 1_500;

pub fn profile_requested() -> bool {
    if std::env::args().skip(1).any(|arg| arg == "--profile-dsp") {
        return true;
    }
    std::env::var("OCTESSERA_PI_PROFILE_DSP")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "profile" | "dsp"
            )
        })
}

pub fn run_dsp_profile() -> Result<(), String> {
    print_csv_header();
    emit_system_row("before");

    let block_frames = profile_block_frames();
    let sample_rate = profile_sample_rate();
    let mode = profile_mode();
    let blocks = match mode {
        ProfileMode::Soak => SOAK_BLOCKS,
        ProfileMode::FxLimits => FX_LIMIT_BLOCKS,
        ProfileMode::Full | ProfileMode::Overload => PROFILE_BLOCKS,
    };

    for scenario in profile_scenarios(sample_rate, mode) {
        let timing = timing::measure_engine_source(&scenario, sample_rate, block_frames, blocks)?;
        let telemetry =
            telemetry::collect_synth_telemetry(&scenario, sample_rate, block_frames, blocks);
        emit_timed_row(TimedRow {
            kind: "engine_source",
            scenario: &scenario.name,
            metric: "raw_ratio",
            samples: &timing,
            block_frames,
            sample_rate,
            blocks,
            notes: &report::notes_for(&telemetry),
        });
    }

    for runtime in runtime_step_scenarios() {
        let runtime_timing =
            timing::measure_runtime_step(&runtime, sample_rate, block_frames, PROFILE_BLOCKS)?;
        emit_timed_row(TimedRow {
            kind: "runtime_step",
            scenario: &runtime.name,
            metric: "wall_ms",
            samples: &runtime_timing,
            block_frames,
            sample_rate,
            blocks: PROFILE_BLOCKS,
            notes: "synth=na;sample=na;preview=na;momentary=na;steals=na;runner=native_runner",
        });
    }

    emit_system_row("after");
    Ok(())
}

fn profile_mode() -> ProfileMode {
    std::env::var("OCTESSERA_PI_PROFILE_MODE")
        .ok()
        .as_deref()
        .and_then(ProfileMode::from_str)
        .unwrap_or(ProfileMode::Full)
}
