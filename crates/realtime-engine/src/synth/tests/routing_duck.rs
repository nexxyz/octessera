use super::super::*;

#[test]
fn duck_reduces_target_bus_when_source_instrument_is_active() {
    let mut dry = duck_test_engine(false);
    let mut ducked = duck_test_engine(true);
    dry.note_on(0, 36, 127, 1_000);
    dry.note_on(1, 36, 127, 1_000);
    ducked.note_on(0, 36, 127, 1_000);
    ducked.note_on(1, 36, 127, 1_000);

    let mut dry_sum = 0.0;
    let mut ducked_sum = 0.0;
    for _ in 0..256 {
        dry_sum += dry.next_sample().abs();
        ducked_sum += ducked.next_sample().abs();
    }

    assert!(
        ducked_sum < dry_sum * 0.8,
        "duck FX should audibly attenuate the target bus"
    );
}
