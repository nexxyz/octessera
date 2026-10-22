use crate::support::{assert_clean_audio, builder, keep_audio, session, Timeline};
use octessera_desktop_lib::headless::Pace;

/// The L2 beat as stored in the default patch: cell index `y * 8 + x` per sequencer column.
const L2_BEAT: [&[u8]; 8] = [&[0], &[17], &[2, 10], &[19], &[4], &[21], &[6, 14], &[23]];

#[test]
fn default_patch_plays_every_layer_and_the_beat_matches_its_grid() {
    let scenario = builder().build();
    let mut session = session("default-patch", Pace::Fast);
    let played = session.play_scenario(&scenario, 0.0, 16.5).unwrap();
    let timeline = Timeline {
        session: &session,
        played,
    };
    let wav = keep_audio(&session, "default-patch");
    assert_clean_audio(&session, "default patch");
    assert_eq!(
        timeline.slots(0.0, 8.0),
        vec![0, 1, 2, 3],
        "audio in {wav:?}"
    );
    assert!(
        timeline.rms_db(0.0, 8.0) > -40.0,
        "too quiet, audio in {wav:?}"
    );

    for bar in 0..8 {
        for (column, cells) in L2_BEAT.iter().enumerate() {
            let step = f64::from(bar) + column as f64 / 8.0;
            let mut hits: Vec<u8> = timeline
                .events(step, step + 1.0 / 8.0)
                .iter()
                .filter(|event| event.slot == 1)
                .map(|event| event.note)
                .collect();
            hits.sort_unstable();
            assert_eq!(
                &hits, cells,
                "L2 bar {bar} column {column}, audio in {wav:?}"
            );
        }
    }
}
