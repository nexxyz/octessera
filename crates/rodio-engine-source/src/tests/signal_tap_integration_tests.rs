use super::*;
use realtime_engine::synth::SignalSource;

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
