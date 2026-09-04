#[path = "dsp_profile/scenarios.rs"]
mod scenario_source;

pub use scenario_source::{profile_scenarios, runtime_step_scenarios, ProfileMode, ScenarioSpec};

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub const LIVE_SAMPLE_SETUP_ALLOWANCE_SECONDS: u32 = 10;
#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub const LIVE_SAMPLE_WARMUP_SECONDS: u32 = 5;
#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub const LIVE_SAMPLE_MAX_MEASURE_SECONDS: u32 = 300;
#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub const LIVE_SAMPLE_SHUTDOWN_MARGIN_SECONDS: u32 = 15;
#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub const LIVE_SAMPLE_LIFETIME_SECONDS: u32 = LIVE_SAMPLE_SETUP_ALLOWANCE_SECONDS
    + LIVE_SAMPLE_WARMUP_SECONDS
    + LIVE_SAMPLE_MAX_MEASURE_SECONDS
    + LIVE_SAMPLE_SHUTDOWN_MARGIN_SECONDS;

#[cfg(test)]
pub const LIVE_SCENARIO_IDS: [&str; 11] = [
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
];

pub(crate) fn is_dynamic_live_scenario_name(name: &str) -> bool {
    #[cfg(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))]
    {
        crate::dsp_profile::capacity_scenarios::parse(name).is_some()
            || crate::dsp_profile::analogue_capacity_scenario::parse(name).is_some()
    }
    #[cfg(not(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    )))]
    {
        let _ = name;
        false
    }
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub const BASELINE_LIVE_SCENARIO_IDS: [&str; 14] = [
    "synth_cross_slot_16",
    "sample_cross_slot_64",
    "mixed_16_synth_32_sample",
    "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
    "synth_cross_slot_32_no_steal",
    "mixed_ramp_16_48",
    "default_envelope_24_synth_8_sample",
    "default_headroom_32_synth_8_sample",
    "default_headroom_32_synth_16_sample",
    "default_headroom_40_synth_16_sample",
    "default_headroom_48_synth_16_sample",
    "default_capacity_64_synth_16_sample",
    "default_capacity_48_synth_64_sample",
    "default_capacity_64_synth_64_sample",
];

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedLiveState {
    pub active_synth_voices: usize,
    pub active_sample_voices: usize,
    pub active_momentary_fx: usize,
    pub active_bus_fx_slots: usize,
    pub active_global_fx_slots: usize,
    pub expected_voice_steals: u64,
    pub expected_voice_admission_drops_start: u64,
    pub expected_voice_admission_drops_end: u64,
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub struct LiveScenarioSpec {
    pub events: Vec<rodio_engine_source::EngineEvent>,
    pub expected: ExpectedLiveState,
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub fn live_scenario(
    name: &str,
    sample_rate: u32,
    note_duration_ms: u32,
) -> Option<LiveScenarioSpec> {
    #[cfg(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))]
    if let Some(scenario) =
        crate::dsp_profile::analogue_capacity_scenario::build(name, sample_rate, note_duration_ms)
    {
        return Some(scenario);
    }
    #[cfg(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))]
    if let Some(scenario) =
        crate::dsp_profile::capacity_scenarios::build(name, sample_rate, note_duration_ms)
    {
        return Some(scenario);
    }
    let expected = expected_live_state(name)?;
    let scenarios = if BASELINE_LIVE_SCENARIO_IDS.contains(&name) {
        profile_scenarios(sample_rate, ProfileMode::Baseline)
    } else {
        profile_scenarios(sample_rate, ProfileMode::Full)
            .into_iter()
            .chain(profile_scenarios(sample_rate, ProfileMode::Overload))
            .collect()
    };
    let scenario = scenarios
        .into_iter()
        .find(|scenario| scenario.name == name)?;
    let events = scenario
        .events
        .into_iter()
        .map(|event| match event {
            rodio_engine_source::EngineEvent::SetPreparedAudioConfig(config)
                if config.sample_banks().is_some() =>
            {
                rodio_engine_source::EngineEvent::SetPreparedAudioConfig(config.with_sample_banks(
                    Some(crate::dsp_profile::samples::long_sample_banks(
                        sample_rate,
                        LIVE_SAMPLE_LIFETIME_SECONDS,
                    )),
                ))
            }
            rodio_engine_source::EngineEvent::NoteOn {
                instrument_slot,
                note,
                velocity,
                ..
            } => rodio_engine_source::EngineEvent::NoteOn {
                instrument_slot,
                note,
                velocity,
                duration_ms: note_duration_ms,
            },
            event => event,
        })
        .collect();
    Some(LiveScenarioSpec { events, expected })
}

#[cfg(any(
    feature = "benchmark-voice-pools-128",
    feature = "benchmark-voice-pools-256",
    feature = "hardware-orange-pi-zero-2w",
    test
))]
pub fn expected_live_state(name: &str) -> Option<ExpectedLiveState> {
    #[cfg(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))]
    if let Some(expected) = crate::dsp_profile::analogue_capacity_scenario::expected(name) {
        return Some(expected);
    }
    #[cfg(any(
        feature = "benchmark-voice-pools-128",
        feature = "benchmark-voice-pools-256"
    ))]
    if let Some(expected) = crate::dsp_profile::capacity_scenarios::expected(name) {
        return Some(expected);
    }
    let state: (usize, usize, usize, usize, usize, u64, u64, u64) = match name {
        "synth_ramp_16" => (16, 0, 0, 0, 0, 0, 0, 0),
        "synth_ramp_32" => (32, 0, 0, 0, 0, 0, 0, 0),
        "synth_ramp_64" => (64, 0, 0, 0, 0, 0, 0, 0),
        "sample_ramp_64" => (0, 64, 0, 0, 0, 0, 0, 0),
        "mixed_ramp_16_16" => (16, 16, 0, 0, 0, 0, 0, 0),
        "mixed_ramp_32_32" => (32, 32, 0, 0, 0, 0, 0, 0),
        "bus_heavy_6_bus_fx_2_global" => (16, 0, 0, 6, 2, 0, 0, 0),
        "momentary_combined" => (16, 0, 2, 0, 0, 0, 0, 0),
        "synth_cross_slot_96_steal" => (64, 0, 0, 0, 0, 32, 0, 0),
        "sample_cross_slot_96_steal" => (0, 64, 0, 0, 0, 32, 0, 0),
        "mixed_cross_slot_48_48_steal" => (32, 32, 0, 0, 0, 32, 0, 0),
        "synth_cross_slot_16" => (16, 0, 0, 0, 0, 0, 0, 0),
        "sample_cross_slot_64" => (0, 64, 0, 0, 0, 0, 0, 0),
        "mixed_16_synth_32_sample" => (16, 32, 0, 0, 0, 0, 0, 0),
        "fixed_8_synth_8_sample_12_bus_2_global_2_momentary" => (8, 8, 2, 12, 2, 0, 0, 0),
        "synth_cross_slot_32_no_steal" => (32, 0, 0, 0, 0, 0, 0, 0),
        "mixed_ramp_16_48" => (16, 48, 0, 0, 0, 0, 0, 0),
        "default_envelope_24_synth_8_sample" => (24, 8, 2, 4, 1, 0, 0, 0),
        "default_headroom_32_synth_8_sample" => (32, 8, 2, 4, 1, 0, 0, 0),
        "default_headroom_32_synth_16_sample" => (32, 16, 2, 4, 1, 0, 0, 0),
        "default_headroom_40_synth_16_sample" => (40, 16, 2, 4, 1, 0, 0, 0),
        "default_headroom_48_synth_16_sample" => (48, 16, 2, 4, 1, 0, 0, 0),
        "default_capacity_64_synth_16_sample" => (64, 16, 2, 4, 1, 0, 0, 0),
        "default_capacity_48_synth_64_sample" => (48, 64, 2, 4, 1, 0, 0, 0),
        "default_capacity_64_synth_64_sample" => (64, 64, 2, 4, 1, 0, 0, 0),
        _ => return None,
    };
    Some(ExpectedLiveState {
        active_synth_voices: state.0,
        active_sample_voices: state.1,
        active_momentary_fx: state.2,
        active_bus_fx_slots: state.3,
        active_global_fx_slots: state.4,
        expected_voice_steals: state.5,
        expected_voice_admission_drops_start: state.6,
        expected_voice_admission_drops_end: state.7,
    })
}

#[cfg(test)]
#[path = "dsp_scenarios_tests.rs"]
mod tests;
