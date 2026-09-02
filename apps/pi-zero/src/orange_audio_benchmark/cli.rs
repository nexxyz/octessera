use std::path::PathBuf;

pub const DEFAULT_WARMUP_SECONDS: u64 = 5;
pub const DEFAULT_MEASURE_SECONDS: u64 = 30;
pub const DEFAULT_RESULT_PATH: &str = "/run/octessera/orange-audio-benchmark-result.json";
pub const DEFAULT_PROGRESS_PATH: &str = "/run/octessera/orange-audio-benchmark-progress.json";
pub const DEFAULT_READINESS_PATH: &str = "/run/octessera/orange-audio-benchmark-readiness.json";
pub const DEFAULT_RELEASE_TIMEOUT_SECONDS: u64 = 30;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScenarioId {
    Synth16,
    Synth32,
    Synth64,
    Sample64,
    Mixed16,
    Mixed32,
    BusHeavy,
    Momentary,
    SynthSteal,
    SampleSteal,
    MixedSteal,
    SynthCrossSlot16,
    SampleCrossSlot64,
    Mixed16Synth32Sample,
    Fixed8Synth8Sample12Bus2Global2Momentary,
    SynthCrossSlot32NoSteal,
    MixedRamp16_48,
}

impl ScenarioId {
    pub const ALL: [Self; 11] = [
        Self::Synth16,
        Self::Synth32,
        Self::Synth64,
        Self::Sample64,
        Self::Mixed16,
        Self::Mixed32,
        Self::BusHeavy,
        Self::Momentary,
        Self::SynthSteal,
        Self::SampleSteal,
        Self::MixedSteal,
    ];

