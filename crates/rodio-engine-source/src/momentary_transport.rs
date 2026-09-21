use crate::event::EngineEvent;
use crate::latest_controls::LatestControls;
use crate::queue_types::QueueSendError;
use realtime_engine::synth::PreparedMomentaryFxStart;

pub(super) fn send_start(
    config: PreparedMomentaryFxStart,
    latest: &LatestControls,
    send_structural: impl FnOnce(EngineEvent) -> Result<(), QueueSendError>,
) -> Result<(), QueueSendError> {
    let epoch = config.epoch();
    latest.with_momentary_lifecycle(|momentary| {
        let newly_reserved = momentary.reserve_epoch_locked(epoch)?;
        if !newly_reserved {
            return Ok(());
        }
        let result = send_structural(EngineEvent::PreparedMomentaryFxStart { config });
        if result.is_err() {
            momentary.abandon_epoch(epoch);
        }
        result
    })
}

pub(super) fn send_stop(
    epoch: u64,
    latest: &LatestControls,
    send_structural: impl FnOnce(EngineEvent) -> Result<(), QueueSendError>,
) -> Result<(), QueueSendError> {
    latest.with_momentary_lifecycle(|momentary| {
        let result = send_structural(EngineEvent::MomentaryFxStop { epoch });
        if result.is_ok() {
            momentary.cancel_epoch(epoch);
        }
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue_types::QueueKind;
    use realtime_engine::synth::{prepare_momentary_fx_start_with_epoch, MomentaryFxTarget};
    use std::cell::Cell;
    use std::collections::BTreeMap;

    #[test]
    fn in_flight_start_blocks_duplicate_until_enqueue_result_is_known() {
        let latest = LatestControls::new();
        let duplicate = prepared_start(7);
        let result = send_start(prepared_start(7), &latest, |_| {
            assert_eq!(
                send_start(duplicate, &latest, |_| panic!("duplicate start enqueued")),
                Err(QueueSendError::Full {
                    queue: QueueKind::Latest
                })
            );
            Err(QueueSendError::Full {
                queue: QueueKind::Structural,
            })
        });

        assert!(matches!(
            result,
            Err(QueueSendError::Full {
                queue: QueueKind::Structural
            })
        ));
        assert!(latest.is_momentary_epoch_cancelled(7));
        assert!(send_start(prepared_start(7), &latest, |_| Ok(())).is_ok());
    }

    #[test]
    fn in_flight_start_blocks_stop_from_overtaking_it() {
        let latest = LatestControls::new();
        let stop_enqueued = Cell::new(false);

        send_start(prepared_start(8), &latest, |_| {
            assert_eq!(
                send_stop(8, &latest, |_| {
                    stop_enqueued.set(true);
                    Ok(())
                }),
                Err(QueueSendError::Full {
                    queue: QueueKind::Latest
                })
            );
            Ok(())
        })
        .unwrap();

        assert!(!stop_enqueued.get());
        assert!(!latest.is_momentary_epoch_cancelled(8));
        send_stop(8, &latest, |_| Ok(())).unwrap();
        assert!(latest.is_momentary_epoch_cancelled(8));
    }

    #[test]
    fn reserved_empty_epoch_fails_before_structural_enqueue() {
        let latest = LatestControls::new();
        let enqueued = Cell::new(false);

        assert_eq!(
            send_start(prepared_start(u64::MAX), &latest, |_| {
                enqueued.set(true);
                Ok(())
            }),
            Err(QueueSendError::Full {
                queue: QueueKind::Latest
            })
        );
        assert!(!enqueued.get());
    }

    fn prepared_start(epoch: u64) -> PreparedMomentaryFxStart {
        prepare_momentary_fx_start_with_epoch(
            "fx".into(),
            epoch,
            "stutter".into(),
            BTreeMap::new(),
            MomentaryFxTarget::Global,
            44_100,
        )
        .expect("momentary start")
    }
}
