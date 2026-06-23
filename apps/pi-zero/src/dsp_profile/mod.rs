mod report;
mod scenarios;
mod telemetry;
mod timing;

use report::{emit_system_row, emit_timed_row, print_csv_header};
use scenarios::{profile_scenarios, runtime_step_scenario, ProfileMode};
use timing::{profile_block_frames, profile_sample_rate};

const PROFILE_BLOCKS: usize = 48;
const SOAK_BLOCKS: usize = 3_750;

pub fn profile_requested() -> bool {
    if std::env::args().skip(1).any(|arg| arg == "--profile-dsp") {
        return true;
    }
    std::env::var("CELLSYMPHONY_PI_PROFILE_DSP")
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
    let blocks = if mode == ProfileMode::Soak {
        SOAK_BLOCKS
    } else {
        PROFILE_BLOCKS
    };

    for scenario in profile_scenarios(sample_rate, mode) {
        let timing = timing::measure_engine_source(&scenario, sample_rate, block_frames, blocks)?;
        let telemetry =
            telemetry::collect_synth_telemetry(&scenario, sample_rate, block_frames, blocks);
        emit_timed_row(
            "engine_source",
            &scenario.name,
            "raw_ratio",
            &timing,
            block_frames,
            sample_rate,
            blocks,
            &report::notes_for(&telemetry),
        );
    }

    let runtime = runtime_step_scenario();
    let runtime_timing = timing::measure_runtime_step(sample_rate, block_frames, PROFILE_BLOCKS)?;
    emit_timed_row(
        "runtime_step",
        &runtime.name,
        "wall_ms",
        &runtime_timing,
        block_frames,
        sample_rate,
        PROFILE_BLOCKS,
        "synth=na;sample=na;preview=na;momentary=na;steals=na;runner=native_runner",
    );

    emit_system_row("after");
    Ok(())
}

fn profile_mode() -> ProfileMode {
    std::env::var("CELLSYMPHONY_PI_PROFILE_MODE")
        .ok()
        .as_deref()
        .and_then(ProfileMode::from_str)
        .unwrap_or(ProfileMode::Full)
}
