use super::NativeBehavior;
use crate::behavior::BehaviorRenderPalette;
use crate::palette::{
    BEHAVIOR_DIM_GREEN, BEHAVIOR_PRIMARY_MAGENTA, BEHAVIOR_PRIMARY_YELLOW, BLACK, BLUE, GRAY,
    GREEN, RED, WHITE, YELLOW,
};
use serde_json::Value;

#[test]
fn behavior_render_palettes_match_themes() {
    let expected = [
        (
            NativeBehavior::None,
            BehaviorRenderPalette {
                active: GRAY,
                inactive: BLACK,
                stable: GRAY,
            },
        ),
        (
            NativeBehavior::Life,
            BehaviorRenderPalette {
                active: BEHAVIOR_PRIMARY_MAGENTA,
                inactive: BEHAVIOR_DIM_GREEN,
                stable: BEHAVIOR_PRIMARY_YELLOW,
            },
        ),
        (
            NativeBehavior::Sequencer,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLACK,
                stable: YELLOW,
            },
        ),
        (
            NativeBehavior::Keys,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLACK,
                stable: YELLOW,
            },
        ),
        (
            NativeBehavior::Looper,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLACK,
                stable: BLUE,
            },
        ),
        (
            NativeBehavior::Brain,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLACK,
                stable: BLUE,
            },
        ),
        (
            NativeBehavior::Cyclic,
            BehaviorRenderPalette {
                active: [255, 120, 220],
                inactive: BLACK,
                stable: [80, 180, 255],
            },
        ),
        (
            NativeBehavior::ForestFire,
            BehaviorRenderPalette {
                active: YELLOW,
                inactive: BLACK,
                stable: GREEN,
            },
        ),
        (
            NativeBehavior::PredatorPrey,
            BehaviorRenderPalette {
                active: [255, 180, 80],
                inactive: BLACK,
                stable: GREEN,
            },
        ),
        (
            NativeBehavior::Twinkle,
            BehaviorRenderPalette {
                active: [80, 170, 255],
                inactive: BLACK,
                stable: [18, 48, 82],
            },
        ),
        (
            NativeBehavior::Ant,
            BehaviorRenderPalette {
                active: BLACK,
                inactive: [80, 48, 24],
                stable: BLACK,
            },
        ),
        (
            NativeBehavior::Bounce,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLACK,
                stable: RED,
            },
        ),
        (
            NativeBehavior::Bubbles,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLUE,
                stable: GRAY,
            },
        ),
        (
            NativeBehavior::Boids,
            BehaviorRenderPalette {
                active: [255, 240, 160],
                inactive: BLACK,
                stable: [120, 200, 255],
            },
        ),
        (
            NativeBehavior::Gravity,
            BehaviorRenderPalette {
                active: [255, 220, 120],
                inactive: BLACK,
                stable: [180, 140, 60],
            },
        ),
        (
            NativeBehavior::Orbit,
            BehaviorRenderPalette {
                active: [255, 210, 120],
                inactive: BLACK,
                stable: [140, 120, 255],
            },
        ),
        (
            NativeBehavior::FractalExplorer,
            BehaviorRenderPalette {
                active: [255, 220, 180],
                inactive: BLACK,
                stable: [100, 80, 200],
            },
        ),
        (
            NativeBehavior::Shapes,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLACK,
                stable: RED,
            },
        ),
        (
            NativeBehavior::Ink,
            BehaviorRenderPalette {
                active: [120, 80, 255],
                inactive: BLACK,
                stable: [40, 30, 140],
            },
        ),
        (
            NativeBehavior::Kuramoto,
            BehaviorRenderPalette {
                active: [255, 255, 200],
                inactive: BLACK,
                stable: [120, 80, 255],
            },
        ),
        (
            NativeBehavior::Lightning,
            BehaviorRenderPalette {
                active: [255, 255, 180],
                inactive: BLACK,
                stable: [80, 180, 255],
            },
        ),
        (
            NativeBehavior::Wave,
            BehaviorRenderPalette {
                active: [180, 240, 255],
                inactive: BLACK,
                stable: [30, 90, 180],
            },
        ),
        (
            NativeBehavior::Raindrops,
            BehaviorRenderPalette {
                active: WHITE,
                inactive: BLUE,
                stable: GRAY,
            },
        ),
        (
            NativeBehavior::Cracks,
            BehaviorRenderPalette {
                active: [255, 245, 210],
                inactive: BLACK,
                stable: [120, 160, 180],
            },
        ),
        (
            NativeBehavior::CrystalGrowth,
            BehaviorRenderPalette {
                active: [220, 255, 255],
                inactive: BLACK,
                stable: [40, 180, 255],
            },
        ),
        (
            NativeBehavior::Physarum,
            BehaviorRenderPalette {
                active: [240, 255, 140],
                inactive: BLACK,
                stable: [160, 180, 70],
            },
        ),
        (
            NativeBehavior::Dla,
            BehaviorRenderPalette {
                active: YELLOW,
                inactive: BLACK,
                stable: GREEN,
            },
        ),
        (
            NativeBehavior::MazeGrowth,
            BehaviorRenderPalette {
                active: YELLOW,
                inactive: BLACK,
                stable: GRAY,
            },
        ),
    ];

    for (behavior, palette) in expected {
        let state = behavior.init(Value::Null).unwrap();
        let model = behavior.render_model(&state).unwrap();
        assert_eq!(model.palette, palette, "{} palette", behavior.id());
    }
}

#[test]
fn themed_background_palettes_keep_live_cells_visible() {
    for behavior in [
        NativeBehavior::Life,
        NativeBehavior::Bubbles,
        NativeBehavior::Raindrops,
    ] {
        let state = behavior.init(Value::Null).unwrap();
        let model = behavior.render_model(&state).unwrap();
        assert_ne!(
            model.palette.active,
            model.palette.inactive,
            "{} active cells must contrast with themed inactive background",
            behavior.id()
        );
        assert_ne!(
            model.palette.stable,
            model.palette.inactive,
            "{} stable cells must contrast with themed inactive background",
            behavior.id()
        );
    }
}

#[test]
fn ant_palette_renders_black_trails_on_brown_background() {
    let state = NativeBehavior::Ant.init(Value::Null).unwrap();
    let model = NativeBehavior::Ant.render_model(&state).unwrap();

    assert_eq!(model.palette.active, BLACK);
    assert_eq!(model.palette.inactive, [80, 48, 24]);
    assert_eq!(model.palette.stable, BLACK);
}
