#[cfg(target_os = "linux")]
mod imp {
    use std::cell::Cell;

    thread_local! {
        static PRIORITY_CONFIGURED: Cell<bool> = const { Cell::new(false) };
    }

    pub(crate) fn configure_callback_thread() {
        PRIORITY_CONFIGURED.with(|configured| {
            if configured.get() {
                return;
            }
            configured.set(true);
            let priority = std::env::var("OCTESSERA_AUDIO_THREAD_PRIORITY")
                .ok()
                .and_then(|value| value.parse::<i32>().ok())
                .unwrap_or(70)
                .clamp(1, 80);
            let params = libc::sched_param {
                sched_priority: priority,
            };
            let result = unsafe {
                libc::pthread_setschedparam(libc::pthread_self(), libc::SCHED_FIFO, &params)
            };
            if result != 0 {
                eprintln!("audio thread realtime priority unavailable: errno {result}");
            }
        });
    }
}

#[cfg(not(target_os = "linux"))]
mod imp {
    pub(crate) fn configure_callback_thread() {}
}

pub(crate) use imp::configure_callback_thread;
