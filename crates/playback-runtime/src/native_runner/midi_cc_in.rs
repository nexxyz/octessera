use super::modulation_source::ModulationSourceId;
use super::{param_binding_from_payload, param_binding_payload, NativeParamBinding, NativeRunner};
use crate::protocol::RunnerMessage;
use crate::runtime::MidiControlChange;
use serde_json::{json, Value};

pub(super) const MIDI_CC_SLOT_COUNT: usize = 8;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeMidiCcMapping {
    pub(super) cc: u8,
    /// 0 listens on every channel, 1-16 on that channel.
    pub(super) channel: u8,
    pub(super) target: Option<NativeParamBinding>,
    /// The last value the slot received; live only, a loaded patch starts without one.
    pub(super) last_value: Option<u8>,
}

impl NativeMidiCcMapping {
    fn new(slot: usize) -> Self {
        Self {
            cc: u8::try_from(slot + 1).expect("MIDI CC slot fits a CC number"),
            channel: 0,
            target: None,
            last_value: None,
        }
    }

    fn listens_to(&self, change: &MidiControlChange) -> bool {
        self.cc == change.controller && (self.channel == 0 || self.channel == change.channel + 1)
    }
}

pub(super) fn default_midi_cc_mappings() -> [NativeMidiCcMapping; MIDI_CC_SLOT_COUNT] {
    std::array::from_fn(NativeMidiCcMapping::new)
}

/// The slot of a `midiCc.N.target` picker.
pub(super) fn midi_cc_target_slot(target: &str) -> Option<usize> {
    let slot = target
        .strip_prefix("midiCc.")?
        .strip_suffix(".target")?
        .parse::<usize>()
        .ok()?;
    (slot < MIDI_CC_SLOT_COUNT).then_some(slot)
}

pub(super) fn midi_cc_mappings_payload(mappings: &[NativeMidiCcMapping]) -> Value {
    Value::Array(
        mappings
            .iter()
            .map(|mapping| {
                json!({
                    "cc": mapping.cc,
                    "channel": mapping.channel,
                    "target": param_binding_payload(mapping.target.as_ref()),
                })
            })
            .collect(),
    )
}

fn channel_from_menu(value: &str) -> u8 {
    value.parse::<u8>().map_or(0, |channel| channel.min(16))
}

impl NativeRunner {
    pub(super) fn apply_midi_cc_payload(&mut self, runtime: &Value) {
        let Some(slots) = runtime.get("midiCcMappings").and_then(Value::as_array) else {
            return;
        };
        for (mapping, value) in self.midi_cc_mappings.iter_mut().zip(slots) {
            if let Some(cc) = value.get("cc").and_then(Value::as_u64) {
                mapping.cc = u8::try_from(cc.min(127)).expect("clamped CC fits u8");
            }
            if let Some(channel) = value.get("channel").and_then(Value::as_u64) {
                mapping.channel = u8::try_from(channel.min(16)).expect("clamped channel fits u8");
            }
            mapping.target = value.get("target").and_then(param_binding_from_payload);
            mapping.last_value = None;
        }
    }

    pub(super) fn send_midi_control_change(
        &mut self,
        change: MidiControlChange,
    ) -> Result<Vec<RunnerMessage>, String> {
        if !self.midi_enabled {
            return self.messages_without_snapshot();
        }
        let mut changed = false;
        for slot in 0..MIDI_CC_SLOT_COUNT {
            if self.midi_cc_mappings[slot].listens_to(&change) {
                self.midi_cc_mappings[slot].last_value = Some(change.value);
                changed |= self.apply_midi_cc_source(slot);
            }
        }
        if changed {
            if let Err(error) = self.process_dirty_modulation_step(true) {
                self.show_toast(format!("modulation composition unavailable: {error}"));
            }
        }
        self.messages_without_snapshot()
    }

    /// Sets the slot's modulation source from its last value; false without a target or value.
    fn apply_midi_cc_source(&mut self, slot: usize) -> bool {
        let mapping = &self.midi_cc_mappings[slot];
        let (Some(binding), Some(value)) = (mapping.target.clone(), mapping.last_value) else {
            return false;
        };
        let normalized = f64::from(value) / 127.0;
        let normalized = if binding.invert {
            1.0 - normalized
        } else {
            normalized
        };
        let source = ModulationSourceId::midi_cc(slot).expect("validated MIDI CC slot");
        self.set_runtime_source_input(source, binding, normalized)
    }

    /// A new Range or Invert reshapes the held value at once.
    pub(super) fn reapply_midi_cc_source_for_target(&mut self, target: &str) {
        if let Some(slot) = midi_cc_target_slot(target) {
            self.apply_midi_cc_source(slot);
        }
    }

    pub(super) fn set_midi_cc_target(&mut self, slot: usize, binding: Option<NativeParamBinding>) {
        let mapping = &mut self.midi_cc_mappings[slot];
        mapping.target = binding;
        mapping.last_value = None;
        let invert = mapping
            .target
            .as_ref()
            .is_some_and(|binding| binding.invert);
        self.menu
            .set_bool_value_for_key(&format!("midiCc.{slot}.invert"), invert);
    }

    pub(super) fn apply_midi_cc_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        let (slot, field) = key.strip_prefix("midiCc.")?.split_once('.')?;
        let slot = slot
            .parse::<usize>()
            .ok()
            .filter(|slot| *slot < MIDI_CC_SLOT_COUNT)?;
        let changed = match field {
            "cc" => {
                let cc = u8::try_from(self.menu.number_for_key(key)?.clamp(0, 127))
                    .expect("clamped CC fits u8");
                super::menu_apply_fast::value_changed(&mut self.midi_cc_mappings[slot].cc, cc)
            }
            "channel" => {
                let channel = channel_from_menu(&self.menu.value_for_key(key)?);
                super::menu_apply_fast::value_changed(
                    &mut self.midi_cc_mappings[slot].channel,
                    channel,
                )
            }
            "invert" => return Some(self.fast_midi_cc_invert_key(slot, key)),
            _ => return None,
        };
        if changed {
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }

    fn fast_midi_cc_invert_key(&mut self, slot: usize, key: &str) -> bool {
        let Some(invert) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        let target = self.midi_cc_mappings[slot].target.as_mut();
        let changed = target.is_some_and(|binding| {
            let changed = binding.invert != invert;
            binding.invert = invert;
            changed
        });
        let stored = self.midi_cc_mappings[slot]
            .target
            .as_ref()
            .is_some_and(|binding| binding.invert);
        self.menu.set_bool_value_for_key(key, stored);
        if changed {
            self.mark_fast_autosave_dirty();
            if self.apply_midi_cc_source(slot) {
                if let Err(error) = self.process_dirty_modulation_step(false) {
                    self.show_toast(format!("modulation composition unavailable: {error}"));
                }
            }
        }
        true
    }
}
