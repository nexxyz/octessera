use super::{BoardWorker, DesiredLedOutputs, SeesawIo, ShutdownRequest};
use octessera_hal::{NeoKey, NeoTrellis, SeesawInterrupt};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

const SEESAW_HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(1);
pub(super) const SEESAW_STALL_TIMEOUT: Duration = Duration::from_secs(5);

struct RaspberryWorker {
    scan_generation: Arc<AtomicU64>,
}

impl BoardWorker for RaspberryWorker {
    fn start(&mut self, _outputs: &mut DesiredLedOutputs) {}

    fn record_scan(&mut self, succeeded: bool) {
        if succeeded {
            self.scan_generation.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn finish_pass(
        &mut self,
        _shutdown: Option<ShutdownRequest>,
        _outputs: &DesiredLedOutputs,
    ) -> bool {
        false
    }
}

pub(crate) fn spawn_interrupt(
    trellis: NeoTrellis,
    neokey: NeoKey,
    interrupt: SeesawInterrupt,
) -> SeesawIo {
    let scan_generation = Arc::new(AtomicU64::new(0));
    let seesaw = super::spawn(
        trellis,
        neokey,
        interrupt,
        RaspberryWorker {
            scan_generation: Arc::clone(&scan_generation),
        },
    );
    spawn_seesaw_health_monitor(scan_generation);
    seesaw
}

impl SeesawIo {
    pub(crate) fn spawn_keyboard(
        &self,
        role: playback_runtime::UsbDataRole,
    ) -> crate::keyboard_capture::KeyboardCapture {
        crate::keyboard_capture::KeyboardCapture::spawn(
            self.input_tx.clone(),
            role == playback_runtime::UsbDataRole::Host,
        )
    }
}

pub(super) fn seesaw_worker_has_stalled(
    last_generation: &mut u64,
    last_progress_at: &mut Instant,
    current_generation: u64,
    now: Instant,
) -> bool {
    if current_generation != *last_generation {
        *last_generation = current_generation;
        *last_progress_at = now;
        return false;
    }
    now.saturating_duration_since(*last_progress_at) >= SEESAW_STALL_TIMEOUT
}

fn spawn_seesaw_health_monitor(scan_generation: Arc<AtomicU64>) {
    let _monitor = thread::spawn(move || {
        let mut last_generation = scan_generation.load(Ordering::Relaxed);
        let mut last_progress_at = Instant::now();
        loop {
            thread::sleep(SEESAW_HEALTH_CHECK_INTERVAL);
            let now = Instant::now();
            let current_generation = scan_generation.load(Ordering::Relaxed);
            if seesaw_worker_has_stalled(
                &mut last_generation,
                &mut last_progress_at,
                current_generation,
                now,
            ) {
                eprintln!(
                    "critical Seesaw worker stall: no successful input scan for {}s; exiting for service restart",
                    SEESAW_STALL_TIMEOUT.as_secs()
                );
                std::process::exit(1);
            }
        }
    });
}
