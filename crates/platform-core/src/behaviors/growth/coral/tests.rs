use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, CellTriggerType, DeviceInput};
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_normalize_serialize_restore_quiet() {
    let menu = coral_config_menu();
    assert_eq!(menu[0].key, "growthPct");
    assert_eq!(menu[1].key, "competitionPct");
    assert_eq!(menu[2].key, "breakawayAge");
    assert_eq!(menu[3].key, "seedCoral");
    assert_eq!(menu[4].key, "breakCoral");
    let state =
        coral_init(json!({"cells":[9,"bad",2],"ages":[300],"growthPct":200,"breakawayAge":0}))
            .unwrap();
    assert_eq!(state.cells[0], 3);
    assert_eq!(state.cells[1], 0);
    assert_eq!(state.ages[0], 255);
    assert_eq!(state.growth_pct, 100);
    assert_eq!(state.breakaway_age, 1);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != CellTriggerType::Activate));
    let value = coral_serialize(&state).unwrap();
    assert!(value.get("triggerTypes").is_none());
    assert!(value.get("tickCounter").is_none());
    let restored = coral_deserialize(value.clone()).unwrap();
    assert_eq!(coral_serialize(&restored).unwrap(), value)
}

#[test]
fn grid_press_cycle_seed_and_break_actions() {
    let mut ctx = context();
    let state = coral_init(json!({})).unwrap();
    let a = coral_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(a.cells[grid_index(0, 0)], A);
    assert_eq!(a.trigger_types[grid_index(0, 0)], CellTriggerType::Activate);
    let b = coral_on_input(a, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(b.cells[grid_index(0, 0)], B);
    assert_eq!(b.trigger_types[grid_index(0, 0)], CellTriggerType::Activate);
    let empty = coral_on_input(b, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(empty.cells[grid_index(0, 0)], EMPTY);
    assert_eq!(
        empty.trigger_types[grid_index(0, 0)],
        CellTriggerType::Deactivate
    );
    let skeleton = coral_init(json!({"cells":[3]})).unwrap();
    let cycled = coral_on_input(skeleton, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(cycled.cells[grid_index(0, 0)], A);
    assert_eq!(
        cycled.trigger_types[grid_index(0, 0)],
        CellTriggerType::Activate
    );
    let mut cells = vec![0; CELL_COUNT];
    cells[grid_index(1, 0)] = B;
    cells[grid_index(2, 0)] = DEAD;
    let overwritten = coral_on_input(
        coral_init(json!({"cells": cells})).unwrap(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCoral".into(),
        }),
        &mut ctx,
    );
    assert_eq!(overwritten.cells[grid_index(1, 0)], A);
    assert_eq!(overwritten.cells[grid_index(2, 0)], A);
    assert_eq!(
        overwritten.trigger_types[grid_index(1, 0)],
        CellTriggerType::Activate
    );
    assert_eq!(
        overwritten.trigger_types[grid_index(2, 0)],
        CellTriggerType::Activate
    );
    let seeded = coral_on_input(
        empty,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCoral".into(),
        }),
        &mut ctx,
    );
    assert_eq!(seeded.cells[grid_index(1, 0)], A);
    assert_eq!(seeded.cells[grid_index(6, 0)], B);
    assert_eq!(
        seeded.trigger_types[grid_index(1, 0)],
        CellTriggerType::Activate
    );
    let broken = coral_on_input(
        seeded,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "breakCoral".into(),
        }),
        &mut ctx,
    );
    assert_eq!(
        broken.trigger_types[grid_index(1, 0)],
        CellTriggerType::Deactivate
    )
}

#[test]
fn growth_competition_breakaway_and_no_wrap() {
    let mut ctx = context();
    let mut cells = vec![0; CELL_COUNT];
    cells[grid_index(1, 0)] = A;
    cells[grid_index(0, 1)] = B;
    let state = coral_init(json!({"cells":cells,"growthPct":100,"competitionPct":0})).unwrap();
    let grown = coral_on_tick(state, &mut ctx);
    assert_eq!(grown.cells[grid_index(0, 0)], A);
    assert_eq!(
        grown.trigger_types[grid_index(0, 0)],
        CellTriggerType::Activate
    );
    assert_eq!(grown.cells[grid_index(7, 0)], EMPTY);
    let mut cells = vec![0; CELL_COUNT];
    cells[grid_index(2, 2)] = A;
    cells[grid_index(3, 2)] = B;
    let state =
        coral_init(json!({"cells":cells,"growthPct":0,"competitionPct":100,"breakawayAge":1}))
            .unwrap();
    let competed = coral_on_tick(state, &mut ctx);
    assert_eq!(competed.cells[grid_index(2, 2)], DEAD);
    assert_eq!(
        competed.trigger_types[grid_index(2, 2)],
        CellTriggerType::Deactivate
    );
    let cleared = coral_on_tick(competed, &mut ctx);
    assert_eq!(cleared.cells[grid_index(2, 2)], EMPTY);
    assert_eq!(
        cleared.trigger_types[grid_index(2, 2)],
        CellTriggerType::Deactivate
    );
    assert!(coral_render_model(&cleared).status_line.starts_with("A:"))
}
