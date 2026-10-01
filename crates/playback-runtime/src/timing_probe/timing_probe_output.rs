use super::{ProbeHost, ProbeRunner};
use crate::{PlaybackRuntime, RuntimeDispatchInput, RuntimeIngest};

pub(super) fn process_probe_output(
    runtime: &mut PlaybackRuntime,
    runner: &mut ProbeRunner,
    host: &mut ProbeHost,
    output: RuntimeIngest,
) -> Result<(), String> {
    for follow_up in output.follow_ups {
        let nested =
            runtime.dispatch(RuntimeDispatchInput::HostMessage(follow_up), runner, host)?;
        process_probe_output(runtime, runner, host, nested)?;
    }
    Ok(())
}
