use super::telemetry::TelemetrySummary;
use super::timing::vcgencmd_output;

pub fn print_csv_header() {
    println!("kind,scenario,metric,value,block_frames,sample_rate,blocks,avg,p95,p99,max,notes");
}

pub fn emit_system_row(phase: &str) {
    for (metric, value) in vcgencmd_output() {
        println!(
            "system,{},{},{},,,,,,,,",
            csv(phase),
            csv(&metric),
            csv(&value),
        );
    }
}

pub fn emit_timed_row(
    kind: &str,
    scenario: &str,
    metric: &str,
    samples: &[f64],
    block_frames: usize,
    sample_rate: u32,
    blocks: usize,
    notes: &str,
) {
    if samples.is_empty() {
        return;
    }
    let mut values = samples.to_vec();
    values.sort_by(|a, b| a.total_cmp(b));
    let avg = values.iter().sum::<f64>() / values.len() as f64;
    let p95 = percentile(&values, 0.95);
    let p99 = percentile(&values, 0.99);
    let max = *values.last().unwrap_or(&0.0);
    println!(
        "{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{}",
        csv(kind),
        csv(scenario),
        csv(metric),
        csv(""),
        block_frames,
        sample_rate,
        blocks,
        avg,
        p95,
        p99,
        max,
        csv(notes),
    );
}

pub fn notes_for(summary: &TelemetrySummary) -> String {
    format!(
        "synth={}/{};sample={}/{};preview={}/{};momentary={}/{};steals={}/{}",
        summary.final_snapshot.active_synth_voices,
        summary.peak_snapshot.active_synth_voices,
        summary.final_snapshot.active_sample_voices,
        summary.peak_snapshot.active_sample_voices,
        summary.final_snapshot.active_preview_sample_voices,
        summary.peak_snapshot.active_preview_sample_voices,
        summary.final_snapshot.active_momentary_fx,
        summary.peak_snapshot.active_momentary_fx,
        summary.final_snapshot.cumulative_voice_steals,
        summary.peak_snapshot.cumulative_voice_steals,
    )
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
