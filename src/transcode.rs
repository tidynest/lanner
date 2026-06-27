//! MKV -> Final format. Pure argv builders (unit-tested) + a detached runner.
//! ffmpeg only: gifski (better GIFs) is optional for later.

use std::{
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Format {
    Av1,
    Gif,
    Mp4,
    Webm,
    Webp,
}

impl Format {
    /// Output file extension for this format.
    fn ext(self) -> &'static str {
        match self {
            Format::Av1 | Format::Mp4 => "mp4",
            Format::Webm => "webm",
            Format::Webp => "webp",
            Format::Gif => "gif",
        }
    }
}

/// Build the ffmpeg argv for `input` -> `output`. Pure: No I/O, so it is the
/// unit-tested core. Flags are verified commands from the design doc.
pub fn args(format: Format, input: &Path, output: &Path) -> Vec<String> {
    let mut a = vec![
        "-y".to_owned(),
        "-i".to_owned(),
        input.to_string_lossy().into_owned(),
    ];
    let codec: &[&str] = match format {
        Format::Av1 => &[
            "-c:v",
            "libsvtav1",
            "-crf",
            "30",
            "-preset",
            "6",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "libopus",
            "-b:a",
            "128k",
        ],
        Format::Mp4 => &[
            "-c:v",
            "libx264",
            "-crf",
            "20",
            "-preset",
            "medium",
            "-pix_fmt",
            "yuv420p",
            "-movflags",
            "+faststart",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
        ],
        Format::Gif => &[
            // ffmpeg palettegen path (no gifski dependency). Cap width at 1280:
            // near-native for a typical region, but bounded so a giant GIF (which
            // many viewers render as a single frame) is avoided. `\,` escapes the
            // comma in min() so the filtergraph parser keeps it in the expression.
            "-vf",
            "fps=20,scale=min(1280\\,iw):-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
        ],
        Format::Webm => &[
            "-c:v",
            "libvpx-vp9",
            "-crf",
            "30",
            "-b:v",
            "0",
            "-row-mt",
            "1",
            "-c:a",
            "libopus",
            "-b:a",
            "128k",
        ],
        Format::Webp => &[
            "-vcodec",
            "libwebp_anim",
            "-loop",
            "0",
            "-q:v",
            "75",
            "-preset",
            "picture",
            "-an",
        ],
    };
    a.extend(codec.iter().map(|s| (*s).to_owned()));
    a.push(output.to_string_lossy().into_owned());
    a
}

/// Start transcoding `input` to `format` beside it, WITHOUT waiting. The child
/// ffmpeg is reparented to init and finishes in the background, so the caller can
/// quit and free the overlay at once. Output is silenced to avoid async spam in
/// the launching terminal; the MKV is kept, so a failed transcode loses nothing.
/// Errors only on a missing binary or a failed spawn (a completion signal is M8).
pub fn spawn(format: Format, input: &Path) -> Result<()> {
    if which::which("ffmpeg").is_err() {
        bail!("ffmpeg not found -> install it: sudo pacman -S ffmpeg");
    }
    let output = input.with_extension(format.ext());
    // Detached `sh` wrapper: run ffmpeg (args via "$@", so the GIF filtergraph's
    // shell metacharacters stay literal), then notify and copy the path once it
    // succeeds. notify-send / wl-copy are optional - `command -v` degrades.
    const WRAP: &str = r#"ffmpeg "$@" || exit 1
command -v notify-send >/dev/null 2>&1 && notify-send lanner "Saved $LANNER_OUT"
command -v wl-copy >/dev/null 2>&1 && printf %s "$LANNER_OUT" | wl-copy"#;
    // setsid + null stdin: run in a new session with no controlling terminal, so
    // the detached ffmpeg can't take SIGTTIN/SIGHUP from the launching terminal
    // once lanner exits. Without this, a terminal-launched run left 0-byte files
    // (ffmpeg died on its first tty stdin read, before writing or notifying).
    Command::new("setsid")
        .arg("sh")
        .arg("-c")
        .arg(WRAP)
        .arg("sh") // $0 placeholder; the ffmpeg args become $1.. for "$@"
        .args(args(format, input, &output))
        .env("LANNER_OUT", &output)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .context("failed to start transcode")?;
    tracing::info!("transcoding in background -> {}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mp4_argv_has_io_and_faststart() {
        let a = args(Format::Mp4, Path::new("in.mkv"), Path::new("out.mp4"));
        assert_eq!(a.first().map(String::as_str), Some("-y"));
        assert!(a.windows(2).any(|w| w == ["-i", "in.mkv"]));
        assert!(a.windows(2).any(|w| w == ["-movflags", "+faststart"]));
        assert_eq!(a.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn ext_per_format() {
        assert_eq!(Format::Webm.ext(), "webm");
        assert_eq!(Format::Av1.ext(), "mp4");
        assert_eq!(Format::Gif.ext(), "gif");
    }

    #[test]
    fn gif_caps_width() {
        let a = args(Format::Gif, Path::new("in.mkv"), Path::new("out.gif"));
        // Width is capped (escaped comma keeps min() inside the filtergraph),
        // else a full-res GIF balloons and viewers show only one frame.
        assert!(a.iter().any(|s| s.contains(r"scale=min(1280\,iw)")));
    }
}
