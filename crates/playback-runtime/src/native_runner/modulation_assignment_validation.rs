use super::modulation_process::target_kind_for_binding;
use super::modulation_source::{ModulationAxis, ModulationSourceId};
use super::{NativeParamBinding, NativeRunner};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BindingChange {
    pub(super) key: String,
    pub(super) binding: Option<NativeParamBinding>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum BindingValidationError {
    UnsupportedTarget,
    KindMismatch,
    LfoTargetNotLive,
    AssignmentClaim,
}

impl BindingValidationError {
    pub(super) fn toast_message(&self) -> &'static str {
        match self {
            Self::UnsupportedTarget => "Mapping rejected: unsupported target",
            Self::KindMismatch => "Mapping rejected: target type mismatch",
            Self::LfoTargetNotLive => "Mapping rejected: LFO needs a live numeric target",
            Self::AssignmentClaim => "Mapping rejected: target already claimed",
        }
    }
}

pub(super) fn validate_binding_changes(
    runner: &NativeRunner,
    changes: &[BindingChange],
) -> Result<(), BindingValidationError> {
    let mut claims = BTreeMap::<String, ModulationSourceId>::new();
    for (source, binding) in current_bindings(runner, changes)? {
        let Some(binding) = binding else {
            continue;
        };
        let (mode, kind) =
            target_kind_for_binding(&binding).map_err(|_| BindingValidationError::KindMismatch)?;
        if mode == super::modulation_target::TargetMode::Discrete
            && claims.insert(binding.key.clone(), source).is_some()
        {
            return Err(BindingValidationError::AssignmentClaim);
        }
        if source.is_global_lfo()
            && (!super::modulation_audio::is_live_link_lfo_target(&binding.key) || kind != "number")
        {
            return Err(BindingValidationError::LfoTargetNotLive);
        }
    }
    Ok(())
}

fn current_bindings(
    runner: &NativeRunner,
    changes: &[BindingChange],
) -> Result<Vec<(ModulationSourceId, Option<NativeParamBinding>)>, BindingValidationError> {
    for change in changes {
        if change.binding.is_some()
            && target_kind_for_binding(change.binding.as_ref().unwrap()).is_err()
        {
            return Err(BindingValidationError::UnsupportedTarget);
        }
    }
    let mut bindings = Vec::new();
    for (layer_index, param_mods) in runner.param_mods.iter().enumerate() {
        for (axis, values) in [
            (ModulationAxis::X, &param_mods.x),
            (ModulationAxis::Y, &param_mods.y),
        ] {
            for (slot, binding) in values.iter().enumerate() {
                let key = format!("param:{layer_index}:{}:{slot}", axis_label(axis));
                if let Some(binding) = changed_binding(&key, binding.as_ref(), changes) {
                    bindings.push((
                        ModulationSourceId::layer_axis(layer_index, axis, slot)
                            .map_err(|_| BindingValidationError::UnsupportedTarget)?,
                        binding,
                    ));
                }
            }
        }
    }
    for (key, source, binding) in [
        (
            "xy:x",
            ModulationSourceId::play_x(),
            runner.xy_x_binding.as_ref(),
        ),
        (
            "xy:y",
            ModulationSourceId::play_y(),
            runner.xy_y_binding.as_ref(),
        ),
    ] {
        if let Some(binding) = changed_binding(key, binding, changes) {
            bindings.push((source, binding));
        }
    }
    for (index, lfo) in runner.link_lfos.iter().enumerate() {
        let key = format!("linkLfos.{index}.target");
        if let Some(binding) = changed_binding(&key, lfo.target.as_ref(), changes) {
            bindings.push((
                ModulationSourceId::global_lfo(index)
                    .map_err(|_| BindingValidationError::UnsupportedTarget)?,
                binding,
            ));
        }
    }
    Ok(bindings)
}

fn changed_binding<'a>(
    key: &str,
    current: Option<&'a NativeParamBinding>,
    changes: &'a [BindingChange],
) -> Option<Option<NativeParamBinding>> {
    changes
        .iter()
        .find(|change| change.key == key)
        .map(|change| change.binding.clone())
        .or_else(|| current.cloned().map(Some))
}

fn axis_label(axis: ModulationAxis) -> &'static str {
    match axis {
        ModulationAxis::X => "x",
        ModulationAxis::Y => "y",
    }
}
