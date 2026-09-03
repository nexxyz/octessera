use super::*;

#[test]
fn persistent_source_staging_matches_inline_when_block_size_shrinks() {
    let mut worker = routed_delay_engine();
    let mut inline = routed_delay_engine();
    worker.note_on(0, 60, 100, 5_000);
    inline.note_on(0, 60, 100, 5_000);
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut worker).expect("persistent runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut worker_left = Vec::with_capacity(BLOCK_SLOT_SCRATCH_FRAMES);
    let mut worker_right = Vec::with_capacity(BLOCK_SLOT_SCRATCH_FRAMES);
    let mut worker_out = Vec::with_capacity(BLOCK_SLOT_SCRATCH_FRAMES * 2);
    let mut inline_left = Vec::with_capacity(BLOCK_SLOT_SCRATCH_FRAMES);
    let mut inline_right = Vec::with_capacity(BLOCK_SLOT_SCRATCH_FRAMES);
    let mut inline_out = Vec::with_capacity(BLOCK_SLOT_SCRATCH_FRAMES * 2);

    for frames in [2048, 64, 256, 32, 128] {
        worker.render_interleaved_block_with_source_runtime(
            &mut runtime,
            frames,
            &mut worker_left,
            &mut worker_right,
            &mut worker_out,
        );
        inline.render_interleaved_block(
            frames,
            &mut inline_left,
            &mut inline_right,
            &mut inline_out,
        );
        assert_eq!(
            runtime.health_snapshot().status,
            SourceWorkerHealth::Healthy
        );
        assert_eq!(worker_out.len(), inline_out.len());
        for (index, (actual, expected)) in worker_out.iter().zip(&inline_out).enumerate() {
            assert_eq!(actual.to_bits(), expected.to_bits(), "sample {index}");
        }
    }

    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

fn routed_delay_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: "B1".into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: (0..BUS_COUNT)
                .map(|_| FxBusConfig {
                    slots: vec![FxBusSlotConfig::Kind("delay".into()); BUS_SLOTS_PER_BUS],
                    ..FxBusConfig::default()
                })
                .collect(),
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    });
    engine
}
