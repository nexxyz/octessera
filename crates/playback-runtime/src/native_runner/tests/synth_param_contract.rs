use super::*;
use crate::native_runner::modulation::audio::instrument_modulation_audio_command;
use crate::native_runner::modulation::process_audio::audio_base_value;

type SynthParam = (
    &'static str,
    &'static [&'static str],
    (f64, f64),
    Option<f64>,
);

/// Each synth binding field a modulation writes: its JSON path, clamp range, and the
/// modulation base read when the saved synth config leaves it out (`None`: no base).
const SYNTH_PARAM_CONTRACT: &[SynthParam] = &[
    ("osc1.octave", &["osc1", "octave"], (-2.0, 2.0), None),
    ("osc1.levelPct", &["osc1", "levelPct"], (0.0, 100.0), None),
    (
        "osc1.detuneCents",
        &["osc1", "detuneCents"],
        (-50.0, 50.0),
        None,
    ),
    (
        "osc1.pulseWidthPct",
        &["osc1", "pulseWidthPct"],
        (5.0, 95.0),
        None,
    ),
    ("osc2.octave", &["osc2", "octave"], (-2.0, 2.0), None),
    ("osc2.levelPct", &["osc2", "levelPct"], (0.0, 100.0), None),
    (
        "osc2.detuneCents",
        &["osc2", "detuneCents"],
        (-50.0, 50.0),
        None,
    ),
    (
        "osc2.pulseWidthPct",
        &["osc2", "pulseWidthPct"],
        (5.0, 95.0),
        None,
    ),
    (
        "filter.resonance",
        &["filter", "resonance"],
        (0.0, 100.0),
        Some(32.0),
    ),
    (
        "filter.envAmountPct",
        &["filter", "envAmountPct"],
        (-100.0, 100.0),
        Some(0.0),
    ),
    (
        "filter.keyTrackingPct",
        &["filter", "keyTrackingPct"],
        (0.0, 100.0),
        Some(0.0),
    ),
    (
        "amp.velocitySensitivityPct",
        &["amp", "velocitySensitivityPct"],
        (0.0, 100.0),
        Some(100.0),
    ),
    (
        "ampEnv.attackMs",
        &["ampEnv", "attackMs"],
        (0.0, 5000.0),
        Some(10.0),
    ),
    (
        "ampEnv.decayMs",
        &["ampEnv", "decayMs"],
        (0.0, 5000.0),
        Some(100.0),
    ),
    (
        "ampEnv.sustainPct",
        &["ampEnv", "sustainPct"],
        (0.0, 100.0),
        Some(80.0),
    ),
    (
        "ampEnv.releaseMs",
        &["ampEnv", "releaseMs"],
        (0.0, 10000.0),
        Some(300.0),
    ),
    (
        "filterEnv.attackMs",
        &["filterEnv", "attackMs"],
        (0.0, 5000.0),
        Some(10.0),
    ),
    (
        "filterEnv.decayMs",
        &["filterEnv", "decayMs"],
        (0.0, 5000.0),
        Some(100.0),
    ),
    (
        "filterEnv.sustainPct",
        &["filterEnv", "sustainPct"],
        (0.0, 100.0),
        Some(80.0),
    ),
    (
        "filterEnv.releaseMs",
        &["filterEnv", "releaseMs"],
        (0.0, 10000.0),
        Some(300.0),
    ),
];

fn json_at<'a>(value: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(value, |value, key| &value[*key])
}

fn synth_command_value(command: Option<RuntimeAudioCommand>) -> Option<f32> {
    match command? {
        RuntimeAudioCommand::SetSynthParam { value, .. } => Some(value),
        other => panic!("unexpected command {other:?}"),
    }
}

#[test]
fn synth_bindings_keep_their_paths_ranges_and_bases() {
    for (field, path, (min, max), missing_base) in SYNTH_PARAM_CONTRACT {
        let key = format!("synth.{field}");
        let octave = field.ends_with(".octave");
        let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
        let instrument = &mut runner.instruments[0];
        for (value, stored) in [
            (min - 1000.0, *min),
            (max + 1000.0, *max),
            (min + 1.4, min + 1.0),
        ] {
            assert!(apply_instrument_numeric_binding_value(
                instrument, &key, value
            ));
            assert_eq!(
                json_at(&instrument.synth_config, path).as_f64(),
                Some(stored),
                "{key}"
            );
        }

        for (value, sent) in [
            (min - 1000.0, *min),
            (max + 1000.0, *max),
            (min + 1.5, min + 1.5),
        ] {
            let sent = (!octave).then_some(sent as f32);
            let command = instrument_modulation_audio_command(0, &key, &json!(value));
            assert_eq!(synth_command_value(command), sent, "{key}");
        }

        let binding = format!("instruments.0.{key}");
        let base = audio_base_value(&runner, &binding);
        assert_eq!(base, (!octave).then_some(min + 1.0), "{key} base");
        runner.instruments[0].synth_config[path[0]]
            .as_object_mut()
            .unwrap()
            .remove(path[1]);
        assert_eq!(
            audio_base_value(&runner, &binding),
            *missing_base,
            "{key} missing base"
        );
    }
}
