use playback_runtime::{
    parse_timing_probe_durations, parse_timing_probe_scenarios, print_timing_probe_summary,
    run_timing_probe, TimingProbeOptions,
};
use std::env;
use std::fs;

#[derive(Default)]
struct Args {
    options: TimingProbeOptions,
    output: Option<String>,
}

fn main() -> Result<(), String> {
    let args = parse_args()?;
    let reports = run_timing_probe(&args.options)?;
    print_timing_probe_summary(&reports);
    let body = serde_json::to_string_pretty(&reports).map_err(|error| error.to_string())?;
    if let Some(output) = args.output {
        fs::write(output, body).map_err(|error| error.to_string())?;
    } else {
        println!("{body}");
    }
    Ok(())
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args::default();
    let mut iter = env::args().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--help" | "-h" => return Err(help()),
            "--duration" | "--durations" => {
                args.options.durations = parse_timing_probe_durations(&next(&mut iter, &arg)?)?
            }
            "--scenario" | "--scenarios" => {
                args.options.scenarios = parse_timing_probe_scenarios(&next(&mut iter, &arg)?)?
            }
            "--output" => args.output = Some(next(&mut iter, &arg)?),
            "--config" => args.options.config = Some(next(&mut iter, &arg)?),
            "--snapshots" => args.options.snapshots = true,
            "--realtime" => args.options.realtime = true,
            other => return Err(format!("unknown arg {other}\n{}", help())),
        }
    }
    Ok(args)
}

fn next(iter: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    iter.next()
        .ok_or_else(|| format!("missing value for {name}"))
}

fn help() -> String {
    "usage: cargo run -p playback-runtime --bin playback_timing_probe -- [--config config/default.json] [--durations 5s,15s,1m] [--scenarios idle,sense,stop-start,encoder] [--realtime] [--snapshots] [--output path]".to_string()
}
