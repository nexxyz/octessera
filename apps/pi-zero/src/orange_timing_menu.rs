use super::dispatch;
use crate::autoaux_menu;
use crate::orange_host_adapter::OrangeHostAdapter;
use playback_runtime::{HostMessage, NativeRunner, PlaybackRuntime};

pub(super) fn navigate_to_cutoff(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    let mut send =
        |playback: &mut PlaybackRuntime, runner: &mut NativeRunner, message: HostMessage| {
            dispatch(playback, runner, host, message)
        };
    autoaux_menu::navigate_to_cutoff(playback, runner, &mut send, "Orange")
}

pub(super) fn enable_study_auto_save(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(), String> {
    let mut send =
        |playback: &mut PlaybackRuntime, runner: &mut NativeRunner, message: HostMessage| {
            dispatch(playback, runner, host, message)
        };
    autoaux_menu::enable_study_auto_save(playback, runner, &mut send, "Orange")
}

pub(super) fn preflight_aux_cutoff(
    playback: &mut PlaybackRuntime,
    runner: &mut NativeRunner,
    host: &mut OrangeHostAdapter,
) -> Result<(u16, [u16; 2]), String> {
    let mut send =
        |playback: &mut PlaybackRuntime, runner: &mut NativeRunner, message: HostMessage| {
            dispatch(playback, runner, host, message)
        };
    autoaux_menu::preflight_aux_cutoff(playback, runner, &mut send, "Orange")
}

pub(super) fn require_stopped_normal_menu(playback: &PlaybackRuntime) -> Result<(), String> {
    autoaux_menu::require_stopped_normal_menu(playback, "Orange")
}
