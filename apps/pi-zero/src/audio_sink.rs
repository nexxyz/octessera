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
    pub(crate) fn startup(outputs: AudioOutputSet) -> Vec<Self> {
        outputs.dac().then_some(Self::Jack).into_iter().collect()
    }

    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    pub(crate) fn optional_recovery(outputs: AudioOutputSet) -> Vec<Self> {
        [Self::Usb, Self::Hdmi]
            .into_iter()
            .filter(|sink| Self::selected(outputs).contains(sink))
            .collect()
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
    fn startup_and_recovery_plan_keeps_optional_routes_out_of_startup() {
        let jack = AudioOutputSet::jack();
        let usb = AudioOutputSet::from_flags(false, true, false).unwrap();
        let both = AudioOutputSet::from_flags(true, true, false).unwrap();

        assert_eq!(AudioSink::startup(jack), vec![AudioSink::Jack]);
        assert!(AudioSink::optional_recovery(jack).is_empty());
        assert!(AudioSink::startup(usb).is_empty());
        assert_eq!(AudioSink::optional_recovery(usb), vec![AudioSink::Usb]);
        assert_eq!(AudioSink::startup(both), vec![AudioSink::Jack]);
        assert_eq!(AudioSink::optional_recovery(both), vec![AudioSink::Usb]);
    }
}
