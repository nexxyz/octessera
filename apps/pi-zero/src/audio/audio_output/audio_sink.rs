use playback_runtime::AudioOutputSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AudioSink {
    Jack,
    Usb,
    Hdmi,
}

impl AudioSink {
    pub(crate) fn scheduler_label(self) -> &'static str {
        match self {
            Self::Jack => "Jack",
            Self::Usb => "USB",
            Self::Hdmi => "HDMI",
        }
    }

    pub(crate) fn selected(outputs: AudioOutputSet) -> Vec<Self> {
        [
            (outputs.dac(), Self::Jack),
            (outputs.usb(), Self::Usb),
            (outputs.hdmi(), Self::Hdmi),
        ]
        .into_iter()
        .filter_map(|(enabled, sink)| enabled.then_some(sink))
        .collect()
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn startup(_outputs: AudioOutputSet) -> Vec<Self> {
        vec![Self::Jack]
    }

    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    pub(crate) fn startup(outputs: AudioOutputSet) -> Vec<Self> {
        let mut sinks = vec![Self::Jack];
        sinks.extend(
            [Self::Usb, Self::Hdmi]
                .into_iter()
                .filter(|sink| Self::selected(outputs).contains(sink)),
        );
        sinks
    }
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
mod tests {
    use super::*;

    #[test]
    fn audio_sink_plan_preserves_desired_audio_outputs() {
        for (flags, expected) in [
            ((true, false, false), vec![AudioSink::Jack]),
            ((false, true, false), vec![AudioSink::Usb]),
            ((true, true, false), vec![AudioSink::Jack, AudioSink::Usb]),
        ] {
            let outputs = AudioOutputSet::from_flags(flags.0, flags.1, flags.2).unwrap();
            assert_eq!(AudioSink::selected(outputs), expected);
        }
    }

    #[test]
    fn startup_opens_only_jack_and_leaves_side_outputs_to_their_workers() {
        for outputs in [
            AudioOutputSet::jack(),
            AudioOutputSet::from_flags(true, true, false).unwrap(),
            AudioOutputSet::from_flags(true, true, true).unwrap(),
        ] {
            assert_eq!(AudioSink::startup(outputs), vec![AudioSink::Jack]);
        }
    }
}