    pub const BASELINE_LIVE: [Self; 6] = [
        Self::SynthCrossSlot16,
        Self::SampleCrossSlot64,
        Self::Mixed16Synth32Sample,
        Self::Fixed8Synth8Sample12Bus2Global2Momentary,
        Self::SynthCrossSlot32NoSteal,
        Self::MixedRamp16_48,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Synth16 => "synth_ramp_16",
            Self::Synth32 => "synth_ramp_32",
            Self::Synth64 => "synth_ramp_64",
            Self::Sample64 => "sample_ramp_64",
            Self::Mixed16 => "mixed_ramp_16_16",
            Self::Mixed32 => "mixed_ramp_32_32",
            Self::BusHeavy => "bus_heavy_6_bus_fx_2_global",
            Self::Momentary => "momentary_combined",
            Self::SynthSteal => "synth_cross_slot_96_steal",
            Self::SampleSteal => "sample_cross_slot_96_steal",
            Self::MixedSteal => "mixed_cross_slot_48_48_steal",
            Self::SynthCrossSlot16 => "synth_cross_slot_16",
            Self::SampleCrossSlot64 => "sample_cross_slot_64",
            Self::Mixed16Synth32Sample => "mixed_16_synth_32_sample",
            Self::Fixed8Synth8Sample12Bus2Global2Momentary => {
                "fixed_8_synth_8_sample_12_bus_2_global_2_momentary"
            }
            Self::SynthCrossSlot32NoSteal => "synth_cross_slot_32_no_steal",
            Self::MixedRamp16_48 => "mixed_ramp_16_48",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .chain(Self::BASELINE_LIVE)
            .find(|id| id.as_str() == value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BenchmarkConfig {
    pub scenario: ScenarioId,
    pub output_frames: u32,
    pub expected_alsa_period_frames: u32,
    pub internal_frames: usize,
    pub warmup_seconds: u64,
    pub measure_seconds: u64,
    pub result_path: PathBuf,
    pub progress_path: PathBuf,
    pub readiness_path: PathBuf,
    pub release_gate_path: PathBuf,
    pub release_timeout_seconds: u64,
    pub artifact_sha256: String,
}

pub fn requested() -> bool {
    requested_from_args(std::env::args().skip(1))
}

pub fn requested_from_args(mut args: impl Iterator<Item = String>) -> bool {
    args.any(|arg| arg == "--benchmark-orange-audio")
}

pub fn parse(args: impl IntoIterator<Item = String>) -> Result<BenchmarkConfig, String> {
    let mut benchmark = false;
    let mut scenario = None;
    let mut output_frames = None;
    let mut engine_block_frames = None;
    let mut warmup_seconds = DEFAULT_WARMUP_SECONDS;
    let mut measure_seconds = DEFAULT_MEASURE_SECONDS;
    let mut result_path = PathBuf::from(DEFAULT_RESULT_PATH);
    let mut progress_path = PathBuf::from(DEFAULT_PROGRESS_PATH);
    let mut readiness_path = PathBuf::from(DEFAULT_READINESS_PATH);
    let mut release_gate_path = None;
    let mut release_timeout_seconds = DEFAULT_RELEASE_TIMEOUT_SECONDS;
    let mut artifact_sha256 = None;
    let mut iter = args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--benchmark-orange-audio" => benchmark = true,
            "--unmuted" | "--no-mute" | "--disable-mute" => {
                return Err("Orange audio benchmark is always post-DSP muted".into())
            }
            "--scenario" => scenario = Some(next_value(&mut iter, "scenario")?),
            "--output-frames" => output_frames = Some(parse_value(&mut iter, "output frames")?),
            "--engine-block-frames" => {
                engine_block_frames = Some(parse_value(&mut iter, "engine block frames")?)
            }
            "--warmup-seconds" => warmup_seconds = parse_value(&mut iter, "warmup seconds")?,
            "--measure-seconds" => measure_seconds = parse_value(&mut iter, "measure seconds")?,
            "--result" => result_path = PathBuf::from(next_value(&mut iter, "result path")?),
            "--progress" => progress_path = PathBuf::from(next_value(&mut iter, "progress path")?),
            "--readiness" => {
                readiness_path = PathBuf::from(next_value(&mut iter, "readiness path")?)
            }
            "--release-gate" => {
                release_gate_path = Some(PathBuf::from(next_value(&mut iter, "release gate path")?))
            }
            "--release-timeout-seconds" => {
                release_timeout_seconds = parse_value(&mut iter, "release timeout seconds")?
            }
            "--artifact-sha256" => {
                artifact_sha256 = Some(next_value(&mut iter, "artifact SHA-256")?)
            }
            value => return Err(format!("unknown Orange benchmark argument: {value}")),
        }
    }
    if !benchmark {
        return Err("--benchmark-orange-audio is required".into());
    }
    let scenario = ScenarioId::parse(scenario.as_deref().unwrap_or_default())
        .ok_or_else(|| "an exact --scenario is required".to_string())?;
    let output_frames = output_frames.ok_or_else(|| "--output-frames is required".to_string())?;
    if !matches!(output_frames, 128 | 256 | 512 | 1024) {
        return Err("output frames must be 128, 256, 512, or 1024".into());
    }
    let expected_alsa_period_frames = match output_frames {
        128 => 32,
        256 => 64,
        512 => 128,
        1024 => 256,
        _ => unreachable!(),
    };
    let internal_frames =
        engine_block_frames.ok_or_else(|| "--engine-block-frames is required".to_string())?;
    if !matches!(internal_frames, 32 | 64 | 128 | 256) {
        return Err("engine block frames must be 32, 64, 128, or 256".into());
    }
    if !approved_tuple(output_frames, internal_frames) {
        return Err(format!(
            "unsupported Orange benchmark geometry tuple: output={output_frames} internal={internal_frames}"
        ));
    }
    if warmup_seconds != DEFAULT_WARMUP_SECONDS {
        return Err("warmup seconds must be 5".into());
    }
    if !matches!(measure_seconds, 30 | 120 | 300) {
        return Err("measure seconds must be 30, 120, or 300".into());
    }
    if !(1..=120).contains(&release_timeout_seconds) {
        return Err("release timeout seconds must be between 1 and 120".into());
    }
    let release_gate_path = release_gate_path
        .ok_or_else(|| "an explicit --release-gate path is required".to_string())?;
    if result_path.as_os_str().is_empty()
        || progress_path.as_os_str().is_empty()
        || readiness_path.as_os_str().is_empty()
        || release_gate_path.as_os_str().is_empty()
    {
        return Err("result, progress, readiness, and release paths must not be empty".into());
    }
    if result_path == progress_path
        || result_path == readiness_path
        || progress_path == readiness_path
        || result_path == release_gate_path
        || progress_path == release_gate_path
        || readiness_path == release_gate_path
    {
        return Err("result, progress, readiness, and release paths must differ".into());
    }
    let artifact_sha256 = artifact_sha256
        .ok_or_else(|| "an explicit --artifact-sha256 value is required".to_string())?;
    if artifact_sha256.len() != 64
        || !artifact_sha256
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err("artifact SHA-256 must be 64 hexadecimal characters".into());
    }
    Ok(BenchmarkConfig {
        scenario,
        output_frames,
        expected_alsa_period_frames,
        internal_frames,
        warmup_seconds,
        measure_seconds,
        result_path,
        progress_path,
        readiness_path,
        release_gate_path,
        release_timeout_seconds,
        artifact_sha256,
    })
}

fn approved_tuple(output_frames: u32, internal_frames: usize) -> bool {
    matches!(
        (output_frames, internal_frames),
        (128, 32) | (256, 64) | (256, 128) | (256, 256) | (512, 128) | (1024, 256)
    )
}

fn next_value(iter: &mut impl Iterator<Item = String>, name: &str) -> Result<String, String> {
    iter.next().ok_or_else(|| format!("missing {name} value"))
}

