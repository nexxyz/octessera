pub(crate) fn finalize_power_request<Terminal, Power>(terminal: Terminal, power: Power) -> bool
where
    Terminal: FnOnce() -> Result<(), String>,
    Power: FnOnce() -> Result<(), String>,
{
    if let Err(error) = terminal() {
        eprintln!("pi terminal render command failed: {error}");
        return true;
    }
    if let Err(error) = power() {
        eprintln!("pi power request failed after terminal render command: {error}");
    }
    true
}

#[cfg(test)]
mod tests {
    use super::finalize_power_request;
    use std::sync::{Arc, Mutex};

    fn record(events: &Arc<Mutex<Vec<&'static str>>>, event: &'static str) {
        events.lock().unwrap().push(event);
    }

    #[test]
    fn terminal_completion_precedes_power() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let terminal_events = Arc::clone(&events);
        let power_events = Arc::clone(&events);
        assert!(finalize_power_request(
            || {
                record(&terminal_events, "terminal");
                Ok(())
            },
            || {
                record(&power_events, "power");
                Ok(())
            },
        ));
        assert_eq!(*events.lock().unwrap(), ["terminal", "power"]);
    }

    #[test]
    fn terminal_failure_exits_without_submitting_power() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let terminal_events = Arc::clone(&events);
        let power_events = Arc::clone(&events);
        assert!(finalize_power_request(
            || {
                record(&terminal_events, "terminal");
                Err("terminal failed".into())
            },
            || {
                record(&power_events, "power");
                Ok(())
            },
        ));
        assert_eq!(*events.lock().unwrap(), ["terminal"]);
    }

    #[test]
    fn power_failure_still_exits_runtime() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let terminal_events = Arc::clone(&events);
        let power_events = Arc::clone(&events);
        assert!(finalize_power_request(
            || {
                record(&terminal_events, "terminal");
                Ok(())
            },
            || {
                record(&power_events, "power");
                Err("power failed".into())
            },
        ));
        assert_eq!(*events.lock().unwrap(), ["terminal", "power"]);
    }
}
