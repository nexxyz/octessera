//! The shipped default patch, as a new install boots it.
use crate::support::{
    assert_clean_audio, keep_audio, rms_frames_db, shipped_builder, shipped_session, Timeline,
};
use playback_runtime::scenario::BAR;

/// The L2 beat as stored in the default patch: per sequencer column, the tiles (`y * 8 + x`)
/// that always play, then the ones Trigger Prob. `custom` may drop (the offbeat hats High, the
/// rim Low).
const L2_BEAT: [(&[u8], &[u8]); 8] = [
    (&[0, 16], &[]),
    (&[], &[17]),
    (&[10, 18], &[]),
    (&[], &[19]),
    (&[4, 20], &[]),
    (&[], &[21, 29]),
    (&[14, 22], &[]),
    (&[], &[23]),
];
const L5_SLOT: u8 = 4;
const WAKE_BAR: u64 = 8;

#[test]
fn default_patch_plays_every_awake_layer_the_beat_matches_its_grid_and_l5_wakes() {
    let mut builder = shipped_builder();
    builder.toggle_mute_at(usize::from(L5_SLOT), WAKE_BAR * BAR);
    let mut session = shipped_session("default-patch");
    let played = session
        .play_scenario(&builder.build(), 0.0, 12.0 * 60.0 / 110.0 * 4.0)
        .unwrap();
    let timeline = Timeline {
        session: &session,
        played,
    };
    let wav = keep_audio(&session, "default-patch");
    assert_clean_audio(&session, "default patch");
    assert_eq!(
        timeline.slots(0.0, WAKE_BAR as f64),
        vec![0, 1, 2, 3],
        "L5 sleeps, everything else plays, audio in {wav:?}"
    );
    let audio_frames = session.audio().len() / 2;
    assert!(
        rms_frames_db(&session, timeline.played.start_frame, audio_frames) > -40.0,
        "too quiet, audio in {wav:?}"
    );
    assert!(
        timeline
            .slots(WAKE_BAR as f64, WAKE_BAR as f64 + 4.0)
            .contains(&L5_SLOT),
        "L5 woke and played, audio in {wav:?}"
    );

    for bar in 0..8 {
        for (column, (always, maybe)) in L2_BEAT.iter().enumerate() {
            let step = f64::from(bar) + column as f64 / 8.0;
            let hits: Vec<u8> = timeline
                .events(step, step + 1.0 / 8.0)
                .iter()
                .filter(|event| event.slot == 1)
                .map(|event| event.note)
                .collect();
            for tile in *always {
                assert!(
                    hits.contains(tile),
                    "L2 bar {bar} column {column} misses tile {tile}: {hits:?}, audio in {wav:?}"
                );
            }
            for tile in &hits {
                assert!(
                    always.contains(tile) || maybe.contains(tile),
                    "L2 bar {bar} column {column} plays tile {tile}, audio in {wav:?}"
                );
            }
        }
    }
}
