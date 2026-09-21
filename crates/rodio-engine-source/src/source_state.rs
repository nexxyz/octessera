use crate::retired_audio_backlog::RetiredAudioBacklog;
use crate::source_worker_reaper::SourceShutdownEnvelope;
use crate::{
    AudioLoadStatusSender, EngineEventReceiver, PcmMirrorProducers, RetiredAudioItem, SynthEngine,
};
use crossbeam_channel::{Sender, TrySendError};
use realtime_engine::synth::RetiredAudioState;
#[cfg(any(test, feature = "routing-tree-executor"))]
use realtime_engine::synth::{SourceWorkerRetirement, SourceWorkerRuntime};

pub(crate) struct SourceOwned<T>(Option<T>);

impl<T> SourceOwned<T> {
    pub(crate) fn new(value: T) -> Self {
        Self(Some(value))
    }

    pub(crate) fn take(&mut self) -> T {
        self.0.take().expect("source-owned state")
    }
}

impl<T> std::ops::Deref for SourceOwned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("source-owned state")
    }
}

impl<T> std::ops::DerefMut for SourceOwned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("source-owned state")
    }
}

pub(crate) struct EngineSlot(Option<Box<SynthEngine>>);

impl EngineSlot {
    pub(crate) fn new(engine: Box<SynthEngine>) -> Self {
        Self(Some(engine))
    }

    pub(crate) fn take(&mut self) -> Box<SynthEngine> {
        self.0.take().expect("source-owned engine")
    }
}

impl std::ops::Deref for EngineSlot {
    type Target = SynthEngine;

    fn deref(&self) -> &Self::Target {
        self.0.as_deref().expect("source-owned engine")
    }
}

impl std::ops::DerefMut for EngineSlot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_deref_mut().expect("source-owned engine")
    }
}

pub(crate) struct SourceOwnedDropState {
    pub(crate) engine: Box<SynthEngine>,
    pub(crate) control_rx: EngineEventReceiver,
    pub(crate) buf: Vec<f32>,
    pub(crate) left_buf: Vec<f32>,
    pub(crate) right_buf: Vec<f32>,
    pub(crate) load_tx: Option<AudioLoadStatusSender>,
    pub(crate) retired_tx: Option<Sender<RetiredAudioItem>>,
    pub(crate) retired_backlog: Option<RetiredAudioBacklog>,
    pub(crate) emergency_retirement: Option<RetiredAudioItem>,
    pub(crate) mirror_producers: PcmMirrorProducers,
    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(crate) worker_runtime: Option<SourceWorkerRuntime>,
    #[cfg(test)]
    pub(crate) drop_probe: Option<SourceOwnedDropProbe>,
}

impl Drop for SourceOwnedDropState {
    fn drop(&mut self) {
        let _ = (
            &self.engine,
            &self.control_rx,
            &self.buf,
            &self.left_buf,
            &self.right_buf,
            &self.load_tx,
            &self.retired_tx,
            &self.retired_backlog,
            &self.emergency_retirement,
            &self.mirror_producers,
            #[cfg(any(test, feature = "routing-tree-executor"))]
            &self.worker_runtime,
            #[cfg(test)]
            &self.drop_probe,
        );
    }
}

impl SourceOwnedDropState {
    pub(crate) fn close_retirement_channel(&mut self) {
        self.retired_tx.take();
    }

    #[cfg(any(test, feature = "routing-tree-executor"))]
    pub(crate) fn retire_worker_runtime(&mut self) -> Option<SourceWorkerRetirement> {
        self.worker_runtime.take().map(SourceWorkerRuntime::retire)
    }
}

pub(crate) struct SourceRetirementChannels {
    pub(crate) retired_tx: Sender<RetiredAudioItem>,
    pub(crate) shutdown_tx: Sender<SourceShutdownEnvelope>,
}

#[cfg(test)]
pub(crate) struct RetiredAudioDropProbe {
    pub(crate) drop_tx: std::sync::mpsc::Sender<std::thread::ThreadId>,
}

#[cfg(test)]
impl Drop for RetiredAudioDropProbe {
    fn drop(&mut self) {
        let _ = self.drop_tx.send(std::thread::current().id());
    }
}

#[cfg(test)]
pub(crate) struct SourceOwnedDropProbe {
    pub(crate) drop_tx: std::sync::mpsc::Sender<std::thread::ThreadId>,
}

#[cfg(test)]
impl Drop for SourceOwnedDropProbe {
    fn drop(&mut self) {
        let _ = self.drop_tx.send(std::thread::current().id());
    }
}

impl crate::EngineSource {
    pub(crate) fn retire_state(&mut self, state: RetiredAudioState) {
        if state.is_empty() {
            return;
        }
        self.retire_item(RetiredAudioItem {
            state: Some(state),
            event: None,
            #[cfg(test)]
            drop_probe: None,
        });
    }

    fn retire_item(&mut self, item: RetiredAudioItem) {
        #[cfg(test)]
        let mut item = item;
        #[cfg(test)]
        {
            item.drop_probe =
                self.retired_drop_probe
                    .as_ref()
                    .map(|drop_tx| RetiredAudioDropProbe {
                        drop_tx: drop_tx.clone(),
                    });
        }
        if self.retired_backlog.is_none() {
            return;
        }
        self.flush_retirement_storage();
        if self.retirement_disconnected {
            self.hold_retirement(item);
            return;
        }
        match self.retired_tx.try_send(item) {
            Ok(()) => {}
            Err(TrySendError::Full(item)) => self.hold_retirement(item),
            Err(TrySendError::Disconnected(item)) => {
                self.retirement_disconnected = true;
                self.hold_retirement(item);
            }
        }
    }

    pub(crate) fn retirement_storage_can_accept_item(&mut self) -> bool {
        if self.retired_backlog.is_none() {
            return false;
        }
        self.flush_retirement_storage();
        self.retired_backlog
            .as_ref()
            .is_some_and(|backlog| backlog.len < crate::RETIREMENT_BACKLOG_CAPACITY)
    }

    fn flush_retirement_storage(&mut self) {
        let Some(backlog) = self.retired_backlog.as_mut() else {
            return;
        };
        backlog.flush(&self.retired_tx, &mut self.retirement_disconnected);
        if self.emergency_retirement.is_some() && backlog.len < crate::RETIREMENT_BACKLOG_CAPACITY {
            let item = self
                .emergency_retirement
                .take()
                .expect("emergency retirement slot");
            let _ = backlog.enqueue(item);
            backlog.flush(&self.retired_tx, &mut self.retirement_disconnected);
        }
    }

    fn hold_retirement(&mut self, item: RetiredAudioItem) {
        if let Some(backlog) = self.retired_backlog.as_mut() {
            if backlog.len < crate::RETIREMENT_BACKLOG_CAPACITY {
                debug_assert!(backlog.enqueue(item));
                return;
            }
        }
        if self.emergency_retirement.is_none() {
            self.emergency_retirement = Some(item);
        }
    }
}

pub(crate) fn drop_retired_item(item: RetiredAudioItem) {
    drop(item.state);
    drop(item.event);
    #[cfg(test)]
    drop(item.drop_probe);
}
