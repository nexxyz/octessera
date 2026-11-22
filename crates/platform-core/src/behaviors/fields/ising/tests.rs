use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, DeviceInput, TileTriggerType};
use serde_json::json;

fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}

#[test]
fn menu_normalize_serialize_and_restore_quiet() {
    let menu = ising_config_menu();
    assert_eq!(menu[0].key, "temperaturePct");
    assert_eq!(menu[1].key, "fieldStrengthPct");
    assert_eq!(menu[2].key, "noisePct");
    assert_eq!(menu[3].key, "heatPulse");
    assert_eq!(menu[4].key, "flipField");
    assert_eq!(menu[5].key, "randomizeSpins");
    let state =
        ising_init(json!({"spins":[-2,0,"bad"],"fieldSign":-9,"temperaturePct":200})).unwrap();
    assert_eq!(state.spins.len(), TILE_COUNT);
    assert_eq!(state.spins[0], -1);
    assert_eq!(state.spins[1], 1);
    assert_eq!(state.field_sign, -1);
    assert_eq!(state.temperature_pct, 100);
    assert!(state
        .trigger_types
        .iter()
        .all(|t| *t != TileTriggerType::Activate));
    let value = ising_serialize(&state).unwrap();
    assert!(value.get("triggerTypes").is_none());
    assert!(value.get("heatTicks").is_none());
    assert!(value.get("tickCounter").is_none());
    let restored = ising_deserialize(value.clone()).unwrap();
    assert_eq!(ising_serialize(&restored).unwrap(), value);
    assert_eq!(ising_render_model(&restored).name, "ising");
}

#[test]
fn grid_press_and_actions_follow_trigger_contract() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut ctx = context();
        let state = ising_init(json!({"spins":[-1],"noisePct":0})).unwrap();
        let pressed = ising_on_input(state, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
        assert_eq!(pressed.spins[grid_index(0, 0)], 1);
        assert_eq!(
            pressed.trigger_types[grid_index(0, 0)],
            TileTriggerType::Activate
        );
        let heated = ising_on_input(
            pressed.clone(),
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "heatPulse".into(),
            }),
            &mut ctx,
        );
        assert_eq!(heated.heat_ticks, 1);
        assert_eq!(
            heated.trigger_types[grid_index(0, 0)],
            TileTriggerType::Stable
        );
        let flipped = ising_on_input(
            heated.clone(),
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "flipField".into(),
            }),
            &mut ctx,
        );
        assert_eq!(flipped.field_sign, -1);
        assert_eq!(
            flipped.trigger_types[grid_index(0, 0)],
            TileTriggerType::Stable
        );
        let randomized = ising_on_input(
            flipped,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "randomizeSpins".into(),
            }),
            &mut ctx,
        );
        let again = ising_on_input(
            heated,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "flipField".into(),
            }),
            &mut ctx,
        );
        let again = ising_on_input(
            again,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "randomizeSpins".into(),
            }),
            &mut ctx,
        );
        assert_ne!(randomized.spins, again.spins);
        let positives = randomized.spins.iter().filter(|spin| **spin > 0).count();
        assert!((8..56).contains(&positives), "{positives}");
    });
}

#[test]
fn tick_uses_neighbors_field_heat_and_clipped_edges() {
    crate::behavior_random::LayerRandom::new(Some(1)).scope(|| {
        let mut ctx = context();
        let mut spins = vec![-1; TILE_COUNT];
        spins[grid_index(1, 0)] = 1;
        spins[grid_index(0, 1)] = 1;
        let state =
            ising_init(json!({"spins":spins,"temperaturePct":0,"noisePct":0,"fieldStrengthPct":0}))
                .unwrap();
        let ticked = ising_on_tick(state, &mut ctx);
        assert_eq!(ticked.spins[grid_index(0, 0)], 1);
        assert_eq!(
            ticked.trigger_types[grid_index(0, 0)],
            TileTriggerType::Activate
        );
        let state = ising_init(
            json!({"spins":[-1],"fieldStrengthPct":100,"temperaturePct":0,"noisePct":0}),
        )
        .unwrap();
        let ticked = ising_on_tick(state, &mut ctx);
        assert_eq!(ticked.spins[grid_index(0, 0)], 1);
        let state = ising_init(
        json!({"spins":[1],"noisePct":100,"fieldStrengthPct":0,"temperaturePct":100,"heatTicks":1}),
    )
    .unwrap();
        assert_eq!(state.heat_ticks, 0);
        let state = ising_on_input(
            state,
            DeviceInput::BehaviorAction(BehaviorActionInput {
                action_type: "heatPulse".into(),
            }),
            &mut ctx,
        );
        let ticked = ising_on_tick(state, &mut ctx);
        assert_eq!(ticked.heat_ticks, 0);
        assert_eq!(
            ticked.trigger_types[grid_index(0, 0)],
            TileTriggerType::Deactivate
        );
        assert!(ising_render_model(&ticked).status_line.starts_with("+:"));
    });
}

#[test]
fn flip_odds_follow_metropolis_in_whole_units() {
    let half = 1u32 << 31;
    assert_eq!(flip_threshold(-1, 0), u32::MAX);
    assert_eq!(flip_threshold(5, 0), 0);
    assert_eq!(flip_threshold(0, 100), half);
    let e_to_minus_one = f64::from(flip_threshold(400, 400)) / 4_294_967_296.0;
    assert!(
        (e_to_minus_one - (-1.0f64).exp()).abs() < 1e-4,
        "{e_to_minus_one}"
    );
    assert_eq!(flip_threshold(1_000_000, 1), 0);
}
