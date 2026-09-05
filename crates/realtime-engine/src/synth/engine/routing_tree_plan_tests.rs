use super::render_plan::RenderPlan;
use super::routing_tree_plan::{RoutingTreePlan, INVALID_COMPONENT_ID};
use crate::synth::test_allocator;
use crate::synth::types::{
    default_synth_config, FxBusConfig, FxBusSlotConfig, InstrumentMixerConfig,
    InstrumentSlotConfig, InstrumentsConfig, MasterFxConfig, MixerConfig, DEFAULT_PAN_POSITIONS,
    INSTRUMENT_SLOT_COUNT,
};
use serde_json::json;

const ROUTING_ROUTES: [&str; INSTRUMENT_SLOT_COUNT] = [
    "fx_bus_1", "direct", "fx_bus_1", "fx_bus_2", "fx_bus_3", "direct", "fx_bus_3", "fx_bus_4",
];

#[test]
fn shipped_analogue_graph_is_one_component_with_configured_tail_buses() {
    let plan = routing_plan(
        [
            "synth", "sampler", "synth", "synth", "none", "none", "none", "none",
        ],
        [
            "fx_bus_1", "direct", "fx_bus_1", "fx_bus_2", "direct", "direct", "direct", "direct",
        ],
        vec![vec![duck("I2")], vec![duck("I1")], vec![], vec![]],
    );

    assert_eq!(
        plan.slot_component,
        [
            0,
            0,
            0,
            0,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID
        ]
    );
    assert_eq!(plan.bus_component, [0, 0, 1, 2]);
    assert_eq!(plan.component_count, 3);
    assert_eq!(
        plan.component_masks,
        [
            mask(&[0, 1, 2, 3, 8, 9]),
            mask(&[10]),
            mask(&[11]),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            0,
        ]
    );
}

#[test]
fn duplicated_analogue_graph_has_two_deterministic_components() {
    let plan = routing_plan(
        [
            "synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth",
        ],
        ROUTING_ROUTES,
        vec![
            vec![duck("I2")],
            vec![duck("I1")],
            vec![duck("I6")],
            vec![duck("I5")],
        ],
    );

    assert_eq!(plan.slot_component, [0, 0, 0, 0, 1, 1, 1, 1]);
    assert_eq!(plan.bus_component, [0, 0, 1, 1]);
    assert_eq!(plan.component_count, 2);
    assert_eq!(plan.component_masks[0], mask(&[0, 1, 2, 3, 8, 9]));
    assert_eq!(plan.component_masks[1], mask(&[4, 5, 6, 7, 10, 11]));
    assert!(plan.component_masks[2..].iter().all(|mask| *mask == 0));
}

#[test]
fn direct_instruments_are_isolated_without_dependencies() {
    let plan = routing_plan(
        [
            "synth", "sampler", "none", "none", "none", "none", "none", "none",
        ],
        [
            "direct", "direct", "direct", "direct", "direct", "direct", "direct", "direct",
        ],
        Vec::new(),
    );

    assert_eq!(
        plan.slot_component,
        [
            0,
            1,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID
        ]
    );
    assert_eq!(plan.bus_component, [INVALID_COMPONENT_ID; 4]);
    assert_eq!(plan.component_masks[0], mask(&[0]));
    assert_eq!(plan.component_masks[1], mask(&[1]));
    assert_eq!(plan.component_count, 2);
}

#[test]
fn shared_instrument_and_bus_dependencies_merge_transitively() {
    let plan = routing_plan(
        [
            "synth", "sampler", "synth", "none", "none", "none", "none", "none",
        ],
        [
            "fx_bus_1", "direct", "direct", "direct", "direct", "direct", "direct", "direct",
        ],
        vec![vec![duck("I2")], vec![duck("B1")], vec![], vec![]],
    );

    assert_eq!(
        plan.slot_component,
        [
            0,
            0,
            1,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID
        ]
    );
    assert_eq!(plan.bus_component, [0, 0, 2, 3]);
    assert_eq!(plan.component_masks[0], mask(&[0, 1, 8, 9]));
    assert_eq!(plan.component_masks[1], mask(&[2]));
    assert_eq!(plan.component_count, 4);
}

