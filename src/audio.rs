//! Optional Mic+System capture: a PipeWire/PulseAudio null sink fed by both the
//! default microphone and the default sink's monitor, recorded via its own
//! monitor. The backing modules are unloaded on drop, so the user's audio
//! routing is restored when the recording ends.

use std::process::Command;

use anyhow::{Context, Result, bail};

/// Name of the virtual sink that mixes mic + system audio.
const SINK_NAME: &str = "lanner_mix";

/// The PipeWire modules backing a combined Mic+System source. Dropping this
/// unloads them (in reverse), restoring the prior audio routing.
pub struct CombinedSource {
    modules: Vec<u32>,
}

impl CombinedSource {
    /// Create the null sink and loop both `mic` (the default source) and
    /// `<default_sink>.monitor` (system audio) into it. Returns the source and
    /// the monitor device name to hand to wf-recorder. If any step fails, the
    /// modules loaded so far are unloaded by the early drop.
    pub fn new(default_sink: &str, mic: &str) -> Result<(Self, String)> {
        let mut this = CombinedSource {
            modules: Vec::new(),
        };
        this.modules.push(load(&[
            "load-module",
            "module-null-sink",
            &format!("sink_name={SINK_NAME}"),
            &format!("sink_properties=device.description={SINK_NAME}"),
        ])?);
        this.modules.push(load(&[
            "load-module",
            "module-loopback",
            &format!("source={mic}"),
            &format!("sink={SINK_NAME}"),
        ])?);
        this.modules.push(load(&[
            "load-module",
            "module-loopback",
            &format!("source={default_sink}.monitor"),
            &format!("sink={SINK_NAME}"),
        ])?);
        Ok((this, format!("{SINK_NAME}.monitor")))
    }
}

impl Drop for CombinedSource {
    fn drop(&mut self) {
        // Reverse order: unload the loopbacks before the sink they feed.
        for id in self.modules.iter().rev() {
            let _ = Command::new("pactl")
                .arg("unload-module")
                .arg(id.to_string())
                .status();
        }
    }
}

/// Run `pactl <args>` and parse the loaded module id from stdout.
fn load(args: &[&str]) -> Result<u32> {
    let out = Command::new("pactl")
        .args(args)
        .output()
        .context("failed to run pactl - is PipeWire/PulseAudio running?")?;
    if !out.status.success() {
        bail!(
            "pactl {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    String::from_utf8_lossy(&out.stdout)
        .trim()
        .parse::<u32>()
        .context("pactl did not return a module id")
}
