use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, DeviceInput, TileTriggerType};
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
        coral_init(json!({"tiles":[9,"bad",2],"ages":[300],"growthPct":200,"breakawayAge":0}))
            .unwrap();
    assert_eq!(state.tiles[0], 3);
    assert_eq!(state.tiles[1], 0);
    assert_eq!(state.ages[0], 255);
    assert_eq!(state.growth_pct, 100);
    assert_eq!(state.breakaway_age, 1);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != TileTriggerType::Activate));
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
    assert_eq!(a.tiles[grid_index(0, 0)], A);
    assert_eq!(a.trigger_types[grid_index(0, 0)], TileTriggerType::Activate);
    let b = coral_on_input(a, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(b.tiles[grid_index(0, 0)], B);
    assert_eq!(b.trigger_types[grid_index(0, 0)], TileTriggerType::Activate);
    let empty = coral_on_input(b, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(empty.tiles[grid_index(0, 0)], EMPTY);
    assert_eq!(
        empty.trigger_types[grid_index(0, 0)],
        TileTriggerType::Deactivate
    );
    let skeleton = coral_init(json!({"tiles":[3]})).unwrap();
    let cycled = coral_on_input(skeleton, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(cycled.tiles[grid_index(0, 0)], A);
    assert_eq!(
        cycled.trigger_types[grid_index(0, 0)],
        TileTriggerType::Activate
    );
    let mut tiles = vec![0; TILE_COUNT];
    tiles[grid_index(1, 0)] = B;
    tiles[grid_index(2, 0)] = DEAD;
    let overwritten = coral_on_input(
        coral_init(json!({"tiles": tiles})).unwrap(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCoral".into(),
        }),
        &mut ctx,
    );
    assert_eq!(overwritten.tiles[grid_index(1, 0)], A);
    assert_eq!(overwritten.tiles[grid_index(2, 0)], A);
    assert_eq!(
        overwritten.trigger_types[grid_index(1, 0)],
        TileTriggerType::Activate
    );
    assert_eq!(
        overwritten.trigger_types[grid_index(2, 0)],
        TileTriggerType::Activate
    );
    let seeded = coral_on_input(
        empty,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCoral".into(),
        }),
        &mut ctx,
    );
    assert_eq!(seeded.tiles[grid_index(1, 0)], A);
    assert_eq!(seeded.tiles[grid_index(6, 0)], B);
    assert_eq!(
        seeded.trigger_types[grid_index(1, 0)],
        TileTriggerType::Activate
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
        TileTriggerType::Deactivate
    )
}

#[test]
fn growth_competition_breakaway_and_no_wrap() {
    let mut ctx = context();
    let mut tiles = vec![0; TILE_COUNT];
    tiles[grid_index(1, 0)] = A;
    tiles[grid_index(0, 1)] = B;
    let state = coral_init(json!({"tiles":tiles,"growthPct":100,"competitionPct":0})).unwrap();
    let grown = coral_on_tick(state, &mut ctx);
    assert_eq!(grown.tiles[grid_index(0, 0)], A);
    assert_eq!(
        grown.trigger_types[grid_index(0, 0)],
        TileTriggerType::Activate
    );
    assert_eq!(grown.tiles[grid_index(7, 0)], EMPTY);
    let mut tiles = vec![0; TILE_COUNT];
    tiles[grid_index(2, 2)] = A;
    tiles[grid_index(3, 2)] = B;
    let state =
        coral_init(json!({"tiles":tiles,"growthPct":0,"competitionPct":100,"breakawayAge":1}))
            .unwrap();
    let competed = coral_on_tick(state, &mut ctx);
    assert_eq!(competed.tiles[grid_index(2, 2)], DEAD);
    assert_eq!(
        competed.trigger_types[grid_index(2, 2)],
        TileTriggerType::Deactivate
    );
    let cleared = coral_on_tick(competed, &mut ctx);
    assert_eq!(cleared.tiles[grid_index(2, 2)], EMPTY);
    assert_eq!(
        cleared.trigger_types[grid_index(2, 2)],
        TileTriggerType::Deactivate
    );
    assert!(coral_render_model(&cleared).status_line.starts_with("A:"))
}

#[test]
fn default_avoids_consecutive_extremes_and_bounded_triggers() {
    let mut ctx = context();
    let mut state = coral_init(json!({})).unwrap();
    let mut full_run = 0;
    let mut empty_run = 0;
    for _ in 0..300 {
        state = coral_on_tick(state, &mut ctx);
        let visible = coral_render_model(&state).tiles;
        full_run = if visible.iter().all(|tile| *tile) {
            full_run + 1
        } else {
            0
        };
        empty_run = if visible.iter().all(|tile| !*tile) {
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
                    TileTriggerType::Activate | TileTriggerType::Deactivate
                )
            })
            .count();
        assert!(bursts <= 16);
    }
}
