use super::{
    format_csv_header, format_system_row, format_timed_row, AudioBudgetSemantics, TimedRow,
    PROFILE_CSV_FIELD_COUNT,
};

fn field_count(line: &str) -> usize {
    let mut quoted = false;
    let mut fields = 1;
    let mut chars = line.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '"' if quoted && chars.peek() == Some(&'"') => {
                chars.next();
            }
            '"' => quoted = !quoted,
            ',' if !quoted => fields += 1,
            _ => {}
        }
    }
    fields
}

#[test]
fn formatted_profile_rows_have_the_schema_field_count() {
    assert_eq!(field_count(&format_csv_header()), PROFILE_CSV_FIELD_COUNT);
    assert_eq!(
        field_count(&format_system_row("before", "metric", "value, \"quoted\"")),
        PROFILE_CSV_FIELD_COUNT
    );
    let samples = [0.25, 0.5];
    let line = format_timed_row(TimedRow {
        kind: "engine_source",
        scenario: "scenario",
        metric: "raw_ratio",
        samples: &samples,
        block_frames: 64,
        internal_block_frames: 256,
        sample_rate: 44_100,
        blocks: 2,
        requested_measure_frames: 64,
        requested_internal_block_frames: 256,
        telemetry: None,
        audio_budget: AudioBudgetSemantics::EngineSourceRawRatio,
        notes: "notes, \"quoted\"",
    })
    .expect("non-empty timing row");
    assert_eq!(field_count(&line), PROFILE_CSV_FIELD_COUNT);
}

#[test]
fn p99_9_uses_nearest_rank_and_counts_only_over_budget_observations() {
    let mut samples: Vec<_> = (0..4_096).map(|value| value as f64).collect();
    samples[0] = 2.0;
    let line = format_timed_row(TimedRow {
        kind: "engine_source",
        scenario: "scenario",
        metric: "raw_ratio",
        samples: &samples,
        block_frames: 64,
        internal_block_frames: 256,
        sample_rate: 44_100,
        blocks: 4_096,
        requested_measure_frames: 64,
        requested_internal_block_frames: 256,
        telemetry: None,
        audio_budget: AudioBudgetSemantics::EngineSourceRawRatio,
        notes: "",
    })
    .unwrap();

    let fields: Vec<_> = line.split(',').collect();
    assert_eq!(fields[14], "4091.000000");
    assert_eq!(fields[15], "4095");
}

#[test]
fn runtime_rows_leave_audio_budget_empty() {
    let line = format_timed_row(TimedRow {
        kind: "runtime_step",
        scenario: "runtime_step_default",
        metric: "wall_ms",
        samples: &[1.5],
        block_frames: 256,
        internal_block_frames: 256,
        sample_rate: 44_100,
        blocks: 1,
        requested_measure_frames: 256,
        requested_internal_block_frames: 256,
        telemetry: None,
        audio_budget: AudioBudgetSemantics::NotApplicable,
        notes: "",
    })
    .unwrap();

    assert_eq!(line.split(',').nth(15), Some(""));
}

#[test]
fn legacy_notes_keep_absolute_endpoint_counters() {
    let start = realtime_engine::synth::SynthProfileSnapshot {
        cumulative_voice_steals: 4,
        synth_parallel_dispatches: 10,
        ..realtime_engine::synth::SynthProfileSnapshot::default()
    };
    let end = realtime_engine::synth::SynthProfileSnapshot {
        cumulative_voice_steals: 7,
        synth_parallel_dispatches: 14,
        ..realtime_engine::synth::SynthProfileSnapshot::default()
    };
    let summary = crate::dsp_profile::telemetry::TelemetrySummary::new(start, end, 2).unwrap();

    let notes = super::notes_for(&summary);

    assert!(notes.contains("steals=7/7"));
    assert!(notes.contains("parallel_dispatch=14/14"));
}
