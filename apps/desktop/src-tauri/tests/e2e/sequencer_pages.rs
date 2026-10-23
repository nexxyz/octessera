//! Sequencer pages turn on musical time and land on the scan's bar line.
use crate::support::{assert_clean_audio, builder, keep_audio, session, Timeline};
use octessera_desktop_lib::headless::Pace;

const SAMPLER: u8 = 1;
/// The hat row (sample slot 2) in a column page 1 leaves empty.
const HAT_ONLY_CELL: u8 = 2 * 8;

#[test]
fn sequencer_pages_advance_every_bar_in_step_with_scanning() {
    let mut take = builder();
    take.open(&["Build", "L2:"]);
    let config = "layers.1.build.behaviorConfig";
    take.set_number("Pages X", &format!("{config}.pagesX"), 2);
    take.set_number("Page X", &format!("{config}.pageX"), 2);
    take.select_layer_now(1);
    take.tap_now(0, 2);
    take.open(&["Build", "L2:"]);
    take.set_number("Page X", &format!("{config}.pageX"), 1);
    // L2 steps at 1/8, one per scan column, so 8 steps make a bar.
    take.set_number("Advance", &format!("{config}.advanceSteps"), 8);
    take.checkpoint("pages set");
    for layer in [0, 2, 3] {
        take.toggle_mute_now(layer);
    }
    let mut session = session("sequencer-pages", Pace::Fast);
    let played = session.play_scenario(&take.build(), 0.0, 8.0).unwrap();
    let timeline = Timeline {
        session: &session,
        played,
    };
    let wav = keep_audio(&session, "sequencer-pages");
    assert_clean_audio(&session, "sequencer pages");

    for bar in 0..4 {
        let cells: Vec<u8> = timeline
            .events(f64::from(bar), f64::from(bar) + 1.0)
            .iter()
            .filter(|event| event.slot == SAMPLER)
            .map(|event| event.note)
            .collect();
        if bar % 2 == 0 {
            assert!(
                cells.contains(&0),
                "bar {bar}: page 1's kick, audio in {wav:?}"
            );
            assert!(
                !cells.contains(&HAT_ONLY_CELL),
                "bar {bar} plays page 1 only"
            );
        } else {
            assert_eq!(
                cells,
                vec![HAT_ONLY_CELL],
                "bar {bar}: page 2 only, audio in {wav:?}"
            );
        }
    }
}
