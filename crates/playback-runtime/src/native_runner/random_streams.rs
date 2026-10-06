use super::menu_apply_fast_values::parse_indexed_key;
use super::{NativeRunner, Value, LAYER_COUNT};
use platform_core::{stream_seed, RANDOM_DOMAIN_BUILD, RANDOM_DOMAIN_LINK};

pub(super) const RANDOM_SEED_KEY: &str = "randomSeed";
pub(super) const DEFAULT_RANDOM_SEED: u16 = 1;
pub(super) const MAX_RANDOM_SEED: u16 = 9999;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct SeededLinkStreams {
    probability: Vec<u64>,
    arp: Vec<u32>,
}

impl SeededLinkStreams {
    pub(super) fn new(global_seed: u16) -> Self {
        let seeds = (0..LAYER_COUNT)
            .map(|layer| stream_seed(global_seed, RANDOM_DOMAIN_LINK, layer))
            .collect::<Vec<_>>();
        Self {
            probability: seeds.clone(),
            arp: seeds.iter().map(|seed| arp_state(*seed)).collect(),
        }
    }

    fn restart_layer(&mut self, global_seed: u16, layer: usize) {
        let seed = stream_seed(global_seed, RANDOM_DOMAIN_LINK, layer);
        if let Some(state) = self.probability.get_mut(layer) {
            *state = seed;
        }
        if let Some(state) = self.arp.get_mut(layer) {
            *state = arp_state(seed);
        }
    }
}

fn arp_state(seed: u64) -> u32 {
    ((seed >> 32) as u32) | 1
}

pub(super) fn random_seed_from_payload(runtime: &Value) -> Option<u16> {
    runtime
        .get(RANDOM_SEED_KEY)
        .and_then(Value::as_u64)
        .map(|seed| seed.clamp(1, u64::from(MAX_RANDOM_SEED)) as u16)
}

impl NativeRunner {
    pub(super) fn layer_random_seed(&self, layer_index: usize) -> Option<u64> {
        self.layer_seeded
            .get(layer_index)
            .copied()
            .unwrap_or(false)
            .then(|| stream_seed(self.random_seed, RANDOM_DOMAIN_BUILD, layer_index))
    }

    fn link_seeded(&self, layer_index: usize) -> bool {
        self.link_layers
            .get(layer_index)
            .is_some_and(|layer| layer.seeded)
    }

    pub(super) fn probability_rng(&self, layer_index: usize) -> u64 {
        if self.link_seeded(layer_index) {
            self.seeded_link_streams.probability[layer_index]
        } else {
            self.trigger_probability_rng
        }
    }

    pub(super) fn store_probability_rng(&mut self, layer_index: usize, rng: u64) {
        if self.link_seeded(layer_index) {
            self.seeded_link_streams.probability[layer_index] = rng;
        } else {
            self.trigger_probability_rng = rng;
        }
    }

    pub(super) fn arp_random_state_mut(&mut self, layer_index: usize) -> &mut u32 {
        if self.link_seeded(layer_index) {
            &mut self.seeded_link_streams.arp[layer_index]
        } else {
            &mut self.link_arp_random_state
        }
    }

    pub(super) fn sync_layer_random_seeds(&mut self) {
        let active_seed = self.layer_random_seed(self.active_layer_index);
        self.engine.set_random_seed(active_seed);
        for index in 0..self.layer_engines.len() {
            let seed = self.layer_random_seed(index);
            if let Some(engine) = self.layer_engines[index].as_mut() {
                engine.set_random_seed(seed);
            }
        }
    }

    pub(super) fn restart_random_streams(&mut self) {
        self.sync_layer_random_seeds();
        self.engine.restart_random();
        for engine in self.layer_engines.iter_mut().flatten() {
            engine.restart_random();
        }
        self.seeded_link_streams = SeededLinkStreams::new(self.random_seed);
    }

    pub(super) fn restart_link_random_stream(&mut self, layer_index: usize) {
        self.seeded_link_streams
            .restart_layer(self.random_seed, layer_index);
    }

    pub(super) fn apply_random_menu_key_fast(&mut self, key: &str) -> Option<bool> {
        if key == RANDOM_SEED_KEY {
            let seed = self
                .menu
                .number_for_key(key)?
                .clamp(1, i32::from(MAX_RANDOM_SEED)) as u16;
            if seed != self.random_seed {
                self.random_seed = seed;
                self.sync_layer_random_seeds();
                self.seeded_link_streams = SeededLinkStreams::new(seed);
                self.mark_fast_autosave_dirty();
            }
            return Some(true);
        }
        let (index, suffix) = parse_indexed_key(key.strip_prefix("layers.")?)?;
        if suffix != "build.seeded" {
            return None;
        }
        let seeded = self.menu.value_for_key(key)? == "true";
        let target = self.layer_seeded.get_mut(index)?;
        if *target != seeded {
            *target = seeded;
            self.sync_layer_random_seeds();
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }
}

#[cfg(test)]
mod tests;
