use super::telemetry::TelemetrySummary;
use super::timing::profile_system_output;

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
    pub notes: &'a str,
}

pub fn emit_timed_row(row: TimedRow<'_>) {
    if let Some(line) = format_timed_row(row) {
        println!("{line}");
    }
}

pub fn format_csv_header() -> String {
    "kind,scenario,metric,value,block_frames,sample_rate,blocks,avg,p95,p99,max,notes,internal_block_frames".into()
}

pub fn format_system_row(phase: &str, metric: &str, value: &str) -> String {
    [
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
    ]
    .join(",")
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
    let max = *values.last().unwrap_or(&0.0);
    Some(format!(
        "{},{},{},{},{},{},{},{:.6},{:.6},{:.6},{:.6},{},{}",
        csv(row.kind),
        csv(row.scenario),
        csv(row.metric),
        csv(""),
        row.block_frames,
        row.sample_rate,
        row.blocks,
        avg,
        p95,
        p99,
        max,
        csv(row.notes),
        row.internal_block_frames,
    ))
}

pub fn notes_for(summary: &TelemetrySummary) -> String {
    format!(
        "synth={}/{};sample={}/{};preview={}/{};momentary={}/{};steals={}/{};parallel_dispatch={}/{};parallel_light_skip={}/{};parallel_backoff_skip={}/{};parallel_timing_backoff={}/{};parallel_fail={}/{};parallel_unhealthy={}",
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
        summary.final_snapshot.synth_parallel_dispatches,
        summary.peak_snapshot.synth_parallel_dispatches,
        summary.final_snapshot.synth_parallel_light_skips,
        summary.peak_snapshot.synth_parallel_light_skips,
        summary.final_snapshot.synth_parallel_backoff_skips,
        summary.peak_snapshot.synth_parallel_backoff_skips,
        summary.final_snapshot.synth_parallel_timing_backoffs,
        summary.peak_snapshot.synth_parallel_timing_backoffs,
        summary.final_snapshot.synth_parallel_failures,
        summary.peak_snapshot.synth_parallel_failures,
        summary.final_snapshot.synth_parallel_unhealthy,
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

#[cfg(test)]
mod tests {
    use super::{format_csv_header, format_system_row, format_timed_row, TimedRow};

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
    fn formatted_profile_rows_have_thirteen_csv_fields() {
        assert_eq!(field_count(&format_csv_header()), 13);
        assert_eq!(
            field_count(&format_system_row("before", "metric", "value, \"quoted\"")),
            13
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
            notes: "notes, \"quoted\"",
        })
        .expect("non-empty timing row");
        assert_eq!(field_count(&line), 13);
    }
}
