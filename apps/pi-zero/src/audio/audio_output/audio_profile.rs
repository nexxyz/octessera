use playback_runtime::AudioOptimization;
use std::time::Duration;

/// Lead time between a pulse and its sequenced output: the smallest delay that logged no late
/// timed events in 2-minute board sweeps (2026-10-29) plus about 1 ms for rarer stragglers.
pub(crate) fn sequenced_output_delay(optimization: AudioOptimization) -> Duration {
    #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
    let micros = match optimization {
        AudioOptimization::Latency => 8_800,
        AudioOptimization::Capacity => 17_000,
    };
    #[cfg(feature = "hardware-orange-pi-zero-2w")]
    let micros = match optimization {
        AudioOptimization::Latency => 4_500,
        AudioOptimization::Capacity => 11_000,
    };
    Duration::from_micros(micros)
}

#[cfg(all(not(feature = "hardware-orange-pi-zero-2w"), feature = "profiling"))]
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

    #[cfg(any(test, feature = "profiling"))]
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

    #[cfg(feature = "profiling")]
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
