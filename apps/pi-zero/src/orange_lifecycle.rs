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
    fn atomic_power_terminal_completes_before_helper_without_double_teardown() {
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
        let helper_events = std::sync::Arc::clone(&events);
        crate::orange_device_apply::finish_shutdown_resolution_with_helper(resolution, move || {
            assert!(render.is_terminated());
            helper_events.lock().unwrap().push("helper");
            crate::orange_device_apply::OrangeHelperOutcome::Accepted
        })
        .unwrap();
        assert_eq!(*events.lock().unwrap(), ["terminal", "helper"]);
    }

    #[test]
    #[cfg(not(feature = "hardware-raspberry-pi-zero-2w"))]
    fn failed_atomic_terminal_is_not_torn_down_again() {
        use crate::oled_frame_cache::OledFramePublication;
        use crate::render::HardwareRenderTargets;
        use octessera_hal::OledSsd1351;
        use playback_runtime::oled_frame::OLED_FRAME_BYTES;
        use serde_json::json;
        use std::sync::mpsc;

        let (seesaw_tx, seesaw_rx) = mpsc::channel();
        drop(seesaw_rx);
        let render = RenderWorker::spawn(HardwareRenderTargets {
            oled: {
                #[cfg(feature = "hardware-orange-pi-zero-2w")]
                {
                    OledSsd1351
                }
                #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
                {
                    OledSsd1351::new().unwrap()
                }
            },
            seesaw_tx,
            oled_handoff: None,
            #[cfg(not(feature = "hardware-orange-pi-zero-2w"))]
            hdmi: None,
        });
        let terminal_result = render.publish_terminal_preserving(
            json!({
                "display": { "off": false },
                "oledFrameRevision": 5
            }),
            OledFramePublication::test_native(5, vec![0; OLED_FRAME_BYTES]),
        );
        assert!(terminal_result.is_err());
        assert!(render.is_terminated());
        let original = Err(OrangeRunError::Ordinary("atomic terminal failed".into()));
        assert_eq!(teardown_render(&original, &render), Ok(()));
    }
}
