use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct SetupPortalPaths {
    pub(crate) request: PathBuf,
    pub(crate) public: PathBuf,
    pub(crate) receipts: PathBuf,
    pub(crate) current: PathBuf,
    pub(crate) boot_id: PathBuf,
}

impl SetupPortalPaths {
    pub(crate) fn production() -> Self {
        let public = PathBuf::from("/run/octessera-setup-status");
        Self {
            request: PathBuf::from("/run/octessera/setup-portal.request"),
            receipts: public.join("receipts"),
            current: public.join("current.json"),
            public,
            boot_id: PathBuf::from("/proc/sys/kernel/random/boot_id"),
        }
    }
}
