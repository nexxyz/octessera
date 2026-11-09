pub(super) struct NativeLayer {
    pub(super) behavior_id: String,
    pub(super) name: String,
    pub(super) auto_name: bool,
    pub(super) seeded: bool,
}

impl NativeLayer {
    pub(super) fn new(behavior_id: &str) -> Self {
        Self {
            behavior_id: behavior_id.into(),
            name: super::behavior_menu::behavior_display_label(behavior_id),
            auto_name: true,
            seeded: false,
        }
    }
}