fn parse_value<T: std::str::FromStr>(
    iter: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<T, String> {
    next_value(iter, name)?
        .parse()
        .map_err(|_| format!("invalid {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_args() -> Vec<String> {
        vec![
            "--benchmark-orange-audio".into(),
            "--scenario".into(),
            "synth_ramp_16".into(),
            "--output-frames".into(),
            "256".into(),
            "--engine-block-frames".into(),
            "64".into(),
            "--release-gate".into(),
            "release.json".into(),
            "--artifact-sha256".into(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".into(),
        ]
    }

    fn args_for(output_frames: u32, internal_frames: usize) -> Vec<String> {
        let mut args = valid_args();
        set_arg(&mut args, "--output-frames", output_frames.to_string());
        set_arg(
            &mut args,
            "--engine-block-frames",
            internal_frames.to_string(),
        );
        args
    }

    fn set_arg(args: &mut [String], name: &str, value: String) {
        let index = args.iter().position(|arg| arg == name).unwrap();
        args[index + 1] = value;
    }

    fn remove_arg(args: &mut Vec<String>, name: &str) {
        let index = args.iter().position(|arg| arg == name).unwrap();
        args.drain(index..=index + 1);
    }

    #[test]
    fn approved_cli_tuples_store_independent_geometry() {
        for (output, internal, period) in [
            (128, 32, 32),
            (256, 64, 64),
            (256, 128, 64),
            (256, 256, 64),
            (512, 128, 128),
            (1024, 256, 256),
        ] {
            let config = parse(args_for(output, internal)).unwrap();
            assert_eq!(config.output_frames, output);
            assert_eq!(config.expected_alsa_period_frames, period);
            assert_eq!(config.internal_frames, internal);
        }
        let config = parse(valid_args()).unwrap();
        assert_ne!(config.result_path, config.progress_path);
        assert_ne!(config.result_path, config.readiness_path);
        assert_ne!(config.progress_path, config.readiness_path);
    }

    #[test]
    fn historical_order_is_unchanged_and_baseline_live_ids_are_separate() {
        let historical: Vec<_> = ScenarioId::ALL
            .into_iter()
            .map(ScenarioId::as_str)
            .collect();
        assert_eq!(
            historical,
            vec![
                "synth_ramp_16",
                "synth_ramp_32",
                "synth_ramp_64",
                "sample_ramp_64",
                "mixed_ramp_16_16",
                "mixed_ramp_32_32",
                "bus_heavy_6_bus_fx_2_global",
                "momentary_combined",
                "synth_cross_slot_96_steal",
                "sample_cross_slot_96_steal",
                "mixed_cross_slot_48_48_steal",
            ]
        );
        for id in ScenarioId::BASELINE_LIVE {
            assert_eq!(ScenarioId::parse(id.as_str()), Some(id));
        }
        assert_eq!(ScenarioId::MixedRamp16_48.as_str(), "mixed_ramp_16_48");
        assert!(ScenarioId::parse("baseline_idle").is_none());
    }

    #[test]
    fn mixed_boundary_cli_accepts_only_approved_geometry_and_duration() {
        for (output, internal) in [
            (128, 32),
            (256, 64),
            (256, 128),
            (256, 256),
            (512, 128),
            (1024, 256),
        ] {
            for seconds in [30, 120, 300] {
                let mut args = args_for(output, internal);
                set_arg(&mut args, "--scenario", "mixed_ramp_16_48".into());
                args.extend(["--measure-seconds".into(), seconds.to_string()]);
                assert_eq!(parse(args).unwrap().measure_seconds, seconds);
            }
        }
        for (output, internal) in [(128, 64), (256, 32), (512, 256), (1024, 128)] {
            let mut args = args_for(output, internal);
            set_arg(&mut args, "--scenario", "mixed_ramp_16_48".into());
            assert!(parse(args).is_err());
        }
        for seconds in [299, 3000] {
            let mut args = valid_args();
            set_arg(&mut args, "--scenario", "mixed_ramp_16_48".into());
            args.extend(["--measure-seconds".into(), seconds.to_string()]);
            assert!(parse(args).is_err());
        }
    }

    #[test]
    fn engine_block_frames_are_mandatory_and_unsupported_tuples_are_rejected() {
        let mut missing = valid_args();
        remove_arg(&mut missing, "--engine-block-frames");
        assert_eq!(
            parse(missing).unwrap_err(),
            "--engine-block-frames is required"
        );
        let mut invalid_block = valid_args();
        set_arg(&mut invalid_block, "--engine-block-frames", "512".into());
        assert!(parse(invalid_block).is_err());
        for (output, internal) in [(128, 64), (64, 32), (256, 32), (512, 256), (1024, 128)] {
            assert!(parse(args_for(output, internal)).is_err());
        }
    }

    #[test]
    fn invalid_scenario_duration_and_unmuted_are_rejected() {
        assert!(parse(vec!["--benchmark-orange-audio".into()]).is_err());
        let mut args = valid_args();
        args[1] = "--unmuted".into();
        assert!(parse(args).is_err());
        let mut args = valid_args();
        args.retain(|arg| arg != "--artifact-sha256" && arg.len() != 64);
        assert!(parse(args).is_err());
        let mut args = valid_args();
        args.push("--measure-seconds".into());
        args.push("300".into());
        assert_eq!(parse(args).unwrap().measure_seconds, 300);
        for seconds in [31, 299, 3000] {
            let mut args = valid_args();
            args.push("--measure-seconds".into());
            args.push(seconds.to_string());
            assert!(
                parse(args).is_err(),
                "duration {seconds} should be rejected"
            );
        }
        let mut args = valid_args();
        set_arg(
            &mut args,
            "--artifact-sha256",
            "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF".into(),
        );
        assert!(parse(args).is_err());
    }
}
