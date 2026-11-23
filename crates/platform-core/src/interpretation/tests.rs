use super::*;

fn snapshot(tiles: &[u8]) -> GridSnapshot {
    GridSnapshot {
        width: 4,
        height: 2,
        tiles: tiles.iter().map(|tile| *tile != 0).collect(),
        trigger_types: None,
    }
}

fn snapshot_with_triggers(tiles: &[u8], trigger_types: Vec<TileTriggerType>) -> GridSnapshot {
    GridSnapshot {
        width: 4,
        height: 2,
        tiles: tiles.iter().map(|tile| *tile != 0).collect(),
        trigger_types: Some(trigger_types),
    }
}

fn profile(tick: TickStrategy) -> InterpretationProfile {
    InterpretationProfile {
        id: "menu_profile".into(),
        event: InterpretationEventProfile { enabled: true },
        state: InterpretationStateProfile {
            enabled: true,
            tick,
        },
        x: AxisStrategy::ScaleStep { step: 1 },
        y: AxisStrategy::ScaleStep { step: 2 },
    }
}

#[test]
fn extracts_transitions() {
    let previous = snapshot(&[0, 1, 0, 0, 1, 1, 0, 0]);
    let next = snapshot(&[1, 1, 0, 0, 0, 1, 1, 0]);
    assert_eq!(
        extract_transitions(&previous, &next),
        vec![
            TileTransition {
                x: 0,
                y: 0,
                kind: TileTransitionKind::Activate
            },
            TileTransition {
                x: 0,
                y: 1,
                kind: TileTransitionKind::Deactivate
            },
            TileTransition {
                x: 2,
                y: 1,
                kind: TileTransitionKind::Activate
            },
        ]
    );
}

fn rows(sections: Option<usize>, reverse: bool) -> TickStrategy {
    TickStrategy::ScanRowActive {
        sections,
        reverse,
        restart_pitch: false,
    }
}

/// What a scan plays at `tick` when the line last saw `seen` and the grid is `next` now, Hold
/// events included.
fn scan(
    seen: &[u8],
    next: &[u8],
    tick: usize,
    profile: &InterpretationProfile,
) -> Vec<(usize, usize, TileTriggerKind)> {
    interpret_grid_with_marker_authority(
        &snapshot(seen),
        &snapshot(next),
        tick,
        profile,
        TriggerMarkerAuthority::FreshTick,
        true,
    )
    .into_iter()
    .map(|intent| (intent.x, intent.y, intent.kind))
    .collect()
}

#[test]
fn a_scan_compares_each_tile_with_the_tile_the_line_touched_one_step_before() {
    let intents = interpret_grid(
        &snapshot(&[1, 1, 0, 0, 0, 0, 0, 0]),
        &snapshot(&[0, 0, 0, 0, 1, 0, 1, 0]),
        1,
        &profile(rows(None, false)),
    );
    assert_eq!(
        intents,
        vec![
            TileTriggerIntent {
                x: 1,
                y: 1,
                kind: TileTriggerKind::Deactivate,
                degree: 3
            },
            TileTriggerIntent {
                x: 2,
                y: 1,
                kind: TileTriggerKind::Activate,
                degree: 4
            },
        ]
    );
    assert_eq!(
        scan(
            &[1, 1, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0, 1, 0, 1, 0],
            1,
            &profile(rows(None, false))
        ),
        vec![
            (0, 1, TileTriggerKind::Stable),
            (1, 1, TileTriggerKind::Deactivate),
            (2, 1, TileTriggerKind::Activate),
        ]
    );
}

#[test]
fn a_scan_compares_across_the_lap_wrap() {
    assert_eq!(
        scan(
            &[0, 0, 0, 0, 0, 1, 0, 1],
            &[0, 1, 1, 0, 0, 0, 0, 0],
            0,
            &profile(rows(None, false))
        ),
        vec![
            (1, 0, TileTriggerKind::Stable),
            (2, 0, TileTriggerKind::Activate),
            (3, 0, TileTriggerKind::Deactivate),
        ]
    );
}

#[test]
fn a_reverse_scan_starts_on_the_last_row_and_compares_with_the_row_above() {
    assert_eq!(
        scan(
            &[1, 0, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0, 1, 0, 0, 1],
            0,
            &profile(rows(None, true))
        ),
        vec![
            (0, 1, TileTriggerKind::Stable),
            (3, 1, TileTriggerKind::Activate),
        ]
    );
}

#[test]
fn a_sectioned_scan_plays_only_its_lane_and_wraps_inside_it() {
    assert_eq!(
        scan(
            &[0, 0, 0, 0, 1, 0, 1, 1],
            &[1, 1, 1, 1, 0, 0, 0, 0],
            0,
            &profile(rows(Some(2), false))
        ),
        vec![
            (0, 0, TileTriggerKind::Stable),
            (1, 0, TileTriggerKind::Activate),
        ]
    );
    let columns = TickStrategy::ScanColumnActive {
        sections: Some(2),
        reverse: true,
        restart_pitch: false,
    };
    assert_eq!(
        scan(
            &[0, 0, 0, 0, 0, 0, 1, 0],
            &[0, 0, 0, 0, 0, 0, 0, 1],
            0,
            &profile(columns)
        ),
        vec![(3, 1, TileTriggerKind::Activate)]
    );
}

