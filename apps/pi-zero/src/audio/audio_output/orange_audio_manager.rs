use super::{mirror_index, AudioManager, AudioSink, OrangeDacStatus, OrangeRecoveryController};
use playback_runtime::AudioOutputSet;
use rodio_engine_source::PcmMirrorProducer;

#[cfg(feature = "hardware-orange-pi-zero-2w")]
impl AudioManager {
    pub(crate) fn recover_audio_if_due(&mut self) {
        if let Some(recovery) = self.orange_dac_recovery.as_mut() {
            let load_rx = &self.load_rx;
            let reset_pending = &mut self.load_status_reset_pending;
            recovery.recover_if_due_with(
                || {
                    if !*reset_pending {
                        if let Some(load_rx) = load_rx.as_ref() {
                            while load_rx.try_recv().is_ok() {}
                        }
                        *reset_pending = true;
                    }
                },
                Some(self.load_tx.clone()),
            );
            if recovery.device_status() == OrangeDacStatus::Healthy {
                self.load_status_reset_pending = false;
            }
        }
        if let Some(recovery) = &self.orange_dac_recovery {
            crate::audio_route::set_status(
                &self.route_registry,
                AudioSink::Jack,
                match recovery.device_status() {
                    OrangeDacStatus::Healthy => crate::audio_route::AudioRouteStatus::Active,
                    OrangeDacStatus::Recovering => crate::audio_route::AudioRouteStatus::Waiting,
                    OrangeDacStatus::Terminal => crate::audio_route::AudioRouteStatus::Faulted,
                },
            );
        }
        let (profile, engine) = (self.orange_profile, self.service.engine.clone());
        follow_selected_side_outputs(
            &mut self.orange_recovery,
            self.service.audio_outputs(),
            &self.side_producers,
            |sink, producer| {
                OrangeRecoveryController::new_optional_missing(
                    sink,
                    profile,
                    engine.clone(),
                    producer,
                )
            },
        );
        for recovery in &mut self.orange_recovery {
            recovery.recover_if_due();
        }
        for recovery in &self.orange_recovery {
            crate::audio_route::set_status(
                &self.route_registry,
                recovery.sink(),
                match recovery.device_status() {
                    OrangeDacStatus::Healthy => crate::audio_route::AudioRouteStatus::Active,
                    OrangeDacStatus::Recovering => crate::audio_route::AudioRouteStatus::Waiting,
                    OrangeDacStatus::Terminal => crate::audio_route::AudioRouteStatus::Faulted,
                },
            );
        }
    }

    pub(crate) fn report_runtime_terminal_diagnostics(&self) {
        if let Some(recovery) = &self.orange_dac_recovery {
            recovery.report_runtime_terminal();
        }
    }

    pub(crate) fn ensure_selected_routes(&self) -> Result<(), String> {
        if let Some(recovery) = &self.orange_dac_recovery {
            if recovery.runtime_status() == OrangeDacStatus::Terminal {
                return Err("Orange Jack audio stream is not active".into());
            }
        }
        Ok(())
    }
}

/// Starts a recovery controller for each newly selected side output and drops
/// the controller, and with it the stream, of each deselected one.
fn follow_selected_side_outputs(
    recovery: &mut Vec<OrangeRecoveryController>,
    outputs: AudioOutputSet,
    producers: &[PcmMirrorProducer; 2],
    start: impl Fn(AudioSink, PcmMirrorProducer) -> OrangeRecoveryController,
) {
    let selected = AudioSink::selected(outputs);
    for sink in [AudioSink::Usb, AudioSink::Hdmi] {
        let producer = &producers[mirror_index(sink).expect("side mirror index")];
        let running = recovery
            .iter()
            .position(|controller| controller.sink() == sink);
        match (selected.contains(&sink), running) {
            (true, None) => {
                producer.reactivate();
                recovery.push(start(sink, producer.clone()));
            }
            (false, Some(index)) => {
                producer.invalidate();
                recovery.swap_remove(index);
                eprintln!("{sink:?} audio stopped");
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio_engine_owner::AudioEngineOwner;
    use crate::audio_replay::ReplayCache;
    use crate::audio_route::RouteOpenError;

    fn sinks(recovery: &[OrangeRecoveryController]) -> Vec<AudioSink> {
        let mut sinks: Vec<_> = recovery
            .iter()
            .map(OrangeRecoveryController::sink)
            .collect();
        sinks.sort();
        sinks
    }

    #[test]
    fn side_outputs_start_and_stop_with_the_selected_outputs() {
        let producers = [
            rodio_engine_source::new_pcm_mirror().producer,
            rodio_engine_source::new_pcm_mirror().producer,
        ];
        let start = |sink, producer| {
            OrangeRecoveryController::new_optional_missing_with_dependencies(
                sink,
                super::super::OrangeAudioProfile::from_optimization(
                    playback_runtime::AudioOptimization::Latency,
                ),
                AudioEngineOwner::new(ReplayCache::default()),
                std::sync::Arc::new(|_, _, _, _, _, _, _| Err(RouteOpenError::Disconnected)),
                std::sync::Arc::new(std::time::Instant::now),
                Some(producer),
            )
        };
        let mut recovery = Vec::new();
        let both = AudioOutputSet::from_flags(true, true, true).unwrap();
        let hdmi = AudioOutputSet::from_flags(true, false, true).unwrap();

        follow_selected_side_outputs(&mut recovery, both, &producers, start);
        assert_eq!(sinks(&recovery), [AudioSink::Usb, AudioSink::Hdmi]);
        follow_selected_side_outputs(&mut recovery, both, &producers, start);
        assert_eq!(sinks(&recovery), [AudioSink::Usb, AudioSink::Hdmi]);
        follow_selected_side_outputs(&mut recovery, hdmi, &producers, start);
        assert_eq!(sinks(&recovery), [AudioSink::Hdmi]);
        follow_selected_side_outputs(&mut recovery, AudioOutputSet::jack(), &producers, start);
        assert!(recovery.is_empty());
    }
}
