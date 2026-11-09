use super::*;
use serde_json::json;

const RATE: u32 = 44_100;
const BLOCK: usize = 128;

fn bus_config() -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: vec![InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "synth".into(),
            synth: default_synth_config(),
            mixer: Some(InstrumentMixerConfig {
                route: MixerRoute::FxBus(0),
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume: 100.0,
            }),
        }],
        mixer: Some(MixerConfig {
            buses: vec![FxBusConfig::default()],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn bus_engine() -> SynthEngine {
    let mut engine = SynthEngine::new(RATE);
    engine.set_instruments(bus_config());
    let params = BTreeMap::from([
        ("timePulses".into(), json!(12.0)),
        ("feedbackPct".into(), json!(40.0)),
        ("mixPct".into(), json!(100.0)),
    ]);
    let start = prepare_momentary_fx_start_with_epoch(
        "throw".into(),
        1,
        "dub_throw".into(),
        params,
        MomentaryFxTarget::FxBus { index: 0 },
        RATE,
    )
    .unwrap();
    drop(engine.apply_prepared_momentary_fx_start(start));
    engine.note_on(0, 60, 100, 40);
    engine
}

fn render_block(engine: &mut SynthEngine) -> Vec<f32> {
    let mut left = Vec::new();
    let mut right = Vec::new();
    let mut out = Vec::new();
    engine.render_interleaved_block(BLOCK, &mut left, &mut right, &mut out);
    out
}

#[test]
fn dub_throw_on_a_bus_keeps_echoing_and_ends_after_the_bus_input_stops() {
    let mut engine = bus_engine();
    let mut blocks = 0;
    while engine.bus_chains[0].is_active()
        || engine.active_synth_slots.contains(&true)
        || blocks < 4
    {
        render_block(&mut engine);
        blocks += 1;
        assert!(blocks < 200, "the bus never went idle");
    }
    let mut echo_peak = 0.0_f32;
    for _ in 0..(RATE as usize / BLOCK) {
        let out = render_block(&mut engine);
        echo_peak = out
            .iter()
            .fold(echo_peak, |peak, sample| peak.max(sample.abs()));
    }
    assert!(echo_peak > 1.0e-3, "echo peak {echo_peak}");

    drop(engine.momentary_fx_stop_by_epoch(1));
    for _ in 0..(6 * RATE as usize / BLOCK) {
        render_block(&mut engine);
    }
    assert!(engine.momentary_fx.is_empty());
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_dub_throw_on_an_idle_bus_matches_the_inline_render() {
    let mut routed = bus_engine();
    let mut inline = bus_engine();
    let (lifecycle, mut runtime) =
        SourceWorkerLifecycle::start_routing_tree_prewarmed(&mut routed, BLOCK)
            .expect("routing-tree runtime");
    runtime.set_deadline_for_test(std::time::Duration::from_secs(1));
    let mut left = vec![0.0; BLOCK];
    let mut right = vec![0.0; BLOCK];
    let mut routed_out = vec![0.0; BLOCK * 2];
    let mut echo_peak = 0.0_f32;
    for block in 0..(RATE as usize / BLOCK) {
        assert_eq!(
            routed.render_interleaved_block_with_source_runtime(
                &mut runtime,
                BLOCK,
                &mut left,
                &mut right,
                &mut routed_out,
            ),
            SourceWorkerRenderDisposition::Fresh
        );
        let inline_out = render_block(&mut inline);
        for (frame, (routed, inline)) in routed_out.iter().zip(&inline_out).enumerate() {
            assert!(
                routed.to_bits() == inline.to_bits(),
                "block {block} sample {frame}: {routed} vs {inline}"
            );
        }
        if !inline.bus_chains[0].is_active() && !inline.active_synth_slots.contains(&true) {
            echo_peak = inline_out
                .iter()
                .fold(echo_peak, |peak, sample| peak.max(sample.abs()));
        }
    }
    assert!(echo_peak > 1.0e-3, "echo peak {echo_peak}");
    lifecycle.shutdown(runtime.retire());
}
