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
