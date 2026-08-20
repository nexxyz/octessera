use crate::orange_device_apply::{OrangeRunError, OrangeShutdownResolution};
use crate::orange_host_adapter::OrangeHostAdapter;
use crate::render_loop::RenderWorker;
use playback_runtime::PlaybackRuntime;

pub(crate) fn publish_power_terminal(
    playback: &mut PlaybackRuntime,
    host: &mut OrangeHostAdapter,
    render: &RenderWorker,
) -> Result<(), String> {
    let snapshot = playback
        .last_snapshot()
        .cloned()
        .ok_or_else(|| "Orange power request has no latest native snapshot".to_string())?;
    let oled = host.oled_publication_for_snapshot(&snapshot, false)?;
    render.publish_terminal_preserving(snapshot, oled)
}

pub(crate) fn teardown_render(
    result: &Result<OrangeShutdownResolution, OrangeRunError>,
    render: &RenderWorker,
) -> Result<(), String> {
    if render.is_terminated() {
        return Ok(());
    }
    if preserves_terminal_frame(result) {
        Err("Orange power resolution has no completed terminal render command".into())
    } else {
        render.publish_shutdown()
    }
}

fn preserves_terminal_frame(result: &Result<OrangeShutdownResolution, OrangeRunError>) -> bool {
    matches!(result, Ok(OrangeShutdownResolution::Power { .. }))
}

#[cfg(test)]
mod tests {
    use super::{preserves_terminal_frame, teardown_render};
    use crate::orange_device_apply::{OrangePowerAction, OrangeRunError, OrangeShutdownResolution};
    use crate::render_loop::RenderWorker;

    #[test]
    fn only_power_resolutions_preserve_the_terminal_frame() {
        assert!(preserves_terminal_frame(&Ok(
            OrangeShutdownResolution::Power {
                action: OrangePowerAction::Reboot,
                safety_failure: None,
            },
        )));
        assert!(!preserves_terminal_frame(&Ok(
            OrangeShutdownResolution::Complete,
        )));
        assert!(!preserves_terminal_frame(&Err(OrangeRunError::Ordinary(
            "runtime failed".into(),
        ))));
    }

    #[test]
    fn atomic_power_terminal_completes_before_power_request_without_double_teardown() {
        let render = RenderWorker::terminated_for_test();
        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        assert!(render.is_terminated());
        events.lock().unwrap().push("terminal");
        let resolution = Ok(OrangeShutdownResolution::Power {
            action: OrangePowerAction::Reboot,
            safety_failure: None,
        });
        assert_eq!(teardown_render(&resolution, &render), Ok(()));
        let resolution = match resolution {
            Ok(resolution) => resolution,
            Err(_) => unreachable!(),
        };
        let power_request_events = std::sync::Arc::clone(&events);
        crate::orange_device_apply::finish_shutdown_resolution_with_power_request(
            resolution,
            move || {
                assert!(render.is_terminated());
                power_request_events.lock().unwrap().push("power-request");
                crate::orange_device_apply::OrangePowerRequestOutcome::Accepted
            },
        )
        .unwrap();
        assert_eq!(*events.lock().unwrap(), ["terminal", "power-request"]);
    }

    #[test]
    fn failed_atomic_terminal_is_not_torn_down_again() {
        let render = RenderWorker::terminated_for_test();
        assert!(render.is_terminated());
        let original = Err(OrangeRunError::Ordinary("atomic terminal failed".into()));
        assert_eq!(teardown_render(&original, &render), Ok(()));
    }
}
