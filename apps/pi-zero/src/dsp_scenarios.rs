#[path = "dsp_profile/scenarios.rs"]
mod scenario_source;

pub use scenario_source::{profile_scenarios, runtime_step_scenarios, ProfileMode, ScenarioSpec};

#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub const LIVE_SAMPLE_SETUP_ALLOWANCE_SECONDS: u32 = 10;
#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub const LIVE_SAMPLE_WARMUP_SECONDS: u32 = 5;
#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub const LIVE_SAMPLE_MAX_MEASURE_SECONDS: u32 = 120;
#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub const LIVE_SAMPLE_SHUTDOWN_MARGIN_SECONDS: u32 = 15;
#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
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

#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExpectedLiveState {
    pub active_synth_voices: usize,
    pub active_sample_voices: usize,
    pub active_momentary_fx: usize,
    pub expected_voice_steals: u64,
}

#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub struct LiveScenarioSpec {
    pub events: Vec<rodio_engine_source::EngineEvent>,
    pub expected: ExpectedLiveState,
}

#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub fn live_scenario(
    name: &str,
    sample_rate: u32,
    note_duration_ms: u32,
) -> Option<LiveScenarioSpec> {
    let expected = expected_live_state(name)?;
    let scenario = profile_scenarios(sample_rate, ProfileMode::Full)
        .into_iter()
        .chain(profile_scenarios(sample_rate, ProfileMode::Overload))
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

#[cfg(any(feature = "hardware-orange-pi-zero-2w", test))]
pub fn expected_live_state(name: &str) -> Option<ExpectedLiveState> {
    let state: (usize, usize, usize, u64) = match name {
        "synth_ramp_16" => (16, 0, 0, 0),
        "synth_ramp_32" => (32, 0, 0, 0),
        "synth_ramp_64" => (64, 0, 0, 0),
        "sample_ramp_64" => (0, 64, 0, 0),
        "mixed_ramp_16_16" => (16, 16, 0, 0),
        "mixed_ramp_32_32" => (32, 32, 0, 0),
        "bus_heavy_6_bus_fx_2_global" => (16, 0, 0, 0),
        "momentary_combined" => (16, 0, 2, 0),
        "synth_cross_slot_96_steal" => (64, 0, 0, 32),
        "sample_cross_slot_96_steal" => (0, 64, 0, 32),
        "mixed_cross_slot_48_48_steal" => (32, 32, 0, 32),
        _ => return None,
    };
    Some(ExpectedLiveState {
        active_synth_voices: state.0,
        active_sample_voices: state.1,
        active_momentary_fx: state.2,
        expected_voice_steals: state.3,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_scenario_order_is_the_approved_historical_matrix() {
        let scenarios: Vec<_> = LIVE_SCENARIO_IDS
            .iter()
            .map(|name| {
                assert!(live_scenario(name, 44_100, 600_000).is_some());
                *name
            })
            .collect();
        assert_eq!(scenarios, LIVE_SCENARIO_IDS);
    }

    #[test]
    fn live_notes_are_longer_than_the_125_second_qualification_run() {
        let scenario = live_scenario("sample_ramp_64", 44_100, 600_000).unwrap();
        assert!(scenario.events.iter().any(|event| matches!(
            event,
            rodio_engine_source::EngineEvent::NoteOn { duration_ms, .. } if *duration_ms >= 125_000
        )));
    }

    #[test]
    fn expected_live_states_match_native_fixture_application() {
        for name in LIVE_SCENARIO_IDS {
            let scenario = live_scenario(name, 44_100, 600_000).unwrap();
            let mut engine = realtime_engine::synth::SynthEngine::new(44_100);
            crate::dsp_profile::telemetry::apply_events(&mut engine, &scenario.events);
            let snapshot = engine.profile_snapshot();
            assert_eq!(
                snapshot.active_synth_voices, scenario.expected.active_synth_voices,
                "{name}"
            );
            assert_eq!(
                snapshot.active_sample_voices, scenario.expected.active_sample_voices,
                "{name}"
            );
            assert_eq!(
                snapshot.active_momentary_fx, scenario.expected.active_momentary_fx,
                "{name}"
            );
            assert_eq!(
                snapshot.cumulative_voice_steals, scenario.expected.expected_voice_steals,
                "{name}"
            );
        }
    }

    #[test]
    fn momentary_combined_expected_count_matches_engine_limit() {
        assert_eq!(
            expected_live_state("momentary_combined")
                .unwrap()
                .active_momentary_fx,
            2
        );
    }

    #[test]
    fn sample_voices_remain_exact_through_worst_case_elapsed_duration() {
        for name in LIVE_SCENARIO_IDS {
            let scenario = live_scenario(name, 44_100, 600_000).unwrap();
            if scenario.expected.active_sample_voices == 0 {
                continue;
            }
            for seconds in [35_u64, 125, u64::from(LIVE_SAMPLE_LIFETIME_SECONDS)] {
                let mut engine = realtime_engine::synth::SynthEngine::new(44_100);
                crate::dsp_profile::telemetry::apply_events(&mut engine, &scenario.events);
                let blocks = seconds * 44_100 / 1_024;
                let mut left = Vec::with_capacity(1_024);
                let mut right = Vec::with_capacity(1_024);
                let mut interleaved = Vec::with_capacity(2_048);
                for _ in 0..blocks {
                    engine.render_interleaved_block(1_024, &mut left, &mut right, &mut interleaved);
                }
                assert_eq!(
                    engine.profile_snapshot().active_sample_voices,
                    scenario.expected.active_sample_voices,
                    "{name} at {seconds}s"
                );
            }
        }
    }

    #[test]
    fn fresh_source_isolation_replaces_default_configuration() {
        let synth = live_scenario("synth_ramp_16", 44_100, 600_000).unwrap();
        let sample = live_scenario("sample_ramp_64", 44_100, 600_000).unwrap();
        let synth_snapshot = source_snapshot(&synth.events);
        let sample_snapshot = source_snapshot(&sample.events);
        assert_eq!(synth_snapshot.active_synth_voices, 16);
        assert_eq!(synth_snapshot.active_sample_voices, 0);
        assert_eq!(sample_snapshot.active_synth_voices, 0);
        assert_eq!(sample_snapshot.active_sample_voices, 64);
    }

    fn source_snapshot(
        events: &[rodio_engine_source::EngineEvent],
    ) -> realtime_engine::synth::SynthProfileSnapshot {
        let (sender, receiver) = rodio_engine_source::event_queue();
        for event in events {
            sender.send(event.clone()).unwrap();
        }
        let mut source = rodio_engine_source::EngineSource::with_block_frames(receiver, 44_100, 64);
        for _ in 0..128 {
            let _ = source.next();
        }
        source.profile_snapshot()
    }
}
