use super::bus_chain_owner::BusChainOwner;
use super::render_routing::FxBusOutputSpreadState;

/// Per-bus mixer state, one entry per bus in every vec. Kept as parallel vecs
/// because the sample loop reads `mono_snapshot` and `volume` across all buses.
#[derive(Clone, Default)]
pub(super) struct BusMixState {
    pub(super) chains: Vec<BusChainOwner>,
    pub(super) pan_pos: Vec<usize>,
    pub(super) pan_gains: Vec<(f32, f32)>,
    pub(super) volume: Vec<f32>,
    pub(super) output_spread_state: Vec<FxBusOutputSpreadState>,
    pub(super) mono_scratch: Vec<f32>,
    pub(super) mono_snapshot: Vec<f32>,
}

impl BusMixState {
    pub(super) fn is_empty(&self) -> bool {
        self.chains.is_empty()
            && self.pan_pos.is_empty()
            && self.pan_gains.is_empty()
            && self.volume.is_empty()
            && self.output_spread_state.is_empty()
            && self.mono_scratch.is_empty()
            && self.mono_snapshot.is_empty()
    }

    /// Carries running chain and spread state over from `previous` by logical bus.
    pub(super) fn preserve_state_from(&mut self, previous: &mut Self) {
        for (next, previous) in self
            .output_spread_state
            .iter_mut()
            .zip(previous.output_spread_state.iter_mut())
        {
            std::mem::swap(next, previous);
        }
        for next in &mut self.chains {
            if let Some(previous) = previous
                .chains
                .iter_mut()
                .find(|previous| previous.logical_bus_id == next.logical_bus_id)
            {
                next.preserve_state_from(previous);
            }
        }
    }
}
