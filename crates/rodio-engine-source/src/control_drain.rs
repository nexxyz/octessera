#[cfg(test)]
use super::RetiredAudioDropProbe;
use super::{
    retired_audio_backlog::RetiredAudioBacklog, DrainedControlEvents, EngineEvent,
    EngineEventReceiver, RetiredAudioItem, SynthEngine,
};
use crate::latest_controls::{LatestCandidate, LatestKey};
use crate::queue_types::QueueEventClass;
use realtime_engine::synth::{
    ScalarMutation, BUS_COUNT, BUS_SLOTS_PER_BUS, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_SLOT_COUNT,
    MAX_CONTROL_EVENTS_PER_CALLBACK,
};

const LATEST_CONTROL_BUDGET: usize = 32;

#[path = "control_drain_events.rs"]
mod control_drain_events;

pub(super) struct OwnerGenerations {
    pub(super) full: u64,
    pub(super) instrument: [u64; INSTRUMENT_SLOT_COUNT],
    pub(super) sample: [u64; INSTRUMENT_SLOT_COUNT],
    pub(super) bus_mixer: [u64; BUS_COUNT],
    pub(super) fx_bus: [[u64; BUS_SLOTS_PER_BUS]; BUS_COUNT],
    pub(super) global_fx: [u64; GLOBAL_FX_SLOT_COUNT],
}

impl Default for OwnerGenerations {
    fn default() -> Self {
        Self {
            full: 0,
            instrument: [0; INSTRUMENT_SLOT_COUNT],
            sample: [0; INSTRUMENT_SLOT_COUNT],
            bus_mixer: [0; BUS_COUNT],
            fx_bus: [[0; BUS_SLOTS_PER_BUS]; BUS_COUNT],
            global_fx: [0; GLOBAL_FX_SLOT_COUNT],
        }
    }
}

pub(super) struct ControlDrain<'a> {
    control_rx: &'a mut EngineEventReceiver,
    retired_tx: &'a crossbeam_channel::Sender<RetiredAudioItem>,
    retired_backlog: &'a mut RetiredAudioBacklog,
    retirement_disconnected: &'a mut bool,
    generations: &'a mut OwnerGenerations,
    emergency_retirement: &'a mut Option<RetiredAudioItem>,
    #[cfg(test)]
    retired_drop_probe: Option<std::sync::mpsc::Sender<std::thread::ThreadId>>,
}

impl<'a> ControlDrain<'a> {
    pub(super) fn new(
        control_rx: &'a mut EngineEventReceiver,
        retired_tx: &'a crossbeam_channel::Sender<RetiredAudioItem>,
        retired_backlog: &'a mut RetiredAudioBacklog,
        retirement_disconnected: &'a mut bool,
        generations: &'a mut OwnerGenerations,
        emergency_retirement: &'a mut Option<RetiredAudioItem>,
        #[cfg(test)] retired_drop_probe: Option<std::sync::mpsc::Sender<std::thread::ThreadId>>,
    ) -> Self {
        Self {
            control_rx,
            retired_tx,
            retired_backlog,
            retirement_disconnected,
            generations,
            emergency_retirement,
            #[cfg(test)]
            retired_drop_probe,
        }
    }

    pub(super) fn drain(&mut self, engine: &mut SynthEngine) -> DrainedControlEvents {
        self.drain_with_source_event_clock(engine, None)
    }

    #[cfg(feature = "routing-tree-executor")]
    pub(super) fn drain_routing_tree(
        &mut self,
        engine: &mut SynthEngine,
        source_event_sample_clock: u64,
    ) -> DrainedControlEvents {
        self.drain_with_source_event_clock(engine, Some(source_event_sample_clock))
    }

