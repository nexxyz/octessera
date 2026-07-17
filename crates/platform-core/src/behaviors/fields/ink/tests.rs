use super::*;

fn ctx() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_palette_normalize_serialize() {
    let m = ink_config_menu();
    assert_eq!(m[0].key, "diffusionPct");
    assert_eq!(m[0].max, Some(100));
    assert_eq!(m[2].key, "dropStrength");
    assert_eq!(m[2].max, Some(255));
    assert_eq!(m[3].key, "autoDropInterval");
    assert_eq!(m[4].key, "spawnStep");
    assert_eq!(m[5].key, "dropInk");
    assert_eq!(m[6].key, "clearInk");
    let s = ink_deserialize(serde_json::json!({"ink":[999,7],"diffusionPct":999,"fadePct":999,"dropStrength":999,"autoDropInterval":999,"spawnStep":999,"tickCounter":9,"triggerTypes":["activate"]})).unwrap();
    assert_eq!(s.ink[0], 255);
    assert_eq!(s.diffusion_pct, 100);
    assert_eq!(s.fade_pct, 100);
    assert_eq!(s.drop_strength, 255);
    assert_eq!(s.auto_drop_interval, 64);
    assert_eq!(s.spawn_step, 63);
    assert_eq!(s.tick_counter, 0);
    assert!(!s.trigger_types.contains(&CellTriggerType::Activate));
    let v = ink_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert_eq!(
        ink_serialize(&ink_deserialize(v.clone()).unwrap()).unwrap(),
        v
    );
    let model = ink_render_model(&s);
    assert_eq!(model.name, "ink");
    assert_eq!(model.palette.active, [120, 80, 255]);
    assert_eq!(model.palette.inactive, crate::palette::BLACK);
}

#[test]
fn default_autodrops_keep_ink_alive_with_bounded_pulses() {
    let mut c = ctx();
    let mut s = ink_init(serde_json::json!({})).unwrap();
    let mut pulse_counts = Vec::new();
    for _ in 0..128 {
        s = ink_on_tick(s, &mut c);
        let activates = s
            .trigger_types
            .iter()
            .filter(|trigger| **trigger == CellTriggerType::Activate)
            .count();
        if activates > 0 {
            pulse_counts.push(activates);
        }
    }
    assert!(s.ink.iter().filter(|value| **value >= VISIBLE).count() > 0);
    assert!(pulse_counts.len() >= 4);
    assert!(pulse_counts.iter().all(|count| *count <= 5));
}

#[test]
fn grid_press_force_activate_and_clear_deactivates() {
    let mut c = ctx();
    let s = ink_init(serde_json::json!({"dropStrength":20})).unwrap();
    let s = ink_on_input(s, DeviceInput::GridPress { x: 2, y: 3 }, &mut c);
    assert_eq!(s.ink[grid_index(2, 3)], 20);
    assert_eq!(s.trigger_types[grid_index(2, 3)], CellTriggerType::Activate);
    let cleared = ink_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "clearInk".into(),
        }),
        &mut c,
    );
    assert_eq!(cleared.ink[grid_index(2, 3)], 0);
    assert_eq!(
        cleared.trigger_types[grid_index(2, 3)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn diffusion_fade_thresholds_and_edges() {
    let mut c = ctx();
    let mut s = ink_init(serde_json::json!({"diffusionPct":100,"fadePct":0})).unwrap();
    s.ink[grid_index(1, 0)] = 255;
    let t = ink_on_tick(s, &mut c);
    assert!(t.ink[grid_index(0, 0)] > 0);
    assert_eq!(t.ink[grid_index(GRID_WIDTH - 1, 0)], 0);
    let mut s = ink_init(serde_json::json!({"diffusionPct":0,"fadePct":100})).unwrap();
    s.ink[0] = VISIBLE;
    let t = ink_on_tick(s, &mut c);
    assert_eq!(t.trigger_types[0], CellTriggerType::Deactivate);
    let mut s = ink_init(serde_json::json!({"diffusionPct":0,"fadePct":0})).unwrap();
    s.ink[0] = ACTIVATE - 1;
    let pressed = ink_on_input(s, DeviceInput::GridPress { x: 0, y: 0 }, &mut c);
    assert_eq!(pressed.trigger_types[0], CellTriggerType::Activate);
}

#[test]
fn drop_ink_splash_forces_activate() {
    let mut c = ctx();
    let s = ink_init(serde_json::json!({"dropStrength":40})).unwrap();
    let s = ink_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "dropInk".into(),
        }),
        &mut c,
    );
    assert_eq!(
        s.trigger_types[grid_index(GRID_WIDTH / 2, GRID_HEIGHT / 2)],
        CellTriggerType::Activate
    );
    assert!(s.trigger_types.contains(&CellTriggerType::Activate));

    let s = ink_init(serde_json::json!({"dropStrength":1})).unwrap();
    let s = ink_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "dropInk".into(),
        }),
        &mut c,
    );
    assert_eq!(
        s.trigger_types
            .iter()
            .filter(|trigger| **trigger == CellTriggerType::Activate)
            .count(),
        1
    );
}
