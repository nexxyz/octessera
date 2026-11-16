//! A live view's analysis: reads the newest tapped samples and turns them into the picture. On the
//! device it runs on a worker the runtime hands one job at a time and never waits on, so a slow
//! analysis only lowers the picture's update rate; the host can lower the worker's priority.

use super::scope::scope_visual;
use super::spectrum::SpectrumHistory;
use super::{LiveView, VisualKind, READ, STEP, TRIGGER_ROOM, WINDOW};
use crate::oled_frame::{OledSignalVisual, SIGNAL_WIDTH};
use realtime_engine::synth::SignalTap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, TryLockError};

static WORKER_START: OnceLock<fn()> = OnceLock::new();

/// Sets what the signal analysis worker runs first on its own thread, such as lowering its
/// scheduling priority.
pub fn set_signal_analysis_worker_start(start: fn()) {
    let _ = WORKER_START.set(start);
}

/// One analysis of a live run, `steps` live steps after the run's previous one.
#[derive(Clone, Copy)]
pub(super) struct AnalysisJob {
    pub(super) run: u64,
    pub(super) view: LiveView,
    /// The tap's sample count at the run's start; older samples are not this run's.
    pub(super) written: u64,
    pub(super) first: bool,
    pub(super) steps: usize,
}

type Finished = (u64, Arc<OledSignalVisual>);

#[derive(Clone)]
pub(super) enum Analyzer {
    /// Runs each job as it is submitted, so the picture of a step is ready at once.
    #[cfg(test)]
    Inline {
        analysis: Box<Analysis>,
        finished: Option<Finished>,
    },
    /// Hands each job to the worker thread, started by the first job; clones share it.
    Worker {
        tap: &'static SignalTap,
        worker: Option<Arc<WorkerHandle>>,
    },
}

impl Analyzer {
    #[cfg(test)]
    pub(super) fn inline(tap: &'static SignalTap) -> Self {
        Self::Inline {
            analysis: Box::new(Analysis::new(tap)),
            finished: None,
        }
    }

    pub(super) fn worker(tap: &'static SignalTap) -> Self {
        Self::Worker { tap, worker: None }
    }

    /// Starts `job` if no submitted job waits or runs; whether it started. The worker's lock is
    /// only tried, never waited for.
    pub(super) fn try_submit(&mut self, job: AnalysisJob) -> bool {
        match self {
            #[cfg(test)]
            Self::Inline { analysis, finished } => {
                *finished = Some((job.run, Arc::new(analysis.run(job))));
                true
            }
            Self::Worker { tap, worker } => {
                let worker = worker.get_or_insert_with(|| WorkerHandle::spawn(tap));
                let Some(mut slot) = worker.shared.try_lock() else {
                    return false;
                };
                if slot.job.is_some() || slot.busy {
                    return false;
                }
                slot.job = Some(job);
                drop(slot);
                worker.shared.wake.notify_one();
                true
            }
        }
    }

    /// The newest finished picture of `run`, once; pictures of earlier runs are dropped.
    pub(super) fn take(&mut self, run: u64) -> Option<Arc<OledSignalVisual>> {
        let finished = match self {
            #[cfg(test)]
            Self::Inline { finished, .. } => finished.take(),
            Self::Worker { worker, .. } => worker.as_ref()?.shared.try_lock()?.finished.take(),
        };
        finished.and_then(|(of, visual)| (of == run).then_some(visual))
    }
}

/// The worker thread's state for one tap: its samples and the spectrum history of the run.
#[derive(Clone)]
pub(super) struct Analysis {
    tap: &'static SignalTap,
    samples: Vec<f32>,
    spectrum: SpectrumHistory,
    /// The tap's count at the last read: a source that stopped writing reads as silence.
    last_written: u64,
}

impl Analysis {
    fn new(tap: &'static SignalTap) -> Self {
        Self {
            tap,
            samples: vec![0.0; READ],
            spectrum: SpectrumHistory::new(WINDOW),
            last_written: 0,
        }
    }

