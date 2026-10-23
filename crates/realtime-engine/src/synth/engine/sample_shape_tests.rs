use super::sample_voice_shape::SampleVoiceShape;
use super::*;

const RATE: u32 = 48_000;

fn sampler_engine(shape: SampleShapeConfig, samples: Vec<f32>) -> SynthEngine {
    let mut engine = SynthEngine::new(RATE);
    engine.set_instrument_slot(
        0,
        InstrumentSlotConfig {
            fm: None,
            pluck: None,
            drum: None,
            kind: "sampler".into(),
            synth: default_synth_config(),
            mixer: None,
        },
    );
    let mut bank = SampleBankConfig::default();
    bank.slots[0] = SampleSlotConfig {
        buffer: Some(SampleBuffer {
            samples: samples.into(),
            channels: 1,
            sample_rate: RATE,
        }),
    };
    bank.filter_cutoff_hz = 20_000.0;
    bank.shape = shape;
    engine.set_sample_banks(vec![bank; INSTRUMENT_SLOT_COUNT]);
    engine
}

fn render(engine: &mut SynthEngine, frames: usize) -> Vec<f32> {
    let (mut left, mut right, mut out) = (Vec::new(), Vec::new(), Vec::new());
    let mut rendered = Vec::with_capacity(frames);
    while rendered.len() < frames {
        engine.render_interleaved_block(64, &mut left, &mut right, &mut out);
        rendered.extend_from_slice(&left);
    }
    rendered.truncate(frames);
    rendered
}

fn ms(value: f32) -> usize {
    (value / 1000.0 * RATE as f32) as usize
}

#[test]
fn neutral_shape_plays_a_sample_at_full_level_until_note_off() {
    let mut engine = sampler_engine(SampleShapeConfig::default(), vec![0.5; RATE as usize]);
    engine.note_on(0, 36, 127, 50_000);
    let held = render(&mut engine, ms(200.0));
    let steady = held[ms(100.0)];
    assert!(steady > 0.05);
    assert!(held[ms(20.0)..]
        .iter()
        .all(|sample| (sample - steady).abs() < 1e-4));

    engine.note_off(0, 36);
    let released = render(&mut engine, ms(20.0));
    assert!(released[ms(10.0)..]
        .iter()
        .all(|sample| sample.abs() < 1e-6));
}

#[test]
fn amp_env_shapes_attack_sustain_and_release() {
    let shape = SampleShapeConfig {
        amp_env: EnvConfig {
            attack_ms: 20.0,
            decay_ms: 20.0,
            sustain_pct: 50.0,
            release_ms: 40.0,
        },
        ..SampleShapeConfig::default()
    };
    let mut engine = sampler_engine(shape, vec![0.5; RATE as usize]);
    engine.note_on(0, 36, 127, 50_000);
    let held = render(&mut engine, ms(150.0));
    let sustained = held[ms(120.0)];
    assert!(held[ms(5.0)] < held[ms(15.0)], "attack ramps up");
    assert!(held[ms(20.0)] > sustained * 1.5, "peak before the decay");
    assert!((held[ms(100.0)] - sustained).abs() < 1e-3, "sustain holds");

    engine.note_off(0, 36);
    let released = render(&mut engine, ms(60.0));
    assert!(released[ms(20.0)] < sustained * 0.6);
    assert!(released[ms(45.0)..]
        .iter()
        .all(|sample| sample.abs() < 1e-6));
    assert_eq!(engine.profile_snapshot().active_sample_voices, 0);
}

#[test]
fn filter_type_env_amount_and_key_tracking_change_the_sound() {
    let dc = vec![0.5; RATE as usize];
    let level = |shape: SampleShapeConfig, note: u8, cutoff: f32| {
        let mut engine = sampler_engine(shape, dc.clone());
        engine.sample_banks[0].filter_cutoff_hz = cutoff;
        engine.note_on(0, note, 127, 50_000);
        render(&mut engine, ms(50.0))[ms(40.0)].abs()
    };
    let lowpass = level(SampleShapeConfig::default(), 36, 1_000.0);
    let highpass = level(
        SampleShapeConfig {
            filter_type: FilterType::Highpass,
            ..SampleShapeConfig::default()
        },
        36,
        1_000.0,
    );
    assert!(
        lowpass > 0.05 && highpass < lowpass * 0.01,
        "{lowpass} vs {highpass}"
    );

    let shape = SampleVoiceShape::start(
        &SampleShapeConfig {
            filter_key_tracking_pct: 100.0,
            ..SampleShapeConfig::default()
        },
        12.0,
        RATE,
    );
    assert!((shape.static_cutoff(1_000.0, RATE) - 2_000.0).abs() < 1.0);

    let enveloped = SampleVoiceShape::start(
        &SampleShapeConfig {
            filter_env_amount_pct: 100.0,
            ..SampleShapeConfig::default()
        },
        0.0,
        RATE,
    );
    assert!(
        !enveloped.is_static(),
        "an enveloped filter leaves the plain loop"
    );
    assert!(SampleVoiceShape::start(&SampleShapeConfig::default(), 0.0, RATE).is_static());
}

#[test]
fn only_neutral_filter_settings_skip_the_filter() {
    use super::super::runtime_state::BiquadState;
    let open = |kind, cutoff, q| {
        let mut filter = BiquadState::new();
        filter.prepare(kind, cutoff, q, RATE);
        filter.is_open()
    };
    assert!(open(FilterType::Lowpass, 20_000.0, 0.5));
    assert!(open(FilterType::Highpass, 20.0, 0.5));
    assert!(!open(FilterType::Lowpass, 19_000.0, 0.5));
    assert!(
        !open(FilterType::Lowpass, 20_000.0, 2.8),
        "resonance colours it"
    );
    assert!(!open(FilterType::Highpass, 40.0, 0.5));
    assert!(!open(FilterType::Bandpass, 20_000.0, 0.5));
    assert!(!open(FilterType::Notch, 20.0, 0.5));
}

#[test]
fn default_sampler_plays_the_sample_through_untouched() {
    let bank = SampleBankConfig::default();
    assert_eq!(
        (bank.filter_cutoff_hz, bank.filter_resonance),
        (20_000.0, 0.0)
    );
    let ramp = (0..4_800)
        .map(|index| (index as f32 * 0.01).sin() * 0.5)
        .collect::<Vec<_>>();
    let mut engine = sampler_engine(SampleShapeConfig::default(), ramp.clone());
    engine.sample_banks[0].filter_cutoff_hz = bank.filter_cutoff_hz;
    engine.sample_banks[0].filter_resonance = bank.filter_resonance;
    engine.note_on(0, 36, 127, 50_000);
    let rendered = render(&mut engine, 2_000);
    let gain = rendered[1_000] / ramp[1_000];
    for index in 10..2_000 {
        assert!(
            (rendered[index] - ramp[index] * gain).abs() < 1e-5,
            "sample {index} is filtered"
        );
    }
}
