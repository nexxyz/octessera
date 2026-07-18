use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, CellTriggerType, DeviceInput};
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_normalize_serialize_restore_quiet() {
    let menu = vines_config_menu();
    assert_eq!(menu[0].key, "growthPct");
    assert_eq!(menu[1].key, "branchPct");
    assert_eq!(menu[2].key, "pruneAge");
    assert_eq!(menu[3].key, "lightBiasPct");
    assert_eq!(menu[4].key, "plantSeed");
    assert_eq!(menu[5].key, "pruneVines");
    let state = vines_init(
        json!({"cells":[9,"bad",2],"energy":[300],"ages":[300],"pruneAge":0,"growthPct":200}),
    )
    .unwrap();
    assert_eq!(state.cells[0], 3);
    assert_eq!(state.cells[1], 0);
    assert_eq!(state.energy[0], 255);
    assert_eq!(state.ages[0], 255);
    assert_eq!(state.prune_age, 1);
    assert_eq!(state.growth_pct, 100);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != CellTriggerType::Activate));
    let value = vines_serialize(&state).unwrap();
    assert!(value.get("triggerTypes").is_none());
    assert!(value.get("tickCounter").is_none());
    let restored = vines_deserialize(value.clone()).unwrap();
    assert_eq!(vines_serialize(&restored).unwrap(), value);
}

#[test]
fn grid_press_seed_and_prune_actions() {
    let mut ctx = context();
    let state = vines_init(json!({})).unwrap();
    let pressed = vines_on_input(state, DeviceInput::GridPress { x: 2, y: 2 }, &mut ctx);
    assert_eq!(pressed.cells[grid_index(2, 2)], TIP);
    assert_eq!(pressed.energy[grid_index(2, 2)], 220);
    assert_eq!(
        pressed.trigger_types[grid_index(2, 2)],
        CellTriggerType::Activate
    );
    let pressed_again = vines_on_input(pressed, DeviceInput::GridPress { x: 2, y: 2 }, &mut ctx);
    assert_eq!(
        pressed_again.trigger_types[grid_index(2, 2)],
        CellTriggerType::Activate
    );
    let seeded = vines_on_input(
        pressed_again,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "plantSeed".into(),
        }),
        &mut ctx,
    );
    assert_eq!(seeded.cells[grid_index(3, 0)], TIP);
    assert_eq!(
        seeded.trigger_types[grid_index(3, 0)],
        CellTriggerType::Activate
    );
    assert_eq!(
        seeded.trigger_types[grid_index(4, 0)],
        CellTriggerType::Activate
    );
    let mut cells = vec![0; CELL_COUNT];
    let mut ages = vec![0; CELL_COUNT];
    for i in 0..10 {
        cells[i] = STEM;
        ages[i] = i as u8;
    }
    let state = vines_init(json!({"cells":cells,"ages":ages})).unwrap();
    let pruned = vines_on_input(
        state,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "pruneVines".into(),
        }),
        &mut ctx,
    );
    assert_eq!(pruned.cells[9], EMPTY);
    assert_eq!(pruned.trigger_types[9], CellTriggerType::Deactivate);
    assert_eq!(pruned.cells[0], STEM);
}

#[test]
fn growth_branch_reservation_prune_and_edges() {
    let mut ctx = context();
    let mut cells = vec![0; CELL_COUNT];
    cells[grid_index(3, 0)] = TIP;
    let state = vines_init(
        json!({"cells":cells,"growthPct":100,"branchPct":100,"lightBiasPct":100,"pruneAge":2}),
    )
    .unwrap();
    let ticked = vines_on_tick(state, &mut ctx);
    assert_eq!(ticked.cells[grid_index(3, 0)], STEM);
    assert_eq!(ticked.cells[grid_index(4, 1)], TIP);
    assert_eq!(
        ticked.trigger_types[grid_index(4, 1)],
        CellTriggerType::Activate
    );
    assert!(ticked.cells.iter().filter(|c| **c == TIP).count() <= 2);
    let mut cells = vec![0; CELL_COUNT];
    let mut ages = vec![0; CELL_COUNT];
    cells[grid_index(0, 0)] = STEM;
    ages[grid_index(0, 0)] = 2;
    let state =
        vines_init(json!({"cells":cells,"ages":ages,"pruneAge":2,"growthPct":100})).unwrap();
    let leaf = vines_on_tick(state, &mut ctx);
    assert_eq!(leaf.cells[grid_index(0, 0)], LEAF);
    assert_eq!(
        leaf.trigger_types[grid_index(0, 0)],
        CellTriggerType::Activate
    );
    let removed = vines_on_tick(leaf, &mut ctx);
    assert_eq!(removed.cells[grid_index(0, 0)], EMPTY);
    assert_eq!(
        removed.trigger_types[grid_index(0, 0)],
        CellTriggerType::Deactivate
    );
    assert_eq!(vines_render_model(&removed).name, "vines");
}

#[test]
fn default_avoids_consecutive_extremes_and_bounded_triggers() {
    let mut ctx = context();
    let mut state = vines_init(json!({})).unwrap();
    let mut full_run = 0;
    let mut empty_run = 0;
    for _ in 0..300 {
        state = vines_on_tick(state, &mut ctx);
        let visible = vines_render_model(&state).cells;
        full_run = if visible.iter().all(|cell| *cell) {
            full_run + 1
        } else {
            0
        };
        empty_run = if visible.iter().all(|cell| !*cell) {
            empty_run + 1
        } else {
            0
        };
        assert!(full_run <= 1);
        assert!(empty_run <= 1);
        let bursts = state
            .trigger_types
            .iter()
            .filter(|trigger| {
                matches!(
                    trigger,
                    CellTriggerType::Activate | CellTriggerType::Deactivate
                )
            })
            .count();
        assert!(bursts <= 16);
    }
}
