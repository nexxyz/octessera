use super::RuntimeWorker;
use std::sync::mpsc;

const MAX_PLATFORM_SERVICE_RESULTS_PER_LOOP: usize = 4;

impl RuntimeWorker {
    pub(super) fn poll_audio_prep_results(&mut self) {
        for _ in 0..MAX_PLATFORM_SERVICE_RESULTS_PER_LOOP {
            match self.audio_prep_result_rx.try_recv() {
                Ok(message) => {
                    let output = self.playback.dispatch(
                        playback_runtime::RuntimeDispatchInput::HostMessage(message),
                        &mut self.runner,
                        &mut self.adapter,
                    );
                    if let Err(error) = output.and_then(|output| self.emit_runtime_output(output)) {
                        self.handle_error(error);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }

    pub(super) fn poll_platform_service_results(&mut self) {
        for _ in 0..MAX_PLATFORM_SERVICE_RESULTS_PER_LOOP {
            match self.platform_service_result_rx.try_recv() {
                Ok(messages) => {
                    for message in messages {
                        let output = self.playback.dispatch(
                            playback_runtime::RuntimeDispatchInput::HostMessage(message),
                            &mut self.runner,
                            &mut self.adapter,
                        );
                        if let Err(error) =
                            output.and_then(|output| self.emit_runtime_output(output))
                        {
                            self.handle_error(error);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => break,
            }
        }
    }
}
