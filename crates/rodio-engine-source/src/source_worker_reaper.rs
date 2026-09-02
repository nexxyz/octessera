use crate::retired_audio_backlog::RetiredAudioBacklog;
use crate::source_worker::EngineSourceWorkerShutdownOwner;
use crate::{drop_retired_item, RetiredAudioItem};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError, TrySendError};
use realtime_engine::synth::{
    SourceWorkerLifecycle, SourceWorkerRetirement, SourceWorkerSetupError, SourceWorkerShutdown,
};
#[cfg(test)]
use std::cell::RefCell;
use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;
use std::thread;

use std::sync::atomic::AtomicBool;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static ACTIVE_REAPERS: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct SourceShutdownEnvelope {
    pub(crate) backlog: RetiredAudioBacklog,
    pub(crate) retirement: Option<SourceWorkerRetirement>,
}

pub const SOURCE_REAPER_THREAD_NAME: &str = "oct-src-reaper";

pub(crate) struct PersistentReaperSpawnFailure {
    pub(crate) lifecycle: SourceWorkerLifecycle,
    pub(crate) error: SourceWorkerSetupError,
}

pub(crate) fn spawn_inline_reaper() -> (Sender<RetiredAudioItem>, Sender<SourceShutdownEnvelope>) {
    let (retired_tx, retired_rx) = bounded(super::RETIREMENT_QUEUE_CAPACITY);
    let (shutdown_tx, shutdown_rx) = bounded(1);
    thread::spawn(move || run_inline_reaper(shutdown_rx, retired_rx));
    (retired_tx, shutdown_tx)
}

pub(crate) fn spawn_persistent_reaper(
    lifecycle: SourceWorkerLifecycle,
    retired_rx: Receiver<RetiredAudioItem>,
    panic_before_envelope: bool,
) -> Result<
    (
        Sender<SourceShutdownEnvelope>,
        EngineSourceWorkerShutdownOwner,
    ),
    Box<PersistentReaperSpawnFailure>,
> {
    spawn_persistent_reaper_with_options(lifecycle, retired_rx, panic_before_envelope, None)
}

#[cfg(test)]
pub(crate) fn spawn_persistent_reaper_for_test(
    lifecycle: SourceWorkerLifecycle,
    retired_rx: Receiver<RetiredAudioItem>,
    panic_before_envelope: bool,
) -> Result<
    (
        Sender<SourceShutdownEnvelope>,
        EngineSourceWorkerShutdownOwner,
        Arc<AtomicBool>,
    ),
    Box<PersistentReaperSpawnFailure>,
> {
    let hold_retired = Arc::new(AtomicBool::new(true));
    let (shutdown_tx, owner) = spawn_persistent_reaper_with_options(
        lifecycle,
        retired_rx,
        panic_before_envelope,
        Some(Arc::clone(&hold_retired)),
    )?;
    Ok((shutdown_tx, owner, hold_retired))
}

fn spawn_persistent_reaper_with_options(
    lifecycle: SourceWorkerLifecycle,
    retired_rx: Receiver<RetiredAudioItem>,
    panic_before_envelope: bool,
    hold_retired: Option<Arc<AtomicBool>>,
) -> Result<
    (
        Sender<SourceShutdownEnvelope>,
        EngineSourceWorkerShutdownOwner,
    ),
    Box<PersistentReaperSpawnFailure>,
> {
    #[cfg(not(test))]
    let _ = hold_retired;
    #[cfg(not(test))]
    let _ = panic_before_envelope;
    let (shutdown_tx, shutdown_rx) = bounded(1);
    let (completion_tx, completion_rx) = bounded(1);
    let state = PersistentReaper {
        lifecycle,
        shutdown_rx,
        retired_rx,
        completion_tx,
        #[cfg(test)]
        panic_before_envelope,
        #[cfg(test)]
        hold_retired,
    };
    let state = Arc::new(state);
    let thread_state = Arc::clone(&state);
    let reaper = spawn_persistent_reaper_thread(move || {
        while Arc::strong_count(&thread_state) > 1 {
            thread::yield_now();
        }
        let state = Arc::try_unwrap(thread_state)
            .unwrap_or_else(|_| unreachable!("source reaper state ownership"));
        run_persistent_reaper(state);
    });
    match reaper {
        Ok(reaper) => {
            drop(state);
            Ok((
                shutdown_tx,
                EngineSourceWorkerShutdownOwner::new(completion_rx, reaper),
            ))
        }
        Err(_) => {
            let PersistentReaper { lifecycle, .. } = Arc::try_unwrap(state)
                .unwrap_or_else(|_| unreachable!("source reaper state ownership on spawn failure"));
            Err(Box::new(PersistentReaperSpawnFailure {
                lifecycle,
                error: SourceWorkerSetupError::RetirementReaperUnavailable,
            }))
        }
    }
}

