use super::EngineSource;

impl EngineSource {
    pub(super) fn refresh_persistent_profile_cache(&mut self) {
        let Self {
            engine,
            worker_state,
            cached_profile_snapshot,
            ..
        } = self;
        let Some(worker) = worker_state.worker.as_mut() else {
            return;
        };
        if let Some(snapshot) = worker
            .runtime
            .with_recovered_owners(engine, |engine| engine.profile_snapshot())
        {
            *cached_profile_snapshot = snapshot;
        }
    }
}
