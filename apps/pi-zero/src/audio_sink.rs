#[cfg(any(test, feature = "hardware-orange-pi-zero-2w"))]
use crate::usb_config::UsbAudioOut;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AudioSink {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    Jack,
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    InternalDac,
    Usb,
}

impl AudioSink {
    pub(crate) fn scheduler_label(self) -> &'static str {
        match self {
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            Self::Jack => "Jack",
            #[cfg(feature = "hardware-orange-pi-zero-2w")]
            Self::InternalDac => "DAC",
            Self::Usb => {
                #[cfg(feature = "hardware-orange-pi-zero-2w")]
                {
                    "UAC2"
                }
                #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
                {
                    "USB"
                }
            }
        }
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
pub(crate) fn orange_audio_sinks(
    audio_out: UsbAudioOut,
) -> Result<Vec<AudioSink>, super::OrangeAudioInitError> {
    match audio_out {
        UsbAudioOut::Jack => Ok(vec![AudioSink::InternalDac]),
        UsbAudioOut::Both => Ok(vec![AudioSink::InternalDac, AudioSink::Usb]),
        UsbAudioOut::Usb => Err(super::OrangeAudioInitError::UsbUnavailable),
    }
}

#[cfg(all(test, not(feature = "hardware-orange-pi-zero-2w")))]
pub(crate) fn audio_sinks(audio_out: UsbAudioOut) -> Vec<AudioSink> {
    match audio_out {
        UsbAudioOut::Jack => vec![AudioSink::Jack],
        UsbAudioOut::Usb => vec![AudioSink::Usb],
        UsbAudioOut::Both => vec![AudioSink::Jack, AudioSink::Usb],
    }
}
