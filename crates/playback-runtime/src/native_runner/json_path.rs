use super::*;

pub(super) fn value_i32_at(value: &Value, path: &[&str], fallback: i32) -> i32 {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback;
        };
        current = next;
    }
    current.as_i64().unwrap_or(i64::from(fallback)) as i32
}

pub(super) fn value_string_at(value: &Value, path: &[&str], fallback: &str) -> String {
    let mut current = value;
    for key in path {
        let Some(next) = current.get(*key) else {
            return fallback.into();
        };
        current = next;
    }
    current.as_str().unwrap_or(fallback).into()
}

pub(super) fn set_json_path_string(value: &mut Value, path: &[&str], text: &str) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        let Some(object) = current.as_object_mut() else {
            return;
        };
        let Some(next) = object.get_mut(*key) else {
            return;
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.insert((*last).to_string(), json!(text));
    }
}

pub(super) fn set_json_path_number(value: &mut Value, path: &[&str], number: f64) {
    let Some((last, parents)) = path.split_last() else {
        return;
    };
    let mut current = value;
    for key in parents {
        let Some(object) = current.as_object_mut() else {
            return;
        };
        let Some(next) = object.get_mut(*key) else {
            return;
        };
        current = next;
    }
    if let Some(object) = current.as_object_mut() {
        object.insert((*last).to_string(), json!(number.round() as i64));
    }
}