#[test]
fn event_triggers_off_leave_a_scan_only_its_hold_events() {
    let mut profile = profile(rows(None, false));
    profile.event.enabled = false;
    assert_eq!(
        scan(
            &[1, 1, 0, 0, 0, 0, 0, 0],
            &[0, 0, 0, 0, 1, 0, 1, 0],
            1,
            &profile
        ),
        vec![(0, 1, TileTriggerKind::Stable)]
    );
}

#[test]
fn zero_sized_scan_snapshot_is_safe() {
    let empty = GridSnapshot {
        width: 0,
        height: 0,
        tiles: Vec::new(),
        trigger_types: None,
    };
    assert!(interpret_grid(
        &empty,
        &empty,
        12,
        &profile(TickStrategy::ScanRowActive {
            sections: None,
            reverse: false,
            restart_pitch: false,
        }),
    )
    .is_empty());
}

#[test]
fn event_candidates_use_render_trigger_types_when_valid() {
    let previous = snapshot_with_triggers(
        &[0, 1, 0, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::None,
            TileTriggerType::Stable,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );
    let next = snapshot_with_triggers(
        &[0, 1, 1, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::None,
            TileTriggerType::Activate,
            TileTriggerType::Deactivate,
            TileTriggerType::Stable,
            TileTriggerType::Scanned,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );
    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::WholeGridTransitions),
    );
    assert_eq!(
        intents,
        vec![
            TileTriggerIntent {
                x: 1,
                y: 0,
                kind: TileTriggerKind::Activate,
                degree: 1
            },
            TileTriggerIntent {
                x: 2,
                y: 0,
                kind: TileTriggerKind::Deactivate,
                degree: 2
            }
        ]
    );
}

#[test]
fn event_candidates_ignore_persistent_stale_render_trigger_types() {
    let stale_triggers = vec![
        TileTriggerType::Activate,
        TileTriggerType::Deactivate,
        TileTriggerType::Stable,
        TileTriggerType::None,
        TileTriggerType::None,
        TileTriggerType::None,
        TileTriggerType::None,
        TileTriggerType::None,
    ];
    let previous = snapshot_with_triggers(&[1, 1, 0, 0, 0, 0, 0, 0], stale_triggers.clone());
    let next = snapshot_with_triggers(&[1, 1, 0, 0, 0, 0, 0, 0], stale_triggers);

    let intents = interpret_grid(
        &previous,
        &next,
        0,
        &profile(TickStrategy::WholeGridTransitions),
    );

    assert!(intents.is_empty());
}

#[test]
fn fresh_tick_markers_preserve_nonzero_activation_intents() {
    let previous = snapshot_with_triggers(
        &[1, 0, 0, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::Activate,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );
    let next = snapshot_with_triggers(
        &[2, 0, 0, 0, 0, 0, 0, 0],
        vec![
            TileTriggerType::Activate,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
            TileTriggerType::None,
        ],
    );

    let intents = interpret_grid_with_marker_authority(
        &previous,
        &next,
        0,
        &profile(TickStrategy::WholeGridTransitions),
        TriggerMarkerAuthority::FreshTick,
        false,
    );

    assert_eq!(intents.len(), 1);
    assert_eq!(intents[0].kind, TileTriggerKind::Activate);
}

#[test]
fn restart_pitch_counts_from_each_section_lane_on_the_split_axis() {
    let previous = snapshot(&[0, 0, 0, 0, 0, 0, 0, 0]);
    let next = snapshot(&[1, 1, 1, 1, 1, 1, 1, 1]);
    let degrees = |tick: TickStrategy, step: usize| {
        let mut intents = interpret_grid(&previous, &next, step, &profile(tick))
            .into_iter()
            .filter(|intent| intent.kind == TileTriggerKind::Activate)
            .map(|intent| (intent.x, intent.y, intent.degree))
            .collect::<Vec<_>>();
        intents.sort();
        intents
    };
    let columns = |restart_pitch| TickStrategy::ScanColumnActive {
        sections: Some(2),
        reverse: false,
        restart_pitch,
    };
    assert_eq!(degrees(columns(false), 5), vec![(1, 1, 3)]);
    assert_eq!(degrees(columns(true), 5), vec![(1, 1, 1)]);

    let rows = |restart_pitch| TickStrategy::ScanRowActive {
        sections: Some(2),
        reverse: false,
        restart_pitch,
    };
    assert_eq!(degrees(rows(false), 2), vec![(2, 0, 2), (3, 0, 3)]);
    assert_eq!(degrees(rows(true), 2), vec![(2, 0, 0), (3, 0, 1)]);
}
