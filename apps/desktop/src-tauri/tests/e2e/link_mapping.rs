//! Link note mapping set up from the menu: pitch steps per column and row, a scale and root, and
//! the difference between clamping and wrapping notes past High Note, heard as played notes.
use crate::support::{assert_clean_audio, builder, keep_audio, session};
use octessera_desktop_lib::headless::Pace;

const SYNTH_SLOT: u8 = 4;

/// Sequencer cells (x, y) and their degree with one step per column and one per row.
const CELLS: [(usize, usize); 4] = [(0, 0), (1, 0), (3, 1), (7, 7)];

/// D major pentatonic within the shipped L5 range 36..74, nearest to Start Note 60 is 59 (B3).
const PENTATONIC: [u8; 16] = [
    38, 40, 42, 45, 47, 50, 52, 54, 57, 59, 62, 64, 66, 69, 71, 74,
];
const START_INDEX: usize = 9;

fn played_notes(out_of_range: &str) -> Vec<u8> {
    let mut take = builder();
    take.open(&["Shape", "Instruments", "I5:"]);
    take.set_value("Type", "instruments.4.type", "synth");
    take.open(&["Build", "L5:", "Behavior", "[Human]", "sequencer"]);
    take.open(&["Link", "L5:", "Scanning"]);
    take.set_value("Scan Mode", "layers.4.link.scanMode", "scanning");
    take.set_value("Scan Unit", "layers.4.link.scanUnit", "1/8");
    take.open(&["Link", "L5:", "Note Mapping"]);
    take.set_value(
        "Out of Range",
        "layers.4.link.pitch.outOfRange",
        out_of_range,
    );
    take.open(&["Link", "L5:", "X Axis", "Pitch Steps"]);
    take.set_number("Steps", "layers.4.link.x.pitch.steps", 1);
    take.checkpoint("l5 mapped");
    for layer in 0..4 {
        take.toggle_mute_now(layer);
    }
    take.select_layer_now(4);
    for (x, y) in CELLS {
        take.tap_now(x, y);
    }
    let name = format!("link-mapping-{out_of_range}");
    let mut session = session(&name, Pace::Fast);
    let played = session.play_scenario(&take.build(), 0.5, 4.0).unwrap();
    let wav = keep_audio(&session, &name);
    assert_clean_audio(&session, &name);
    let mut notes: Vec<u8> = session
        .events()
        .iter()
        .filter(|event| event.slot == SYNTH_SLOT && event.frame >= played.start_frame)
        .map(|event| event.note)
        .collect();
    assert!(!notes.is_empty(), "L5 never played, audio in {wav:?}");
    notes.sort_unstable();
    notes.dedup();
    notes
}

fn expected(wrap: bool) -> Vec<u8> {
    let mut notes: Vec<u8> = CELLS
        .iter()
        .map(|(x, y)| {
            let index = START_INDEX + x + y;
            let index = if wrap {
                index % PENTATONIC.len()
            } else {
                index.min(PENTATONIC.len() - 1)
            };
            PENTATONIC[index]
        })
        .collect();
    notes.sort_unstable();
    notes
}

#[test]
fn link_note_mapping_steps_through_the_scale_and_clamps_or_wraps_at_the_range() {
    let clamped = played_notes("clamp");
    assert_eq!(clamped, expected(false), "clamp");
    assert_eq!(
        clamped.last(),
        Some(&74),
        "the top cell clamps to High Note"
    );
    let wrapped = played_notes("wrap");
    assert_eq!(wrapped, expected(true), "wrap");
    assert!(
        !wrapped.contains(&74),
        "the top cell wraps back into the range instead"
    );
}