fn spawn_persistent_reaper_thread<F>(run: F) -> io::Result<thread::JoinHandle<()>>
where
    F: FnOnce() + Send + 'static,
{
    #[cfg(test)]
    if let Some(attempts) = FAIL_NEXT_REAPER_SPAWN.with(|failure| failure.borrow_mut().take()) {
        attempts.fetch_add(1, Ordering::AcqRel);
        return Err(io::Error::other("injected source reaper spawn failure"));
    }
    thread::Builder::new()
        .name(SOURCE_REAPER_THREAD_NAME.into())
        .spawn(run)
}

#[cfg(test)]
thread_local! {
    static FAIL_NEXT_REAPER_SPAWN: RefCell<Option<Arc<AtomicUsize>>> =
        const { RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct ReaperSpawnFailureGuard {
    attempts: Arc<AtomicUsize>,
}

#[cfg(test)]
pub(crate) fn fail_next_reaper_spawn_for_test() -> ReaperSpawnFailureGuard {
    let attempts = Arc::new(AtomicUsize::new(0));
    FAIL_NEXT_REAPER_SPAWN.with(|failure| failure.replace(Some(Arc::clone(&attempts))));
    ReaperSpawnFailureGuard { attempts }
}

#[cfg(test)]
impl Drop for ReaperSpawnFailureGuard {
    fn drop(&mut self) {
        FAIL_NEXT_REAPER_SPAWN.with(|failure| failure.replace(None));
    }
}

#[cfg(test)]
impl ReaperSpawnFailureGuard {
    pub(crate) fn attempts_for_test(&self) -> usize {
        self.attempts.load(Ordering::Acquire)
    }
}

pub(crate) fn abort_failed_shutdown_handoff(error: TrySendError<SourceShutdownEnvelope>) -> ! {
    match error {
        TrySendError::Full(_) | TrySendError::Disconnected(_) => std::process::abort(),
    }
}

struct PersistentReaper {
    lifecycle: SourceWorkerLifecycle,
    shutdown_rx: Receiver<SourceShutdownEnvelope>,
    retired_rx: Receiver<RetiredAudioItem>,
    completion_tx: Sender<SourceWorkerShutdown>,
    #[cfg(test)]
    panic_before_envelope: bool,
    #[cfg(test)]
    hold_retired: Option<Arc<AtomicBool>>,
}

fn run_persistent_reaper(mut state: PersistentReaper) {
    #[cfg(test)]
    ACTIVE_REAPERS.fetch_add(1, Ordering::AcqRel);
    let envelope = match catch_unwind(AssertUnwindSafe(|| state.wait_for_envelope())) {
        Ok(envelope) => envelope,
        Err(_) => state.wait_for_envelope_after_panic(),
    };
    let PersistentReaper {
        lifecycle,
        shutdown_rx: _,
        retired_rx,
        completion_tx,
        ..
    } = state;
    let (backlog, retirement) = match envelope {
        Some(envelope) => (Some(envelope.backlog), envelope.retirement),
        None => (None, None),
    };
    drain_retired_audio(retired_rx);
    if let Some(backlog) = backlog {
        backlog.drain();
    }
    let shutdown = match retirement {
        Some(retirement) => match lifecycle.validate_retirement(&retirement) {
            Ok(()) => lifecycle.shutdown(retirement),
            Err(error) => lifecycle.shutdown_after_retirement_error(error),
        },
        None => lifecycle.shutdown_after_runtime_drop(),
    };
    let _ = completion_tx.send(shutdown);
    #[cfg(test)]
    ACTIVE_REAPERS.fetch_sub(1, Ordering::AcqRel);
}

#[cfg(test)]
pub(crate) fn active_reapers_for_test() -> usize {
    ACTIVE_REAPERS.load(Ordering::Acquire)
}

impl PersistentReaper {
    fn wait_for_envelope(&mut self) -> Option<SourceShutdownEnvelope> {
        #[cfg(test)]
        if self.panic_before_envelope {
            self.panic_before_envelope = false;
            panic!("source reaper test panic");
        }
        self.wait_for_envelope_after_panic()
    }

    fn wait_for_envelope_after_panic(&self) -> Option<SourceShutdownEnvelope> {
        #[cfg(test)]
        if let Some(hold_retired) = &self.hold_retired {
            while hold_retired.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            return self.shutdown_rx.recv().ok();
        }
        let mut retired_open = true;
        loop {
            match self.shutdown_rx.try_recv() {
                Ok(envelope) => return Some(envelope),
                Err(TryRecvError::Disconnected) => return None,
                Err(TryRecvError::Empty) => {}
            }
            if !retired_open {
                return self.shutdown_rx.recv().ok();
            }
            crossbeam_channel::select_biased! {
                recv(&self.shutdown_rx) -> envelope => return envelope.ok(),
                recv(&self.retired_rx) -> retired => {
                    if let Ok(item) = retired {
                        drop_retired_item(item);
                    } else {
                        retired_open = false;
                    }
                }
            }
        }
    }
}

fn run_inline_reaper(
    shutdown_rx: Receiver<SourceShutdownEnvelope>,
    retired_rx: Receiver<RetiredAudioItem>,
) {
    let envelope = wait_for_inline_envelope(&shutdown_rx, &retired_rx);
    if let Some(envelope) = envelope {
        drain_retired_audio(retired_rx);
        envelope.backlog.drain();
    } else {
        drain_retired_audio(retired_rx);
    }
}

#[cfg(test)]
pub(crate) fn spawn_inline_reaper_for_test() -> (
    Sender<RetiredAudioItem>,
    Sender<SourceShutdownEnvelope>,
    Arc<AtomicBool>,
    thread::JoinHandle<()>,
) {
    let (retired_tx, retired_rx) = bounded(super::RETIREMENT_QUEUE_CAPACITY);
    let (shutdown_tx, shutdown_rx) = bounded(1);
    let hold_retired = Arc::new(AtomicBool::new(true));
    let thread_hold = Arc::clone(&hold_retired);
    let reaper = thread::Builder::new()
        .name("octessera-inline-reaper-test".into())
        .spawn(move || {
            while thread_hold.load(Ordering::Acquire) {
                std::hint::spin_loop();
            }
            run_inline_reaper(shutdown_rx, retired_rx);
        })
        .expect("inline test reaper");
    (retired_tx, shutdown_tx, hold_retired, reaper)
}

fn wait_for_inline_envelope(
    shutdown_rx: &Receiver<SourceShutdownEnvelope>,
    retired_rx: &Receiver<RetiredAudioItem>,
) -> Option<SourceShutdownEnvelope> {
    let mut retired_open = true;
    loop {
        match shutdown_rx.try_recv() {
            Ok(envelope) => return Some(envelope),
            Err(TryRecvError::Disconnected) => return None,
            Err(TryRecvError::Empty) => {}
        }
        if !retired_open {
            return shutdown_rx.recv().ok();
        }
        crossbeam_channel::select_biased! {
            recv(shutdown_rx) -> envelope => return envelope.ok(),
            recv(retired_rx) -> retired => {
                if let Ok(item) = retired {
                    drop_retired_item(item);
                } else {
                    retired_open = false;
                }
            }
        }
    }
}

fn drain_retired_audio(retired_rx: Receiver<RetiredAudioItem>) {
    while let Ok(item) = retired_rx.recv() {
        drop_retired_item(item);
    }
}

#[cfg(test)]
mod tests {
    use super::{spawn_persistent_reaper_thread, SOURCE_REAPER_THREAD_NAME};

    #[test]
    fn reaper_thread_name_is_linux_visible_and_bounded() {
        assert!(SOURCE_REAPER_THREAD_NAME.len() <= 15);
        let reaper = spawn_persistent_reaper_thread(|| {}).unwrap();
        assert_eq!(reaper.thread().name(), Some(SOURCE_REAPER_THREAD_NAME));
        reaper.join().unwrap();
    }
}
