use playback_runtime::{
    CoreRunner, HostMessage, NativeRunner, NativeRunnerConfig, RuntimeStoreResult,
};
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};

struct CountingAllocator;

static LIVE: AtomicIsize = AtomicIsize::new(0);
static PEAK: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size() as isize;
        let live = LIVE.fetch_add(size, Ordering::Relaxed) + size;
        PEAK.fetch_max(live, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size() as isize, Ordering::Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// A load builds the new menu tree after the old one is gone, so the boards' allocator never has
/// to hold two trees (it keeps the peak as resident memory).
#[test]
fn patch_load_never_holds_two_menu_trees() {
    let mut runner = NativeRunner::new(NativeRunnerConfig::default()).unwrap();
    let _ = runner.messages_with_snapshot().unwrap();
    let patch = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    let mut loaded = patch;
    loaded["runtimeConfig"]["transport"]["bpm"] = serde_json::json!(97);
    let live = LIVE.load(Ordering::Relaxed);
    PEAK.store(live, Ordering::Relaxed);

    let _ = runner
        .send(HostMessage::RuntimeResult {
            result: RuntimeStoreResult::LoadDefaultResult {
                payload: Some(loaded),
            },
        })
        .unwrap();

    let applied = runner
        .capture_config_snapshot()
        .into_local_patch_payload()
        .unwrap();
    assert_eq!(applied["runtimeConfig"]["transport"]["bpm"], 97);
    let extra = PEAK.load(Ordering::Relaxed) - live;
    assert!(
        extra < live / 2,
        "load peaked {extra} bytes above a {live}-byte runner"
    );
}
