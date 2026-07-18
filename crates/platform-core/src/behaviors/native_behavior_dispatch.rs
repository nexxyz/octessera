use super::{cellular, native_impl, pattern_music, play, NativeBehavior, NativeBehaviorState};
use crate::behavior::{BehaviorContext, BehaviorRenderModel, DeviceInput};
use serde_json::Value;

impl NativeBehavior {
    pub fn init(self, config: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::None => Ok(NativeBehaviorState::None(play::none::init(config)?)),
            NativeBehavior::Life => Ok(NativeBehaviorState::Life(cellular::life::init(config)?)),
            NativeBehavior::Sequencer => Ok(NativeBehaviorState::Sequencer(play::sequencer::init(
                config,
            )?)),
            _ => self.init_native(config),
        }
    }

    pub fn on_input(
        self,
        state: NativeBehaviorState,
        input: DeviceInput,
        context: &mut BehaviorContext,
    ) -> Result<NativeBehaviorState, String> {
        match (self, state) {
            (behavior, NativeBehaviorState::Pattern(state)) if behavior.is_pattern() => {
                Ok(NativeBehaviorState::Pattern(
                    pattern_music::pattern_on_input(state, input, context),
                ))
            }
            (NativeBehavior::None, NativeBehaviorState::None(state)) => Ok(
                NativeBehaviorState::None(play::none::on_input(state, input, context)),
            ),
            (NativeBehavior::Life, NativeBehaviorState::Life(state)) => Ok(
                NativeBehaviorState::Life(cellular::life::on_input(state, input, context)),
            ),
            (NativeBehavior::Sequencer, NativeBehaviorState::Sequencer(state)) => Ok(
                NativeBehaviorState::Sequencer(play::sequencer::on_input(state, input, context)),
            ),
            (NativeBehavior::Keys, NativeBehaviorState::Keys(state)) => Ok(
                NativeBehaviorState::Keys(native_impl::keys_on_input(state, input, context)),
            ),
            (NativeBehavior::Looper, NativeBehaviorState::Looper(state)) => Ok(
                NativeBehaviorState::Looper(native_impl::looper_on_input(state, input, context)),
            ),
            (NativeBehavior::Brain, NativeBehaviorState::Brain(state)) => Ok(
                NativeBehaviorState::Brain(native_impl::brain_on_input(state, input, context)),
            ),
            (NativeBehavior::Cyclic, NativeBehaviorState::Cyclic(state)) => Ok(
                NativeBehaviorState::Cyclic(native_impl::cyclic_on_input(state, input, context)),
            ),
            (NativeBehavior::ForestFire, NativeBehaviorState::ForestFire(state)) => {
                Ok(NativeBehaviorState::ForestFire(
                    native_impl::forest_fire_on_input(state, input, context),
                ))
            }
            (NativeBehavior::PredatorPrey, NativeBehaviorState::PredatorPrey(state)) => {
                Ok(NativeBehaviorState::PredatorPrey(
                    native_impl::predator_prey_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Ant, NativeBehaviorState::Ant(state)) => Ok(NativeBehaviorState::Ant(
                native_impl::ant_on_input(state, input, context),
            )),
            (NativeBehavior::Boids, NativeBehaviorState::Boids(state)) => Ok(
                NativeBehaviorState::Boids(native_impl::boids_on_input(state, input, context)),
            ),
            (NativeBehavior::Bounce, NativeBehaviorState::Bounce(state)) => Ok(
                NativeBehaviorState::Bounce(native_impl::bounce_on_input(state, input, context)),
            ),
            (NativeBehavior::Bubbles, NativeBehaviorState::Bubbles(state)) => Ok(
                NativeBehaviorState::Bubbles(native_impl::bubbles_on_input(state, input, context)),
            ),
            (NativeBehavior::Gravity, NativeBehaviorState::Gravity(state)) => Ok(
                NativeBehaviorState::Gravity(native_impl::gravity_on_input(state, input, context)),
            ),
            (NativeBehavior::LavaLamp, NativeBehaviorState::LavaLamp(state)) => {
                Ok(NativeBehaviorState::LavaLamp(
                    native_impl::lava_lamp_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Orbit, NativeBehaviorState::Orbit(state)) => Ok(
                NativeBehaviorState::Orbit(native_impl::orbit_on_input(state, input, context)),
            ),
            (NativeBehavior::SandRipples, NativeBehaviorState::SandRipples(state)) => {
                Ok(NativeBehaviorState::SandRipples(
                    native_impl::sand_ripples_on_input(state, input, context),
                ))
            }
            (NativeBehavior::FractalExplorer, NativeBehaviorState::FractalExplorer(state)) => {
                Ok(NativeBehaviorState::FractalExplorer(
                    native_impl::fractal_explorer_on_input(state, input, context),
                ))
            }
            (NativeBehavior::MazeGrowth, NativeBehaviorState::MazeGrowth(state)) => {
                Ok(NativeBehaviorState::MazeGrowth(
                    native_impl::maze_growth_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Shapes, NativeBehaviorState::Shapes(state)) => Ok(
                NativeBehaviorState::Shapes(native_impl::shapes_on_input(state, input, context)),
            ),
            (NativeBehavior::Ink, NativeBehaviorState::Ink(state)) => Ok(NativeBehaviorState::Ink(
                native_impl::ink_on_input(state, input, context),
            )),
            (NativeBehavior::Ising, NativeBehaviorState::Ising(state)) => Ok(
                NativeBehaviorState::Ising(native_impl::ising_on_input(state, input, context)),
            ),
            (NativeBehavior::Kuramoto, NativeBehaviorState::Kuramoto(state)) => {
                Ok(NativeBehaviorState::Kuramoto(
                    native_impl::kuramoto_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Lightning, NativeBehaviorState::Lightning(state)) => {
                Ok(NativeBehaviorState::Lightning(
                    native_impl::lightning_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Wave, NativeBehaviorState::Wave(state)) => Ok(
                NativeBehaviorState::Wave(native_impl::wave_on_input(state, input, context)),
            ),
            (NativeBehavior::Raindrops, NativeBehaviorState::Raindrops(state)) => {
                Ok(NativeBehaviorState::Raindrops(
                    native_impl::raindrops_on_input(state, input, context),
                ))
            }
            (NativeBehavior::ReactionDiffusion, NativeBehaviorState::ReactionDiffusion(state)) => {
                Ok(NativeBehaviorState::ReactionDiffusion(
                    native_impl::reaction_diffusion_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Rivers, NativeBehaviorState::Rivers(state)) => Ok(
                NativeBehaviorState::Rivers(native_impl::rivers_on_input(state, input, context)),
            ),
            (NativeBehavior::Cracks, NativeBehaviorState::Cracks(state)) => Ok(
                NativeBehaviorState::Cracks(native_impl::cracks_on_input(state, input, context)),
            ),
            (NativeBehavior::Coral, NativeBehaviorState::Coral(state)) => Ok(
                NativeBehaviorState::Coral(native_impl::coral_on_input(state, input, context)),
            ),
            (NativeBehavior::CrystalGrowth, NativeBehaviorState::CrystalGrowth(state)) => {
                Ok(NativeBehaviorState::CrystalGrowth(
                    native_impl::crystal_growth_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Dla, NativeBehaviorState::Dla(state)) => Ok(NativeBehaviorState::Dla(
                native_impl::dla_on_input(state, input, context),
            )),
            (NativeBehavior::Physarum, NativeBehaviorState::Physarum(state)) => {
                Ok(NativeBehaviorState::Physarum(
                    native_impl::physarum_on_input(state, input, context),
                ))
            }
            (NativeBehavior::Vines, NativeBehaviorState::Vines(state)) => Ok(
                NativeBehaviorState::Vines(native_impl::vines_on_input(state, input, context)),
            ),
            _ => Err(format!("state mismatch for behavior {}", self.id())),
        }
    }

    pub fn on_tick(
        self,
        state: NativeBehaviorState,
        context: &mut BehaviorContext,
    ) -> Result<NativeBehaviorState, String> {
        super::native_behavior_tick::on_tick(self, state, context)
    }

    pub fn render_model(self, state: &NativeBehaviorState) -> Result<BehaviorRenderModel, String> {
        super::native_behavior_render::render_model(self, state)
    }

    pub fn serialize(self, state: &NativeBehaviorState) -> Result<Value, String> {
        super::native_behavior_serialize::serialize(self, state)
    }

    pub fn deserialize(self, data: Value) -> Result<NativeBehaviorState, String> {
        match self {
            NativeBehavior::None => Ok(NativeBehaviorState::None(play::none::deserialize(data)?)),
            NativeBehavior::Life => Ok(NativeBehaviorState::Life(cellular::life::deserialize(
                data,
            )?)),
            NativeBehavior::Sequencer => Ok(NativeBehaviorState::Sequencer(
                play::sequencer::deserialize(data)?,
            )),
            _ => self.deserialize_native(data),
        }
    }
}
