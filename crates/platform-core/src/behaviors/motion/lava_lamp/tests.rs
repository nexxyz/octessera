use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, CellTriggerType, DeviceInput};
use serde_json::json;
fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}
#[test]
fn menu_serialize_restore() {
    let m = lava_lamp_config_menu();
    assert_eq!(m[0].key, "blobCount");
    assert_eq!(m[5].key, "resetBlobs");
    let s=lava_lamp_init(json!({"x":[999],"vx":[-99],"radius":[99],"activeCount":99,"blobCount":99,"viscosityPct":200,"heatTicks":9,"lastMergeCount":1})).unwrap();
    assert_eq!(s.x.len(), 8);
    assert_eq!(s.radius[0], 40);
    assert_eq!(s.vx[0], -12);
    assert_eq!(s.active_count, 8);
    assert_eq!(s.heat_ticks, 0);
    assert_eq!(s.last_merge_count, 0);
    assert!(s
        .trigger_types
        .iter()
        .all(|t| *t != CellTriggerType::Activate));
    let v = lava_lamp_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert!(v.get("heatTicks").is_none());
    assert!(v.get("lastMergeCount").is_none());
    assert_eq!(
        lava_lamp_serialize(&lava_lamp_deserialize(v.clone()).unwrap()).unwrap(),
        v
    )
}

#[test]
fn init_uses_tuned_defaults() {
    let s = lava_lamp_init(json!({})).unwrap();
    assert_eq!(s.viscosity_pct, 30);
    assert_eq!(s.heat_pct, 45);
}
#[test]
fn input_actions() {
    let mut c = ctx();
    let s = lava_lamp_init(json!({"activeCount":1,"blobCount":1,"x":[0],"y":[0]})).unwrap();
    let h = lava_lamp_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "heatLamp".into(),
        }),
        &mut c,
    );
    assert_eq!(h.heat_ticks, 1);
    let r = lava_lamp_on_input(
        h,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "resetBlobs".into(),
        }),
        &mut c,
    );
    assert_eq!(r.active_count, 4);
    assert_eq!(r.trigger_types[grid_index(2, 1)], CellTriggerType::Activate);
    let r2 = lava_lamp_on_input(
        r.clone(),
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "resetBlobs".into(),
        }),
        &mut c,
    );
    assert_eq!(r2.trigger_types[grid_index(2, 1)], CellTriggerType::Stable);
    let p = lava_lamp_on_input(r, DeviceInput::GridPress { x: 0, y: 0 }, &mut c);
    assert!(p.trigger_types[grid_index(0, 0)] != CellTriggerType::None)
}

#[test]
fn grid_press_at_blob_cap_replaces_nearest_blob() {
    let mut c = ctx();
    let s = lava_lamp_init(json!({
        "activeCount": 8,
        "blobCount": 8,
        "x": [8, 24, 40, 56, 72, 88, 104, 120],
        "y": [8, 24, 40, 56, 72, 88, 104, 120],
        "radius": [18, 18, 18, 18, 18, 18, 18, 18]
    }))
    .unwrap();
    let pressed = lava_lamp_on_input(s, DeviceInput::GridPress { x: 7, y: 7 }, &mut c);

    assert_eq!(pressed.active_count, 8);
    assert_eq!(pressed.blob_count, 8);
    assert_eq!(pressed.x[7], 120);
    assert_eq!(pressed.y[7], 120);
    assert_eq!(pressed.vx[7], 0);
    assert_eq!(pressed.vy[7], 0);
    assert_eq!(pressed.x[0], 8);
}

