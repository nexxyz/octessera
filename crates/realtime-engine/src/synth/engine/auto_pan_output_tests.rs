use super::render_routing::render_bus_stereo_output;
use super::*;

const SAMPLE_RATE: u32 = 48_000;

fn auto_pan_chain(depth: f32) -> BusChainOwner {
    let params = FxBusParams::AutoPan {
        rate_hz: 1.0,
        depth,
    };
    BusChainOwner::new(
        0,
        [params, FxBusParams::None, FxBusParams::None],
        [
            fx_bus_state_from_params(&params, SAMPLE_RATE),
            FxBusState::None,
            FxBusState::None,
        ],
        [0; BUS_SLOTS_PER_BUS],
    )
}

#[test]
fn a_centered_auto_pan_bus_follows_the_constant_power_pan_law_around_the_lfo() {
    let level = 0.5;
    let quarter = SAMPLE_RATE as usize / 4;
    for depth in [1.0_f32, 0.5] {
        let mut chain = auto_pan_chain(depth);
        let mut spread = FxBusOutputSpreadState::new(SAMPLE_RATE);
        let center = (
            std::f32::consts::FRAC_1_SQRT_2,
            std::f32::consts::FRAC_1_SQRT_2,
        );
        for frame in 0..=3 * quarter {
            let output = chain.process(level, &[0.0; BUS_SLOTS_PER_BUS], SAMPLE_RATE);
            assert_eq!(output.mono, level);
            let (left, right) = render_bus_stereo_output(output, &mut spread, center, 1.0);
            if frame % quarter != 0 {
                continue;
            }
            let lfo = (std::f32::consts::TAU * frame as f32 / SAMPLE_RATE as f32).sin();
            let angle = (0.5 + 0.5 * depth * lfo) * std::f32::consts::FRAC_PI_2;
            let (expected_left, expected_right) = (level * angle.cos(), level * angle.sin());
            assert!(
                (left - expected_left).abs() < 2.0e-3 && (right - expected_right).abs() < 2.0e-3,
                "depth {depth} frame {frame}: ({left}, {right}), expected \
                 ({expected_left}, {expected_right})"
            );
            assert!((left * left + right * right - level * level).abs() < 1.0e-3);
        }
    }
}
