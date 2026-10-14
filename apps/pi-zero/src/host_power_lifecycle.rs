//! The ordinary reboot/shutdown lifecycle on the shared host: save, silence,
//! show the terminal frame, then hand the board its power command.

use crate::host_adapter::PiHostAdapter;
use crate::power_lifecycle::{
    PowerAction, PowerLifecycle, PowerLifecycleCallbacks, PowerLifecycleResult,
};
use crate::render_loop::RenderWorker;
use playback_runtime::{HostAdapter, NativeRunner, PlaybackRuntime};

pub(crate) fn run_ordinary_power_lifecycle(
    playback: &PlaybackRuntime,
    runner: &mut NativeRunner,
    adapter: &mut PiHostAdapter,
    render_worker: &RenderWorker,
    action: PowerAction,
    submit_power: impl FnMut(PowerAction) -> Result<(), String>,
) -> PowerLifecycleResult {
    let mut callbacks = HostPowerCallbacks {
        playback,
        runner,
        adapter,
        render_worker,
        submit_power,
    };
    PowerLifecycle::default().execute(action, &mut callbacks)
}

/// Shows the current scene as the terminal frame before power goes away.
pub(crate) fn publish_terminal_frame(
    playback: &PlaybackRuntime,
    runner: &mut NativeRunner,
    render_worker: &RenderWorker,
) -> Result<(), String> {
    let scene = runner.capture_display_scene()?;
    let (metrics, error) = playback.native_presentation_state();
    render_worker.publish_terminal_preserving(scene, metrics, error)
}

struct HostPowerCallbacks<'a, F> {
    playback: &'a PlaybackRuntime,
    runner: &'a mut NativeRunner,
    adapter: &'a mut PiHostAdapter,
    render_worker: &'a RenderWorker,
    submit_power: F,
}

impl<F: FnMut(PowerAction) -> Result<(), String>> PowerLifecycleCallbacks
    for HostPowerCallbacks<'_, F>
{
    fn save_recovery(&mut self) -> Result<(), String> {
        self.adapter.save_recovery_for_power()
    }

    fn panic_external_midi(&mut self) -> Result<(), String> {
        HostAdapter::panic_external_midi(self.adapter).map_err(|error| error.to_string())
    }

    fn silence_internal_audio(&mut self) -> Result<(), String> {
        HostAdapter::silence_internal_audio(self.adapter).map_err(|error| error.to_string())
    }

    fn acknowledge_terminal(&mut self, _action: PowerAction) -> Result<(), String> {
        publish_terminal_frame(self.playback, self.runner, self.render_worker)
    }

    fn submit_power(&mut self, action: PowerAction) -> Result<(), String> {
        (self.submit_power)(action)
    }
}
