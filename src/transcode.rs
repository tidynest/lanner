//! MKV -> Final format. Pure argv builders (unit-tested) + a blocking runner.
//! ffmpeg only: gifski (better GIFs) is optional for later.

use std::{
    path::{Path, PathBuf},
    process::Command,
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
            // ffmpeg palettegen path (no gifski dependency); good enough for v1.
            "-vf",
            "fps=20,scale=iw:-1:flags=lanczos,split[s0][s1];[s0]palettegen[p];[s1][p]paletteuse",
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

/// Transcode `input` to `format` beside it, returning the new path. Blocking.
pub fn run(format: Format, input: &Path) -> Result<PathBuf> {
    if which::which("ffmpeg").is_err() {
        bail!("ffmpeg not found -> install it: sudo pacman -S ffmpeg");
    }
    let output = input.with_extension(format.ext());
    let status = Command::new("ffmpeg")
        .args(args(format, input, &output))
        .status()
        .context("failed to run ffmpeg")?;
    if !status.success() {
        bail!("ffmpeg exited with {status}");
    }
    tracing::info!("transcoded -> {}", output.display());
    Ok(output)
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
}
