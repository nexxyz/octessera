use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, CellTriggerType, DeviceInput};
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

fn assert_tail_activity(mut state: ReactionDiffusionState) {
    let mut ctx = context();
    let mut consecutive_empty = 0;
    let mut consecutive_full = 0;
    let mut consecutive_same = 0;
    let mut previous = Vec::new();
    for tick in 1..=300 {
        state = reaction_diffusion_on_tick(state, &mut ctx);
        let cells = reaction_diffusion_render_model(&state).cells;
        consecutive_empty = if cells.iter().all(|cell| !*cell) {
            consecutive_empty + 1
        } else {
            0
        };
        consecutive_full = if cells.iter().all(|cell| *cell) {
            consecutive_full + 1
        } else {
            0
        };
        consecutive_same = if cells == previous {
            consecutive_same + 1
        } else {
            0
        };
        assert!(consecutive_empty <= 1, "empty run at tick {tick}");
        assert!(consecutive_full <= 1, "full run at tick {tick}");
        assert!(consecutive_same <= 2, "same-frame run at tick {tick}");
        previous = cells;
    }
}

#[test]
fn menu_palette_normalize_serialize() {
    let menu = reaction_diffusion_config_menu();
    assert_eq!(menu[0].key, "feedPct");
    assert_eq!(menu[1].key, "killPct");
    assert_eq!(menu[2].key, "diffusionPct");
    assert_eq!(menu[3].key, "reactionPct");
    assert_eq!(menu[4].key, "seedInterval");
    assert_eq!(menu[5].key, "spawnStep");
    assert_eq!(menu[6].key, "seedChemicals");
    assert_eq!(menu[7].key, "clearChemicals");
    let state = reaction_diffusion_init(
        json!({"a":[300,"bad"],"b":[40],"feedPct":200,"seedInterval":200,"spawnStep":200}),
    )
    .unwrap();
    assert_eq!(state.a.len(), CELL_COUNT);
    assert_eq!(state.a[0], 255);
    assert_eq!(state.a[1], 255);
    assert_eq!(state.b[0], 40);
    assert_eq!(state.feed_pct, 100);
    assert_eq!(state.seed_interval, 64);
    assert_eq!(state.spawn_step, 63);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != CellTriggerType::Activate));
    let value = reaction_diffusion_serialize(&state).unwrap();
    assert!(value.get("triggerTypes").is_none());
    assert!(value.get("tickCounter").is_none());
    let restored = reaction_diffusion_deserialize(value.clone()).unwrap();
    assert_eq!(reaction_diffusion_serialize(&restored).unwrap(), value);
    assert_eq!(
        reaction_diffusion_render_model(&state).name,
        "reaction diffusion"
    );
}

#[test]
fn default_seed_interval_keeps_reaction_visible_with_bounded_pulses() {
    let mut ctx = context();
    let mut state = reaction_diffusion_init(json!({})).unwrap();
    let mut pulse_counts = Vec::new();
    for _ in 0..128 {
        state = reaction_diffusion_on_tick(state, &mut ctx);
        let activates = state
            .trigger_types
            .iter()
            .filter(|trigger| **trigger == CellTriggerType::Activate)
            .count();
        if activates > 0 {
            pulse_counts.push(activates);
        }
    }
    assert!(state.b.iter().filter(|value| **value >= VISIBLE).count() > 0);
    assert!(pulse_counts.len() >= 4);
    assert!(pulse_counts.iter().all(|count| *count <= 5));
}

#[test]
fn defaults_keep_tail_activity_bounded_through_300_ticks() {
    assert_tail_activity(reaction_diffusion_init(json!({})).unwrap());
}

#[test]
fn grid_press_seed_and_clear_trigger_contract() {
    let mut ctx = context();
    let state = reaction_diffusion_init(json!({})).unwrap();
    let pressed =
        reaction_diffusion_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(pressed.b[grid_index(0, 0)], 160);
    assert_eq!(pressed.a[grid_index(0, 0)], 175);
    assert_eq!(
        pressed.trigger_types[grid_index(0, 0)],
        CellTriggerType::Activate
    );
    assert_eq!(
        pressed.trigger_types[grid_index(1, 0)],
        CellTriggerType::Activate
    );
    let seeded = reaction_diffusion_on_input(
        pressed.clone(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedChemicals".into(),
        }),
        &mut ctx,
    );
    assert_eq!(
        seeded.trigger_types[grid_index(3, 3)],
        CellTriggerType::Activate
    );
    let cleared = reaction_diffusion_on_input(
        seeded,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "clearChemicals".into(),
        }),
        &mut ctx,
    );
    assert_eq!(cleared.b.iter().sum::<u8>(), 0);
    assert_eq!(
        cleared.trigger_types[grid_index(0, 0)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn integer_tick_uses_start_snapshot_thresholds_and_edges() {
    let mut ctx = context();
    let mut b = vec![0; CELL_COUNT];
    b[grid_index(1, 1)] = 100;
    let state = reaction_diffusion_init(
        json!({"b":b,"feedPct":0,"killPct":0,"diffusionPct":0,"reactionPct":50}),
    )
    .unwrap();
    let ticked = reaction_diffusion_on_tick(state, &mut ctx);
    assert!(ticked.b[grid_index(1, 1)] > 100);
    assert_eq!(
        ticked.trigger_types[grid_index(1, 1)],
        CellTriggerType::Stable
    );
    let mut b = vec![0; CELL_COUNT];
    b[grid_index(0, 0)] = 100;
    let edge = reaction_diffusion_init(
        json!({"b":b,"feedPct":0,"killPct":100,"diffusionPct":100,"reactionPct":0}),
    )
    .unwrap();
    let edge = reaction_diffusion_on_tick(edge, &mut ctx);
    assert_eq!(edge.b[grid_index(0, 0)], 0);
    assert_eq!(edge.b[grid_index(7, 7)], 0);
    assert_eq!(
        edge.trigger_types[grid_index(0, 0)],
        CellTriggerType::Deactivate
    );
    assert_eq!(
        reaction_diffusion_render_model(&edge).status_line,
        "B:2 E:2"
    );
}
