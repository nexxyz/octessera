use super::super::freeze::{FreezeParams, FreezeVoice};

#[test]
fn freeze_inject_fades_out_over_its_last_five_ms_and_ends_on_the_same_frame() {
    let params = FreezeParams {
        mix: 1.0,
        release_len: 1,
    };
    let mut voice = FreezeVoice::new(params, 44_100);
    let gains: Vec<f32> = (0..6_000).map(|_| voice.inject_gain()).collect();
    let inject_frames = 44_100 * 120 / 1000;
    let fade_frames = 44_100 * 5 / 1000;

    assert_eq!(
        gains.iter().filter(|gain| **gain > 0.0).count(),
        inject_frames
    );
    assert!(gains[inject_frames..].iter().all(|gain| *gain == 0.0));
    assert!(gains[..inject_frames - fade_frames + 1]
        .iter()
        .all(|gain| *gain == 1.0));
    let allowed_step = 1.0 / fade_frames as f32 + 1.0e-6;
    for frame in inject_frames - fade_frames + 1..=inject_frames {
        let step = gains[frame - 1] - gains[frame];
        assert!(
            step > 0.0 && step <= allowed_step,
            "frame {frame}: step {step}"
        );
    }
}
