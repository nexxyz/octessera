use super::*;
use playback_runtime::AudioOutputSet;
use std::sync::mpsc;

struct OpenStream {
    dropped: mpsc::Sender<()>,
}

impl Drop for OpenStream {
    fn drop(&mut self) {
        let _ = self.dropped.send(());
    }
}

fn outputs(usb: bool) -> AudioOutputSet {
    AudioOutputSet::from_flags(true, usb, false).unwrap()
}

fn usb_worker(
    usb: bool,
    result: fn() -> Result<(), RouteOpenError>,
) -> (
    OptionalRecoveryWorker,
    Arc<SelectedOutputs>,
    mpsc::Receiver<()>,
    mpsc::Receiver<()>,
) {
    let selected = SelectedOutputs::new(outputs(usb));
    let (opened_tx, opened) = mpsc::channel();
    let (dropped_tx, dropped) = mpsc::channel();
    let worker = spawn_with_opener(
        crate::audio_route::new_registry(outputs(usb)),
        AudioSink::Usb,
        rodio_engine_source::new_pcm_mirror().producer,
        selected.clone(),
        Box::new(move |_consumer| {
            let _ = opened_tx.send(());
            result().map(|()| ManagedStream {
                _stream: Box::new(OpenStream {
                    dropped: dropped_tx.clone(),
                }),
                health: AudioStreamHealth::optional("USB".into()),
            })
        }),
    );
    (worker, selected, opened, dropped)
}

const WAIT: Duration = Duration::from_secs(2);

#[test]
fn a_side_output_starts_and_stops_when_system_selects_it() {
    let (worker, selected, opened, dropped) = usb_worker(false, || Ok(()));
    assert!(opened.recv_timeout(Duration::from_millis(100)).is_err());

    selected.set(outputs(true));
    opened.recv_timeout(WAIT).unwrap();

    selected.set(outputs(false));
    dropped.recv_timeout(WAIT).unwrap();

    selected.set(outputs(true));
    opened.recv_timeout(WAIT).unwrap();
    drop(worker);
    dropped.recv_timeout(WAIT).unwrap();
}

#[test]
fn a_faulted_side_output_waits_for_a_new_selection_before_retrying() {
    let (worker, selected, opened, _dropped) =
        usb_worker(true, || Err(RouteOpenError::Fault("backend".into())));
    opened.recv_timeout(WAIT).unwrap();
    assert!(opened.recv_timeout(Duration::from_millis(100)).is_err());

    selected.set(outputs(false));
    selected.set(outputs(true));
    opened.recv_timeout(WAIT).unwrap();
    drop(worker);
}

#[test]
fn only_disconnected_open_failures_retry() {
    assert!(should_retry_open(&RouteOpenError::Disconnected));
    assert!(!should_retry_open(&RouteOpenError::Busy));
    assert!(!should_retry_open(&RouteOpenError::Unsupported(
        "format".into()
    )));
    assert!(!should_retry_open(&RouteOpenError::Fault("backend".into())));
}
