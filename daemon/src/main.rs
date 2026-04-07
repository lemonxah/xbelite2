use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

mod daemon;

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();

    ctrlc::set_handler(move || {
        log::info!("Shutdown signal received");
        r.store(false, Ordering::Relaxed);
    })
    .expect("Failed to set signal handler");

    if let Err(e) = daemon::run(running) {
        log::error!("Daemon error: {e:#}");
        std::process::exit(1);
    }
}