#[test]
fn reset_normalizes_inactive_tail_and_updates_visible_triggers() {
    let mut c = ctx();
    let s = lava_lamp_init(json!({
        "activeCount": 8,
        "blobCount": 8,
        "x": [8,24,40,56,72,88,104,120],
        "y": [120,104,88,72,56,40,24,8],
        "vx": [12,12,12,12,12,12,12,12],
        "vy": [-12,-12,-12,-12,-12,-12,-12,-12],
        "radius": [40,40,40,40,40,40,40,40]
    }))
    .unwrap();

    let reset = lava_lamp_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "resetBlobs".into(),
        }),
        &mut c,
    );

    assert_eq!(reset.active_count, 4);
    assert_eq!(reset.blob_count, 4);
    assert_eq!(&reset.x[4..], &[0, 0, 0, 0]);
    assert_eq!(&reset.y[4..], &[0, 0, 0, 0]);
    assert_eq!(&reset.vx[4..], &[0, 0, 0, 0]);
    assert_eq!(&reset.vy[4..], &[0, 0, 0, 0]);
    assert_eq!(&reset.radius[4..], &[18, 18, 18, 18]);
    assert!(reset
        .trigger_types
        .iter()
        .any(|trigger| *trigger == CellTriggerType::Activate));
    assert!(reset
        .trigger_types
        .iter()
        .any(|trigger| *trigger == CellTriggerType::Deactivate));
}

#[test]
fn boundary_velocity_reflects_on_both_axes() {
    let mut c = ctx();
    let s = lava_lamp_init(json!({
        "activeCount": 1,
        "blobCount": 1,
        "x": [0],
        "y": [127],
        "vx": [-12],
        "vy": [12],
        "viscosityPct": 0,
        "heatPct": 0,
        "mergePct": 0
    }))
    .unwrap();
    let ticked = lava_lamp_on_tick(s, &mut c);

    assert!((0..=16).contains(&ticked.x[0]));
    assert!((MAX_POS - 16..=MAX_POS).contains(&ticked.y[0]));
    assert!(ticked.vx[0] > 0);
}

#[test]
fn tick_merge_split_reflect() {
    let mut c = ctx();
    let s = lava_lamp_init(
        json!({"activeCount":1,"blobCount":1,"x":[0],"y":[0],"vx":[-12],"vy":[-12],"heatPct":100}),
    )
    .unwrap();
    let t = lava_lamp_on_tick(s, &mut c);
    assert!(t.x[0] >= 0 && t.y[0] >= 0);
    let s=lava_lamp_init(json!({"activeCount":2,"blobCount":2,"x":[32,34],"y":[32,34],"radius":[24,24],"mergePct":100})).unwrap();
    let t = lava_lamp_on_tick(s, &mut c);
    assert_eq!(t.active_count, 1);
    assert_eq!(t.last_merge_count, 1);
    let v = lava_lamp_serialize(&t).unwrap();
    assert_eq!(
        lava_lamp_serialize(&lava_lamp_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let s =
        lava_lamp_init(json!({"activeCount":1,"blobCount":1,"radius":[40],"heatPct":100})).unwrap();
    let t = lava_lamp_on_tick(s, &mut c);
    assert!(t.active_count >= 1);
    assert_eq!(lava_lamp_render_model(&t).name, "lava lamp")
}

#[test]
fn default_self_sustains_with_bounded_forced_activity() {
    let mut c = ctx();
    let mut state = lava_lamp_init(json!({})).unwrap();
    let mut previous = lava_lamp_render_model(&state).cells;
    let mut terminal_same = 1usize;
    let mut final_frames = Vec::new();
    for _ in 0..300 {
        state = lava_lamp_on_tick(state, &mut c);
        let frame = lava_lamp_render_model(&state).cells;
        terminal_same = if frame == previous {
            terminal_same + 1
        } else {
            1
        };
        assert!(
            state
                .trigger_types
                .iter()
                .filter(|trigger| **trigger == CellTriggerType::Activate)
                .count()
                <= 16
        );
        final_frames.push(frame.clone());
        if final_frames.len() > 16 {
            final_frames.remove(0);
        }
        previous = frame;
    }
    assert!(terminal_same <= 2);
    assert!(final_frames.windows(2).any(|window| window[0] != window[1]));
}
