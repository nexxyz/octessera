//! Play Transpose on the Sampler-only L2 beat: its cells shift only when the Sampler's
//! Follow Transp. is on, heard as the pitch each hit asks the engine for.
use crate::support::{assert_clean_audio, builder, keep_audio, session, Timeline};
use octessera_desktop_lib::headless::Pace;
use playback_runtime::scenario::{grid, BAR};
use std::collections::BTreeMap;

const SAMPLER: u8 = 1;
/// The Transpose page's +7 key: row 3 is the middle octave's white keys, C at column 1.
const FIFTH_KEY: (usize, usize) = (5, 3);

/// Pitch per L2 cell before (bars 0-2) and after (bars 2-4) the +7 key is pressed at bar 2.
fn cell_pitches(follow: bool) -> (BTreeMap<u8, i16>, BTreeMap<u8, i16>) {
    let mut take = builder();
    if follow {
        take.open(&["Shape", "Instruments", "I2:", "Sampler"]);
        take.set_value(
            "Follow Transp.",
            "instruments.1.sample.followTranspose",
            "true",
        );
    }
    for layer in [0, 2, 3] {
        take.toggle_mute_now(layer);
    }
    take.back_to_root();
    take.with_mods_now(&["button_fn"], &[grid(7, 4, true), grid(7, 4, false)]);
    take.checkpoint("transpose page");
    take.tap_at(FIFTH_KEY.0, FIFTH_KEY.1, 2 * BAR - 6, true);
    let name = format!("follow-transpose-{follow}");
    let mut session = session(&name, Pace::Fast);
    let played = session.play_scenario(&take.build(), 0.5, 8.0).unwrap();
    let wav = keep_audio(&session, &name);
    assert_clean_audio(&session, &name);
    let timeline = Timeline {
        session: &session,
        played,
    };
    let pitches = |from_bar: f64, to_bar: f64| {
        let mut cells = BTreeMap::new();
        for event in timeline.events(from_bar, to_bar) {
            if event.slot == SAMPLER {
                let previous = cells.insert(event.note, event.pitch_cents);
                assert!(
                    previous.is_none_or(|cents| cents == event.pitch_cents),
                    "cell {} changes pitch within bars {from_bar}-{to_bar}, audio in {wav:?}",
                    event.note
                );
            }
        }
        assert!(!cells.is_empty(), "L2 silent, audio in {wav:?}");
        cells
    };
    (pitches(0.0, 2.0), pitches(2.0, 4.0))
}

#[test]
fn play_transpose_shifts_sampler_cells_only_with_follow_transpose() {
    let (before, after) = cell_pitches(false);
    assert_eq!(after, before, "Follow Transp. off keeps the stamped pitch");

    let (before, after) = cell_pitches(true);
    let shifted: BTreeMap<u8, i16> = before
        .iter()
        .map(|(cell, cents)| (*cell, cents + 700))
        .collect();
    assert_eq!(after, shifted, "Follow Transp. on shifts every cell +7 st");
}
