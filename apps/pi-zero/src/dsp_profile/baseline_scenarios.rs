use super::baseline_events::{
    fx_events, mixed_events, mixed_ramp_16_48_events, sample_events, synth_events,
};
use super::{ExpectedProfileState, ScenarioSpec};
use crate::dsp_profile::samples::profile_sample_banks;
use realtime_engine::synth::VoiceStealingMode;

pub(super) fn scenarios(sample_rate: u32) -> Vec<ScenarioSpec> {
    let sample_banks = profile_sample_banks(sample_rate);
    vec![
        ScenarioSpec::with_expected(
            "baseline_idle",
            super::scenario_events::baseline_events(),
            expected(0, 0, 0, 0, 0),
        ),
        ScenarioSpec::with_expected(
            "synth_shipped_policy_8",
            synth_events(8, VoiceStealingMode::AutoBalanced, sample_rate, 1),
            expected(8, 0, 0, 0, 0),
        ),
        ScenarioSpec::with_expected(
            "synth_cross_slot_16",
            synth_events(16, VoiceStealingMode::AutoBalanced, sample_rate, 8),
            expected(16, 0, 0, 0, 0),
        ),
        ScenarioSpec::with_expected(
            "sample_8",
            sample_events(8, sample_rate, &sample_banks, 1),
            expected(0, 8, 0, 0, 0),
        ),
        ScenarioSpec::with_expected(
            "sample_cross_slot_64",
            sample_events(64, sample_rate, &sample_banks, 8),
            expected(0, 64, 0, 0, 0),
        ),
        ScenarioSpec::with_expected(
            "mixed_16_synth_32_sample",
            mixed_events(16, 32, sample_rate, &sample_banks),
            expected(16, 32, 0, 0, 0),
        ),
        fx_scenario(0, 2, 0, sample_rate, &sample_banks),
        fx_scenario(6, 2, 2, sample_rate, &sample_banks),
        fx_scenario(12, 2, 0, sample_rate, &sample_banks),
        fx_scenario(12, 2, 2, sample_rate, &sample_banks),
        ScenarioSpec::with_expected(
            "synth_cross_slot_32_no_steal",
            synth_events(32, VoiceStealingMode::None, sample_rate, 8),
            expected(32, 0, 0, 0, 0),
        ),
        ScenarioSpec::with_expected(
            "synth_cross_slot_64_no_steal",
            synth_events(64, VoiceStealingMode::None, sample_rate, 8),
            expected_with_admission_drops(64, 0, 0, 0, 0, 0, 0),
        ),
        ScenarioSpec::with_expected(
            "mixed_ramp_16_48",
            mixed_ramp_16_48_events(sample_rate, &sample_banks),
            expected(16, 48, 0, 0, 0),
        ),
    ]
}

fn fx_scenario(
    bus_slots: usize,
    global_slots: usize,
    momentary: usize,
    sample_rate: u32,
    sample_banks: &[realtime_engine::synth::SampleBankConfig],
) -> ScenarioSpec {
    ScenarioSpec::with_expected(
        format!(
            "fixed_8_synth_8_sample_{bus_slots}_bus_{global_slots}_global_{momentary}_momentary"
        ),
        fx_events(
            bus_slots,
            global_slots,
            momentary,
            sample_rate,
            sample_banks,
        ),
        expected(8, 8, bus_slots, global_slots, momentary),
    )
}

fn expected(
    active_synth_voices: usize,
    active_sample_voices: usize,
    active_bus_fx_slots: usize,
    active_global_fx_slots: usize,
    active_momentary_fx: usize,
) -> ExpectedProfileState {
    expected_with_admission_drops(
        active_synth_voices,
        active_sample_voices,
        active_bus_fx_slots,
        active_global_fx_slots,
        active_momentary_fx,
        0,
        0,
    )
}

fn expected_with_admission_drops(
    active_synth_voices: usize,
    active_sample_voices: usize,
    active_bus_fx_slots: usize,
    active_global_fx_slots: usize,
    active_momentary_fx: usize,
    expected_voice_admission_drops_start: u64,
    expected_voice_admission_drops_end: u64,
) -> ExpectedProfileState {
    ExpectedProfileState {
        active_synth_voices,
        active_sample_voices,
        active_preview_sample_voices: 0,
        active_momentary_fx,
        active_bus_fx_slots,
        active_global_fx_slots,
        cumulative_voice_steals: 0,
        expected_voice_admission_drops_start,
        expected_voice_admission_drops_end,
    }
}

#[cfg(test)]
mod tests {
    use super::scenarios;
    use crate::dsp_profile::telemetry::apply_events;
    use realtime_engine::synth::SynthEngine;

    #[test]
    fn baseline_vocabulary_contains_current_capacity_matrix() {
        let names: Vec<_> = scenarios(44_100)
            .into_iter()
            .map(|scenario| scenario.name)
            .collect();

        for name in [
            "baseline_idle",
            "synth_shipped_policy_8",
            "synth_cross_slot_16",
            "sample_8",
            "sample_cross_slot_64",
            "mixed_16_synth_32_sample",
            "fixed_8_synth_8_sample_0_bus_2_global_0_momentary",
            "fixed_8_synth_8_sample_6_bus_2_global_2_momentary",
            "fixed_8_synth_8_sample_12_bus_2_global_0_momentary",
            "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
            "synth_cross_slot_32_no_steal",
            "synth_cross_slot_64_no_steal",
            "mixed_ramp_16_48",
        ] {
            assert!(names.contains(&name.to_string()), "missing {name}");
        }
    }

    #[test]
    fn every_baseline_scenario_proves_its_expected_applied_state() {
        for scenario in scenarios(44_100) {
            let mut engine = SynthEngine::new(44_100);
            let retired_audio_states = apply_events(&mut engine, &scenario.events);

            for phase in ["application", "measurement"] {
                scenario
                    .validate_snapshot(phase, &engine.profile_snapshot())
                    .unwrap_or_else(|error| panic!("{error}"));
            }
            drop(retired_audio_states);
        }
    }
}
