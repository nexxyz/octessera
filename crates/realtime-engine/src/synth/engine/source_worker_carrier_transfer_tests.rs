use super::source_worker_lifecycle::OwnerEnvelope;
use super::*;

#[test]
fn repeated_generic_owner_transfer_preserves_carrier_scratch_and_dsp_state() {
    let mut engine = transfer_test_engine();
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
    seed_transfer_state(
        &mut owners,
        engine.dsp_config.bus_idle_threshold,
        engine.fx_activity_hold_frames,
    );
    let expected = transfer_state_snapshot(&owners);
    runtime.return_home_owners_for_test(owners);

    let mut runtime = runtime;
    for _ in 0..3 {
        assert!(runtime.with_controls_ready(&mut engine, |_| ()).is_some());
        let owners = runtime.take_home_owners_for_test().expect("owner pair");
        assert_eq!(transfer_state_snapshot(&owners), expected);
        runtime.return_home_owners_for_test(owners);
    }

    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
}

fn transfer_test_engine() -> SynthEngine {
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

fn seed_transfer_state(
    owners: &mut [OwnerEnvelope; 2],
    threshold: super::super::dsp_config::BusIdleThreshold,
    hold_frames: u32,
) {
    for carrier in owners
        .iter_mut()
        .flat_map(|owner| owner.bus_carriers.iter_mut().flatten())
    {
        carrier.scratch.input[0] = 0.25;
        carrier.scratch.input[64] = -0.5;
        carrier.scratch.resolved_duck[0][0] = 0.75;
        carrier.scratch.resolved_duck[1][64] = -0.25;
        carrier.scratch.mono_output[0] = 0.125;
        carrier.scratch.auto_pan_pos[64] = 0.375;
        carrier.scratch.processed_prefix = 128;
        carrier.scratch.spread = 0.625;
        carrier.scratch.executed = true;
        let Some(owner) = carrier.owner.as_mut() else {
            continue;
        };
        assert!(carrier.scratch.prepare(64));
        carrier.scratch.input[..64].fill(0.25);
        assert!(owner
            .process_block(&mut carrier.scratch, 64, 48_000, threshold, hold_frames,)
            .is_ok());
    }
}

fn transfer_state_snapshot(owners: &[OwnerEnvelope; 2]) -> Vec<(usize, Vec<u32>, String)> {
    let mut snapshot = owners
        .iter()
        .flat_map(|owner| owner.bus_carriers.iter().flatten())
        .map(|carrier| {
            let mut scratch = Vec::new();
            scratch.extend(carrier.scratch.input.iter().map(|value| value.to_bits()));
            for buffer in &carrier.scratch.resolved_duck {
                scratch.extend(buffer.iter().map(|value| value.to_bits()));
            }
            scratch.extend(
                carrier
                    .scratch
                    .mono_output
                    .iter()
                    .map(|value| value.to_bits()),
            );
            scratch.extend(
                carrier
                    .scratch
                    .auto_pan_pos
                    .iter()
                    .map(|value| value.to_bits()),
            );
            scratch.extend([
                carrier.scratch.processed_prefix as u32,
                carrier.scratch.spread.to_bits(),
                carrier.scratch.executed as u32,
            ]);
            (
                carrier.logical_bus_id,
                scratch,
                format!(
                    "{:?}",
                    carrier.owner.as_ref().map(|owner| &owner.slot_state)
                ),
            )
        })
        .collect::<Vec<_>>();
    snapshot.sort_by_key(|(logical_bus_id, _, _)| *logical_bus_id);
    snapshot
}
