use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct SetupPortalPaths {
    pub(crate) request: PathBuf,
    pub(crate) public: PathBuf,
    pub(crate) current: PathBuf,
}

impl SetupPortalPaths {
    pub(crate) fn production() -> Self {
        let public = PathBuf::from("/run/octessera-setup-status");
        Self {
            request: PathBuf::from("/run/octessera-setup-request/inbox/start"),
            current: public.join("current.json"),
            public,
        }
    }
}
