use crate::platform_service::PlatformResultLane;
use crate::setup_portal::SetupPortalService;
use crate::user_data_transfer::UserDataTransferService;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const SETUP_POLL_INTERVAL: Duration = Duration::from_millis(25);

pub(crate) fn spawn(
    results: Arc<PlatformResultLane>,
    setup_portal: SetupPortalService,
    stop: Arc<AtomicBool>,
    user_data_transfer: UserDataTransferService,
) {
    thread::Builder::new()
        .name("octessera-setup-portal".into())
        .spawn(move || run(results, setup_portal, stop, user_data_transfer))
        .expect("setup portal worker should start");
}

fn run(
    results: Arc<PlatformResultLane>,
    setup_portal: SetupPortalService,
    stop: Arc<AtomicBool>,
    user_data_transfer: UserDataTransferService,
) {
    while !stop.load(Ordering::Acquire) {
        thread::sleep(SETUP_POLL_INTERVAL);
        user_data_transfer.expire_if_needed();
        if !setup_portal.has_pending() {
            user_data_transfer.stop();
            continue;
        }
        let Some(result) = setup_portal.poll_one() else {
            continue;
        };
        let result = user_data_transfer.decorate_setup_result(result);
        if results.send_setup(result).is_err() {
            break;
        }
        if !setup_portal.has_pending() {
            user_data_transfer.stop();
        }
    }
    user_data_transfer.stop();
}
