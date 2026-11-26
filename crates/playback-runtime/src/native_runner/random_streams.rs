use super::menu_apply::fast_values::parse_indexed_key;
use super::{NativeRunner, Value, LAYER_COUNT};
use platform_core::{stream_seed, RandomSeed, RANDOM_DOMAIN_BUILD, RANDOM_DOMAIN_LINK};

pub(super) const RANDOM_SEED_KEY: &str = "randomSeed";
pub(super) const DEFAULT_RANDOM_SEED: u16 = 1;
pub(super) const MAX_RANDOM_SEED: u16 = 9999;

#[derive(Clone, Debug, PartialEq)]
pub(super) struct LinkStreams {
    probability: [u64; LAYER_COUNT],
    arp: [u32; LAYER_COUNT],
}

impl LinkStreams {
    pub(super) fn seeded(global_seed: u16) -> Self {
        Self::from_seeds(|layer| stream_seed(global_seed, RANDOM_DOMAIN_LINK, layer))
    }

    pub(super) fn unseeded() -> Self {
        Self::from_seeds(|layer| unseeded_stream_seed(RANDOM_DOMAIN_LINK, layer))
    }

    fn from_seeds(seed: impl Fn(usize) -> u64) -> Self {
        let probability = std::array::from_fn(seed);
        Self {
            arp: probability.map(arp_state),
            probability,
        }
    }

    fn restart_layer(&mut self, layer: usize, seed: u64) {
        if let Some(state) = self.probability.get_mut(layer) {
            *state = seed;
        }
        if let Some(state) = self.arp.get_mut(layer) {
            *state = arp_state(seed);
        }
    }
}

pub(super) fn unseeded_build_seeds() -> [u64; LAYER_COUNT] {
    std::array::from_fn(fresh_unseeded_build_seed)
}

pub(super) fn fresh_unseeded_build_seed(layer: usize) -> u64 {
    unseeded_stream_seed(RANDOM_DOMAIN_BUILD, layer)
}

#[cfg(not(test))]
fn unseeded_stream_seed(_domain: u64, _layer: usize) -> u64 {
    platform_core::entropy_seed()
}

#[cfg(test)]
thread_local! {
    static TEST_UNSEEDED_DRAWS: std::cell::RefCell<[[u64; LAYER_COUNT]; 2]> =
        const { std::cell::RefCell::new([[0; LAYER_COUNT]; 2]) };
}

/// Tests give each unseeded layer a new stream on every draw, from a sequence of its own that
/// starts over with every runner and stays apart from every Seed's (`1..=9999`), so they replay.
#[cfg(test)]
fn unseeded_stream_seed(domain: u64, layer: usize) -> u64 {
    let draw = TEST_UNSEEDED_DRAWS.with_borrow_mut(|draws| {
        let draw = &mut draws[domain as usize - 1][layer];
        *draw += 1;
        *draw
    });
    stream_seed(0, domain ^ (draw << 8), layer)
}

#[cfg(test)]
pub(super) fn restart_test_unseeded_seeds() {
    TEST_UNSEEDED_DRAWS.with_borrow_mut(|draws| *draws = [[0; LAYER_COUNT]; 2]);
}

fn arp_state(seed: u64) -> u32 {
    ((seed >> 32) as u32) | 1
}

pub(super) fn default_missing_seed_fields(patch: &mut Value) {
    let Some(runtime) = patch
        .get_mut("runtimeConfig")
        .and_then(Value::as_object_mut)
    else {
        return;
    };
    runtime
        .entry(RANDOM_SEED_KEY)
        .or_insert_with(|| DEFAULT_RANDOM_SEED.into());
    let Some(layers) = runtime.get_mut("layers").and_then(Value::as_array_mut) else {
        return;
    };
    for layer in layers.iter_mut().filter_map(Value::as_object_mut) {
        for section in ["build", "link"] {
            if let Some(section) = layer.get_mut(section).and_then(Value::as_object_mut) {
                section.entry("seeded").or_insert(Value::Bool(false));
            }
        }
    }
}

impl NativeRunner {
    pub(super) fn layer_random_seed(&self, layer_index: usize) -> RandomSeed {
        if self
            .layers
            .get(layer_index)
            .is_some_and(|layer| layer.seeded)
        {
            RandomSeed::Seeded(stream_seed(
                self.random_seed,
                RANDOM_DOMAIN_BUILD,
                layer_index,
            ))
        } else {
            RandomSeed::Unseeded(self.unseeded_build_seeds[layer_index])
        }
    }

    fn link_seeded(&self, layer_index: usize) -> bool {
        self.layers
            .get(layer_index)
            .map(|layer| &layer.link)
            .is_some_and(|layer| layer.seeded)
    }

    fn link_streams_mut(&mut self, seeded: bool) -> &mut LinkStreams {
        if seeded {
            &mut self.seeded_link_streams
        } else {
            &mut self.unseeded_link_streams
        }
    }

    pub(super) fn probability_rng(&self, layer_index: usize) -> u64 {
        let streams = if self.link_seeded(layer_index) {
            &self.seeded_link_streams
        } else {
            &self.unseeded_link_streams
        };
        streams.probability[layer_index]
    }

    pub(super) fn store_probability_rng(&mut self, layer_index: usize, rng: u64) {
        let seeded = self.link_seeded(layer_index);
        self.link_streams_mut(seeded).probability[layer_index] = rng;
    }

    /// The Link arp's random walk; `seeded` is the layer's Link Seeded, as routing has the
    /// layer's Link lent out.
    pub(super) fn arp_random_state_mut(&mut self, layer_index: usize, seeded: bool) -> &mut u32 {
        &mut self.link_streams_mut(seeded).arp[layer_index]
    }

    pub(super) fn sync_layer_random_seeds(&mut self) {
        for index in 0..self.layers.len() {
            let seed = self.layer_random_seed(index);
            self.layers[index].engine.set_random_seed(seed);
        }
    }

    pub(super) fn restart_random_streams(&mut self) {
        self.unseeded_build_seeds = unseeded_build_seeds();
        self.sync_layer_random_seeds();
        for layer in &mut self.layers {
            layer.engine.restart_random();
        }
        self.seeded_link_streams = LinkStreams::seeded(self.random_seed);
        self.unseeded_link_streams = LinkStreams::unseeded();
    }

    pub(super) fn restart_link_random_stream(&mut self, layer_index: usize) {
        let seed = stream_seed(self.random_seed, RANDOM_DOMAIN_LINK, layer_index);
        self.seeded_link_streams.restart_layer(layer_index, seed);
        let fresh = unseeded_stream_seed(RANDOM_DOMAIN_LINK, layer_index);
        self.unseeded_link_streams.restart_layer(layer_index, fresh);
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
                self.seeded_link_streams = LinkStreams::seeded(seed);
                self.mark_fast_autosave_dirty();
            }
            return Some(true);
        }
        let (index, suffix) = parse_indexed_key(key.strip_prefix("layers.")?)?;
        if suffix != "build.seeded" {
            return None;
        }
        let seeded = self.menu.value_for_key(key)? == "true";
        let layer = self.layers.get_mut(index)?;
        if layer.seeded != seeded {
            layer.seeded = seeded;
            if !seeded {
                self.unseeded_build_seeds[index] = fresh_unseeded_build_seed(index);
            }
            self.sync_layer_random_seeds();
            self.mark_fast_autosave_dirty();
        }
        Some(true)
    }
}

#[cfg(test)]
mod tests;
