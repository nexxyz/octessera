use playback_runtime::{CoreRunner, HostMessage, RunnerMessage};

pub(super) fn is_async_desktop_visible(message: &RunnerMessage) -> bool {
    matches!(
        message,
        RunnerMessage::Snapshot { .. } | RunnerMessage::UiPulse { .. }
    )
}

pub(super) struct CapturingCoreRunner<'a, R> {
    pub(super) inner: &'a mut R,
    pub(super) captured: Vec<RunnerMessage>,
}

impl<R: CoreRunner> CoreRunner for CapturingCoreRunner<'_, R> {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let responses = self.inner.send(message)?;
        for response in responses.iter() {
            if is_async_desktop_visible(response) {
                self.captured.push(response.clone());
            }
        }
        Ok(responses)
    }
}
