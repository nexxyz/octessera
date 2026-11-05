use super::super::menu_apply_fast::value_changed;
use super::super::midi_cc_in::midi_cc_target_slot;
use super::super::modulation_source::{ModulationAxis, ModulationSourceId};
use super::super::play_pages::faders::fader_target_side;
use super::super::{NativeParamBinding, NativeRunner, GRID_HEIGHT};

/// A parameter picker's binding target, parsed once from its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::native_runner) enum BindingTarget {
    LayerAxis {
        layer: usize,
        axis: ModulationAxis,
        slot: usize,
    },
    PlayX,
    PlayY,
    MidiCc(usize),
    Fader {
        column: usize,
        shifted: bool,
    },
    Lfo(usize),
}

impl BindingTarget {
    pub(super) fn parse(target: &str) -> Option<Self> {
        if let Some(rest) = target.strip_prefix("param:") {
            let mut parts = rest.split(':');
            let (Some(layer), Some(axis), Some(slot), None) =
                (parts.next(), parts.next(), parts.next(), parts.next())
            else {
                return None;
            };
            let axis = match axis {
                "x" => ModulationAxis::X,
                "y" => ModulationAxis::Y,
                _ => return None,
            };
            return Some(Self::LayerAxis {
                layer: layer.parse().ok()?,
                axis,
                slot: slot.parse().ok()?,
            });
        }
        match target {
            "xy:x" => return Some(Self::PlayX),
            "xy:y" => return Some(Self::PlayY),
            _ => {}
        }
        if let Some(slot) = midi_cc_target_slot(target) {
            return Some(Self::MidiCc(slot));
        }
        if let Some((column, shifted)) = fader_target_side(target) {
            return Some(Self::Fader { column, shifted });
        }
        target
            .strip_prefix("linkLfos.")?
            .strip_suffix(".target")?
            .parse()
            .ok()
            .map(Self::Lfo)
    }

    fn runtime_source(self) -> Option<ModulationSourceId> {
        match self {
            Self::LayerAxis { layer, axis, slot } => {
                ModulationSourceId::layer_axis(layer, axis, slot).ok()
            }
            Self::PlayX => Some(ModulationSourceId::play_x()),
            Self::PlayY => Some(ModulationSourceId::play_y()),
            Self::MidiCc(slot) => {
                Some(ModulationSourceId::midi_cc(slot).expect("validated MIDI CC slot"))
            }
            Self::Fader { column, shifted } => {
                Some(ModulationSourceId::fader(column, shifted).expect("validated Faders column"))
            }
            Self::Lfo(_) => None,
        }
    }
}

impl NativeRunner {
    /// Where a target's binding is stored; a layer axis clamps to the last layer and slot 2.
    pub(super) fn binding_slot_mut(
        &mut self,
        target: BindingTarget,
    ) -> Option<&mut Option<NativeParamBinding>> {
        match target {
            BindingTarget::LayerAxis { layer, axis, slot } => {
                let mods = self.param_mods.get_mut(layer.min(GRID_HEIGHT - 1))?;
                let lanes = match axis {
                    ModulationAxis::X => &mut mods.x,
                    ModulationAxis::Y => &mut mods.y,
                };
                lanes.get_mut(slot.min(1))
            }
            BindingTarget::PlayX => Some(&mut self.xy_x_binding),
            BindingTarget::PlayY => Some(&mut self.xy_y_binding),
            BindingTarget::MidiCc(slot) => Some(&mut self.midi_cc_mappings[slot].target),
            BindingTarget::Fader { column, shifted } => {
                Some(self.fader_target_mut(column, shifted))
            }
            BindingTarget::Lfo(index) => Some(&mut self.link_lfos.get_mut(index)?.target),
        }
    }

    pub(super) fn clear_runtime_source_for_binding_target(&mut self, target: BindingTarget) {
        if let Some(source) = target.runtime_source() {
            self.clear_runtime_source_input(source);
        }
    }

    /// Sets a binding's invert flag from its menu row and shows the stored flag on the row, so a
    /// row without a binding stays off.
    pub(in crate::native_runner) fn fast_binding_invert_key(
        &mut self,
        target: BindingTarget,
        key: &str,
    ) -> bool {
        let Some(invert) = self.menu.value_for_key(key).map(|value| value == "true") else {
            return false;
        };
        let Some(binding) = self.binding_slot_mut(target) else {
            return false;
        };
        let changed = binding
            .as_mut()
            .is_some_and(|binding| value_changed(&mut binding.invert, invert));
        let stored = binding.as_ref().is_some_and(|binding| binding.invert);
        self.menu.set_bool_value_for_key(key, stored);
        if !changed {
            return true;
        }
        self.mark_fast_autosave_dirty();
        let source_changed = match target {
            BindingTarget::MidiCc(slot) => self.apply_midi_cc_source(slot),
            BindingTarget::Fader { column, shifted } => self.apply_fader_source(column, shifted),
            _ => true,
        };
        if source_changed {
            if let Err(error) = self.process_dirty_modulation_step(false) {
                self.show_toast(format!("modulation composition unavailable: {error}"));
            }
        }
        true
    }
}
