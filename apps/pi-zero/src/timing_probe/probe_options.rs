use super::{summarize, LiveSummary};
use crate::audio::AudioManager;
use crate::main_paths::default_store_dir;
use playback_runtime::{
    parse_timing_probe_durations, parse_timing_probe_scenarios, print_timing_probe_summary,
    run_timing_probe, TimingProbeOptions,
};
use rodio_engine_source::EngineEvent;
use serde::Serialize;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[derive(Serialize)]
struct AudioDrainProbeReport {
    duration_ms: u64,
    interval_ms: u64,
    marks: usize,
    drain_latency_us: LiveSummary,
}

pub(crate) fn requested() -> bool {
    std::env::var("OCTESSERA_PI_TIMING_PROBE").as_deref() == Ok("1")
        || std::env::args().any(|arg| arg == "--timing-probe")
}

pub(super) fn options_from_env_and_args() -> Result<TimingProbeOptions, String> {
    let mut options = TimingProbeOptions {
        realtime: true,
        config: default_config_path(),
        ..TimingProbeOptions::default()
    };
    if let Ok(value) = std::env::var("OCTESSERA_PI_TIMING_PROBE_DURATIONS") {
        options.durations = parse_timing_probe_durations(&value)?;
    }
    if let Ok(value) = std::env::var("OCTESSERA_PI_TIMING_PROBE_SCENARIOS") {
        options.scenarios = parse_timing_probe_scenarios(&value)?;
    }
    if let Ok(value) = std::env::var("OCTESSERA_PI_TIMING_PROBE_CONFIG") {
        options.config = Some(value);
    }
    let mut iter = std::env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--timing-probe" => {}
            "--timing-probe-duration" | "--timing-probe-durations" => {
                options.durations = parse_timing_probe_durations(&next(&mut iter, &arg)?)?
            }
            "--timing-probe-scenario" | "--timing-probe-scenarios" => {
                options.scenarios = parse_timing_probe_scenarios(&next(&mut iter, &arg)?)?
            }
            "--timing-probe-config" => options.config = Some(next(&mut iter, &arg)?),
            "--timing-probe-no-config" => options.config = None,
            "--timing-probe-snapshots" => options.snapshots = true,
            _ => {}
        }
    }
    Ok(options)
}

pub(super) fn run_runtime_only(options: &TimingProbeOptions) -> bool {
    match run_timing_probe(options) {
        Ok(reports) => {
            print_timing_probe_summary(&reports);
            match serde_json::to_string_pretty(&reports) {
                Ok(body) => println!("{body}"),
                Err(error) => {
                    eprintln!("timing probe JSON encode failed: {error}");
                    return false;
                }
            }
            true
        }
        Err(error) => {
            eprintln!("timing probe failed: {error}");
            false
        }
    }
}

pub(super) fn run_audio_drain_probe(options: &TimingProbeOptions) -> bool {
    let duration = options
        .durations
        .first()
        .copied()
        .unwrap_or_else(|| Duration::from_secs(60));
    match run_audio_drain_one(duration) {
        Ok(report) => {
            eprintln!(
                "AudioDrain {}ms marks={} p99={:.0}us p999={:.0}us p9999={:.0}us max={:.0}us >5ms={} >10ms={} >20ms={}",
                report.duration_ms,
                report.marks,
                report.drain_latency_us.p99,
                report.drain_latency_us.p999,
                report.drain_latency_us.p9999,
                report.drain_latency_us.max,
                report.drain_latency_us.over_5ms,
                report.drain_latency_us.over_10ms,
                report.drain_latency_us.over_20ms
            );
            match serde_json::to_string_pretty(&report) {
                Ok(body) => println!("{body}"),
                Err(error) => {
                    eprintln!("audio drain probe JSON encode failed: {error}");
                    return false;
                }
            }
            true
        }
        Err(error) => {
            eprintln!("audio drain probe failed: {error}");
            false
        }
    }
}

fn run_audio_drain_one(duration: Duration) -> Result<AudioDrainProbeReport, String> {
    let audio = AudioManager::new(None, crate::usb_config::UsbAudioOut::Jack)?;
    let service = audio.service();
    let interval = audio_drain_interval();
    let (report_tx, report_rx) = std::sync::mpsc::channel::<u128>();
    let started_at = Instant::now();
    let mut sent = 0usize;
    while started_at.elapsed() < duration {
        let target = started_at + interval * (sent as u32);
        let now = Instant::now();
        if now < target {
            std::thread::sleep(target.duration_since(now));
        }
        service.send_realtime(EngineEvent::ProbeMark {
            sent_at: Instant::now(),
            report_tx: report_tx.clone(),
        })?;
        sent += 1;
    }
    drop(report_tx);
    std::thread::sleep(Duration::from_millis(100));
    let latencies = report_rx
        .try_iter()
        .map(|latency| latency as f64)
        .collect::<Vec<_>>();
    Ok(AudioDrainProbeReport {
        duration_ms: duration.as_millis() as u64,
        interval_ms: interval.as_millis() as u64,
        marks: latencies.len(),
        drain_latency_us: summarize(&latencies),
    })
}

pub(super) fn runtime_only_requested() -> bool {
    std::env::var("OCTESSERA_PI_TIMING_PROBE_RUNTIME_ONLY").as_deref() == Ok("1")
        || std::env::args().any(|arg| arg == "--timing-probe-runtime-only")
}

pub(super) fn audio_drain_requested() -> bool {
    std::env::var("OCTESSERA_PI_TIMING_PROBE_AUDIO_DRAIN").as_deref() == Ok("1")
        || std::env::args().any(|arg| arg == "--timing-probe-audio-drain")
}

fn audio_drain_interval() -> Duration {
    let millis = std::env::var("OCTESSERA_PI_TIMING_PROBE_AUDIO_DRAIN_INTERVAL_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(10)
        .clamp(1, 1000);
    Duration::from_millis(millis)
}

fn default_config_path() -> Option<String> {
    let path: PathBuf = default_store_dir().join("default.json");
    path.exists().then(|| path.to_string_lossy().into_owned())
}

fn next(iter: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("missing value for {name}"))
}
