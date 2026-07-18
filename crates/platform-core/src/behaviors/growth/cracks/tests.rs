use super::*;
fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_palette_normalize_serialize() {
    let m = cracks_config_menu();
    assert_eq!(m[0].key, "stressPct");
    assert_eq!(m[3].key, "shatterThreshold");
    assert_eq!(m[4].key, "impact");
    assert_eq!(m[5].key, "replacePane");
    let s=cracks_deserialize(serde_json::json!({"cells":[9],"stress":[999],"pendingShatter":true,"stressPct":999,"branchPct":999,"propagationPct":999,"shatterThreshold":99,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(s.cells[0], TIP);
    assert_eq!(s.stress[0], 255);
    assert!(s.pending_shatter);
    assert_eq!(s.stress_pct, 100);
    assert_eq!(s.shatter_threshold, 64);
    assert!(!s.trigger_types.contains(&CellTriggerType::Activate));
    let v = cracks_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert_eq!(
        cracks_serialize(&cracks_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let model = cracks_render_model(&s);
    assert_eq!(model.name, "cracks");
    assert_eq!(model.palette.active, [255, 245, 210]);

    let malformed = cracks_deserialize(serde_json::json!({
        "cells": [2], "stressPct": 77, "pendingShatter": "yes please"
    }))
    .unwrap();
    assert_eq!(malformed.cells[0], CRACK);
    assert_eq!(malformed.stress_pct, 77);
    assert!(!malformed.pending_shatter);
}

#[test]
fn impact_and_replace_triggers() {
    let mut c = ctx();
    let s = cracks_init(serde_json::json!({})).unwrap();
    let s = cracks_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.cells[grid_index(2, 3)], TIP);
    assert_eq!(s.trigger_types[grid_index(2, 3)], CellTriggerType::Activate);
    let r = cracks_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "replacePane".into(),
        }),
        &mut c,
    );
    assert_eq!(r.cells[grid_index(2, 3)], CLEAR);
    assert_eq!(
        r.trigger_types[grid_index(2, 3)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn propagation_branch_no_wrap_and_stress_stable() {
    let mut c = ctx();
    let mut s =
        cracks_init(serde_json::json!({"propagationPct":100,"branchPct":100,"stressPct":0}))
            .unwrap();
    s.cells[grid_index(0, 0)] = TIP;
    s.stress[grid_index(0, 1)] = 200;
    s.stress[grid_index(1, 0)] = 100;
    let t = cracks_on_tick(s, &mut c);
    assert_eq!(t.cells[grid_index(0, 0)], CRACK);
    assert_eq!(t.cells[grid_index(0, 1)], TIP);
    assert_ne!(t.cells[grid_index(GRID_WIDTH - 1, 0)], TIP);
    assert!(t.cells.iter().filter(|v| **v == TIP).count() >= 1);
    let mut s =
        cracks_init(serde_json::json!({"propagationPct":100,"branchPct":0,"stressPct":0})).unwrap();
    s.cells[grid_index(3, 3)] = TIP;
    s.stress[grid_index(3, 4)] = 100;
    s.stress[grid_index(4, 3)] = 100;
    let t = cracks_on_tick(s, &mut c);
    assert_eq!(t.cells[grid_index(3, 4)], TIP);
    assert_ne!(t.cells[grid_index(4, 3)], TIP);

    let mut s = cracks_init(serde_json::json!({"stressPct":0})).unwrap();
    s.cells[0] = STRESS;
    s.stress[0] = STRESS_VISIBLE;
    let t = cracks_on_tick(s, &mut c);
    assert_eq!(t.trigger_types[0], CellTriggerType::Stable);
}

#[test]
fn edge_connection_requires_connected_component() {
    let mut c = ctx();
    let mut disconnected = cracks_init(serde_json::json!({
        "shatterThreshold": 64, "stressPct": 0, "propagationPct": 0
    }))
    .unwrap();
    disconnected.cells[grid_index(0, 0)] = CRACK;
    disconnected.cells[grid_index(7, 7)] = TIP;
    let disconnected = cracks_on_tick(disconnected, &mut c);
    assert!(!disconnected.pending_shatter);

    let mut connected = cracks_init(serde_json::json!({
        "shatterThreshold": 64, "stressPct": 0, "propagationPct": 0
    }))
    .unwrap();
    for y in 0..GRID_HEIGHT {
        connected.cells[grid_index(2, y)] = CRACK;
    }
    let connected = cracks_on_tick(connected, &mut c);
    assert!(connected.pending_shatter);
}

#[test]
fn shatter_pending_then_replacement() {
    let mut c = ctx();
    let mut s = cracks_init(serde_json::json!({"shatterThreshold":1})).unwrap();
    s.cells[0] = TIP;
    let t = cracks_on_tick(s, &mut c);
    assert!(t.pending_shatter);
    let dissolving = cracks_on_tick(t, &mut c);
    assert!(dissolving.cells.iter().filter(|v| **v != CLEAR).count() < CELL_COUNT);
    assert!(dissolving.pending_shatter);
}

#[test]
fn default_tail_stays_bounded_and_non_terminal() {
    let mut c = ctx();
    let mut s = cracks_init(serde_json::json!({})).unwrap();
    let mut same = 0;
    let mut terminal = 0;
    let mut previous = visible_cells(&s.cells, &s.stress);
    for _ in 0..300 {
        s = cracks_on_tick(s, &mut c);
        let visible = visible_cells(&s.cells, &s.stress);
        same = if visible == previous { same + 1 } else { 0 };
        terminal = if visible.iter().all(|v| *v) || visible.iter().all(|v| !*v) {
            terminal + 1
        } else {
            0
        };
        assert!(same <= 2);
        assert!(terminal <= 2);
        let bursts = s
            .trigger_types
            .iter()
            .filter(|trigger| {
                matches!(
                    trigger,
                    CellTriggerType::Activate | CellTriggerType::Deactivate
                )
            })
            .count();
        assert!(bursts <= 24);
        previous = visible;
    }
}

#[test]
fn stress_accumulates_decays_and_static_nudge_is_deterministic() {
    let mut c = ctx();
    let s = cracks_deserialize(serde_json::json!({
        "cells": [0], "stress": [0], "stressPct": 100,
        "propagationPct": 0, "branchPct": 0, "shatterThreshold": 64
    }))
    .unwrap();
    let t = cracks_on_tick(s, &mut c);
    assert_eq!(t.stress[0], 96);
    assert_eq!(t.cells[0], TIP);

    let mut stressed = cracks_init(serde_json::json!({
        "cells": vec![STRESS; CELL_COUNT],
        "stress": vec![STRESS_VISIBLE; CELL_COUNT],
        "stressPct": 0,
        "propagationPct": 0
    }))
    .unwrap();
    stressed.tick_counter = 0;
    let decayed = cracks_on_tick(stressed, &mut c);
    let decayed_index = decayed
        .stress
        .iter()
        .position(|stress| *stress == STRESS_VISIBLE - 16)
        .unwrap();
    assert_eq!(decayed.cells[decayed_index], CLEAR);
    assert_eq!(
        decayed.trigger_types[decayed_index],
        CellTriggerType::Deactivate
    );

    let mut static_state = cracks_deserialize(serde_json::json!({
        "stressPct": 100, "propagationPct": 0
    }))
    .unwrap();
    static_state.cells.fill(CLEAR);
    static_state.stress.fill(0);
    let nudged = cracks_on_tick(static_state, &mut c);
    assert_eq!(nudged.cells[0], TIP);
    assert_eq!(nudged.stress[0], 96);
}

#[test]
fn pending_shatter_dissolves_to_new_impact_and_replace_clears_flag_state() {
    let mut c = ctx();
    let mut s = cracks_init(serde_json::json!({"pendingShatter": true, "stressPct": 0})).unwrap();
    s.cells.fill(CLEAR);
    s.stress.fill(0);
    s.cells[grid_index(1, 1)] = CRACK;
    s.stress[grid_index(2, 2)] = STRESS_VISIBLE;
    for _ in 0..20 {
        s = cracks_on_tick(s, &mut c);
        if !s.pending_shatter {
            break;
        }
    }
    assert!(!s.pending_shatter);
    assert_eq!(s.cells[grid_index(GRID_WIDTH / 2, GRID_HEIGHT / 2)], TIP);

    let replaced = cracks_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "replacePane".into(),
        }),
        &mut c,
    );
    assert!(replaced.cells.iter().all(|cell| *cell == CLEAR));
    assert!(replaced.stress.iter().all(|stress| *stress == 0));
}

#[test]
fn threshold_and_blocked_tip_semantics() {
    let mut c = ctx();
    let mut threshold = cracks_init(serde_json::json!({
        "shatterThreshold": 3, "stressPct": 0, "propagationPct": 0
    }))
    .unwrap();
    threshold.cells[0] = CRACK;
    threshold.cells[1] = CRACK;
    threshold.cells[2] = TIP;
    assert!(cracks_on_tick(threshold, &mut c).pending_shatter);

    let mut blocked = cracks_init(serde_json::json!({
        "stressPct": 0, "propagationPct": 100, "branchPct": 100, "shatterThreshold": 64
    }))
    .unwrap();
    blocked.cells.fill(CRACK);
    blocked.cells[grid_index(3, 3)] = TIP;
    let ticked = cracks_on_tick(blocked, &mut c);
    assert_eq!(ticked.cells[grid_index(3, 3)], TIP);
}
