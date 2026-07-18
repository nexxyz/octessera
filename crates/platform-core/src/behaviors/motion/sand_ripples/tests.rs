use super::*;
use crate::behavior::{BehaviorActionInput, BehaviorContext, CellTriggerType, DeviceInput};
use serde_json::json;
fn context() -> BehaviorContext {
    BehaviorContext::new(120.0)
}
#[test]
fn menu_serialization() {
    let menu = sand_ripples_config_menu();
    assert_eq!(menu[0].key, "windStrengthPct");
    assert_eq!(menu[5].key, "seedDunes");
    let s = sand_ripples_init(
        json!({"sand":[300],"crest":[8],"windDir":"bogus","windStrengthPct":200,"gustTicks":9}),
    )
    .unwrap();
    assert_eq!(s.sand[0], 255);
    assert_eq!(s.wind_dir, "east");
    assert_eq!(s.wind_strength_pct, 100);
    assert_eq!(s.gust_ticks, 0);
    assert!(s
        .trigger_types
        .iter()
        .all(|t| *t != CellTriggerType::Activate));
    let v = sand_ripples_serialize(&s).unwrap();
    assert!(v.get("triggerTypes").is_none());
    assert!(v.get("tickCounter").is_none());
    assert!(v.get("gustTicks").is_none());
    assert_eq!(
        sand_ripples_serialize(&sand_ripples_deserialize(v.clone()).unwrap()).unwrap(),
        v
    )
}
#[test]
fn input_actions() {
    let mut ctx = context();
    let s = sand_ripples_init(json!({})).unwrap();
    let p = sand_ripples_on_input(s, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(p.sand[0], 96);
    assert_eq!(p.crest[0], 64);
    assert_eq!(p.trigger_types[0], CellTriggerType::Activate);
    let g = sand_ripples_on_input(
        p,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "gust".into(),
        }),
        &mut ctx,
    );
    assert_eq!(g.gust_ticks, 1);
    let w = sand_ripples_on_input(
        g,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "shiftWind".into(),
        }),
        &mut ctx,
    );
    assert_eq!(w.wind_dir, "north");
    let seeded = sand_ripples_on_input(
        w,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedDunes".into(),
        }),
        &mut ctx,
    );
    assert_eq!(seeded.sand[grid_index(7, 6)], 120);
    assert_eq!(
        seeded.trigger_types[grid_index(7, 6)],
        CellTriggerType::Activate
    );
    let s = sand_ripples_init(json!({"sand":[120],"crest":[0]})).unwrap();
    let p = sand_ripples_on_input(s, DeviceInput::GridPress { x: 0, y: 0 }, &mut ctx);
    assert_eq!(p.trigger_types[0], CellTriggerType::Activate);
    let mut sand = vec![0; CELL_COUNT];
    let mut crest = vec![0; CELL_COUNT];
    sand[grid_index(0, 1)] = 120;
    crest[grid_index(0, 1)] = 0;
    let s = sand_ripples_init(json!({"sand":sand,"crest":crest})).unwrap();
    let seeded = sand_ripples_on_input(
        s,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedDunes".into(),
        }),
        &mut ctx,
    );
    assert_eq!(
        seeded.trigger_types[grid_index(0, 1)],
        CellTriggerType::Activate
    )
}
#[test]
fn transport_decay_and_nonwrap() {
    let mut ctx = context();
    let mut sand = vec![0; CELL_COUNT];
    sand[grid_index(0, 0)] = 100;
    let s=sand_ripples_init(json!({"sand":sand,"windStrengthPct":100,"erosionPct":100,"depositionPct":100,"windDir":"east"})).unwrap();
    let t = sand_ripples_on_tick(s, &mut ctx);
    assert!(t.sand[grid_index(1, 0)] > 0);
    assert!(t.crest[grid_index(1, 0)] > 0);
    let mut sand = vec![0; CELL_COUNT];
    sand[grid_index(0, 0)] = 100;
    let s = sand_ripples_init(
        json!({"sand":sand,"windStrengthPct":0,"erosionPct":100,"windDir":"east"}),
    )
    .unwrap();
    let t = sand_ripples_on_tick(s, &mut ctx);
    assert_eq!(t.sand[grid_index(0, 0)], 100);
    assert_eq!(t.sand[grid_index(1, 0)], 0);
    let mut sand = vec![0; CELL_COUNT];
    sand[grid_index(7, 0)] = 100;
    let s = sand_ripples_init(
        json!({"sand":sand,"windStrengthPct":100,"erosionPct":100,"windDir":"east"}),
    )
    .unwrap();
    let t = sand_ripples_on_tick(s, &mut ctx);
    assert_eq!(t.sand[grid_index(7, 0)], 100);
    let s = sand_ripples_init(json!({"crest":[16],"erosionPct":0})).unwrap();
    let t = sand_ripples_on_tick(s, &mut ctx);
    assert_eq!(t.crest[0], 15);
    assert_eq!(t.trigger_types[0], CellTriggerType::Deactivate)
}

#[test]
fn default_self_sustains_with_bounded_renewal() {
    let mut ctx = context();
    let mut state = sand_ripples_init(json!({})).unwrap();
    let mut previous = sand_ripples_render_model(&state).cells;
    let mut terminal_same = 1usize;
    let mut final_frames = Vec::new();
    let mut full_run = 0usize;
    let mut empty_run = 0usize;
    for _ in 0..300 {
        state = sand_ripples_on_tick(state, &mut ctx);
        let frame = sand_ripples_render_model(&state).cells;
        let visible = frame.iter().filter(|cell| **cell).count();
        full_run = if visible == CELL_COUNT {
            full_run + 1
        } else {
            0
        };
        empty_run = if visible == 0 { empty_run + 1 } else { 0 };
        assert!(full_run <= 1);
        assert!(empty_run <= 1);
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
                <= CELL_COUNT / 2
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
