//! Both boards run the runtime loop on a spawned thread. On-board AutoAux
//! studies measured ~50 us higher lateness and early underruns on Raspberry
//! when it ran inline on main; Orange measured the same or slightly better on
//! the thread.

pub(crate) fn run_on_runtime_thread<T: Send + 'static>(
    run: impl FnOnce() -> T + Send + 'static,
) -> T {
    let runtime = std::thread::Builder::new()
        .name("octessera-runtime".into())
        .spawn(run)
        .expect("runtime thread should start");
    runtime.join().unwrap_or_else(|_| {
        eprintln!("runtime thread panicked");
        std::process::exit(1);
    })
}
