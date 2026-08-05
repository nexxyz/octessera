use crate::setup_portal::SetupPortalService;
use playback_runtime::HostMessage;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{SyncSender, TrySendError};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const SETUP_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(crate) fn spawn(
    results: SyncSender<HostMessage>,
    setup_portal: SetupPortalService,
    stop: Arc<AtomicBool>,
) {
    thread::Builder::new()
        .name("octessera-setup-portal".into())
        .spawn(move || run(results, setup_portal, stop))
        .expect("setup portal worker should start");
}

fn run(results: SyncSender<HostMessage>, setup_portal: SetupPortalService, stop: Arc<AtomicBool>) {
    while !stop.load(Ordering::Acquire) {
        thread::sleep(SETUP_POLL_INTERVAL);
        if !setup_portal.has_published_pending() || setup_portal.has_buffered_result() {
            continue;
        }
        let Some(result) = setup_portal.poll_one() else {
            continue;
        };
        match results.try_send(result) {
            Ok(()) => {}
            Err(TrySendError::Full(result)) => setup_portal.buffer_result(result),
            Err(TrySendError::Disconnected(_)) => break,
        }
    }
}
