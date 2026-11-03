use super::modulation::RoutedMusicalEvents;
use super::{NativeRunner, Value, BUS_COUNT, INSTRUMENT_COUNT};
use crate::protocol::RuntimeAudioCommand;
use platform_core::MusicalEvent;

impl NativeRunner {
    pub(super) fn instrument_muted(&self, index: usize) -> bool {
        self.instrument_mutes.get(index) == Some(&true)
    }

    pub(super) fn toggle_instrument_mute(&mut self, index: usize) {
        if self
            .instruments
            .get(index)
            .is_none_or(|instrument| instrument.kind == "none")
        {
            return;
        }
        let muted = !self.instrument_mutes[index];
        if muted {
            self.release_midi_notes_on_mute(index);
        }
        self.instrument_mutes[index] = muted;
        let mut slots = [false; INSTRUMENT_COUNT];
        slots[index] = true;
        self.resend_engine_volumes(slots);
        let state = if muted { "muted" } else { "unmuted" };
        self.show_toast(format!("I{} {state}", index + 1));
    }

    pub(super) fn take_instrument_mutes(&mut self) -> [bool; INSTRUMENT_COUNT] {
        std::mem::take(&mut self.instrument_mutes)
    }

    pub(super) fn bus_muted(&self, index: usize) -> bool {
        self.play_live.bus_mutes.get(index) == Some(&true)
    }

    pub(super) fn toggle_bus_mute(&mut self, index: usize) {
        let Some(muted) = self.play_live.bus_mutes.get_mut(index) else {
            return;
        };
        *muted = !*muted;
        let state = if *muted { "muted" } else { "unmuted" };
        let mut buses = [false; BUS_COUNT];
        buses[index] = true;
        self.resend_bus_volumes(buses);
        self.show_toast(format!("B{} {state}", index + 1));
    }

    pub(super) fn take_bus_mutes(&mut self) -> [bool; BUS_COUNT] {
        std::mem::take(&mut self.play_live.bus_mutes)
    }

    pub(super) fn resend_bus_volumes(&mut self, buses: [bool; BUS_COUNT]) {
        if !buses.contains(&true) {
            return;
        }
        for (index, _) in buses.iter().enumerate().filter(|(_, resend)| **resend) {
            let Some(volume) = self.fx_buses.get(index).map(|bus| bus.volume_pct) else {
                continue;
            };
            self.queue_audio_command(RuntimeAudioCommand::SetFxBusMixer {
                bus_index: index,
                generation: 0,
                pan_pos: None,
                volume_pct: Some(f32::from(volume)),
            });
        }
        self.invalidate_lfo_audio_cache();
        if let Err(error) = self.process_modulation_step(true) {
            self.show_toast(format!("LFO composition unavailable: {error}"));
        }
    }

    pub(super) fn resend_engine_volumes(&mut self, slots: [bool; INSTRUMENT_COUNT]) {
        if !slots.contains(&true) {
            return;
        }
        for (index, _) in slots.iter().enumerate().filter(|(_, resend)| **resend) {
            let Some(volume) = self.instruments.get(index).map(|slot| slot.volume) else {
                continue;
            };
            self.queue_audio_command(RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot: index,
                generation: 0,
                volume_pct: Some(f32::from(volume)),
                pan_pos: None,
            });
        }
        self.invalidate_lfo_audio_cache();
        if let Err(error) = self.process_modulation_step(true) {
            self.show_toast(format!("LFO composition unavailable: {error}"));
        }
    }

    pub(super) fn engine_audio_command(
        &self,
        mut command: RuntimeAudioCommand,
    ) -> RuntimeAudioCommand {
        if !self.instrument_mutes.contains(&true) && !self.play_live.bus_mutes.contains(&true) {
            return command;
        }
        match &mut command {
            RuntimeAudioCommand::SetFxBusMixer {
                bus_index,
                volume_pct: Some(volume),
                ..
            } if self.bus_muted(*bus_index) => *volume = 0.0,
            RuntimeAudioCommand::SetInstrumentMixer {
                instrument_slot,
                volume_pct: Some(volume),
                ..
            } if self.instrument_muted(*instrument_slot) => *volume = 0.0,
            RuntimeAudioCommand::SetInstrumentSlot {
                instrument_slot,
                config,
                ..
            } if self.instrument_muted(*instrument_slot) => silence_payload_volume(config),
            RuntimeAudioCommand::SetAudioConfig { config, .. } => {
                let instruments = config
                    .get_mut("instruments")
                    .and_then(Value::as_array_mut)
                    .into_iter()
                    .flatten();
                for (index, instrument) in instruments.enumerate() {
                    if self.instrument_muted(index) {
                        silence_payload_volume(instrument);
                    }
                }
                let buses = config
                    .get_mut("mixer")
                    .and_then(|mixer| mixer.get_mut("buses"))
                    .and_then(Value::as_array_mut)
                    .into_iter()
                    .flatten();
                for (index, bus) in buses.enumerate() {
                    if self.bus_muted(index) {
                        bus["volumePct"] = Value::from(0);
                    }
                }
            }
            _ => {}
        }
        command
    }

    pub(super) fn drop_muted_midi_note_ons(&self, events: &mut RoutedMusicalEvents) {
        if events.midi.is_empty() || !self.instrument_mutes.contains(&true) {
            return;
        }
        let channels = (0..INSTRUMENT_COUNT)
            .filter(|index| self.instrument_muted(*index))
            .filter_map(|index| self.midi_output_channel(index))
            .fold(0_u16, |mask, channel| mask | 1 << channel);
        if channels == 0 {
            return;
        }
        events.midi.retain(|event| {
            !matches!(event, MusicalEvent::NoteOn { channel, .. } if channels & 1 << channel != 0)
        });
    }

    fn release_midi_notes_on_mute(&mut self, index: usize) {
        let Some(channel) = self.midi_output_channel(index) else {
            return;
        };
        for layer_index in 0..self.emitted_route_note_owners.len() {
            let mut held = self.layer_owned_route_note_offs(layer_index);
            held.audio.clear();
            held.tiles.clear();
            held.midi.retain(|event| {
                matches!(event, MusicalEvent::NoteOff { channel: held, .. } if *held == channel)
            });
            let released = self.release_layer_owned_route_notes(layer_index, held);
            self.pending_transpose_note_offs.extend(released);
        }
    }

    fn midi_output_channel(&self, index: usize) -> Option<u8> {
        self.instruments
            .get(index)
            .filter(|instrument| instrument.kind == "midi" && instrument.midi_enabled)
            .map(|instrument| instrument.midi_channel.saturating_sub(1).min(15))
    }
}

fn silence_payload_volume(config: &mut Value) {
    if let Some(mixer) = config.get_mut("mixer").and_then(Value::as_object_mut) {
        mixer.insert("volume".into(), Value::from(0));
    }
}
