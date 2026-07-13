use super::*;

pub(super) const PROFILE_SAMPLE_VOICES: usize = 0;
pub(super) const PROFILE_PREVIEW_SAMPLE_VOICES: usize = 1;
pub(super) const PROFILE_SYNTH_VOICES: usize = 2;
pub(super) const PROFILE_PREPARE_MIX_SLOTS: usize = 3;
pub(super) const PROFILE_FX_BUSES: usize = 4;
pub(super) const PROFILE_DRY_HISTORY: usize = 5;
pub(super) const PROFILE_MASTER_GLOBAL_FX: usize = 6;
pub(super) const PROFILE_CLOCK_VOLUME_CLAMP: usize = 7;

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct RenderProfileState {
    pub(super) enabled: bool,
    pub(super) frames_observed: u64,
    pub(super) blocks_observed: u64,
    pub(super) last_block_frames: usize,
    pub(super) last_frame_total_ns: u64,
    pub(super) last_block_total_ns: u64,
    pub(super) stage_ns: [u64; RENDER_PROFILE_STAGE_COUNT],
    pub(super) interleave_ns: u64,
}

impl RenderProfileState {
    pub(super) fn snapshot(&self) -> RenderProfileSnapshot {
        RenderProfileSnapshot {
            enabled: self.enabled,
            frames_observed: self.frames_observed,
            blocks_observed: self.blocks_observed,
            last_block_frames: self.last_block_frames,
            last_frame_total_ns: self.last_frame_total_ns,
            last_block_total_ns: self.last_block_total_ns,
            stage_ns: self.stage_ns,
            interleave_ns: self.interleave_ns,
        }
    }
}

impl SynthEngine {
    pub fn set_render_profile_enabled(&mut self, enabled: bool) {
        self.render_profile = RenderProfileState {
            enabled,
            ..RenderProfileState::default()
        };
    }

    pub fn render_profile_snapshot(&self) -> RenderProfileSnapshot {
        self.render_profile.snapshot()
    }
}