#[test]
fn component_ids_are_stable_when_dependency_order_changes() {
    let first = routing_plan(
        [
            "synth", "sampler", "synth", "none", "none", "none", "none", "none",
        ],
        [
            "fx_bus_1", "direct", "direct", "direct", "direct", "direct", "direct", "direct",
        ],
        vec![vec![duck("I2"), duck("B2")], vec![], vec![], vec![]],
    );
    let second = routing_plan(
        [
            "synth", "sampler", "synth", "none", "none", "none", "none", "none",
        ],
        [
            "fx_bus_1", "direct", "direct", "direct", "direct", "direct", "direct", "direct",
        ],
        vec![vec![duck("B2"), duck("I2")], vec![], vec![], vec![]],
    );

    assert_eq!(first, second);
    assert_eq!(first.component_masks[0], mask(&[0, 1, 8, 9]));
    assert_eq!(first.slot_component[2], 1);
    assert_eq!(first.bus_component[2], 2);
}

#[test]
fn invalid_routes_and_dependencies_fail_closed_and_unused_nodes_are_invalid() {
    let plan = routing_plan(
        [
            "synth", "sampler", "none", "none", "none", "none", "none", "none",
        ],
        [
            "fx_bus_99",
            "direct",
            "direct",
            "direct",
            "direct",
            "direct",
            "direct",
            "direct",
        ],
        vec![
            vec![duck("I99")],
            vec![duck("B99")],
            vec![],
            vec![],
            vec![fx("reverb")],
        ],
    );

    assert_eq!(
        plan.slot_component,
        [
            0,
            1,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID,
            INVALID_COMPONENT_ID
        ]
    );
    assert_eq!(plan.bus_component, [2, 3, 4, 5]);
    assert_eq!(plan.component_masks[0], mask(&[0]));
    assert_eq!(plan.component_masks[1], mask(&[1]));
    assert_eq!(plan.component_masks[2], mask(&[8]));
    assert_eq!(plan.component_masks[3], mask(&[9]));
    assert_eq!(plan.component_masks[4], mask(&[10]));
    assert_eq!(plan.component_masks[5], mask(&[11]));
    assert_eq!(plan.component_count, 6);
}

#[test]
fn parameter_changes_do_not_change_the_plan_but_topology_changes_do() {
    let base_config = duplicated_config();
    let base = render_plan(&base_config);
    let mut parameter_only = base_config.clone();
    parameter_only.mixer.as_mut().unwrap().buses[0].slots[0] = duck_with_amount("I2", 25.0);
    assert_eq!(
        RoutingTreePlan::from_render_plan(&base),
        routing_plan_from(&parameter_only)
    );

    let mut master_only = base_config.clone();
    master_only.mixer.as_mut().unwrap().master = Some(MasterFxConfig {
        slots: vec![duck("I2")],
    });
    assert_eq!(
        RoutingTreePlan::from_render_plan(&base),
        routing_plan_from(&master_only)
    );

    let mut route_changed = base_config.clone();
    route_changed.instruments[0].mixer.as_mut().unwrap().route = "direct".into();
    assert_ne!(
        RoutingTreePlan::from_render_plan(&base),
        routing_plan_from(&route_changed)
    );

    let mut duck_changed = base_config.clone();
    duck_changed.mixer.as_mut().unwrap().buses[2].slots[0] = duck("I2");
    assert_ne!(
        RoutingTreePlan::from_render_plan(&base),
        routing_plan_from(&duck_changed)
    );

    let mut type_changed = base_config.clone();
    type_changed.mixer.as_mut().unwrap().buses[0].slots[0] = fx("delay");
    assert_ne!(
        RoutingTreePlan::from_render_plan(&base),
        routing_plan_from(&type_changed)
    );

    let mut occupancy_changed = base_config;
    occupancy_changed.instruments[1].kind = "none".into();
    assert_ne!(
        RoutingTreePlan::from_render_plan(&base),
        routing_plan_from(&occupancy_changed)
    );
}

