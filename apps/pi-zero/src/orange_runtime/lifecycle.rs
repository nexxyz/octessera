use crate::power_lifecycle::PowerAction;
use crate::render_loop::RenderWorker;

pub(crate) fn submit_orange_power(action: PowerAction) -> Result<(), String> {
    let outcome = match action {
        PowerAction::Reboot => crate::orange_reboot::request_reboot(),
        PowerAction::Shutdown => crate::orange_reboot::request_shutdown(),
        PowerAction::RestartApp => {
            crate::power_lifecycle::submit_app_restart();
            return Ok(());
        }
    };
    match outcome {
        crate::orange_reboot::OrangePowerRequestOutcome::Accepted => Ok(()),
        outcome => Err(format!("Orange power request outcome: {outcome:?}")),
    }
}

pub(crate) fn teardown_render(
    _result: &Result<(), String>,
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
    use crate::render_loop::RenderWorker;

    #[test]
    fn completed_terminal_render_is_not_torn_down_again() {
        let render = RenderWorker::terminated_for_test();
        assert!(render.is_terminated());
        assert_eq!(teardown_render(&Ok(()), &render), Ok(()));
    }
}
