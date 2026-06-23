//! Spawns and stops the screen recorder (wf-recorder), writing a crash-safe
//! MKV of the selected region. Transcoding to the final format comes later.

use std::{
    path::PathBuf,
    process::{Child, Command},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, bail};
use signal_child::Signalable;

use crate::overlay::Rect;

/// A running recording. Always call `stop` so the MKV is finalised; dropping
/// without it leaves wf-recorder running.
pub struct Recorder {
    child: Child,
    output: PathBuf,
}

impl Recorder {
    /// Start recording the given region to a fresh MKV under `Videos`.
    pub fn start(rect: Rect) -> Result<Self> {
        if which::which("wf-recorder").is_err() {
            bail!("wf-recorder not found - install it: sudo pacman -S wf-recorder");
        }

        let geometry = geometry_arg(rect)?;
        let output   = output_path()?;

        let child = Command::new("wf-recorder")
            .arg("-g")
            .arg(&geometry)
            .arg("-f")
            .arg(&output)
            .spawn()
            .context("failed to spawn wf-recorder")?;

        tracing::info!("recording {geometry} -> {}", output.display());
        Ok(Self { child, output })
    }

    /// Stop recording: SIGINT lets wf-recorder finalise the MKV, then we wait.
    /// Never use kill() - SIGINT truncates the file.
    pub fn stop(mut self) {
        if let Err(e) = self.child.interrupt() {
            tracing::error!("could not signal wf-recorder: {e}");
        }
        let _ = self.child.wait();
        tracing::info!("saved {}", self.output.display());
    }
}

/// Format a selection as wf-recorder's "X,Y WxH" geometry. Logical == physical
/// on a single output at scale 1; HiDPI/multiple-output offsets come later.
fn geometry_arg(rect: Rect) -> Result<String> {
    let w = rect.w.round() as i32;
    let h = rect.h.round() as i32;
    if w < 1 || h < 1 {
        bail!("selection too small to record");
    }
    Ok(format!("{},{} {w}x{h}", rect.x.round() as i32, rect.y.round() as i32))
}

/// Build a timestamped output path under `~/Videos`, creating the directory.
fn output_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let dir  = PathBuf::from(&home).join("Videos");
    std::fs::create_dir_all(&dir).context("could not create ~/Videos")?;
    let ts   = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(dir.join(format!("lanner-{ts}.mkv")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_formats_and_rejects_degenerate() {
        assert_eq!(
            geometry_arg(Rect { x: 100.4, y: 200.6, w: 1280.0, h: 720.0 }).ok(),
            Some("100,201 1280x720".to_owned())
        );
        assert!(geometry_arg(Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 }).is_err());
    }
}