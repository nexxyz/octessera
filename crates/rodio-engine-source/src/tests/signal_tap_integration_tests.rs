use super::*;
use realtime_engine::synth::{
    default_synth_config, prepare_instruments_config, FxBusConfig, FxBusSlotConfig,
    InstrumentMixerConfig, InstrumentSlotConfig, InstrumentsConfig, MixerConfig, SignalSource,
    DEFAULT_PAN_POSITIONS,
};

fn sounding_source(tap: &'static SignalTap) -> EngineSource {
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 1_000,
        at: None,
    })
    .unwrap();
    let mut source = EngineSource::with_block_frames(rx, 48_000, 32);
    source.set_signal_tap_for_test(tap);
    source
}

#[test]
fn a_selected_tap_receives_the_mono_average_of_the_final_blocks() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    tap.select(Some(SignalSource::Master));
    let mut source = sounding_source(tap);

    let jack: Vec<f32> = (0..128).map(|_| source.next().unwrap()).collect();
    assert_eq!(tap.written(), 32);
    assert_eq!(tap.sample_rate(), 24_000);
    let mut tapped = [0.0_f32; 32];
    assert_eq!(tap.read_latest(0, &mut tapped), 32);
    let expected: Vec<f32> = jack
        .as_chunks::<4>()
        .0
        .iter()
        .map(|pair| pair.iter().sum::<f32>() * 0.25)
        .collect();
    assert_eq!(tapped.to_vec(), expected);
    assert!(tapped.iter().any(|sample| *sample != 0.0));
}

#[test]
fn an_unselected_tap_stays_untouched_while_the_source_renders() {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    let mut source = sounding_source(tap);
    for _ in 0..128 {
        source.next();
    }
    assert_eq!(tap.written(), 0);
}

/// Events that route a sounding synth on slot 0 through a reverb on bus 1.
fn bus_routed_synth_events() -> (EngineEventSender, EngineEventReceiver) {
    let instruments = InstrumentsConfig {
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
            buses: vec![FxBusConfig {
                slots: vec![FxBusSlotConfig::Kind("reverb".into())],
                pan_pos: DEFAULT_PAN_POSITIONS / 2,
                volume_pct: 100.0,
            }],
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    };
    let (tx, rx) = event_queue();
    tx.send(EngineEvent::SetPreparedInstruments {
        generation: 0,
        config: prepare_instruments_config(instruments, 48_000),
    })
    .unwrap();
    tx.send(EngineEvent::NoteOn {
        instrument_slot: 0,
        note: 60,
        velocity: 100,
        duration_ms: 2_000,
        at: None,
    })
    .unwrap();
    (tx, rx)
}

/// The samples `signal` gets while `source` renders eight 128-frame blocks, after two blocks
/// that let a quantum the workers render ahead with the previous selection land.
fn tapped(source: &mut EngineSource, tap: &SignalTap, signal: SignalSource) -> Vec<f32> {
    tap.select(Some(signal));
    for _ in 0..128 * 2 * 2 {
        source.next();
    }
    let since = tap.written();
    for _ in 0..128 * 2 * 8 {
        source.next();
    }
    let mut samples = vec![0.0_f32; 512];
    let count = tap.read_latest(since, &mut samples);
    samples.split_off(samples.len() - count)
}

fn assert_instrument_and_bus_taps(source: &mut EngineSource) {
    let tap: &'static SignalTap = Box::leak(Box::new(SignalTap::new()));
    source.set_signal_tap_for_test(tap);
    let instrument = tapped(source, tap, SignalSource::Instrument(0));
    assert!(instrument.iter().any(|sample| sample.abs() > 1.0e-3));
    let bus = tapped(source, tap, SignalSource::Bus(0));
    assert!(bus.iter().any(|sample| sample.abs() > 1.0e-4));
    let silent = tapped(source, tap, SignalSource::Instrument(1));
    assert!(silent.iter().all(|sample| *sample == 0.0));
    assert_eq!(tapped(source, tap, SignalSource::Master).len(), 512);
    assert_eq!(tap.sample_rate(), 24_000);
}

#[test]
fn the_inline_engine_taps_its_instrument_and_bus_blocks() {
    let (_tx, rx) = bus_routed_synth_events();
    let mut source = EngineSource::with_block_frames(rx, 48_000, 128);
    assert_instrument_and_bus_taps(&mut source);
}

#[cfg(feature = "routing-tree-executor")]
#[test]
fn routing_tree_workers_tap_the_instrument_and_bus_they_own() {
    let (_tx, rx) = bus_routed_synth_events();
    let (mut source, shutdown) =
        EngineSource::with_routing_tree_persistent_workers(rx, 48_000, 128, None)
            .expect("routing-tree runtime");
    assert_instrument_and_bus_taps(&mut source);
    drop(source);
    assert_eq!(shutdown.shutdown().joined_workers, 2);
}
