use super::{fill_unless_panicked, AudioStreamHealth};
use crate::audio_stream_health::AudioStreamStatus;

#[test]
fn a_fill_that_returns_leaves_its_output_and_the_stream_health_alone() {
    let health = AudioStreamHealth::new("test".into());
    let mut panicked = false;
    let mut output = vec![0.0_f32; 8];

    fill_unless_panicked(&mut output, &mut panicked, &health, |data| data.fill(0.5));

    assert!(!panicked);
    assert!(output.iter().all(|sample| *sample == 0.5));
    assert_eq!(health.runtime_status(), AudioStreamStatus::Healthy);
}

#[test]
fn a_panicking_fill_leaves_the_stream_silent_and_terminal_for_good() {
    let health = AudioStreamHealth::optional("mirror".into());
    let mut panicked = false;
    let mut output = vec![1.0_f32; 8];

    fill_unless_panicked(&mut output, &mut panicked, &health, |data| {
        data[0] = 0.25;
        panic!("callback source fault");
    });

    assert!(panicked);
    assert!(output.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(health.external_status(), AudioStreamStatus::Terminal);
    assert_eq!(health.runtime_status(), AudioStreamStatus::Terminal);

    let mut later = vec![1.0_f32; 8];
    let mut source_touched = false;
    fill_unless_panicked(&mut later, &mut panicked, &health, |data| {
        source_touched = true;
        data.fill(0.5);
    });

    assert!(!source_touched);
    assert!(later.iter().all(|sample| sample.to_bits() == 0));
    assert_eq!(health.external_status(), AudioStreamStatus::Terminal);
}
