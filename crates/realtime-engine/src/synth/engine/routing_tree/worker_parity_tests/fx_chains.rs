use super::*;

const BUS_FX_TYPES: [&str; 16] = [
    "tremolo",
    "delay",
    "vibrato",
    "chorus",
    "flanger",
    "filter_lfo",
    "wah",
    "reverb",
    "glitch",
    "auto_pan",
    "saturator",
    "distortion",
    "bitcrusher",
    "compressor",
    "eq",
    "vinyl",
];

const MASTER_FX_TYPES: [&str; 5] = ["saturator", "distortion", "compressor", "eq", "vinyl"];

fn assert_fx_config_renders_the_same(config: &InstrumentsConfig, label: &str) {
    for rate in RATES {
        let label = format!("{label} at {rate} Hz");
        let mut routed = SynthEngine::new(rate);
        let mut inline = SynthEngine::new(rate);
        for engine in [&mut routed, &mut inline] {
            engine.set_instruments(config.clone());
            for slot in 0..config.instruments.len() as u8 {
                engine.note_on(slot, 60 + slot * 7, 100, 300);
            }
        }
        let has_master = config
            .mixer
            .as_ref()
            .is_some_and(|mixer| mixer.master.is_some());
        assert_worker_render_matches_inline(
            &mut routed,
            &mut inline,
            &label,
            |engine, block, routed| {
                if block + usize::from(routed) == 60 {
                    let glided = [FxParamId::MixPct, FxParamId::DepthPct, FxParamId::AmountPct]
                        .into_iter()
                        .any(|id| {
                            engine.set_fx_bus_param(0, 0, id, 25.0) == FxParamMutation::Changed
                        });
                    assert!(glided, "{label} bus glide");
                }
                if has_master && block + usize::from(routed) == 80 {
                    assert_eq!(
                        engine.set_global_fx_param(0, FxParamId::MixPct, 40.0),
                        FxParamMutation::Changed,
                        "{label} master glide"
                    );
                }
            },
        );
    }
}

fn bus_chain_config(slots: &[&str]) -> InstrumentsConfig {
    let mut config = one_instrument_config("synth", "fx_bus_1");
    config.mixer.as_mut().unwrap().buses[0].slots = slots
        .iter()
        .map(|fx_type| FxBusSlotConfig::Kind((*fx_type).into()))
        .collect();
    config
}

#[test]
fn every_bus_fx_type_renders_the_same_on_the_routing_tree_workers() {
    for fx_type in BUS_FX_TYPES {
        assert_fx_config_renders_the_same(&bus_chain_config(&[fx_type]), &format!("bus {fx_type}"));
    }
}

#[test]
fn two_fx_bus_chains_render_the_same_on_the_routing_tree_workers() {
    for (index, first) in BUS_FX_TYPES.into_iter().enumerate() {
        let second = BUS_FX_TYPES[(index + 1) % BUS_FX_TYPES.len()];
        assert_fx_config_renders_the_same(
            &bus_chain_config(&[first, second]),
            &format!("bus {first} into {second}"),
        );
    }
}

#[test]
fn duck_renders_the_same_on_the_routing_tree_workers() {
    for source in ["I2", "B2"] {
        let mut config = one_instrument_config("synth", "fx_bus_1");
        let mut source_slot = config.instruments[0].clone();
        source_slot.mixer.as_mut().unwrap().route = MixerRoute::FxBus(1);
        config.instruments.push(source_slot);
        let mixer = config.mixer.as_mut().unwrap();
        mixer.buses[0].slots = vec![FxBusSlotConfig::Config {
            kind: "duck".into(),
            params: [
                ("source".into(), Value::from(source)),
                ("amountPct".into(), Value::from(80.0)),
            ]
            .into_iter()
            .collect(),
        }];
        mixer.buses.push(FxBusConfig {
            slots: vec![FxBusSlotConfig::Kind("delay".into())],
            ..FxBusConfig::default()
        });
        assert_fx_config_renders_the_same(&config, &format!("duck from {source}"));
    }
}

#[test]
fn every_master_fx_type_renders_the_same_on_the_routing_tree_workers() {
    for fx_type in MASTER_FX_TYPES {
        for route in ["direct", "fx_bus_1"] {
            let mut config = one_instrument_config("synth", route);
            config.mixer.as_mut().unwrap().master = Some(MasterFxConfig {
                slots: vec![FxBusSlotConfig::Kind(fx_type.into())],
            });
            assert_fx_config_renders_the_same(&config, &format!("master {fx_type} via {route}"));
        }
    }
}
