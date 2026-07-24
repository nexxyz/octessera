use platform_core::{BUS_COUNT, GLOBAL_FX_SLOT_COUNT, INSTRUMENT_COUNT, LAYER_COUNT};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum TargetMode {
    Numeric,
    Discrete,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum TargetValueKind {
    Numeric,
    Enum,
    Bool,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Endpoint {
    GlobalControl { key: String },
    LayerControl { layer_index: usize, key: String },
    InstrumentMixer { index: usize },
    InstrumentParameter { index: usize, field: String },
    FxBusMixer { index: usize },
    FxBusSlot { bus_index: usize, slot: usize },
    GlobalFxSlot { slot: usize },
    SparksFx,
}

pub(crate) fn classify_key(key: &str) -> Option<(TargetValueKind, TargetMode, Endpoint)> {
    if let Some(result) = classify_global_key(key) {
        return Some(result);
    }
    if let Some((index, field)) = indexed_key(key, "layers.") {
        return classify_layer_key(index, field);
    }
    if let Some((index, field)) = indexed_key(key, "instruments.") {
        return classify_instrument_key(index, field);
    }
    if let Some((index, field)) = indexed_key(key, "mixer.buses.") {
        return classify_bus_key(index, field);
    }
    if let Some((index, field)) = indexed_key(key, "mixer.master.slots.") {
        return classify_global_fx_key(index, field);
    }
    classify_sparks_key(key).map(|(kind, mode)| (kind, mode, Endpoint::SparksFx))
}

fn classify_global_key(key: &str) -> Option<(TargetValueKind, TargetMode, Endpoint)> {
    let (value_kind, mode) = match key {
        "algorithmStep" => (TargetValueKind::Enum, TargetMode::Discrete),
        "transport.bpm" | "transport.swingPct" => (TargetValueKind::Numeric, TargetMode::Discrete),
        "sound.noteLengthMs" | "sound.velocityScalePct" => {
            (TargetValueKind::Numeric, TargetMode::Discrete)
        }
        "sound.voiceStealingMode" => (TargetValueKind::Enum, TargetMode::Discrete),
        _ => return None,
    };
    Some((
        value_kind,
        mode,
        Endpoint::GlobalControl { key: key.into() },
    ))
}

fn classify_layer_key(
    layer_index: usize,
    field: &str,
) -> Option<(TargetValueKind, TargetMode, Endpoint)> {
    if layer_index >= LAYER_COUNT {
        return None;
    }
    let value_kind = if field == "algorithmStep" {
        TargetValueKind::Enum
    } else if let Some(field) = field.strip_prefix("worlds.behaviorConfig.") {
        super::modulation_target_table::behavior_field_kind(field)?
    } else if let Some(field) = field.strip_prefix("pulses.") {
        super::modulation_target_table::pulses_field_kind(field)?
    } else {
        return None;
    };
    Some((
        value_kind,
        TargetMode::Discrete,
        Endpoint::LayerControl {
            layer_index,
            key: field.into(),
        },
    ))
}

fn classify_instrument_key(
    index: usize,
    field: &str,
) -> Option<(TargetValueKind, TargetMode, Endpoint)> {
    if index >= INSTRUMENT_COUNT {
        return None;
    }
    let (value_kind, additive) = super::modulation_target_table::instrument_field_kind(field)?;
    let endpoint = if matches!(field, "mixer.volume" | "mixer.panPos") {
        Endpoint::InstrumentMixer { index }
    } else {
        Endpoint::InstrumentParameter {
            index,
            field: field.into(),
        }
    };
    Some((
        value_kind,
        if additive {
            TargetMode::Numeric
        } else {
            TargetMode::Discrete
        },
        endpoint,
    ))
}

fn classify_bus_key(index: usize, field: &str) -> Option<(TargetValueKind, TargetMode, Endpoint)> {
    if index >= BUS_COUNT {
        return None;
    }
    if matches!(field, "volume" | "panPos") {
        return Some((
            TargetValueKind::Numeric,
            TargetMode::Numeric,
            Endpoint::FxBusMixer { index },
        ));
    }
    let (slot, value_kind, mode) = if let Some((slot, "type")) = parse_fx_slot_type_field(field) {
        (slot, TargetValueKind::Enum, TargetMode::Discrete)
    } else {
        let (slot, field) = parse_fx_slot_field(field)?;
        let (value_kind, exclusive) = super::modulation_target_table::fx_field_kind(field)?;
        (
            slot,
            value_kind,
            if exclusive {
                TargetMode::Discrete
            } else {
                TargetMode::Numeric
            },
        )
    };
    Some((
        value_kind,
        mode,
        Endpoint::FxBusSlot {
            bus_index: index,
            slot,
        },
    ))
}

fn classify_global_fx_key(
    index: usize,
    field: &str,
) -> Option<(TargetValueKind, TargetMode, Endpoint)> {
    if index >= GLOBAL_FX_SLOT_COUNT {
        return None;
    }
    let (value_kind, mode) = if field == "type" {
        (TargetValueKind::Enum, TargetMode::Discrete)
    } else {
        let field = field.strip_prefix("params.")?;
        let (value_kind, exclusive) = super::modulation_target_table::fx_field_kind(field)?;
        (
            value_kind,
            if exclusive {
                TargetMode::Discrete
            } else {
                TargetMode::Numeric
            },
        )
    };
    Some((value_kind, mode, Endpoint::GlobalFxSlot { slot: index }))
}

fn classify_sparks_key(key: &str) -> Option<(TargetValueKind, TargetMode)> {
    if key == "sparks.fx.type" || key == "sparks.fx.target" {
        return Some((TargetValueKind::Enum, TargetMode::Discrete));
    }
    let field = key.strip_prefix("sparks.fx.params.")?;
    if !super::modulation_target_table::sparks_field_is_known(field) {
        return None;
    }
    Some((
        TargetValueKind::Numeric,
        if super::modulation_target_table::sparks_field_is_exclusive(field) {
            TargetMode::Discrete
        } else {
            TargetMode::Numeric
        },
    ))
}

fn parse_fx_slot_field(field: &str) -> Option<(usize, &str)> {
    let (slot, field) = field.split_once(".params.")?;
    let slot = match slot {
        "slot1" => 0,
        "slot2" => 1,
        "slot3" => 2,
        _ => return None,
    };
    Some((slot, field))
}

fn parse_fx_slot_type_field(field: &str) -> Option<(usize, &str)> {
    let (slot, field) = field.split_once('.')?;
    let slot = match slot {
        "slot1" => 0,
        "slot2" => 1,
        "slot3" => 2,
        _ => return None,
    };
    Some((slot, field))
}

fn indexed_key<'a>(key: &'a str, prefix: &str) -> Option<(usize, &'a str)> {
    let rest = key.strip_prefix(prefix)?;
    let (index, field) = rest.split_once('.')?;
    let parsed = index.parse::<usize>().ok()?;
    (index == parsed.to_string()).then_some((parsed, field))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_is_closed_world_and_endpoint_aware() {
        let volume = classify_key("instruments.0.mixer.volume").unwrap();
        let pan = classify_key("instruments.0.mixer.panPos").unwrap();
        assert_eq!(volume.1, TargetMode::Numeric);
        assert_eq!(volume.2, pan.2);
        assert_eq!(
            classify_key("mixer.buses.0.slot1.params.feedback")
                .unwrap()
                .2,
            classify_key("mixer.buses.0.slot1.params.mixPct").unwrap().2
        );
        assert_eq!(
            classify_key("mixer.buses.0.slot1.params.timeMs").unwrap().1,
            TargetMode::Discrete
        );
        for key in [
            "unknown.target",
            "instruments.0.unknown",
            "instruments.99.mixer.volume",
            "layers.0.pulses.unknown",
            "layers.0.worlds.behaviorConfig.unknown",
        ] {
            assert!(classify_key(key).is_none(), "{key} must be unsupported");
        }
    }
}
