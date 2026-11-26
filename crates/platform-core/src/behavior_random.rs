use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha12Rng;
use std::cell::{Cell, RefCell};

thread_local! {
    static ACTIVE_RNG: RefCell<Option<ChaCha12Rng>> = const { RefCell::new(None) };
    static ACTIVE_SALT: Cell<u64> = const { Cell::new(0) };
}

pub const RANDOM_DOMAIN_BUILD: u64 = 1;
pub const RANDOM_DOMAIN_LINK: u64 = 2;

/// A seed from the operating system's randomness, for a stream that should not repeat.
pub fn entropy_seed() -> u64 {
    rand::random()
}

pub fn stream_seed(global_seed: u16, domain: u64, layer_index: usize) -> u64 {
    let mut value =
        u64::from(global_seed) ^ domain.rotate_left(32) ^ (layer_index as u64).rotate_left(48);
    value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    value ^ (value >> 31)
}

/// A layer's own random stream: `Seeded` follows Seed, hash-style choices included, while
/// `Unseeded` only gives the layer's draws a stream of their own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RandomSeed {
    Seeded(u64),
    Unseeded(u64),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayerRandom {
    seed: Option<u64>,
    salt: u64,
    rng: Option<ChaCha12Rng>,
}

impl LayerRandom {
    pub fn new(seed: Option<u64>) -> Self {
        Self::with_salt(seed, seed.map_or(0, |seed| seed | 1))
    }

    pub fn for_seed(seed: Option<RandomSeed>) -> Self {
        match seed {
            Some(RandomSeed::Unseeded(stream)) => Self::with_salt(Some(stream), 0),
            Some(RandomSeed::Seeded(seed)) => Self::new(Some(seed)),
            None => Self::new(None),
        }
    }

    fn with_salt(seed: Option<u64>, salt: u64) -> Self {
        Self {
            seed,
            salt,
            rng: seed.map(ChaCha12Rng::seed_from_u64),
        }
    }

    pub fn random_seed(&self) -> Option<RandomSeed> {
        self.seed.map(|seed| match self.salt {
            0 => RandomSeed::Unseeded(seed),
            _ => RandomSeed::Seeded(seed),
        })
    }

    pub fn restart(&mut self) {
        self.rng = self.seed.map(ChaCha12Rng::seed_from_u64);
    }

    pub fn scope<R>(&mut self, apply: impl FnOnce() -> R) -> R {
        let rng = self.rng.take();
        let _guard = ScopeGuard {
            previous_salt: ACTIVE_SALT.replace(self.salt),
            owner: self,
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

/// A repeatable `0..100` roll for tile `index` on `tick`, varied by `salt` per use and by the
/// layer's random stream.
pub(crate) fn tile_roll_pct(tick: u64, index: usize, salt: u64) -> u32 {
    let tick = tick ^ self::salt();
    let mut x = tick
        .wrapping_mul(0x9E37_79B9)
        .wrapping_add(index as u64 * 0x85EB_CA6B)
        .wrapping_add(salt * 0xC2B2_AE35);
    x ^= x >> 16;
    ((x.wrapping_mul(0x27D4_EB2D) >> 24) % 100) as u32
}

/// A well-mixed repeatable hash of `tick` and tile `index` in the layer's random stream: every
/// bit depends on both, so neighboring tiles and ticks do not fall into bands or columns.
pub(crate) fn tile_hash(tick: u64, index: usize) -> u64 {
    let mut x = (tick ^ salt()).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (index as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// `pct` percent of `full_scale`, rounded up or down by a repeatable roll so the average stays
/// exact: every percent counts, even where the whole-number scale has only a few steps.
pub(crate) fn dithered_pct(pct: u8, full_scale: i16, tick: u64, index: usize, salt: u64) -> i16 {
    (i16::from(pct) * full_scale + tile_roll_pct(tick, index, salt) as i16) / 100
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
        None => draw_unscoped(draw),
    })
}

#[cfg(not(test))]
fn draw_unscoped<R>(draw: impl FnOnce(&mut dyn RngCore) -> R) -> R {
    draw(&mut rand::thread_rng())
}

/// Unit tests that call a behavior outside a layer's random stream draw from a fixed seed per
/// test thread, so a random reseed can't make a test pass or fail by chance.
#[cfg(test)]
fn draw_unscoped<R>(draw: impl FnOnce(&mut dyn RngCore) -> R) -> R {
    thread_local! {
        static UNSCOPED_TEST_RNG: RefCell<ChaCha12Rng> = RefCell::new(ChaCha12Rng::seed_from_u64(0));
    }
    UNSCOPED_TEST_RNG.with(|rng| draw(&mut *rng.borrow_mut()))
}

#[cfg(test)]
mod tests;
