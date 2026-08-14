#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
use crate::audio::AudioSink;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
pub(crate) fn recording_owner(outputs: playback_runtime::AudioOutputSet) -> Option<AudioSink> {
    AudioSink::selected(outputs).into_iter().next()
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    #[test]
    fn recording_owner_covers_every_non_empty_output_set() {
        use super::recording_owner;
        use crate::audio::AudioSink;
        let cases = [
            ((true, false, false), Some(AudioSink::Jack)),
            ((false, true, false), Some(AudioSink::Usb)),
            ((false, false, true), Some(AudioSink::Hdmi)),
            ((true, true, false), Some(AudioSink::Jack)),
            ((true, false, true), Some(AudioSink::Jack)),
            ((false, true, true), Some(AudioSink::Usb)),
            ((true, true, true), Some(AudioSink::Jack)),
        ];
        for ((dac, usb, hdmi), owner) in cases {
            let outputs = playback_runtime::AudioOutputSet::from_flags(dac, usb, hdmi).unwrap();
            assert_eq!(recording_owner(outputs), owner);
        }
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    #[test]
    fn usb_recording_tap_policy_keeps_single_callback_owner() {
        use super::recording_owner;
        use crate::audio::AudioSink;

        assert_eq!(
            recording_owner(
                playback_runtime::AudioOutputSet::from_flags(false, true, false).unwrap()
            ),
            Some(AudioSink::Usb)
        );
        assert_eq!(
            recording_owner(
                playback_runtime::AudioOutputSet::from_flags(true, true, false).unwrap()
            ),
            Some(AudioSink::Jack)
        );
        assert_eq!(
            recording_owner(playback_runtime::AudioOutputSet::jack()),
            Some(AudioSink::Jack)
        );
    }
}
