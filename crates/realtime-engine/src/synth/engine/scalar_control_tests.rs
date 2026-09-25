#[test]
fn ten_thousand_typed_updates_are_allocation_free_and_final_values_are_exact() {
    let mut engine = SynthEngine::new(48_000);
    let ((), allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            for index in 0..10_000 {
                assert_eq!(
                    engine
                        .set_synth_param_typed(0, SynthParamId::AmpGainPct, (index % 100) as f32,),
                    ScalarMutation::Changed
                );
                assert_eq!(
                    engine.set_sample_bank_param_typed(
                        0,
                        SampleBankParamId::TuneSemis,
                        -24.0 + (index % 49) as f32,
                    ),
                    ScalarMutation::Changed
                );
            }
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(
        synth_scalar_values(&engine, 0)[0].to_bits(),
        99.0_f32.to_bits()
    );
    assert_eq!(
        sample_scalar_values(&engine, 0)[0].to_bits(),
        (-21.0_f32).to_bits()
    );
}

#[test]
fn unchanged_and_rejected_typed_updates_do_not_rebuild_or_allocate() {
    let mut engine = SynthEngine::new(48_000);
    assert_eq!(
        engine.set_synth_param_typed(0, SynthParamId::AmpGainPct, 42.0),
        ScalarMutation::Changed
    );
    assert_eq!(
        engine.set_sample_bank_param_typed(0, SampleBankParamId::TuneSemis, 4.0),
        ScalarMutation::Changed
    );
    let revision = engine.synth_render_revisions[0];
    let synth_values = synth_scalar_values(&engine, 0);
    let sample_values = sample_scalar_values(&engine, 0);
    let ((), allocations, deallocations) =
        crate::synth::test_allocator::count_allocations_and_deallocations(|| {
            for _ in 0..10_000 {
                assert_eq!(
                    engine.set_synth_param_typed(0, SynthParamId::AmpGainPct, 42.0),
                    ScalarMutation::Unchanged
                );
                assert_eq!(
                    engine.set_synth_param_typed(0, SynthParamId::AmpGainPct, f32::NAN),
                    ScalarMutation::Rejected
                );
                assert_eq!(
                    engine.set_sample_bank_param_typed(0, SampleBankParamId::TuneSemis, 4.0),
                    ScalarMutation::Unchanged
                );
                assert_eq!(
                    engine.set_sample_bank_param_typed(
                        0,
                        SampleBankParamId::TuneSemis,
                        f32::INFINITY,
                    ),
                    ScalarMutation::Rejected
                );
            }
        });
    assert_eq!((allocations, deallocations), (0, 0));
    assert_eq!(engine.synth_render_revisions[0], revision);
    assert_f32_arrays_bitwise_equal(synth_values, synth_scalar_values(&engine, 0));
    assert_f32_arrays_bitwise_equal(sample_values, sample_scalar_values(&engine, 0));
}

#[test]
fn sample_filter_updates_only_changed_active_voice_parameters() {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        }],
        mixer: None,
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    let mut bank = sample_bank(vec![1.0, 0.0, 0.0, 0.0]);
    bank.filter_cutoff_hz = 8_000.0;
    bank.filter_resonance = 20.0;
    engine.set_sample_banks(vec![bank]);
    engine.note_on(0, 36, 127, 1_000);
    let lane = engine
        .sample_voice_pool
        .first_active_lane_for_slot(0)
        .expect("active sample voice");

    assert_eq!(
        engine.set_sample_bank_param_typed(0, SampleBankParamId::FilterCutoffHz, 1_200.0),
        ScalarMutation::Changed
    );
    let voice = engine.sample_voice_pool.lane(lane).expect("sample voice");
    assert_eq!(voice.filter_cutoff_hz, 1_200.0);
    assert_eq!(
        engine.set_sample_bank_param_typed(0, SampleBankParamId::FilterCutoffHz, 1_200.0),
        ScalarMutation::Unchanged
    );
    assert_eq!(
        engine.set_sample_bank_param_typed(0, SampleBankParamId::FilterCutoffHz, f32::NAN),
        ScalarMutation::Rejected
    );
    assert_eq!(
        engine.set_sample_bank_param_typed(0, SampleBankParamId::FilterResonance, 42.0),
        ScalarMutation::Changed
    );
    let voice = engine.sample_voice_pool.lane(lane).expect("sample voice");
    assert_eq!(voice.filter_cutoff_hz, 1_200.0);
    assert_eq!(voice.filter_resonance, 42.0);
}

