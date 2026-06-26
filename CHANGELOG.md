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
