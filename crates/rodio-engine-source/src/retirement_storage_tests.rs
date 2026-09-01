use super::*;
use realtime_engine::synth::MAX_CONTROL_EVENTS_PER_CALLBACK;

const RETIREMENT_FILL_COUNT: usize =
    RETIREMENT_QUEUE_CAPACITY + RETIREMENT_CONTROL_BACKLOG_CAPACITY;
const RETIREMENT_EVENT_COUNT: usize = RETIREMENT_FILL_COUNT + 8;

impl EngineSource {
    fn retired_backlog_read(&self) -> usize {
        self.retired_backlog.as_ref().expect("retired backlog").read
    }

    fn retired_backlog_item(&self, index: usize) -> Option<&RetiredAudioItem> {
        self.retired_backlog
            .as_ref()
            .expect("retired backlog")
            .items
            .get(index)
            .and_then(Option::as_ref)
    }

    fn flush_retired_backlog(&mut self) {
        self.retired_backlog
            .as_mut()
            .expect("retired backlog")
            .flush(&self.retired_tx, &mut self.retirement_disconnected);
    }
}

fn retired_event_id(item: &RetiredAudioItem) -> &str {
    match item.event.as_ref() {
        Some(EngineEvent::MomentaryFxStop { id }) => id,
        _ => panic!("expected a retired momentary FX event"),
    }
}

fn drain_retired_event_ids(
    retired_rx: &crossbeam_channel::Receiver<RetiredAudioItem>,
    ids: &mut Vec<String>,
) {
    while let Ok(item) = retired_rx.try_recv() {
        ids.push(retired_event_id(&item).to_owned());
    }
}

fn fill_retirement_storage(
    tx: &EngineEventSender,
    source: &mut EngineSource,
    retired_rx: &crossbeam_channel::Receiver<RetiredAudioItem>,
) {
    for index in 0..RETIREMENT_FILL_COUNT {
        tx.send(EngineEvent::MomentaryFxStop {
            id: format!("fill-{index}"),
        })
        .unwrap();
    }
    assert_eq!(
        source.drain_control_events().control_events,
        MAX_CONTROL_EVENTS_PER_CALLBACK as u64
    );
    assert_eq!(
        source.drain_control_events().control_events,
        (RETIREMENT_FILL_COUNT - MAX_CONTROL_EVENTS_PER_CALLBACK) as u64
    );
    assert!(!source.retirement_disconnected);
    assert_eq!(retired_rx.len(), RETIREMENT_QUEUE_CAPACITY);
    assert_eq!(
        source.retired_backlog_len(),
        RETIREMENT_CONTROL_BACKLOG_CAPACITY
    );
}

#[test]
fn retirement_backpressure_preserves_fifo_and_resumes_after_receiver_drain() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    for index in 0..RETIREMENT_EVENT_COUNT {
        tx.send(EngineEvent::MomentaryFxStop {
            id: format!("event-{index}"),
        })
        .unwrap();
    }

    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        assert_eq!(
            source.drain_control_events().control_events,
            MAX_CONTROL_EVENTS_PER_CALLBACK as u64
        );
        assert_eq!(
            source.drain_control_events().control_events,
            (RETIREMENT_FILL_COUNT - MAX_CONTROL_EVENTS_PER_CALLBACK) as u64
        );
        assert_eq!(source.drain_control_events().control_events, 0);
    });
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    assert!(!source.retirement_disconnected);
    assert_eq!(retired_rx.len(), RETIREMENT_QUEUE_CAPACITY);
    assert_eq!(
        source.retired_backlog_len(),
        RETIREMENT_CONTROL_BACKLOG_CAPACITY
    );

    let mut retired_ids = Vec::new();
    drain_retired_event_ids(&retired_rx, &mut retired_ids);
    assert_eq!(retired_ids.len(), RETIREMENT_QUEUE_CAPACITY);
    assert_eq!(
        source.drain_control_events().control_events,
        (RETIREMENT_EVENT_COUNT - RETIREMENT_FILL_COUNT) as u64
    );
    while source.retired_backlog_len() > 0 {
        drain_retired_event_ids(&retired_rx, &mut retired_ids);
        source.flush_retired_backlog();
    }
    drain_retired_event_ids(&retired_rx, &mut retired_ids);
    assert_eq!(source.retired_backlog_len(), 0);
    let expected_ids: Vec<_> = (0..RETIREMENT_EVENT_COUNT)
        .map(|index| format!("event-{index}"))
        .collect();
    assert_eq!(retired_ids, expected_ids);
}

#[test]
fn retirement_disconnect_preserves_current_item_and_stops_control_drain() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    drop(retired_rx);
    tx.send(EngineEvent::MomentaryFxStop {
        id: "disconnected-current".into(),
    })
    .unwrap();
    tx.send(EngineEvent::MomentaryFxStop {
        id: "disconnected-next".into(),
    })
    .unwrap();

    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        assert_eq!(source.drain_control_events().control_events, 1);
    });
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    assert!(source.retirement_disconnected);
    assert_eq!(source.retired_backlog_len(), 1);
    let item = source
        .retired_backlog_item(source.retired_backlog_read())
        .expect("disconnected item must remain in backlog");
    assert_eq!(retired_event_id(item), "disconnected-current");
    assert_eq!(source.drain_control_events().control_events, 0);
    let queued = source
        .control_rx
        .try_recv_ordered()
        .expect("later control event must remain queued");
    match queued {
        EngineEvent::MomentaryFxStop { id } => assert_eq!(id, "disconnected-next"),
        _ => panic!("expected queued momentary FX event"),
    }
}

#[test]
fn pending_render_retirement_stays_owned_until_capacity_returns() {
    let (tx, rx) = event_queue();
    let (mut source, retired_rx) = EngineSource::with_test_retirement_receiver(rx, 44_100);
    super::retirement_tests::warm_source(&mut source);
    fill_retirement_storage(&tx, &mut source, &retired_rx);
    source.retire_event(EngineEvent::MomentaryFxStop {
        id: "pending-capacity".into(),
    });
    assert_eq!(source.retired_backlog_len(), RETIREMENT_BACKLOG_CAPACITY);

    let _ = source
        .engine
        .preview_sample(0, super::retirement_tests::preview_buffer(), 100);
    let (allocation_count, deallocation_count) = allocations_and_deallocations(|| {
        let _ = source.next();
    });
    assert_eq!(allocation_count, 0);
    assert_eq!(deallocation_count, 0);
    assert_eq!(
        source
            .engine
            .profile_snapshot()
            .active_preview_sample_voices,
        0
    );
    assert!(!source.engine.pending_render_retired_is_empty());
    assert_eq!(source.retired_backlog_len(), RETIREMENT_BACKLOG_CAPACITY);

    let mut retired_ids = Vec::new();
    drain_retired_event_ids(&retired_rx, &mut retired_ids);
    source.refill();
    assert!(source.engine.pending_render_retired_is_empty());
    assert!(source
        .retired_backlog_items()
        .any(|item| { item.state.as_ref().is_some_and(|state| !state.is_empty()) }));
}
