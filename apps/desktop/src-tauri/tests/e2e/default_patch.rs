//! The shipped default patch, as a new install boots it.
use crate::support::{
    assert_clean_audio, keep_audio, rms_frames_db, shipped_builder, shipped_session, Timeline,
};
use playback_runtime::scenario::BAR;

/// The L2 beat as stored in the default patch: per sequencer column, the Sampler tiles
/// (`y * 8 + x`) it plays: kicks on the even columns, claps on 2 and 6, open hats on the odd ones.
const L2_BEAT: [&[u8]; 8] = [&[0], &[17], &[2, 10], &[19], &[4], &[21], &[6, 14], &[23]];
const L5_SLOT: u8 = 4;
const RAIN_SLOT: u8 = 5;
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
        vec![0, 1, 2, 3, 5],
        "L5 sleeps, everything else plays, audio in {wav:?}"
    );
    let rain: Vec<u8> = timeline
        .events(0.0, WAKE_BAR as f64)
        .iter()
        .filter(|event| event.slot == RAIN_SLOT)
        .map(|event| event.note)
        .collect();
    assert!(
        (1..=2 * WAKE_BAR as usize).contains(&rain.len()) && rain.iter().all(|tile| *tile < 8),
        "L6 lands a few drops on its bottom row: {rain:?}, audio in {wav:?}"
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
        for (column, tiles) in L2_BEAT.iter().enumerate() {
            let step = f64::from(bar) + column as f64 / 8.0;
            let mut hits: Vec<u8> = timeline
                .events(step, step + 1.0 / 8.0)
                .iter()
                .filter(|event| event.slot == 1)
                .map(|event| event.note)
                .collect();
            hits.sort_unstable();
            assert_eq!(
                hits, *tiles,
                "L2 bar {bar} column {column}, audio in {wav:?}"
            );
        }
    }
}
