use super::*;
use serde_json::json;
use std::collections::BTreeMap;

#[path = "source_worker_bus_kernel_tests.rs"]
mod kernel;
#[path = "source_worker_bus_protocol_tests.rs"]
mod protocol;

fn bus_config(buses: Vec<Vec<FxBusSlotConfig>>, route: &str) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: route.into(),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: buses
                .into_iter()
                .map(|slots| FxBusConfig {
                    slots,
                    ..FxBusConfig::default()
                })
                .collect(),
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn one_bus(slots: Vec<FxBusSlotConfig>) -> InstrumentsConfig {
    bus_config(vec![slots], "fx_bus_1")
}

fn slot_out(frames: usize, scale: f32) -> [Vec<f32>; INSTRUMENT_SLOT_COUNT] {
    std::array::from_fn(|slot| {
        (0..frames)
            .map(|frame| {
                if slot == 0 {
                    scale * (frame as f32 * 0.17).sin()
                } else {
                    0.0
                }
            })
            .collect()
    })
}

fn configured_fx(kind: &str) -> FxBusSlotConfig {
    FxBusSlotConfig::Kind(kind.into())
}

fn configured_delay_with_spread() -> FxBusSlotConfig {
    FxBusSlotConfig::Config {
        kind: "delay".into(),
        params: BTreeMap::from([
            ("timeMs".into(), json!(2.0)),
            ("feedback".into(), json!(0.35)),
            ("mixPct".into(), json!(55.0)),
            ("spreadPct".into(), json!(100.0)),
        ]),
    }
}

fn install_momentary(
    engine: &mut SynthEngine,
    id: &str,
    kind: &str,
    target: MomentaryFxTarget,
    params: BTreeMap<String, serde_json::Value>,
) {
    let prepared =
        prepare_momentary_fx_start(id.into(), kind.into(), params, target, engine.sample_rate)
            .expect("momentary FX");
    drop(engine.apply_prepared_momentary_fx_start(prepared));
}

fn inline_bus_output(
    engine: &mut SynthEngine,
    slot_out: &[Vec<f32>; INSTRUMENT_SLOT_COUNT],
    frames: usize,
) -> Vec<f32> {
    let mut output = Vec::with_capacity(frames * 2);
    for frame_out in (0..frames).map(|frame| std::array::from_fn(|slot| slot_out[slot][frame])) {
        engine.prepare_bus_buffers();
        let (left, right) = engine.mix_instrument_slots(&frame_out);
        let (left, right) = engine.mix_fx_buses(&frame_out, left, right);
        output.extend([left, right]);
    }
    output
}

fn staged_bus_output(
    config: InstrumentsConfig,
    slot_out: &[Vec<f32>; INSTRUMENT_SLOT_COUNT],
    frames: usize,
    reverse_owner_order: bool,
    momentary: Option<(
        &str,
        &str,
        MomentaryFxTarget,
        BTreeMap<String, serde_json::Value>,
    )>,
) -> Vec<f32> {
    let mut engine = SynthEngine::new(48_000);
    engine.set_instruments(config);
    if let Some((id, kind, target, params)) = momentary {
        install_momentary(&mut engine, id, kind, target, params);
    }
    let (lifecycle, runtime) =
        SourceWorkerLifecycle::start_prewarmed(&mut engine).expect("persistent runtime");
    let mut owners = runtime.take_home_owners_for_test().expect("owner pair");
    assert!(super::source_worker_bus::stage_bus_block(
        &mut engine,
        &mut owners,
        slot_out,
        frames,
    ));
    let stamp = runtime.stamp_for_test(&engine, frames);
    let threshold = engine.dsp_config.bus_idle_threshold;
    let hold_frames = engine.fx_activity_hold_frames;
    if reverse_owner_order {
        let [first, second] = owners;
        owners = [second, first];
    }
    for owner in &mut owners {
        assert!(super::source_worker_bus::render_bus_block(
            owner,
            owner.parity,
            stamp,
            frames,
            engine.sample_rate,
            threshold,
            hold_frames,
        )
        .is_ok());
    }
    let mut left = vec![0.0; frames];
    let mut right = vec![0.0; frames];
    assert!(super::source_worker_bus::apply_bus_block(
        &mut engine,
        &owners,
        frames,
        &mut left,
        &mut right,
    ));
    let mut output = Vec::with_capacity(frames * 2);
    for frame in 0..frames {
        output.extend([left[frame], right[frame]]);
    }
    runtime.return_home_owners_for_test(owners);
    assert_eq!(lifecycle.shutdown(runtime.retire()).joined_workers, 2);
    output
}

fn assert_bits_equal(actual: &[f32], expected: &[f32]) {
    assert_eq!(actual.len(), expected.len());
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        assert_eq!(actual.to_bits(), expected.to_bits(), "sample {index}");
    }
}
