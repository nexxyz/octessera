use crate::RuntimePlatformRequest;
use serde_json::Value;
use std::time::Instant;

#[derive(Clone, Debug, PartialEq)]
pub struct DeferredDefaultSaveEntry {
    pub payload: Value,
    pub due_at: Instant,
    pub request: RuntimePlatformRequest,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeferredDefaultSave {
    pending: Option<DeferredDefaultSaveEntry>,
}

impl DeferredDefaultSave {
    pub fn schedule(&mut self, payload: Value, due_at: Instant, request: RuntimePlatformRequest) {
        self.replace(payload, due_at, request);
    }

    pub fn replace(&mut self, payload: Value, due_at: Instant, request: RuntimePlatformRequest) {
        self.pending = Some(DeferredDefaultSaveEntry {
            payload,
            due_at,
            request,
        });
    }

    pub fn take_due(&mut self, now: Instant) -> Option<DeferredDefaultSaveEntry> {
        if self
            .pending
            .as_ref()
            .is_some_and(|entry| now >= entry.due_at)
        {
            self.pending.take()
        } else {
            None
        }
    }

    pub fn take_now(&mut self) -> Option<DeferredDefaultSaveEntry> {
        self.pending.take()
    }

    pub fn retry(&mut self, entry: DeferredDefaultSaveEntry, due_at: Instant) {
        self.reschedule(entry.payload, due_at, entry.request);
    }

    pub fn reschedule(&mut self, payload: Value, due_at: Instant, request: RuntimePlatformRequest) {
        self.replace(payload, due_at, request);
    }

    pub fn cancel(&mut self) {
        self.pending = None;
    }

    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimePlatformEffect;
    use std::time::Duration;

    fn request(id: &str) -> RuntimePlatformRequest {
        RuntimePlatformRequest::new(
            RuntimePlatformEffect::StoreSaveDefault {
                payload: Value::Null,
                mode: None,
            },
            id.into(),
            Some(7),
        )
    }

    #[test]
    fn deferred_state_boundaries_replacement_identity_and_lifecycle() {
        let now = Instant::now();
        let mut cases = vec![
            ("before deadline", now - Duration::from_millis(1), false),
            ("at deadline", now, true),
            ("after deadline", now + Duration::from_millis(1), true),
        ];
        for (_, check, due) in cases.drain(..) {
            let mut state = DeferredDefaultSave::default();
            state.schedule(Value::from(1), now, request("one"));
            assert_eq!(state.take_due(check).is_some(), due);
        }

        let mut state = DeferredDefaultSave::default();
        state.schedule(Value::from(1), now + Duration::from_secs(1), request("one"));
        state.replace(Value::from(2), now, request("two"));
        let entry = state.take_due(now).unwrap();
        assert_eq!(entry.payload, Value::from(2));
        assert_eq!(entry.request.request_id, "two");
        state.schedule(Value::Null, now, request("three"));
        let entry = state.take_now().unwrap();
        state.retry(entry, now + Duration::from_secs(1));
        assert!(state.take_due(now).is_none());
        state.cancel();
        assert!(!state.is_pending());
    }
}
