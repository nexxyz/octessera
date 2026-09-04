use super::*;

#[test]
fn live_scenario_order_is_the_approved_historical_matrix() {
    assert_eq!(
        LIVE_SCENARIO_IDS,
        [
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
fn baseline_live_vocabulary_is_separate_and_idle_stays_offline_only() {
    assert_eq!(BASELINE_LIVE_SCENARIO_IDS.len(), 14);
    for name in BASELINE_LIVE_SCENARIO_IDS {
        assert!(live_scenario(name, 44_100, 600_000).is_some(), "{name}");
    }
    assert!(live_scenario("baseline_idle", 44_100, 600_000).is_none());
}

#[test]
fn default_capacity_live_fixtures_prove_slot_feasibility_and_zero_drops() {
    let fixtures = [
        (
            "default_envelope_24_synth_8_sample",
            [8, 8, 8, 8, 0, 0, 0, 0],
            24,
            8,
        ),
        (
            "default_headroom_32_synth_8_sample",
            [8, 8, 8, 8, 8, 0, 0, 0],
            32,
            8,
        ),
        (
            "default_headroom_32_synth_16_sample",
            [8, 8, 8, 8, 8, 8, 0, 0],
            32,
            16,
        ),
        (
            "default_headroom_40_synth_16_sample",
            [8, 8, 8, 8, 8, 8, 8, 0],
            40,
            16,
        ),
        (
            "default_headroom_48_synth_16_sample",
            [8, 8, 8, 8, 8, 8, 8, 8],
            48,
            16,
        ),
        (
            "default_capacity_64_synth_16_sample",
            [11, 8, 11, 11, 11, 8, 10, 10],
            64,
            16,
        ),
        (
            "default_capacity_48_synth_64_sample",
            [8, 32, 8, 8, 8, 32, 8, 8],
            48,
            64,
        ),
        (
            "default_capacity_64_synth_64_sample",
            [11, 32, 11, 11, 11, 32, 10, 10],
            64,
            64,
        ),
    ];
    for (name, expected_slots, expected_synth, expected_sample) in fixtures {
        let scenario = live_scenario(name, 44_100, 600_000).unwrap();
        assert_eq!(
            scenario.expected.active_synth_voices, expected_synth,
            "{name}"
        );
        assert_eq!(
            scenario.expected.active_sample_voices, expected_sample,
            "{name}"
        );
        let mut engine = realtime_engine::synth::SynthEngine::new(44_100);
        let retired_audio_states =
            crate::dsp_profile::telemetry::apply_events(&mut engine, &scenario.events);
        let snapshot = engine.profile_snapshot();
        assert_eq!(snapshot.active_synth_voices, expected_synth, "{name}");
        assert_eq!(snapshot.active_sample_voices, expected_sample, "{name}");
        assert_eq!(snapshot.active_momentary_fx, 2, "{name}");
        assert_eq!(snapshot.active_bus_fx_slots, 4, "{name}");
        assert_eq!(snapshot.active_global_fx_slots, 1, "{name}");
        assert_eq!(snapshot.cumulative_voice_steals, 0, "{name}");
        assert_eq!(snapshot.cumulative_voice_admission_drops, 0, "{name}");

        let mut observed_slots = [0; 8];
        for event in &scenario.events {
            if let rodio_engine_source::EngineEvent::NoteOn {
                instrument_slot,
                duration_ms,
                ..
            } = event
            {
                observed_slots[*instrument_slot as usize] += 1;
                assert_eq!(*duration_ms, 600_000, "{name}");
            }
        }
        assert_eq!(observed_slots, expected_slots, "{name}");
        drop(retired_audio_states);
    }
}

#[test]
fn mixed_boundary_live_state_is_exact_and_uses_one_long_sample_backing() {
    let scenario = live_scenario("mixed_ramp_16_48", 44_100, 600_000).unwrap();
    let mut engine = realtime_engine::synth::SynthEngine::new(44_100);
    let retired_audio_states =
        crate::dsp_profile::telemetry::apply_events(&mut engine, &scenario.events);
    let snapshot = engine.profile_snapshot();
    assert_eq!(snapshot.active_synth_voices, 16);
    assert_eq!(snapshot.active_sample_voices, 48);
    assert_eq!(snapshot.cumulative_voice_steals, 0);
    assert_eq!(snapshot.cumulative_voice_admission_drops, 0);
    assert_eq!(LIVE_SAMPLE_LIFETIME_SECONDS, 330);
    let config = scenario
        .events
        .iter()
        .find_map(|event| match event {
            rodio_engine_source::EngineEvent::SetPreparedAudioConfig(config) => Some(config),
            _ => None,
        })
        .unwrap();
    let banks = config.sample_banks().unwrap();
    assert_eq!(banks.len(), realtime_engine::synth::INSTRUMENT_SLOT_COUNT);
    let first = &banks[0].slots[0].buffer.as_ref().unwrap().samples;
    assert_eq!(
        first.len(),
        44_100 * (LIVE_SAMPLE_LIFETIME_SECONDS as usize + 1)
    );
    for bank in banks.iter().skip(1) {
        assert!(std::sync::Arc::ptr_eq(
            first,
            &bank.slots[0].buffer.as_ref().unwrap().samples
        ));
    }
    drop(retired_audio_states);
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
        let retired_audio_states =
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
            snapshot.active_bus_fx_slots, scenario.expected.active_bus_fx_slots,
            "{name}"
        );
        assert_eq!(
            snapshot.active_global_fx_slots, scenario.expected.active_global_fx_slots,
            "{name}"
        );
        assert_eq!(
            snapshot.cumulative_voice_steals, scenario.expected.expected_voice_steals,
            "{name}"
        );
        assert_eq!(
            snapshot.cumulative_voice_admission_drops,
            scenario.expected.expected_voice_admission_drops_start,
            "{name}"
        );
        drop(retired_audio_states);
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
fn baseline_max_fx_expected_state_reaches_native_limits() {
    let scenario = live_scenario(
        "fixed_8_synth_8_sample_12_bus_2_global_2_momentary",
        44_100,
        600_000,
    )
    .unwrap();
    let mut engine = realtime_engine::synth::SynthEngine::new(44_100);
    let retired_audio_states =
        crate::dsp_profile::telemetry::apply_events(&mut engine, &scenario.events);
    let snapshot = engine.profile_snapshot();
    assert_eq!(snapshot.active_bus_fx_slots, 12);
    assert_eq!(snapshot.active_global_fx_slots, 2);
    assert_eq!(snapshot.active_momentary_fx, 2);
    assert_eq!(scenario.expected.active_bus_fx_slots, 12);
    assert_eq!(scenario.expected.active_global_fx_slots, 2);
    drop(retired_audio_states);
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
            let retired_audio_states =
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
            drop(retired_audio_states);
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
