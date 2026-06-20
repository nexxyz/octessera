use super::{action_item, NativeMenuAction, NativeMenuItem};

pub(super) fn synth_preset_items(index: usize) -> Vec<NativeMenuItem> {
    [
        ("init", "init"),
        ("soft_pad", "soft pad"),
        ("bright_pluck", "bright pluck"),
        ("bass_mono", "bass mono"),
        ("hollow_pwm", "hollow pwm"),
        ("lead", "lead"),
        ("bell", "bell"),
        ("perc_hit", "perc hit"),
    ]
    .iter()
    .map(|(id, label)| {
        action_item(
            *label,
            format!("synth.preset.{index}.{id}"),
            NativeMenuAction::PlatformEffect(format!("synth.preset:{index}:{id}")),
        )
    })
    .collect()
}
