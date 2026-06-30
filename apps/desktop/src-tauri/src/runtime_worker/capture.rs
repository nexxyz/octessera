use playback_runtime::{CoreRunner, HostMessage, RunnerMessage};

pub(super) struct CapturingCoreRunner<'a, R> {
    pub(super) inner: &'a mut R,
    pub(super) captured: Vec<RunnerMessage>,
}

impl<R: CoreRunner> CoreRunner for CapturingCoreRunner<'_, R> {
    fn send(&mut self, message: HostMessage) -> Result<Vec<RunnerMessage>, String> {
        let responses = self.inner.send(message)?;
        for response in responses.iter().cloned() {
            if !matches!(response, RunnerMessage::AudioCommands { .. }) {
                self.captured.push(response);
            }
        }
        Ok(responses)
    }
}