    fn run(&mut self, job: AnalysisJob) -> OledSignalVisual {
        if job.first {
            self.spectrum.clear();
            self.last_written = job.written;
        }
        let samples = &mut self.samples[READ - read_length(job.view)..];
        self.tap.read_latest(job.written, samples);
        let written = self.tap.written();
        if written == self.last_written {
            samples.fill(0.0);
        }
        self.last_written = written;
        let rate = self.tap.sample_rate();
        picture(job.view, samples, rate, job.steps, &mut self.spectrum)
    }
}

/// How many of the newest tapped samples one step of `view` reads.
fn read_length(view: LiveView) -> usize {
    match view.kind {
        VisualKind::Scope => (SIGNAL_WIDTH * view.tuning.scope_stride() + TRIGGER_ROOM).min(READ),
        VisualKind::Waterfall if view.tuning.waterfall_columns() == 2 => READ,
        VisualKind::Spectrum | VisualKind::Waterfall => WINDOW,
    }
}

/// The picture of `view` from the newest `samples` the tap took at `rate`, `steps` live steps
/// after the previous one: bars fall for every step, and a waterfall repeats its newest column
/// for the steps it missed, so its history keeps its length in time.
fn picture(
    view: LiveView,
    samples: &[f32],
    rate: u32,
    steps: usize,
    spectrum: &mut SpectrumHistory,
) -> OledSignalVisual {
    let end = samples.len();
    let newest = || &samples[end - WINDOW..];
    let columns = view.tuning.waterfall_columns();
    let missed = steps.saturating_sub(1) * columns;
    match view.kind {
        VisualKind::Scope => scope_visual(samples, view.tuning.scope_stride()),
        VisualKind::Spectrum => {
            spectrum.spectrum(newest(), rate, view.tuning.spectrum_bands(), steps)
        }
        VisualKind::Waterfall if columns == 2 => {
            let back = (rate as usize * STEP.as_millis() as usize / 2000).min(end - WINDOW);
            let older = &samples[end - WINDOW - back..end - back];
            spectrum.waterfall(&[older, newest()], rate, view.rows, missed)
        }
        VisualKind::Waterfall => spectrum.waterfall(&[newest()], rate, view.rows, missed),
    }
}

pub(super) struct WorkerHandle {
    shared: Arc<Shared>,
}

struct Shared {
    slot: Mutex<Slot>,
    wake: Condvar,
}

#[derive(Default)]
struct Slot {
    job: Option<AnalysisJob>,
    busy: bool,
    finished: Option<Finished>,
    stopped: bool,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, Slot> {
        self.slot
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn try_lock(&self) -> Option<MutexGuard<'_, Slot>> {
        match self.slot.try_lock() {
            Ok(slot) => Some(slot),
            Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
            Err(TryLockError::WouldBlock) => None,
        }
    }
}

impl WorkerHandle {
    fn spawn(tap: &'static SignalTap) -> Arc<Self> {
        let shared = Arc::new(Shared {
            slot: Mutex::new(Slot::default()),
            wake: Condvar::new(),
        });
        let thread_shared = Arc::clone(&shared);
        let spawned = std::thread::Builder::new()
            .name("octessera-signal".into())
            .spawn(move || work(&thread_shared, Analysis::new(tap)));
        if let Err(error) = spawned {
            eprintln!("signal analysis worker failed to start: {error}");
            shared.lock().stopped = true;
        }
        Arc::new(Self { shared })
    }
}

impl Drop for WorkerHandle {
    fn drop(&mut self) {
        self.shared.lock().stopped = true;
        self.shared.wake.notify_one();
    }
}

fn work(shared: &Shared, mut analysis: Analysis) {
    if let Some(start) = WORKER_START.get() {
        start();
    }
    let mut slot = shared.lock();
    loop {
        if slot.stopped {
            return;
        }
        let Some(job) = slot.job.take() else {
            slot = shared
                .wake
                .wait(slot)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            continue;
        };
        slot.busy = true;
        drop(slot);
        let visual = Arc::new(analysis.run(job));
        slot = shared.lock();
        slot.busy = false;
        let unshown = slot.finished.replace((job.run, visual));
        drop(slot);
        drop(unshown);
        slot = shared.lock();
    }
}
