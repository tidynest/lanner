//! Lanner - spotlight region video recorder for wlroots compositors.
#![forbid(unsafe_code)]

mod app;
mod audio;
mod controls;
mod lockfile;
mod overlay;
mod recorder;
mod transcode;
mod window;

use anyhow::Result;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // A second invocation while one runs = toggle: delete the lock the running
    // instance watches, then exit. It sees the lock vanish and stops itself.
    if lockfile::live_pid().is_some() {
        lockfile::release();
        return Ok(());
    }

    lockfile::claim();
    let result = app::run();
    lockfile::release();
    result
}
