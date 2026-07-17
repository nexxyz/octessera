use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, CellTriggerType, DeviceInput};
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_serialize_restore_quiet() {
    let menu = rivers_config_menu();
    assert_eq!(menu[0].key, "rainPct");
    assert_eq!(menu[1].key, "flowPct");
    assert_eq!(menu[2].key, "erosionPct");
    assert_eq!(menu[3].key, "evaporationPct");
    assert_eq!(menu[4].key, "rainBurst");
    assert_eq!(menu[5].key, "resetTerrain");
    let state =
        rivers_init(json!({"height":[300],"water":[40],"sediment":[9],"rainPct":200})).unwrap();
    assert_eq!(state.height[0], 255);
    assert_eq!(state.height[1], default_height()[1]);
    assert_eq!(state.water[0], 40);
    assert_eq!(state.rain_pct, 100);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != CellTriggerType::Activate));
    let value = rivers_serialize(&state).unwrap();
    assert!(value.get("triggerTypes").is_none());
    assert!(value.get("tickCounter").is_none());
    let restored = rivers_deserialize(value.clone()).unwrap();
    assert_eq!(rivers_serialize(&restored).unwrap(), value)
}

#[test]
fn grid_actions_and_reset() {
    let mut ctx = context();
    let state = rivers_init(json!({})).unwrap();
    let pressed = rivers_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(pressed.water[grid_index(0, 0)], 96);
    assert_eq!(
        pressed.trigger_types[grid_index(0, 0)],
        CellTriggerType::Activate
    );
    let burst = rivers_on_input(
        pressed,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "rainBurst".into(),
        }),
        &mut ctx,
    );
    assert_eq!(burst.water[grid_index(1, 6)], 64);
    assert_eq!(
        burst.trigger_types[grid_index(1, 6)],
        CellTriggerType::Activate
    );
    let reset = rivers_on_input(
        burst,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "resetTerrain".into(),
        }),
        &mut ctx,
    );
    assert_eq!(reset.water.iter().sum::<u8>(), 0);
    assert_eq!(
        reset.trigger_types[grid_index(1, 6)],
        CellTriggerType::Deactivate
    )
}

#[test]
fn flow_erosion_evaporation_status() {
    let mut ctx = context();
    let mut h = vec![100; CELL_COUNT];
    h[grid_index(0, 0)] = 100;
    h[grid_index(1, 0)] = 0;
    let mut w = vec![0; CELL_COUNT];
    w[grid_index(0, 0)] = 64;
    let state = rivers_init(
        json!({"height":h,"water":w,"rainPct":0,"flowPct":50,"erosionPct":100,"evaporationPct":0}),
    )
    .unwrap();
    let ticked = rivers_on_tick(state, &mut ctx);
    assert!(ticked.water[grid_index(1, 0)] > 0);
    assert_eq!(ticked.height[grid_index(0, 0)], 99);
    assert!(rivers_render_model(&ticked).status_line.contains("E:"));
    let mut h = vec![100; CELL_COUNT];
    h[grid_index(0, 0)] = 100;
    h[grid_index(1, 0)] = 0;
    let mut w = vec![0; CELL_COUNT];
    w[grid_index(0, 0)] = 64;
    let state = rivers_init(
        json!({"height":h,"water":w,"rainPct":0,"flowPct":0,"erosionPct":100,"evaporationPct":0}),
    )
    .unwrap();
    let ticked = rivers_on_tick(state, &mut ctx);
    assert_eq!(ticked.water[grid_index(0, 0)], 64);
    assert_eq!(ticked.water[grid_index(1, 0)], 0);
    let state = rivers_init(
        json!({"water":[8],"rainPct":0,"flowPct":0,"erosionPct":0,"evaporationPct":100}),
    )
    .unwrap();
    let ticked = rivers_on_tick(state, &mut ctx);
    assert_eq!(ticked.water[0], 0);
    assert_eq!(ticked.trigger_types[0], CellTriggerType::Deactivate);
}
