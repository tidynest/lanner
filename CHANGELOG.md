# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Layer-shell spotlight overlay that dims everything outside the selection (M1).
- Live rubber-band region selection with a true transparent hole (M2).
- Region recording to a crash-safe MKV via `wf-recorder`, finalised cleanly with
  a SIGINT (M3).
- Stop methods and a usable desktop while recording (M4): an on-overlay Stop
  button, plus a global keybind that toggles recording through a lockfile in
  `$XDG_RUNTIME_DIR`. Once recording starts the dim lifts to a single border, and
  pointer and keyboard pass through to the app being filmed, so the rest of the
  screen stays usable. The control bar is auto-placed clear of the region and is
  never captured.
- Transcode of the finalised MKV to a final format via `ffmpeg` (M5). Pure,
  unit-tested argv builders for MP4 (H.264), WebM (VP9), AV1 (SVT-AV1), animated
  WebP, and GIF (palettegen). The source MKV is kept as the crash-safe original.
- Pre-draw control bar (M6): segmented pickers for the audio source (None,
  System, or Mic) and the output format, written to a shared `Settings`. The
  audio choice resolves a `pactl` device and passes `--audio` to `wf-recorder`
  (System = the default sink's monitor, Mic = the default source); the format
  choice drives the transcode on stop. The bar shows during selection and
  collapses to the Stop button once recording starts.
- Optional pre-record countdown (M7): a fourth segmented group (Off, 3s, 5s,
  10s) on the bar. When set, Enter shows the number counting down over the
  selection and only spawns `wf-recorder` at zero, so the countdown is never
  part of the recording. Off keeps the immediate-record path.
- Niceties (M8): a live REC dot and elapsed timer on the overlay while
  recording; a Mic+System audio source that mixes the default mic and the system
  monitor through a temporary PipeWire null sink (torn down on stop); and, when
  the background transcode finishes, a desktop notification plus a clipboard copy
  of the saved path via a detached `sh` wrapper (`notify-send` / `wl-copy`, both
  optional).

### Changed

- Transcoding runs detached in a new session (`setsid`): stopping a recording
  quits the app and frees the overlay immediately, and `ffmpeg` finishes in the
  background independent of the launching terminal (it previously blocked the UI
  until the transcode completed). The MKV is kept if it fails.
- The audio-source picker is disabled while the GIF format is selected, since
  GIF carries no audio track.

### Fixed

- GIF width is capped (1280 px). A full-resolution GIF reached 100+ MB, which
  made many image viewers display only the first frame (the file was animated,
  just too large to render). With the transcode now backgrounded the speed cost
  is hidden, so the cap is generous rather than aggressive.