#[test]
fn master_and_mixer_scalar_updates_handle_invalid_partial_fields() {
    let mut engine = mixer_engine();
    let initial_master = engine.master_volume;
    assert_eq!(engine.set_master_volume(f32::NAN), ScalarMutation::Rejected);
    assert_eq!(engine.master_volume, initial_master);
    assert_eq!(engine.set_master_volume(50.0), ScalarMutation::Changed);
    assert_eq!(engine.set_master_volume(50.0), ScalarMutation::Unchanged);

    let initial_instrument_volume = engine.slot_volume[0];
    assert_eq!(
        engine.set_instrument_mixer(0, Some(f32::NAN), Some(4)),
        ScalarMutation::Changed
    );
    assert_eq!(engine.slot_volume[0], initial_instrument_volume);
    assert_eq!(engine.slot_pan_pos[0], 4);
    let instrument_gains = engine.slot_pan_gains[0];
    assert_eq!(
        engine.set_instrument_mixer(0, None, Some(4)),
        ScalarMutation::Unchanged
    );
    assert_eq!(engine.slot_pan_gains[0], instrument_gains);
    assert_eq!(
        engine.set_instrument_mixer(0, Some(f32::NEG_INFINITY), None),
        ScalarMutation::Rejected
    );
    assert_eq!(engine.slot_volume[0], initial_instrument_volume);

    let initial_bus_volume = engine.bus_volume[0];
    assert_eq!(
        engine.set_fx_bus_mixer(0, Some(4), Some(f32::INFINITY)),
        ScalarMutation::Changed
    );
    assert_eq!(engine.bus_volume[0], initial_bus_volume);
    assert_eq!(engine.bus_pan_pos[0], 4);
    let bus_gains = engine.bus_pan_gains_cache[0];
    assert_eq!(
        engine.set_fx_bus_mixer(0, None, Some(f32::NAN)),
        ScalarMutation::Rejected
    );
    assert_eq!(engine.bus_pan_gains_cache[0], bus_gains);
    assert_eq!(
        engine.set_fx_bus_mixer(0, None, Some(35.0)),
        ScalarMutation::Changed
    );
    assert_eq!(
        engine.set_fx_bus_mixer(0, None, Some(35.0)),
        ScalarMutation::Unchanged
    );
}

#[cfg(feature = "routing-tree-benchmark")]
#[test]
fn routing_tree_typed_synth_scalar_control_preserves_owner_pair() {
    use std::time::Duration;

    let mut engine = SynthEngine::new(44_100);
    engine.set_instruments(scalar_config());
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut engine, 128)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(Duration::from_secs(1));
    let initial = runtime.home_owner_identities_for_test();
    let mut left = vec![0.0; 128];
    let mut right = vec![0.0; 128];
    let mut output = vec![0.0; 256];
    let disposition = engine.render_interleaved_block_with_source_runtime_ready_with_controls(
        &mut runtime,
        128,
        &mut left,
        &mut right,
        &mut output,
        |engine| {
            assert_eq!(
                engine.set_synth_param_typed(0, SynthParamId::AmpGainPct, 42.0),
                ScalarMutation::Changed
            );
            Ok(())
        },
    );
    assert_eq!(disposition, SourceWorkerRenderDisposition::Fresh);
    assert!(!engine.take_routing_tree_rejection());
    assert!(runtime.collect_wait_for_test(&mut engine));
    assert_eq!(runtime.home_owner_identities_for_test(), initial);
    let owners = runtime.take_home_owners_for_test().expect("owner pair");
    runtime.return_home_owners_for_test(owners);
    let shutdown = lifecycle.shutdown(runtime.retire());
    assert_eq!(shutdown.joined_workers, 2);
}

fn mixer_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "fx_bus_1".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![Default::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}
