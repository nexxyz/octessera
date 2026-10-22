//! Instrument setup through the menu: type change, kit load with its confirmation, voice edits
//! and Cell Tune stamps, checked by the pitch of what actually sounds.
use crate::support::{assert_clean_audio, builder, keep_audio, session};
use octessera_desktop_lib::headless::{HeadlessSession, Pace};
use playback_runtime::scenario::button;

const DRUM_SLOT: u8 = 4;

const LOG_STEP: f64 = 1.003;

/// Magnitude spectrum on a log-frequency grid (60 Hz up, `LOG_STEP` apart) of `seconds` of
/// mono audio from `frame`.
fn log_spectrum(session: &HeadlessSession, frame: usize, seconds: f64) -> Vec<f64> {
    let rate = f64::from(session.sample_rate());
    let audio = session.audio();
    let len = (seconds * rate) as usize;
    let mono: Vec<f64> = (0..len)
        .map(|i| {
            let index = (frame + i) * 2;
            f64::from(audio[index] + audio[index + 1]) * 0.5
        })
        .collect();
    let mut spectrum = Vec::new();
    let mut hz = 60.0;
    while hz < 4000.0 {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, sample) in mono.iter().enumerate() {
            let phase = std::f64::consts::TAU * hz * i as f64 / rate;
            re += sample * phase.cos();
            im += sample * phase.sin();
        }
        spectrum.push((re * re + im * im).sqrt());
        hz *= LOG_STEP;
    }
    spectrum
}

/// How far `higher`'s spectrum sits above `lower`'s, as a frequency ratio: the log-frequency
/// shift that lines the two spectra up best. Robust to a drum's inharmonic modes.
fn pitch_ratio(lower: &[f64], higher: &[f64]) -> f64 {
    let best = (0..300)
        .map(|shift| {
            let score: f64 = lower
                .iter()
                .zip(higher.iter().skip(shift))
                .map(|(a, b)| a * b)
                .sum();
            (shift, score)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap()
        .0;
    LOG_STEP.powi(best as i32)
}

#[test]
fn drum_set_up_from_the_menu_plays_its_stamped_tunes() {
    let mut take = builder();
    take.open(&["Shape", "Instruments", "I5:"]);
    take.set_value("Type", "instruments.4.type", "drum");
    take.open(&[
        "Shape",
        "Instruments",
        "I5:",
        "Drum",
        "Kit",
        "Load",
        "Tight",
    ]);
    take.confirm();
    take.checkpoint("kit loaded");
    take.open(&["Shape", "Instruments", "I5:", "Drum"]);
    take.set_value("Voice", "instruments.4.drum.voice", "V6");
    take.enter("Edit");
    for (row, field, value) in [
        ("Sweep st", "sweepSemis", 0),
        ("Noise %", "noiseMixPct", 0),
        ("Decay", "decayMs", 400),
    ] {
        take.set_number(row, &format!("instruments.4.drum.voices.5.{field}"), value);
    }
    take.open(&["Shape", "Instruments", "I5:", "Drum"]);
    for (row, tune) in [(0, 0), (1, 7)] {
        take.set_number("Cell Tune", "instruments.4.cell.tuneSemis", tune);
        take.enter("Assign");
        take.tap_now(row * 4, row);
        take.input(button("button_a", true));
        take.input(button("button_a", false));
    }
    take.open(&["Build", "L5:", "Behavior", "[Human]", "sequencer"]);
    take.open(&["Link", "L5:", "Scanning"]);
    take.set_value("Scan Mode", "layers.4.link.scanMode", "scanning");
    take.set_value("Scan Unit", "layers.4.link.scanUnit", "1/8");
    take.checkpoint("l5 sequencer");
    for layer in 0..4 {
        take.toggle_mute_now(layer);
    }
    take.select_layer_now(4);
    take.tap_now(0, 0);
    take.tap_now(4, 1);
    let mut session = session("instruments", Pace::Fast);
    // The pre-roll lets the engine apply the queued voice edits, as a running device would.
    let played = session.play_scenario(&take.build(), 0.5, 4.0).unwrap();
    let wav = keep_audio(&session, "instruments");
    assert_clean_audio(&session, "instruments");

    let hits: Vec<_> = session
        .events()
        .iter()
        .filter(|event| event.slot == DRUM_SLOT && event.frame >= played.start_frame)
        .collect();
    let first = |cell: u8| {
        hits.iter()
            .find(|event| event.note == cell)
            .unwrap_or_else(|| panic!("cell {cell} never played, audio in {wav:?}"))
    };
    let (root, fifth) = (first(0), first(12));
    assert_eq!(
        (root.voice, fifth.voice),
        (Some(5), Some(5)),
        "both cells play V6"
    );
    let ratio = pitch_ratio(
        &log_spectrum(&session, root.frame + 441, 0.08),
        &log_spectrum(&session, fifth.frame + 441, 0.08),
    );
    let expected = 2.0_f64.powf(7.0 / 12.0);
    assert!(
        (ratio / expected - 1.0).abs() < 0.02,
        "+7 st cell sounds {ratio:.3}x the root, expected {expected:.3}, audio in {wav:?}"
    );
}
