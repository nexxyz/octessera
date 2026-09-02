use realtime_engine::synth::SourceWorkerRetirementError;
use rodio_engine_source::{EngineSourceWorkerShutdownError, EngineSourceWorkerShutdownOwner};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioStreamRetirementError {
    CallbackSourceUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AudioStreamShutdownError {
    WorkerStatus {
        joined_workers: usize,
        retirement_error: Option<SourceWorkerRetirementError>,
    },
    Retirement(AudioStreamRetirementError),
    ReaperCompletionUnavailable,
    ReaperThreadPanicked,
}

pub(crate) type AudioStreamRetirementWaiter =
    Box<dyn FnOnce() -> Result<(), AudioStreamRetirementError> + Send>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct AudioStreamShutdownReport {
    pub(crate) joined_workers: usize,
    pub(crate) retirement_error: Option<SourceWorkerRetirementError>,
}

pub(crate) trait AudioStreamShutdownOwner {
    fn shutdown(self) -> Result<AudioStreamShutdownReport, AudioStreamShutdownError>;
}

impl AudioStreamShutdownOwner for EngineSourceWorkerShutdownOwner {
    fn shutdown(self) -> Result<AudioStreamShutdownReport, AudioStreamShutdownError> {
        let result =
            EngineSourceWorkerShutdownOwner::try_shutdown(self).map_err(|error| match error {
                EngineSourceWorkerShutdownError::ReaperCompletionUnavailable => {
                    AudioStreamShutdownError::ReaperCompletionUnavailable
                }
                EngineSourceWorkerShutdownError::ReaperThreadPanicked => {
                    AudioStreamShutdownError::ReaperThreadPanicked
                }
            })?;
        if result.joined_workers == 2 && result.retirement_error.is_none() {
            Ok(AudioStreamShutdownReport {
                joined_workers: result.joined_workers,
                retirement_error: result.retirement_error,
            })
        } else {
            Err(AudioStreamShutdownError::WorkerStatus {
                joined_workers: result.joined_workers,
                retirement_error: result.retirement_error,
            })
        }
    }
}

#[derive(Debug)]
pub(crate) enum AudioStreamBuildError<E> {
    Stream(E),
    Shutdown(AudioStreamShutdownError),
}

pub(crate) trait PlayableAudioStream {
    type Error;

    fn play(&self) -> Result<(), Self::Error>;
}

pub(crate) struct AudioStreamLifecycle<S, O: AudioStreamShutdownOwner> {
    stream: Option<S>,
    shutdown_owner: Option<O>,
    retirement_waiter: Option<AudioStreamRetirementWaiter>,
}

impl<S, O: AudioStreamShutdownOwner> AudioStreamLifecycle<S, O> {
    pub(crate) fn new(
        stream: S,
        shutdown_owner: Option<O>,
        retirement_waiter: Option<AudioStreamRetirementWaiter>,
    ) -> Self {
        Self {
            stream: Some(stream),
            shutdown_owner,
            retirement_waiter,
        }
    }

    pub(crate) fn from_build_result<E>(
        result: Result<S, E>,
        shutdown_owner: Option<O>,
        retirement_waiter: Option<AudioStreamRetirementWaiter>,
    ) -> Result<Self, AudioStreamBuildError<E>> {
        match result {
            Ok(stream) => Ok(Self::new(stream, shutdown_owner, retirement_waiter)),
            Err(error) => {
                let retirement_result = retirement_waiter.map(|waiter| waiter());
                let retirement_error = retirement_result.and_then(Result::err);
                if let Some(owner) = shutdown_owner {
                    if let Err(status) = owner.shutdown() {
                        return Err(AudioStreamBuildError::Shutdown(status));
                    }
                }
                if let Some(error) = retirement_error {
                    return Err(AudioStreamBuildError::Shutdown(
                        AudioStreamShutdownError::Retirement(error),
                    ));
                }
                Err(AudioStreamBuildError::Stream(error))
            }
        }
    }

    pub(crate) fn play(&self) -> Result<(), <S as PlayableAudioStream>::Error>
    where
        S: PlayableAudioStream,
    {
        self.stream
            .as_ref()
            .expect("audio stream lifecycle must contain a stream")
            .play()
    }

    pub(crate) fn teardown(
        mut self,
    ) -> Result<AudioStreamShutdownReport, AudioStreamShutdownError> {
        self.teardown_inner(true)
    }

    fn teardown_inner(
        &mut self,
        shutdown: bool,
    ) -> Result<AudioStreamShutdownReport, AudioStreamShutdownError> {
        drop(self.stream.take());
        if shutdown {
            let retirement_result = self.retirement_waiter.take().map(|waiter| waiter());
            if let Some(owner) = self.shutdown_owner.take() {
                let owner_result = owner.shutdown();
                if let Some(error) = retirement_result.and_then(Result::err) {
                    return Err(AudioStreamShutdownError::Retirement(error));
                }
                return owner_result;
            }
            if let Some(error) = retirement_result.and_then(Result::err) {
                return Err(AudioStreamShutdownError::Retirement(error));
            }
        }
        Ok(AudioStreamShutdownReport::default())
    }
}

impl<S, O: AudioStreamShutdownOwner> Drop for AudioStreamLifecycle<S, O> {
    fn drop(&mut self) {
        if let Err(status) = self.teardown_inner(!std::thread::panicking()) {
            eprintln!("audio stream teardown failed: {status:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AudioStreamBuildError, AudioStreamLifecycle, AudioStreamShutdownError,
        AudioStreamShutdownOwner, PlayableAudioStream,
    };
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::sync::{Arc, Mutex};

    struct ProbeStream {
        events: Arc<Mutex<Vec<&'static str>>>,
        plays: Result<(), &'static str>,
    }

    impl PlayableAudioStream for ProbeStream {
        type Error = &'static str;

        fn play(&self) -> Result<(), Self::Error> {
            self.plays
        }
    }

    impl Drop for ProbeStream {
        fn drop(&mut self) {
            self.events.lock().unwrap().push("stream drop");
        }
    }

    struct ProbeOwner {
        events: Arc<Mutex<Vec<&'static str>>>,
        shutdown: Result<(), AudioStreamShutdownError>,
    }

    impl AudioStreamShutdownOwner for ProbeOwner {
        fn shutdown(self) -> Result<super::AudioStreamShutdownReport, AudioStreamShutdownError> {
            self.events.lock().unwrap().push("owner shutdown");
            self.shutdown
                .map(|_| super::AudioStreamShutdownReport::default())
        }
    }

    impl Drop for ProbeOwner {
        fn drop(&mut self) {
            self.events.lock().unwrap().push("owner drop");
        }
    }

    fn retirement_waiter(
        events: &Arc<Mutex<Vec<&'static str>>>,
    ) -> Option<super::AudioStreamRetirementWaiter> {
        let events = events.clone();
        Some(Box::new(move || {
            events.lock().unwrap().push("source retired");
            Ok(())
        }))
    }

    fn failed_retirement_waiter() -> Option<super::AudioStreamRetirementWaiter> {
        Some(Box::new(|| {
            Err(super::AudioStreamRetirementError::CallbackSourceUnavailable)
        }))
    }

    fn lifecycle(
        events: &Arc<Mutex<Vec<&'static str>>>,
        plays: Result<(), &'static str>,
    ) -> AudioStreamLifecycle<ProbeStream, ProbeOwner> {
        AudioStreamLifecycle::new(
            ProbeStream {
                events: events.clone(),
                plays,
            },
            Some(ProbeOwner {
                events: events.clone(),
                shutdown: Ok(()),
            }),
            retirement_waiter(events),
        )
    }

    #[test]
    fn teardown_drops_stream_before_shutdown_owner() {
        let events = Arc::new(Mutex::new(Vec::new()));
        lifecycle(&events, Ok(())).teardown().unwrap();

        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "stream drop",
                "source retired",
                "owner shutdown",
                "owner drop"
            ]
        );
    }

    #[test]
    fn build_failure_shuts_down_owner_without_a_stream() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let result = AudioStreamLifecycle::<ProbeStream, ProbeOwner>::from_build_result::<_>(
            Err("build"),
            Some(ProbeOwner {
                events: events.clone(),
                shutdown: Ok(()),
            }),
            retirement_waiter(&events),
        );

        assert!(matches!(
            result,
            Err(AudioStreamBuildError::Stream("build"))
        ));
        assert_eq!(
            *events.lock().unwrap(),
            vec!["source retired", "owner shutdown", "owner drop"]
        );
    }

    #[test]
    fn play_failure_uses_the_same_teardown_order() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let lifecycle = lifecycle(&events, Err("play"));

        assert_eq!(lifecycle.play(), Err("play"));
        lifecycle.teardown().unwrap();

        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "stream drop",
                "source retired",
                "owner shutdown",
                "owner drop"
            ]
        );
    }

    #[test]
    fn teardown_returns_invalid_worker_shutdown_status_without_panicking() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let status = AudioStreamShutdownError::WorkerStatus {
            joined_workers: 1,
            retirement_error: None,
        };
        let lifecycle = AudioStreamLifecycle::new(
            ProbeStream {
                events: events.clone(),
                plays: Ok(()),
            },
            Some(ProbeOwner {
                events: events.clone(),
                shutdown: Err(status),
            }),
            retirement_waiter(&events),
        );

        assert_eq!(lifecycle.teardown(), Err(status));
        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "stream drop",
                "source retired",
                "owner shutdown",
                "owner drop"
            ]
        );
    }

    #[test]
    fn teardown_returns_retirement_failure_after_owner_shutdown() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let lifecycle = AudioStreamLifecycle::new(
            ProbeStream {
                events: events.clone(),
                plays: Ok(()),
            },
            Some(ProbeOwner {
                events: events.clone(),
                shutdown: Ok(()),
            }),
            failed_retirement_waiter(),
        );

        assert_eq!(
            lifecycle.teardown(),
            Err(AudioStreamShutdownError::Retirement(
                super::AudioStreamRetirementError::CallbackSourceUnavailable
            ))
        );
        assert_eq!(
            *events.lock().unwrap(),
            vec!["stream drop", "owner shutdown", "owner drop"]
        );
    }

    #[test]
    fn replacement_tears_down_old_stream_before_new_stream() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let old = lifecycle(&events, Ok(()));
        let new = lifecycle(&events, Ok(()));
        old.teardown().unwrap();
        new.teardown().unwrap();

        assert_eq!(
            *events.lock().unwrap(),
            vec![
                "stream drop",
                "source retired",
                "owner shutdown",
                "owner drop",
                "stream drop",
                "source retired",
                "owner shutdown",
                "owner drop"
            ]
        );
    }

    #[test]
    fn unwind_drops_stream_and_detaches_owner_without_shutdown() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let panic_events = events.clone();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _lifecycle = lifecycle(&panic_events, Ok(()));
            panic!("test unwind");
        }));

        assert!(result.is_err());
        assert_eq!(*events.lock().unwrap(), vec!["stream drop", "owner drop"]);
    }
}