    fn drain_with_source_event_clock(
        &mut self,
        engine: &mut SynthEngine,
        source_event_sample_clock: Option<u64>,
    ) -> DrainedControlEvents {
        #[cfg(not(feature = "routing-tree-executor"))]
        let _ = source_event_sample_clock;
        let mut drained = DrainedControlEvents::default();
        let mut work_units = 0;
        let mut owner_mutation_applied = false;
        let mut panic_processed = false;

        let emergency_redundant = self.emergency_retirement_is_blocked();
        if let Some(EngineEvent::AllNotesOff) = self.control_rx.try_take_emergency() {
            panic_processed = true;
            work_units += 1;
            if !emergency_redundant {
                let retired =
                    Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                        engine.all_notes_off()
                    });
                self.retire_state(retired);
            }
            drained.control_events += 1;
        }

        while work_units < LATEST_CONTROL_BUDGET {
            let Some(candidate) = self.control_rx.take_latest_candidate() else {
                break;
            };
            work_units += 1;
            if let LatestKey::MomentaryUpdate(_) = candidate.key {
                self.apply_latest_momentary(engine, candidate, &mut drained);
                continue;
            }
            match self.latest_owner_generation(candidate.key) {
                Some(owner) if candidate.generation < owner => {
                    self.control_rx.mark_latest_applied(candidate);
                }
                Some(owner) if candidate.generation > owner => {}
                _ => {
                    self.apply_latest(engine, candidate);
                    self.control_rx.mark_latest_applied(candidate);
                    drained.config_events += 1;
                }
            }
        }

        while work_units < MAX_CONTROL_EVENTS_PER_CALLBACK {
            let retirement_ready = self.retirement_storage_can_accept_item();
            let allow_structural_owner =
                retirement_ready && !owner_mutation_applied && !*self.retirement_disconnected;
            let allow_emergency = !panic_processed;
            let allow_structural_barrier =
                retirement_ready && !owner_mutation_applied && !*self.retirement_disconnected;
            let dequeue = self.control_rx.try_take_next_allowed(
                allow_emergency,
                allow_structural_owner,
                allow_structural_barrier,
            );
            let dequeue = match dequeue {
                Ok(dequeue) => dequeue,
                Err(_) => break,
            };
            let class = dequeue.class;
            let event = match dequeue.event {
                Some(event) => event,
                None => {
                    if class == QueueEventClass::Emergency {
                        if self.control_rx.try_take_emergency().is_some() {
                            work_units += 1;
                            drained.control_events += 1;
                        }
                        panic_processed = true;
                        continue;
                    }
                    if class == QueueEventClass::StructuralBarrier || allow_structural_owner {
                        break;
                    }
                    let Some(event) = self.control_rx.try_take_musical() else {
                        break;
                    };
                    work_units += 1;
                    drained.control_events += 1;
                    let allow_note_on = !self.emergency_retirement_is_blocked();
                    self.apply_musical_event(
                        engine,
                        event,
                        source_event_sample_clock,
                        allow_note_on,
                    );
                    continue;
                }
            };
            if class == QueueEventClass::Emergency {
                panic_processed = true;
                if !self.emergency_retirement_is_blocked() {
                    let retired =
                        Self::apply_source_event(engine, source_event_sample_clock, |engine| {
                            engine.all_notes_off()
                        });
                    self.retire_state(retired);
                }
                work_units += 1;
                drained.control_events += 1;
                continue;
            }
            let owner_class = matches!(
                class,
                QueueEventClass::StructuralRetiring | QueueEventClass::StructuralNonRetiring
            );
            let retirement_ready = self.retirement_storage_can_accept_item();
            if owner_class
                && (!retirement_ready || owner_mutation_applied || *self.retirement_disconnected)
            {
                break;
            }
            work_units += 1;
            drained.control_events += 1;
            let is_owner = matches!(
                class,
                QueueEventClass::StructuralRetiring | QueueEventClass::StructuralNonRetiring
            );
            if is_owner {
                owner_mutation_applied = true;
            }
            let stop_after = match class {
                QueueEventClass::Musical => {
                    let allow_note_on = !self.emergency_retirement_is_blocked();
                    self.apply_musical_event(
                        engine,
                        event,
                        source_event_sample_clock,
                        allow_note_on,
                    );
                    false
                }
                QueueEventClass::StructuralBarrier
                | QueueEventClass::StructuralNonRetiring
                | QueueEventClass::StructuralRetiring => {
                    self.apply_event(engine, event, source_event_sample_clock, &mut drained)
                }
                QueueEventClass::Emergency => false,
            };
            if stop_after {
                break;
            }
        }

        if work_units < MAX_CONTROL_EVENTS_PER_CALLBACK
            && !owner_mutation_applied
            && !*self.retirement_disconnected
            && !self.control_rx.has_pending_structural()
            && self.retirement_storage_can_accept_item()
        {
            if let Some(event) = self.control_rx.try_take_preview() {
                drained.control_events += 1;
                self.apply_preview(engine, event, source_event_sample_clock);
            }
        }
        drained
    }

    fn apply_latest_momentary(
        &mut self,
        engine: &mut SynthEngine,
        candidate: LatestCandidate,
        drained: &mut DrainedControlEvents,
    ) {
        let Some(update) = candidate.momentary else {
            self.control_rx.mark_latest_applied(candidate);
            return;
        };
        if self.control_rx.latest_epoch_cancelled(candidate.generation) {
            self.control_rx.mark_latest_applied(candidate);
            return;
        }
        let applied = engine.apply_prepared_momentary_fx_update(update);
        if applied != ScalarMutation::Rejected
            || self.control_rx.latest_epoch_cancelled(candidate.generation)
        {
            self.control_rx.mark_latest_applied(candidate);
        }
        drained.config_events += 1;
    }

    fn retire_state(&mut self, state: realtime_engine::synth::RetiredAudioState) {
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

    fn retire_event(&mut self, event: EngineEvent) {
        self.retire_item(RetiredAudioItem {
            state: None,
            event: Some(event),
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
        self.flush_retirement_storage();
        if *self.retirement_disconnected {
            self.hold_retirement(item);
            return;
        }
        match self.retired_tx.try_send(item) {
            Ok(()) => {}
            Err(crossbeam_channel::TrySendError::Full(item)) => self.hold_retirement(item),
            Err(crossbeam_channel::TrySendError::Disconnected(item)) => {
                *self.retirement_disconnected = true;
                self.hold_retirement(item);
            }
        }
    }

    fn retirement_storage_can_accept_item(&mut self) -> bool {
        self.flush_retirement_storage();
        self.retired_backlog.len < super::RETIREMENT_CONTROL_BACKLOG_CAPACITY
    }

    fn emergency_retirement_is_blocked(&mut self) -> bool {
        self.flush_retirement_storage();
        self.retired_backlog.len >= super::RETIREMENT_BACKLOG_CAPACITY
            && self.emergency_retirement.is_some()
    }

    fn flush_retirement_storage(&mut self) {
        self.retired_backlog
            .flush(self.retired_tx, self.retirement_disconnected);
        if self.emergency_retirement.is_some()
            && self.retired_backlog.len < super::RETIREMENT_CONTROL_BACKLOG_CAPACITY
        {
            let item = self
                .emergency_retirement
                .take()
                .expect("emergency retirement slot");
            let _ = self.retired_backlog.enqueue(item);
            self.retired_backlog
                .flush(self.retired_tx, self.retirement_disconnected);
        }
    }

    fn hold_retirement(&mut self, item: RetiredAudioItem) {
        if self.retired_backlog.len < super::RETIREMENT_BACKLOG_CAPACITY {
            debug_assert!(self.retired_backlog.enqueue(item));
            return;
        }
        if self.emergency_retirement.is_none() {
            *self.emergency_retirement = Some(item);
        }
    }
}