#[test]
fn topology_rebuild_generation_is_carried_by_derived_plan() {
    let mut installed = render_plan(&duplicated_config());
    let initial = RoutingTreePlan::from_render_plan(&installed);
    let mut changed = duplicated_config();
    changed.instruments[0].mixer.as_mut().unwrap().route = "direct".into();
    let previous = installed.install_complete(render_plan(&changed));
    let rebuilt = RoutingTreePlan::from_render_plan(&installed);

    assert_eq!(initial.generation, 0);
    assert_eq!(previous.generation, 0);
    assert_eq!(installed.generation, 1);
    assert_eq!(rebuilt.generation, 1);
    assert_ne!(initial, rebuilt);
}

#[test]
fn topology_structure_comparison_ignores_render_generation() {
    let mut first = RoutingTreePlan::from_render_plan(&render_plan(&duplicated_config()));
    let mut second = first;
    second.generation = first.generation.wrapping_add(1);

    assert_ne!(first, second);
    assert!(first.same_structure(&second));
    first.generation = second.generation;
    assert_eq!(first, second);
}

#[test]
fn deriving_a_fixed_plan_does_not_allocate() {
    let render_plan = render_plan(&duplicated_config());
    let (plan, allocations, deallocations) =
        test_allocator::count_allocations_and_deallocations(|| {
            RoutingTreePlan::from_render_plan(&render_plan)
        });

    assert_eq!(plan.component_count, 2);
    assert_eq!((allocations, deallocations), (0, 0));
}

fn duplicated_config() -> InstrumentsConfig {
    config(
        [
            "synth", "sampler", "synth", "synth", "synth", "sampler", "synth", "synth",
        ],
        ROUTING_ROUTES,
        vec![
            vec![duck("I2")],
            vec![duck("I1")],
            vec![duck("I6")],
            vec![duck("I5")],
        ],
    )
}

fn routing_plan(
    kinds: [&str; INSTRUMENT_SLOT_COUNT],
    routes: [&str; INSTRUMENT_SLOT_COUNT],
    buses: Vec<Vec<FxBusSlotConfig>>,
) -> RoutingTreePlan {
    routing_plan_from(&config(kinds, routes, buses))
}

fn routing_plan_from(config: &InstrumentsConfig) -> RoutingTreePlan {
    RoutingTreePlan::from_render_plan(&render_plan(config))
}

fn render_plan(config: &InstrumentsConfig) -> RenderPlan {
    RenderPlan::from_config(config)
}

fn config(
    kinds: [&str; INSTRUMENT_SLOT_COUNT],
    routes: [&str; INSTRUMENT_SLOT_COUNT],
    buses: Vec<Vec<FxBusSlotConfig>>,
) -> InstrumentsConfig {
    InstrumentsConfig {
        instruments: kinds
            .into_iter()
            .zip(routes)
            .map(|(kind, route)| InstrumentSlotConfig {
                kind: kind.into(),
                synth: default_synth_config(),
                mixer: Some(InstrumentMixerConfig {
                    route: route.into(),
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume: 100.0,
                }),
            })
            .collect(),
        mixer: Some(MixerConfig {
            buses: buses
                .into_iter()
                .map(|slots| FxBusConfig {
                    slots,
                    pan_pos: DEFAULT_PAN_POSITIONS / 2,
                    volume_pct: 100.0,
                })
                .collect(),
            master: None,
        }),
        pan_positions: DEFAULT_PAN_POSITIONS,
        master_volume: 100.0,
    }
}

fn duck(source: &str) -> FxBusSlotConfig {
    duck_with_amount(source, 60.0)
}

fn duck_with_amount(source: &str, amount_pct: f64) -> FxBusSlotConfig {
    FxBusSlotConfig::Config {
        kind: "duck".into(),
        params: [
            ("source".into(), json!(source)),
            ("amountPct".into(), json!(amount_pct)),
        ]
        .into_iter()
        .collect(),
    }
}

fn fx(kind: &str) -> FxBusSlotConfig {
    FxBusSlotConfig::Kind(kind.into())
}

fn mask(nodes: &[usize]) -> u16 {
    nodes.iter().fold(0, |mask, node| mask | (1_u16 << node))
}
