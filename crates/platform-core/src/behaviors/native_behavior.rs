use super::cellular::LifeState;
use super::games::blockout::BlockoutState;
use super::games::pong::PongState;
use super::native_impl::{
    AntState, BoidsState, BounceState, BrainState, BubblesState, CoralState, CracksState,
    CrystalGrowthState, CyclicState, DlaState, ForestFireState, FractalExplorerState, GravityState,
    InkState, IsingState, KeysState, KuramotoState, LavaLampState, LightningState, LooperState,
    MazeGrowthState, OrbitState, PhysarumState, PredatorPreyState, RaindropsState,
    ReactionDiffusionState, RiversState, SandRipplesState, ShapesState, TwinkleState, VinesState,
    WaveState,
};
use super::play::{NoneState, SequencerState};
use super::PatternBehaviorState;

#[derive(Clone, Debug, PartialEq)]
pub enum NativeBehaviorState {
    None(NoneState),
    Life(LifeState),
    Sequencer(SequencerState),
    Keys(KeysState),
    Looper(LooperState),
    Brain(BrainState),
    Cyclic(CyclicState),
    ForestFire(ForestFireState),
    PredatorPrey(PredatorPreyState),
    Twinkle(TwinkleState),
    Ant(AntState),
    Boids(BoidsState),
    Bounce(BounceState),
    Bubbles(BubblesState),
    Gravity(GravityState),
    LavaLamp(LavaLampState),
    Orbit(OrbitState),
    SandRipples(SandRipplesState),
    FractalExplorer(FractalExplorerState),
    MazeGrowth(MazeGrowthState),
    Shapes(ShapesState),
    Ink(InkState),
    Ising(IsingState),
    Kuramoto(KuramotoState),
    Lightning(LightningState),
    Wave(WaveState),
    Raindrops(RaindropsState),
    ReactionDiffusion(ReactionDiffusionState),
    Rivers(RiversState),
    Cracks(CracksState),
    Coral(CoralState),
    CrystalGrowth(CrystalGrowthState),
    Dla(DlaState),
    Physarum(PhysarumState),
    Vines(VinesState),
    Pong(PongState),
    Blockout(BlockoutState),
    Pattern(PatternBehaviorState),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeBehavior {
    None,
    Life,
    Sequencer,
    Keys,
    Looper,
    Brain,
    Cyclic,
    ForestFire,
    PredatorPrey,
    Twinkle,
    Ant,
    Boids,
    Bounce,
    Bubbles,
    Gravity,
    LavaLamp,
    Orbit,
    SandRipples,
    FractalExplorer,
    MazeGrowth,
    Shapes,
    Ink,
    Ising,
    Kuramoto,
    Lightning,
    Wave,
    Raindrops,
    ReactionDiffusion,
    Rivers,
    Cracks,
    Coral,
    CrystalGrowth,
    Dla,
    Physarum,
    Vines,
    Pong,
    Blockout,
    Weave,
    Polyrhythm,
    Breaks,
    Fills,
    Clave,
    Groove,
    Euclid,
    Ostinato,
    Motif,
    Canon,
    Chords,
    Contour,
    Cadence,
    Phrase,
}

pub fn get_native_behavior(id: &str) -> Option<NativeBehavior> {
    NativeBehavior::ALL
        .into_iter()
        .find(|behavior| behavior.id() == id)
}

pub fn list_native_behavior_ids() -> &'static [&'static str] {
    &NATIVE_BEHAVIOR_IDS
}

const NATIVE_BEHAVIOR_IDS: [&str; NativeBehavior::ALL.len()] = {
    let mut ids = [""; NativeBehavior::ALL.len()];
    let mut index = 0;
    while index < ids.len() {
        ids[index] = NativeBehavior::ALL[index].id();
        index += 1;
    }
    ids
};

