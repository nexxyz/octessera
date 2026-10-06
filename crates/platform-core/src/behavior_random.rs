use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::cell::{Cell, RefCell};

thread_local! {
    static ACTIVE_RNG: RefCell<Option<ChaCha12Rng>> = const { RefCell::new(None) };
    static ACTIVE_SALT: Cell<u64> = const { Cell::new(0) };
}

pub const RANDOM_DOMAIN_BUILD: u64 = 1;
pub const RANDOM_DOMAIN_LINK: u64 = 2;

pub fn stream_seed(global_seed: u16, domain: u64, layer_index: usize) -> u64 {
    let mut value =
        u64::from(global_seed) ^ domain.rotate_left(32) ^ (layer_index as u64).rotate_left(48);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayerRandom {
    seed: Option<u64>,
    rng: Option<ChaCha12Rng>,
}

impl LayerRandom {
    pub fn new(seed: Option<u64>) -> Self {
        Self {
            seed,
            rng: seed.map(ChaCha12Rng::seed_from_u64),
        }
    }

    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    pub fn restart(&mut self) {
        self.rng = self.seed.map(ChaCha12Rng::seed_from_u64);
    }

    pub fn scope<R>(&mut self, apply: impl FnOnce() -> R) -> R {
        let salt = self.seed.map_or(0, |seed| seed | 1);
        let rng = self.rng.take();
        let _guard = ScopeGuard {
            owner: self,
            previous_salt: ACTIVE_SALT.replace(salt),
            previous_rng: ACTIVE_RNG.with(|active| active.replace(rng)),
        };
        apply()
    }
}

struct ScopeGuard<'a> {
    owner: &'a mut LayerRandom,
    previous_salt: u64,
    previous_rng: Option<ChaCha12Rng>,
}

impl Drop for ScopeGuard<'_> {
    fn drop(&mut self) {
        let previous_rng = self.previous_rng.take();
        self.owner.rng = ACTIVE_RNG.with(|active| active.replace(previous_rng));
        ACTIVE_SALT.set(self.previous_salt);
    }
}

pub struct BehaviorRng;

pub fn rng() -> BehaviorRng {
    BehaviorRng
}

pub fn salt() -> u64 {
    ACTIVE_SALT.get()
}

impl RngCore for BehaviorRng {
    fn next_u32(&mut self) -> u32 {
        with_active(|rng| rng.next_u32())
    }

    fn next_u64(&mut self) -> u64 {
        with_active(|rng| rng.next_u64())
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        with_active(|rng| rng.fill_bytes(dest));
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), rand::Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}

fn with_active<R>(draw: impl FnOnce(&mut dyn RngCore) -> R) -> R {
    ACTIVE_RNG.with(|active| match active.borrow_mut().as_mut() {
        Some(seeded) => draw(seeded),
        None => draw(&mut rand::thread_rng()),
    })
}

#[cfg(test)]
mod tests;
