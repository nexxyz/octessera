use super::*;

#[test]
fn init_reads_spawn_step_from_config() {
    let state = dla_init(serde_json::json!({ "spawnStep": 7 })).unwrap();
    assert_eq!(state.spawn_step, 7);
    let state = dla_init(serde_json::json!({ "spawnStep": 999 })).unwrap();
    assert_eq!(state.spawn_step, 63);
}

#[test]
fn init_creates_seed_cluster_and_grid_press_toggles_cell() {
    let mut context = BehaviorContext::new(120.0);
    let state = dla_init(Value::Null).unwrap();
    let cx = GRID_WIDTH / 2;
    let cy = GRID_HEIGHT / 2;
    assert!(state.cells[grid_index(cx, cy)]);
    assert!(state.cells[grid_index(cx + 1, cy)]);
    assert!(state.cells[grid_index(cx, cy + 1)]);

    let toggled = dla_on_input(state, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
    assert!(toggled.cells[grid_index(1, 1)]);
    assert_eq!(
        toggled.trigger_types[grid_index(1, 1)],
        CellTriggerType::Activate
    );
    let toggled = dla_on_input(toggled, DeviceInput::GridPress { x: 1, y: 1 }, &mut context);
    assert!(!toggled.cells[grid_index(1, 1)]);
    assert_eq!(
        toggled.trigger_types[grid_index(1, 1)],
        CellTriggerType::Deactivate
    );
}

#[test]
fn dla_render_and_config_menu_match_contract() {
    let state = dla_init(Value::Null).unwrap();
    let model = dla_render_model(&state);
    assert_eq!(model.name, "dla");
    assert_eq!(model.trigger_types.as_ref().unwrap().len(), CELL_COUNT);
    assert_eq!(
        dla_config_menu()
            .iter()
            .map(|item| item.key.as_str())
            .collect::<Vec<_>>(),
        vec!["spawnInterval", "spawnStep", "cellLife", "seedCluster"]
    );
}

#[test]
fn old_payloads_normalize_ages_and_cell_life() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true, false],
        ages: Vec::new(),
        trigger_types: Vec::new(),
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 300,
        tick_counter: 0,
    };

    let ticked = dla_on_tick(state, &mut context);

    assert_eq!(ticked.cells.len(), CELL_COUNT);
    assert_eq!(ticked.ages.len(), CELL_COUNT);
    assert_eq!(ticked.cell_life, MAX_CELL_LIFE);
}

#[test]
fn old_short_payload_grid_input_normalizes_before_trigger_recompute() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true],
        ages: Vec::new(),
        trigger_types: Vec::new(),
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 0,
        tick_counter: 0,
    };

    let next = dla_on_input(state, DeviceInput::GridPress { x: 1, y: 0 }, &mut context);

    assert_eq!(next.cells.len(), CELL_COUNT);
    assert_eq!(next.ages.len(), CELL_COUNT);
    assert_eq!(next.trigger_types.len(), CELL_COUNT);
    assert_eq!(
        next.trigger_types[grid_index(1, 0)],
        CellTriggerType::Activate
    );
}

#[test]
fn seed_cluster_action_and_no_frontier_tick_are_stable() {
    let mut context = BehaviorContext::new(120.0);
    let empty = DlaState {
        cells: vec![false; CELL_COUNT],
        ages: vec![0; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        spawn_interval: 1,
        spawn_step: 0,
        cell_life: 0,
        tick_counter: 0,
    };
    let ticked = dla_on_tick(empty.clone(), &mut context);
    assert!(ticked.cells.iter().all(|cell| !*cell));
    assert!(ticked
        .trigger_types
        .iter()
        .all(|trigger| *trigger == CellTriggerType::None));

    let seeded = dla_on_input(
        empty,
        DeviceInput::BehaviorAction(BehaviorActionInput {
            action_type: "seedCluster".into(),
        }),
        &mut context,
    );
    assert!(seeded.cells.iter().filter(|cell| **cell).count() >= 1);
    assert!(seeded.trigger_types.contains(&CellTriggerType::Activate));
}

#[test]
fn cell_life_expires_old_cells_and_reseeds_when_empty() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true; CELL_COUNT],
        ages: vec![0; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 1,
        tick_counter: 0,
    };

    let ticked = dla_on_tick(state, &mut context);

    assert_eq!(ticked.cells.iter().filter(|cell| **cell).count(), 3);
    assert!(ticked.ages.iter().all(|age| *age == 0));
}

#[test]
fn cell_life_zero_disables_aging_and_expiry() {
    let mut context = BehaviorContext::new(120.0);
    let state = DlaState {
        cells: vec![true; CELL_COUNT],
        ages: vec![250; CELL_COUNT],
        trigger_types: vec![CellTriggerType::None; CELL_COUNT],
        spawn_interval: 0,
        spawn_step: 0,
        cell_life: 0,
        tick_counter: 0,
    };

    let ticked = dla_on_tick(state, &mut context);

    assert_eq!(
        ticked.cells.iter().filter(|cell| **cell).count(),
        CELL_COUNT
    );
    assert!(ticked.ages.iter().all(|age| *age == 250));
}

#[test]
fn default_tail_stays_bounded_and_non_terminal() {
    let mut context = BehaviorContext::new(120.0);
    let mut state = dla_init(Value::Null).unwrap();
    let mut same = 0;
    let mut terminal = 0;
    let mut previous = state.cells.clone();
    for _ in 0..300 {
        state = dla_on_tick(state, &mut context);
        same = if state.cells == previous { same + 1 } else { 0 };
        terminal = if state.cells.iter().all(|cell| *cell) || state.cells.iter().all(|cell| !*cell)
        {
            terminal + 1
        } else {
            0
        };
        assert!(same <= 2);
        assert!(terminal <= 2);
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
        assert!(bursts <= 4);
        previous = state.cells.clone();
    }
}
