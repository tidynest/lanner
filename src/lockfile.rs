//! Single-instance lock in $XDG_RUNTIME_DIR. A second invocation deletes the
//! lock; the running instance watches it and stops itself. That is how the
//! keybind toggles recording, with no compositor IPC.

use std::{fs, path::PathBuf, process};

fn path() -> PathBuf {
    let dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp/".to_owned());
    PathBuf::from(dir).join("lanner.lock")
}

/// True if a process with this PID currently exists (Linux /proc check).
fn pid_alive(pid: u32) -> bool {
    PathBuf::from(format!("/proc/{pid}")).exists()
}

/// PID of a live lanner instance, if one holds the lock. A stale lock (process
/// died without releasing) reads as None and gets overwritten on the next claim.
pub fn live_pid() -> Option<u32> {
    let raw = fs::read_to_string(path()).ok()?;
    let pid: u32 = raw.parse().ok()?;
    pid_alive(pid).then_some(pid)
}

/// Record this process as the lock holder.
pub fn claim() {
    if let Err(e) = fs::write(path(), process::id().to_string()) {
        tracing::warn!("could not write lockfile: {e}");
    }
}

/// Drop the lock (best-effort; a stale file is harmless).
pub fn release() {
    let _ = fs::remove_file(path());
}

/// True while this process still owns the lock. A second invocation deletes the
/// file to request a stop, so the running instance watches this.
pub fn is_held() -> bool {
    fs::read_to_string(path())
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
        .is_some_and(|pid| pid == process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pid_alive_self_yes_bogus_no() {
        assert!(pid_alive(process::id()));
        assert!(!pid_alive(4_000_000_000)); // above pid_max, never exists
    }
}
