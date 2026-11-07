//! The default-init Activate cadence of every behavior. The desktop e2e suite reads this file
//! through `#[path]` to hold the device path to the same table.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Expect {
    /// Activates within every window of this many ticks.
    Within(usize),
    /// Never activates without input.
    Silent,
}
use Expect::{Silent, Within};

pub(crate) struct Row {
    pub(crate) id: &'static str,
    pub(crate) expect: Expect,
    pub(crate) why: &'static str,
}

const fn row(id: &'static str, expect: Expect, why: &'static str) -> Row {
    Row { id, expect, why }
}

/// How each behavior sounds from its default init with no input. `Within` windows are about twice
/// the longest Activate-free gap of a 200-salt, 3000-tick sweep, rounded to the behavior's cadence.
pub(crate) const CADENCE: &[Row] = &[
    row("none", Silent, "nothing to run"),
    row(
        "sequencer",
        Silent,
        "plays only what is drawn; a default canvas is empty",
    ),
    row("keys", Silent, "plays only what is pressed"),
    row("looper", Silent, "plays only what is recorded"),
    row("life", Within(4), "a generation always flips some cell on"),
    row(
        "brain",
        Within(4),
        "firing cells hand on to their neighbors every tick",
    ),
    row(
        "cyclic",
        Within(4),
        "every state keeps eating its successor",
    ),
    row(
        "forest_fire",
        Within(300),
        "a fire burns out and the forest regrows until lightning or a reseed",
    ),
    row(
        "predator_prey",
        Within(4),
        "animals move, breed and reseed every tick",
    ),
    row("twinkle", Within(64), "sparse random twinkles"),
    row("ant", Within(8), "the ant flips a tile every step"),
    row("bounce", Within(8), "balls keep moving"),
    row("bubbles", Within(16), "bubbles rise and new ones spawn"),
    row("gravity", Within(16), "grains fall and new ones spawn"),
    row("boids", Within(8), "the flock keeps moving"),
    row("lava_lamp", Within(24), "blobs drift slowly"),
    row("orbit", Within(12), "bodies keep orbiting"),
    row("sand_ripples", Within(8), "dunes keep shifting"),
    row(
        "fractal_explorer",
        Within(64),
        "the view drifts and zooms between class changes",
    ),
    row(
        "maze_growth",
        Within(4),
        "carving and walkers move every tick",
    ),
    row("shapes", Within(4), "geometric pulses every tick"),
    row("ink", Within(32), "drops land on a 16-tick cadence"),
    row(
        "ising",
        Within(8),
        "spins keep flipping at the default temperature",
    ),
    row(
        "kuramoto",
        Within(12),
        "oscillators keep crossing their phase",
    ),
    row("lightning", Within(8), "bolts strike every few ticks"),
    row("raindrops", Within(4), "rain falls every tick"),
    row(
        "reaction_diffusion",
        Within(8),
        "scheduled splashes keep the pattern moving",
    ),
    row("rivers", Within(4), "rain falls every tick"),
    row("wave", Within(8), "impulses keep travelling"),
    row(
        "cracks",
        Within(8),
        "a tip moves or retires every tick; only a shatter's few-tick dissolve is quiet",
    ),
    row("coral", Within(48), "colonies grow, break off and regrow"),
    row("crystal_growth", Within(8), "clusters grow and reseed"),
    row("dla", Within(8), "walkers keep sticking"),
    row("physarum", Within(8), "agents keep moving"),
    row("vines", Within(32), "tendrils grow, branch and prune back"),
    row(
        "pong",
        Within(6),
        "the ball moves every step; a miss pauses only two steps before the serve",
    ),
    row(
        "blockout",
        Within(6),
        "the ball moves every step; a drop pauses only two steps before the serve",
    ),
    row(
        "weave",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "polyrhythm",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "breaks",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "fills",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "clave",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "groove",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "euclid",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "ostinato",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "motif",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "canon",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "chords",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "contour",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "cadence",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
    row(
        "phrase",
        Within(32),
        "a pattern that sounds within every two 16-step bars",
    ),
];
