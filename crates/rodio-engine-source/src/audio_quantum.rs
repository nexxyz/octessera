use super::{MAX_BLOCK_FRAMES, MIN_BLOCK_FRAMES};

pub(super) fn audio_render_quantum_frames(default_frames: usize) -> usize {
    resolve_audio_render_quantum_frames(
        std::env::var("OCTESSERA_AUDIO_RENDER_QUANTUM_FRAMES")
            .ok()
            .as_deref(),
        default_frames,
    )
}

pub(super) fn resolve_audio_render_quantum_frames(
    env_value: Option<&str>,
    default_frames: usize,
) -> usize {
    env_value
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(default_frames)
        .clamp(MIN_BLOCK_FRAMES, MAX_BLOCK_FRAMES)
}