impl NativeBehavior {
    /// Every native behavior in registry order.
    pub const ALL: [NativeBehavior; 51] = [
        NativeBehavior::None,
        NativeBehavior::Life,
        NativeBehavior::Sequencer,
        NativeBehavior::Keys,
        NativeBehavior::Looper,
        NativeBehavior::Brain,
        NativeBehavior::Cyclic,
        NativeBehavior::ForestFire,
        NativeBehavior::PredatorPrey,
        NativeBehavior::Twinkle,
        NativeBehavior::Ant,
        NativeBehavior::Bounce,
        NativeBehavior::Bubbles,
        NativeBehavior::Gravity,
        NativeBehavior::Boids,
        NativeBehavior::LavaLamp,
        NativeBehavior::Orbit,
        NativeBehavior::SandRipples,
        NativeBehavior::FractalExplorer,
        NativeBehavior::MazeGrowth,
        NativeBehavior::Shapes,
        NativeBehavior::Ink,
        NativeBehavior::Ising,
        NativeBehavior::Kuramoto,
        NativeBehavior::Lightning,
        NativeBehavior::Raindrops,
        NativeBehavior::ReactionDiffusion,
        NativeBehavior::Rivers,
        NativeBehavior::Wave,
        NativeBehavior::Cracks,
        NativeBehavior::Coral,
        NativeBehavior::CrystalGrowth,
        NativeBehavior::Dla,
        NativeBehavior::Physarum,
        NativeBehavior::Vines,
        NativeBehavior::Pong,
        NativeBehavior::Blockout,
        NativeBehavior::Weave,
        NativeBehavior::Polyrhythm,
        NativeBehavior::Breaks,
        NativeBehavior::Fills,
        NativeBehavior::Clave,
        NativeBehavior::Groove,
        NativeBehavior::Euclid,
        NativeBehavior::Ostinato,
        NativeBehavior::Motif,
        NativeBehavior::Canon,
        NativeBehavior::Chords,
        NativeBehavior::Contour,
        NativeBehavior::Cadence,
        NativeBehavior::Phrase,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            NativeBehavior::None => "none",
            NativeBehavior::Life => "life",
            NativeBehavior::Sequencer => "sequencer",
            NativeBehavior::Keys => "keys",
            NativeBehavior::Looper => "looper",
            NativeBehavior::Brain => "brain",
            NativeBehavior::Cyclic => "cyclic",
            NativeBehavior::ForestFire => "forest_fire",
            NativeBehavior::PredatorPrey => "predator_prey",
            NativeBehavior::Twinkle => "twinkle",
            NativeBehavior::Ant => "ant",
            NativeBehavior::Boids => "boids",
            NativeBehavior::Bounce => "bounce",
            NativeBehavior::Bubbles => "bubbles",
            NativeBehavior::Gravity => "gravity",
            NativeBehavior::LavaLamp => "lava_lamp",
            NativeBehavior::Orbit => "orbit",
            NativeBehavior::SandRipples => "sand_ripples",
            NativeBehavior::FractalExplorer => "fractal_explorer",
            NativeBehavior::MazeGrowth => "maze_growth",
            NativeBehavior::Shapes => "shapes",
            NativeBehavior::Ink => "ink",
            NativeBehavior::Ising => "ising",
            NativeBehavior::Kuramoto => "kuramoto",
            NativeBehavior::Lightning => "lightning",
            NativeBehavior::Wave => "wave",
            NativeBehavior::Raindrops => "raindrops",
            NativeBehavior::ReactionDiffusion => "reaction_diffusion",
            NativeBehavior::Rivers => "rivers",
            NativeBehavior::Cracks => "cracks",
            NativeBehavior::Coral => "coral",
            NativeBehavior::CrystalGrowth => "crystal_growth",
            NativeBehavior::Dla => "dla",
            NativeBehavior::Physarum => "physarum",
            NativeBehavior::Vines => "vines",
            NativeBehavior::Pong => "pong",
            NativeBehavior::Blockout => "blockout",
            NativeBehavior::Weave => "weave",
            NativeBehavior::Polyrhythm => "polyrhythm",
            NativeBehavior::Breaks => "breaks",
            NativeBehavior::Fills => "fills",
            NativeBehavior::Clave => "clave",
            NativeBehavior::Groove => "groove",
            NativeBehavior::Euclid => "euclid",
            NativeBehavior::Ostinato => "ostinato",
            NativeBehavior::Motif => "motif",
            NativeBehavior::Canon => "canon",
            NativeBehavior::Chords => "chords",
            NativeBehavior::Contour => "contour",
            NativeBehavior::Cadence => "cadence",
            NativeBehavior::Phrase => "phrase",
        }
    }

    pub(crate) fn is_pattern(self) -> bool {
        matches!(
            self,
            NativeBehavior::Weave
                | NativeBehavior::Polyrhythm
                | NativeBehavior::Breaks
                | NativeBehavior::Fills
                | NativeBehavior::Clave
                | NativeBehavior::Groove
                | NativeBehavior::Euclid
                | NativeBehavior::Ostinato
                | NativeBehavior::Motif
                | NativeBehavior::Canon
                | NativeBehavior::Chords
                | NativeBehavior::Contour
                | NativeBehavior::Cadence
                | NativeBehavior::Phrase
        )
    }
}
