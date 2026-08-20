use crate::behavior::{BehaviorConfigItem, BehaviorConfigItemType};

pub(crate) fn number_item(
    key: &str,
    label: &str,
    min: i32,
    max: i32,
    step: i32,
) -> BehaviorConfigItem {
    BehaviorConfigItem {
        key: key.into(),
        label: label.into(),
        item_type: BehaviorConfigItemType::Number,
        min: Some(min),
        max: Some(max),
        step: Some(step),
        options: None,
    }
}

pub(crate) fn enum_item(key: &str, label: &str, options: &[&str]) -> BehaviorConfigItem {
    BehaviorConfigItem {
        key: key.into(),
        label: label.into(),
        item_type: BehaviorConfigItemType::Enum,
        min: None,
        max: None,
        step: None,
        options: Some(options.iter().map(|option| (*option).to_string()).collect()),
    }
}

pub(crate) fn action_item(key: &str, label: &str) -> BehaviorConfigItem {
    BehaviorConfigItem {
        key: key.into(),
        label: label.into(),
        item_type: BehaviorConfigItemType::Action,
        min: None,
        max: None,
        step: None,
        options: None,
    }
}
