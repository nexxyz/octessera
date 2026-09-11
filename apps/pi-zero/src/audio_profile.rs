use playback_runtime::AudioOptimization;

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AudioProfileGeometry {
    pub(crate) output_buffer_frames: u32,
    pub(crate) internal_block_frames: usize,
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RaspberryAudioProfile {
    pub(super) optimization: AudioOptimization,
    pub(super) output_buffer_frames: u32,
    pub(super) expected_alsa_period_frames: u32,
    pub(super) internal_block_frames: usize,
    pub(super) lookahead_frames: usize,
}

#[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
impl RaspberryAudioProfile {
    pub(crate) fn from_optimization(optimization: AudioOptimization) -> Self {
        match optimization {
            AudioOptimization::Latency => Self {
                optimization,
                output_buffer_frames: 256,
                expected_alsa_period_frames: 64,
                internal_block_frames: 128,
                lookahead_frames: 0,
            },
            AudioOptimization::Capacity => Self {
                optimization,
                output_buffer_frames: 256,
                expected_alsa_period_frames: 64,
                internal_block_frames: 256,
                lookahead_frames: 256,
            },
        }
    }

    pub(crate) fn from_timing_probe(
        output_buffer_frames: Option<u32>,
        internal_block_frames: Option<usize>,
    ) -> Self {
        let mut profile = Self::from_optimization(AudioOptimization::Latency);
        if let Some(output_buffer_frames) = output_buffer_frames {
            profile.output_buffer_frames = output_buffer_frames.clamp(32, 2_048);
        }
        if let Some(internal_block_frames) = internal_block_frames {
            profile.internal_block_frames = internal_block_frames.clamp(32, 2_048);
        }
        profile
    }

    pub(crate) fn geometry(self) -> AudioProfileGeometry {
        AudioProfileGeometry {
            output_buffer_frames: self.output_buffer_frames,
            internal_block_frames: self.internal_block_frames,
        }
    }
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OrangeAudioProfile {
    pub(super) optimization: AudioOptimization,
    pub(super) output_buffer_frames: u32,
    pub(super) expected_alsa_period_frames: u32,
    pub(super) internal_block_frames: usize,
    pub(super) lookahead_frames: usize,
}

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl OrangeAudioProfile {
    pub(crate) fn from_optimization(optimization: AudioOptimization) -> Self {
        match optimization {
            AudioOptimization::Latency => Self {
                optimization,
                output_buffer_frames: 128,
                expected_alsa_period_frames: 32,
                internal_block_frames: 32,
                lookahead_frames: 0,
            },
            AudioOptimization::Capacity => Self {
                optimization,
                output_buffer_frames: 256,
                expected_alsa_period_frames: 64,
                internal_block_frames: 128,
                lookahead_frames: 128,
            },
        }
    }
}
