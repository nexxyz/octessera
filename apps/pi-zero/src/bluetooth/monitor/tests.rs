use super::*;
use crate::audio::MixTaps;
use std::sync::RwLock;

#[test]
fn player_plays_raw_stereo_on_the_speaker_a2dp_pcm() {
    assert_eq!(
        player_args("AA:BB:CC:DD:EE:FF", 48_000),
        [
            "-q",
            "-t",
            "raw",
            "-f",
            "S16_LE",
            "-c",
            "2",
            "-r",
            "48000",
            "--buffer-time=200000",
            "-D",
            "bluealsa:DEV=AA:BB:CC:DD:EE:FF,PROFILE=a2dp",
        ]
    );
}

#[test]
fn the_monitor_tap_lives_exactly_as_long_as_the_monitor() {
    let taps: MixTapState = Arc::new(RwLock::new(MixTaps::default()));
    let monitor = BluetoothAudioMonitor::start("AA:BB:CC:DD:EE:FF", taps.clone(), 48_000);
    assert_eq!(monitor.address(), "AA:BB:CC:DD:EE:FF");
    assert!(taps.read().unwrap().monitor.is_some());
    assert!(taps.read().unwrap().recording.is_none());

    drop(monitor);
    assert!(taps.read().unwrap().monitor.is_none());
}

#[test]
fn a_player_that_keeps_failing_backs_off_to_once_a_minute() {
    let mut delay = RESTART_DELAY;
    let mut delays = Vec::new();
    for _ in 0..6 {
        delay = next_restart_delay(delay, Duration::from_millis(50));
        delays.push(delay.as_secs());
    }
    assert_eq!(delays, [4, 8, 16, 32, 60, 60]);
    assert_eq!(
        next_restart_delay(MAX_RESTART_DELAY, Duration::from_secs(300)),
        RESTART_DELAY
    );
}
