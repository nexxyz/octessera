use super::system::profile_system_output;
use super::telemetry::{CounterDelta, TelemetrySummary};

pub const PROFILE_CSV_SCHEMA_VERSION: u32 = 2;
pub const PROFILE_CSV_FIELD_COUNT: usize = 34;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioBudgetSemantics {
    EngineSourceRawRatio,
    NotApplicable,
}

pub fn print_csv_header() {
    println!("{}", format_csv_header());
}

pub fn emit_system_row(phase: &str) {
    for (metric, value) in profile_system_output() {
        println!("{}", format_system_row(phase, &metric, &value));
    }
}

pub struct TimedRow<'a> {
    pub kind: &'a str,
    pub scenario: &'a str,
    pub metric: &'a str,
    pub samples: &'a [f64],
    pub block_frames: usize,
    pub internal_block_frames: usize,
    pub sample_rate: u32,
    pub blocks: usize,
    pub requested_measure_frames: usize,
    pub requested_internal_block_frames: usize,
    pub telemetry: Option<&'a TelemetrySummary>,
    pub audio_budget: AudioBudgetSemantics,
    pub notes: &'a str,
}

pub fn emit_timed_row(row: TimedRow<'_>) {
    if let Some(line) = format_timed_row(row) {
        println!("{line}");
    }
}

pub fn format_csv_header() -> String {
    [
        "kind",
        "scenario",
        "metric",
        "value",
        "block_frames",
        "sample_rate",
        "blocks",
        "avg",
        "p95",
        "p99",
        "max",
        "notes",
        "internal_block_frames",
        "schema_version",
        "p99_9",
        "over_audio_duration_budget_count",
        "requested_measure_frames",
        "requested_internal_block_frames",
        "workers_requested_count",
        "workers_effective_count",
        "peak_synth_voices",
        "peak_sample_voices",
        "peak_preview_sample_voices",
        "peak_momentary_fx",
        "peak_bus_fx_slots",
        "peak_global_fx_slots",
        "peak_voice_steals",
        "voice_steal_delta",
        "synth_parallel_dispatch_delta",
        "synth_parallel_light_skip_delta",
        "synth_parallel_backoff_skip_delta",
        "synth_parallel_timing_backoff_delta",
        "synth_parallel_failure_delta",
        "synth_parallel_unhealthy",
    ]
    .join(",")
}

pub fn format_system_row(phase: &str, metric: &str, value: &str) -> String {
    let mut fields = vec![
        "system".to_string(),
        csv(phase),
        csv(metric),
        csv(value),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        String::new(),
        PROFILE_CSV_SCHEMA_VERSION.to_string(),
    ];
    fields.resize(PROFILE_CSV_FIELD_COUNT, String::new());
    fields.join(",")
}

pub fn format_timed_row(row: TimedRow<'_>) -> Option<String> {
    if row.samples.is_empty() {
        return None;
    }
    let mut values = row.samples.to_vec();
    values.sort_by(|a, b| a.total_cmp(b));
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    let p95 = percentile(&values, 0.95);
    let p99 = percentile(&values, 0.99);
    let p99_9 = percentile(&values, 0.999);
    let max = *values.last().unwrap_or(&0.0);
    let mut fields = vec![
        csv(row.kind),
        csv(row.scenario),
        csv(row.metric),
        csv(""),
        row.block_frames.to_string(),
        row.sample_rate.to_string(),
        row.blocks.to_string(),
        format!("{avg:.6}"),
        format!("{p95:.6}"),
        format!("{p99:.6}"),
        format!("{max:.6}"),
        csv(row.notes),
        row.internal_block_frames.to_string(),
        PROFILE_CSV_SCHEMA_VERSION.to_string(),
        format!("{p99_9:.6}"),
        audio_budget_count(&values, row.audio_budget),
        row.requested_measure_frames.to_string(),
        row.requested_internal_block_frames.to_string(),
    ];
    fields.extend(telemetry_fields(row.telemetry));
    Some(fields.join(","))
}

pub fn notes_for(summary: &TelemetrySummary) -> String {
    format!(
        "synth={}/{};sample={}/{};preview={}/{};momentary={}/{};steals={}/{};parallel_dispatch={}/{};parallel_light_skip={}/{};parallel_backoff_skip={}/{};parallel_timing_backoff={}/{};parallel_fail={}/{};parallel_unhealthy={}",
        summary.end_snapshot.active_synth_voices,
        summary.peak_snapshot.active_synth_voices,
        summary.end_snapshot.active_sample_voices,
        summary.peak_snapshot.active_sample_voices,
        summary.end_snapshot.active_preview_sample_voices,
        summary.peak_snapshot.active_preview_sample_voices,
        summary.end_snapshot.active_momentary_fx,
        summary.peak_snapshot.active_momentary_fx,
        summary.end_snapshot.cumulative_voice_steals,
        summary.peak_snapshot.cumulative_voice_steals,
        summary.end_snapshot.synth_parallel_dispatches,
        summary.peak_snapshot.synth_parallel_dispatches,
        summary.end_snapshot.synth_parallel_light_skips,
        summary.peak_snapshot.synth_parallel_light_skips,
        summary.end_snapshot.synth_parallel_backoff_skips,
        summary.peak_snapshot.synth_parallel_backoff_skips,
        summary.end_snapshot.synth_parallel_timing_backoffs,
        summary.peak_snapshot.synth_parallel_timing_backoffs,
        summary.end_snapshot.synth_parallel_failures,
        summary.peak_snapshot.synth_parallel_failures,
        summary.end_snapshot.synth_parallel_unhealthy,
    )
}

fn audio_budget_count(values: &[f64], semantics: AudioBudgetSemantics) -> String {
    match semantics {
        AudioBudgetSemantics::EngineSourceRawRatio => values
            .iter()
            .filter(|value| **value > 1.0)
            .count()
            .to_string(),
        AudioBudgetSemantics::NotApplicable => String::new(),
    }
}

fn telemetry_fields(summary: Option<&TelemetrySummary>) -> Vec<String> {
    let Some(summary) = summary else {
        return vec![String::new(); PROFILE_CSV_FIELD_COUNT - 18];
    };
    let delta: CounterDelta = summary.counter_delta();
    [
        summary.worker_requested.to_string(),
        summary.worker_effective().to_string(),
        summary.peak_snapshot.active_synth_voices.to_string(),
        summary.peak_snapshot.active_sample_voices.to_string(),
        summary
            .peak_snapshot
            .active_preview_sample_voices
            .to_string(),
        summary.peak_snapshot.active_momentary_fx.to_string(),
        summary.peak_snapshot.active_bus_fx_slots.to_string(),
        summary.peak_snapshot.active_global_fx_slots.to_string(),
        summary.peak_snapshot.cumulative_voice_steals.to_string(),
        delta.cumulative_voice_steals.to_string(),
        delta.synth_parallel_dispatches.to_string(),
        delta.synth_parallel_light_skips.to_string(),
        delta.synth_parallel_backoff_skips.to_string(),
        delta.synth_parallel_timing_backoffs.to_string(),
        delta.synth_parallel_failures.to_string(),
        delta.synth_parallel_unhealthy.to_string(),
    ]
    .into_iter()
    .collect()
}

fn percentile(values: &[f64], percentile: f64) -> f64 {
    let index = ((values.len() as f64 * percentile).ceil() as usize).saturating_sub(1);
    values[index.min(values.len() - 1)]
}

fn csv(value: &str) -> String {
    if value.contains([',', '"', '\n']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

#[cfg(test)]
#[path = "report_tests.rs"]
mod tests;
