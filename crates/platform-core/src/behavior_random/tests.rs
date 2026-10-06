use super::*;
use rand::Rng;

fn draws(random: &mut LayerRandom) -> Vec<u32> {
    random.scope(|| (0..8).map(|_| rng().gen_range(0..1000)).collect())
}

#[test]
fn seeded_stream_replays_after_restart() {
    let mut random = LayerRandom::new(Some(stream_seed(42, RANDOM_DOMAIN_BUILD, 0)));
    let first = draws(&mut random);
    let continued = draws(&mut random);
    random.restart();
    assert_eq!(draws(&mut random), first);
    assert_ne!(first, continued);
}

#[test]
fn stream_seed_separates_layers_domains_and_global_seeds() {
    let base = stream_seed(42, RANDOM_DOMAIN_BUILD, 0);
    assert_ne!(base, stream_seed(42, RANDOM_DOMAIN_BUILD, 1));
    assert_ne!(base, stream_seed(42, RANDOM_DOMAIN_LINK, 0));
    assert_ne!(base, stream_seed(43, RANDOM_DOMAIN_BUILD, 0));
}

#[test]
fn salt_is_zero_unless_a_seeded_scope_is_active() {
    assert_eq!(salt(), 0);
    LayerRandom::new(None).scope(|| assert_eq!(salt(), 0));
    LayerRandom::new(Some(8)).scope(|| assert_ne!(salt(), 0));
    assert_eq!(salt(), 0);
}

#[test]
fn nested_scopes_restore_the_outer_stream() {
    let mut outer = LayerRandom::new(Some(1));
    let mut reference = LayerRandom::new(Some(1));
    let expected = draws(&mut reference);
    let observed = outer.scope(|| {
        let mut values = vec![rng().gen_range(0..1000)];
        LayerRandom::new(Some(2)).scope(|| rng().gen_range(0..1000));
        values.extend((0..7).map(|_| rng().gen_range(0..1000)));
        values
    });
    assert_eq!(observed, expected);
}
