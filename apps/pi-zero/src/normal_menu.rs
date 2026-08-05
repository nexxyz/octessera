use serde_json::Value;

pub(crate) fn is_normal_menu_snapshot(snapshot: &Value) -> bool {
    let Some(display) = snapshot.get("display") else {
        return false;
    };
    display.get("off").and_then(Value::as_bool) == Some(false)
        && display.get("splash").and_then(Value::as_str) == Some("")
        && display
            .get("title")
            .and_then(Value::as_str)
            .is_some_and(|title| !title.is_empty())
        && display
            .get("lines")
            .and_then(Value::as_array)
            .is_some_and(|lines| !lines.is_empty() && lines.iter().all(Value::is_string))
        && snapshot.get("runtimeError").is_none_or(Value::is_null)
}

#[cfg(test)]
mod tests {
    use super::is_normal_menu_snapshot;
    use serde_json::json;

    fn snapshot() -> serde_json::Value {
        json!({
            "display": {
                "off": false,
                "splash": "",
                "title": "Build",
                "lines": ["ready"]
            }
        })
    }

    #[test]
    fn normal_menu_requires_visible_title_and_lines() {
        assert!(is_normal_menu_snapshot(&snapshot()));
        let mut hidden = snapshot();
        hidden["display"]["off"] = json!(true);
        assert!(!is_normal_menu_snapshot(&hidden));
        let mut splash = snapshot();
        splash["display"]["splash"] = json!("startup");
        assert!(!is_normal_menu_snapshot(&splash));
    }

    #[test]
    fn runtime_error_is_not_a_normal_menu() {
        let mut error = snapshot();
        error["runtimeError"] = json!({ "code": "failed" });
        assert!(!is_normal_menu_snapshot(&error));
    }
}
