use crate::orange_device_apply::{OrangeRunError, OrangeShutdownResolution};
use crate::orange_host_adapter::OrangeHostAdapter;
use crate::power_lifecycle::{
    PowerAction, PowerLifecycle, PowerLifecycleCallbacks, PowerLifecycleResult,
};
use crate::render_loop::RenderWorker;
use playback_runtime::PlaybackRuntime;

pub(crate) fn run_ordinary_power_lifecycle(
    playback: &PlaybackRuntime,
    host: &mut OrangeHostAdapter,
    render: &RenderWorker,
    action: PowerAction,
) -> PowerLifecycleResult {
    let mut callbacks = OrangePowerCallbacks {
        playback,
        host,
        render,
    };
    let mut lifecycle = PowerLifecycle::default();
    lifecycle.execute(action, &mut callbacks)
}

struct OrangePowerCallbacks<'a> {
    playback: &'a PlaybackRuntime,
    host: &'a mut OrangeHostAdapter,
    render: &'a RenderWorker,
}

impl PowerLifecycleCallbacks for OrangePowerCallbacks<'_> {
    fn save_recovery(&mut self) -> Result<(), String> {
        self.host.save_recovery_for_power()
    }

    fn panic_external_midi(&mut self) -> Result<(), String> {
        playback_runtime::HostAdapter::panic_external_midi(self.host)
            .map_err(|error| error.to_string())
    }

    fn silence_internal_audio(&mut self) -> Result<(), String> {
        playback_runtime::HostAdapter::silence_internal_audio(self.host)
            .map_err(|error| error.to_string())
    }

    fn acknowledge_terminal(&mut self, _action: PowerAction) -> Result<(), String> {
        let snapshot = self
            .playback
            .last_snapshot()
            .cloned()
            .ok_or_else(|| "Orange power request has no latest native snapshot".to_string())?;
        let oled = self.host.oled_publication_for_snapshot(&snapshot, false)?;
        self.render.publish_terminal_preserving(snapshot, oled)
    }

    fn submit_power(&mut self, action: PowerAction) -> Result<(), String> {
        let outcome = match action {
            PowerAction::Reboot => crate::orange_reboot::request_reboot(),
            PowerAction::Shutdown => crate::orange_reboot::request_shutdown(),
        };
        match outcome {
            crate::orange_reboot::OrangePowerRequestOutcome::Accepted => Ok(()),
            outcome => Err(format!("Orange power request outcome: {outcome:?}")),
        }
    }
}

pub(crate) fn teardown_render(
    _result: &Result<OrangeShutdownResolution, OrangeRunError>,
    render: &RenderWorker,
) -> Result<(), String> {
    if render.is_terminated() {
        return Ok(());
    }
    render.publish_shutdown()
}

#[cfg(test)]
mod tests {
    use super::teardown_render;
    use crate::orange_device_apply::OrangeShutdownResolution;
    use crate::render_loop::RenderWorker;

    #[test]
    fn completed_terminal_render_is_not_torn_down_again() {
        let render = RenderWorker::terminated_for_test();
        assert!(render.is_terminated());
        assert_eq!(
            teardown_render(&Ok(OrangeShutdownResolution::Complete), &render),
            Ok(())
        );
    }
}
