use platform_core::LAYER_COUNT;

const AXIS_SLOT_COUNT: usize = 2;
const GLOBAL_LFO_SOURCE_COUNT: usize = 8;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum ModulationAxis {
    X,
    Y,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct ModulationSourceId(usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ModulationSourceError {
    LayerIndex,
    AxisSlot,
    LfoSlot,
}

impl ModulationSourceId {
    #[cfg(test)]
    pub(crate) const COUNT: usize = LAYER_COUNT * AXIS_SLOT_COUNT * 2 + 2 + GLOBAL_LFO_SOURCE_COUNT;

    pub(crate) fn layer_axis(
        layer_index: usize,
        axis: ModulationAxis,
        slot: usize,
    ) -> Result<Self, ModulationSourceError> {
        if layer_index >= LAYER_COUNT {
            return Err(ModulationSourceError::LayerIndex);
        }
        if slot >= AXIS_SLOT_COUNT {
            return Err(ModulationSourceError::AxisSlot);
        }
        let axis_offset = match axis {
            ModulationAxis::X => 0,
            ModulationAxis::Y => AXIS_SLOT_COUNT,
        };
        Ok(Self(layer_index * AXIS_SLOT_COUNT * 2 + axis_offset + slot))
    }

    pub(crate) fn global_lfo(slot: usize) -> Result<Self, ModulationSourceError> {
        if slot >= GLOBAL_LFO_SOURCE_COUNT {
            return Err(ModulationSourceError::LfoSlot);
        }
        Ok(Self(LAYER_COUNT * AXIS_SLOT_COUNT * 2 + 2 + slot))
    }

    pub(crate) const fn play_x() -> Self {
        Self(LAYER_COUNT * AXIS_SLOT_COUNT * 2)
    }

    pub(crate) const fn play_y() -> Self {
        Self(LAYER_COUNT * AXIS_SLOT_COUNT * 2 + 1)
    }

    pub(crate) fn is_global_lfo(self) -> bool {
        self.0 >= LAYER_COUNT * AXIS_SLOT_COUNT * 2 + 2
    }

    #[cfg(test)]
    pub(crate) fn all() -> Vec<Self> {
        (0..Self::COUNT)
            .filter_map(Self::from_stable_index)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn from_stable_index(index: usize) -> Option<Self> {
        (index < Self::COUNT).then_some(Self(index))
    }

    #[cfg(test)]
    pub(crate) fn stable_index(self) -> usize {
        self.0
    }

    #[cfg(test)]
    pub(crate) fn label(self) -> String {
        if self.0 < LAYER_COUNT * AXIS_SLOT_COUNT * 2 {
            let layer_index = self.0 / (AXIS_SLOT_COUNT * 2);
            let axis_slot = self.0 % (AXIS_SLOT_COUNT * 2);
            let axis = if axis_slot < AXIS_SLOT_COUNT {
                'X'
            } else {
                'Y'
            };
            return format!(
                "L{} {}{}",
                layer_index + 1,
                axis,
                axis_slot % AXIS_SLOT_COUNT + 1
            );
        }
        match self.0 - LAYER_COUNT * AXIS_SLOT_COUNT * 2 {
            0 => "Play X".into(),
            1 => "Play Y".into(),
            slot => format!("LFO {}", slot - 1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_order_is_complete_and_stable() {
        let sources = ModulationSourceId::all();
        assert_eq!(sources.len(), ModulationSourceId::COUNT);
        assert_eq!(sources[0].label(), "L1 X1");
        assert_eq!(sources[31].label(), "L8 Y2");
        assert_eq!(sources[32].label(), "Play X");
        assert_eq!(sources[33].label(), "Play Y");
        assert_eq!(sources[34].label(), "LFO 1");
        assert_eq!(sources[41].label(), "LFO 8");
        for (index, source) in sources.into_iter().enumerate() {
            assert_eq!(source.stable_index(), index);
            assert_eq!(ModulationSourceId::from_stable_index(index), Some(source));
        }
    }

    #[test]
    fn source_constructors_reject_invalid_identity_parts() {
        assert_eq!(
            ModulationSourceId::layer_axis(LAYER_COUNT, ModulationAxis::X, 0),
            Err(ModulationSourceError::LayerIndex)
        );
        assert_eq!(
            ModulationSourceId::layer_axis(0, ModulationAxis::X, AXIS_SLOT_COUNT),
            Err(ModulationSourceError::AxisSlot)
        );
        assert_eq!(
            ModulationSourceId::global_lfo(GLOBAL_LFO_SOURCE_COUNT),
            Err(ModulationSourceError::LfoSlot)
        );
    }
}
