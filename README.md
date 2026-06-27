# lanner

> Spotlight screen-region video recorder for wlroots Wayland compositors.

[![CI](https://github.com/tidynest/lanner/actions/workflows/ci.yml/badge.svg)](https://github.com/tidynest/lanner/actions/workflows/ci.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)

Draw a rectangle, record only that area to video. While you select, everything
outside the rectangle is dimmed like a spotlight; once recording starts the dim
lifts to a single bright border, so you can keep using the rest of your screen
normally while the region records. The overlay never appears in the recording.
Built and tested on Hyprland, designed for any wlroots-based compositor (Sway,
river, Wayfire).

## Demo

A short clip recorded with lanner itself will live here (tracked as the M8
demo).

## Why

`slurp` plus `wf-recorder` can already record a region, but the dimmed selection
disappears the instant you finish drawing it. lanner keeps a live spotlight on
screen for the whole recording and wraps the full flow (select, record, stop,
and later transcode) into a single tool.

## Features

- Spotlight selection: a fullscreen dim with a live rubber-band rectangle and a
  true transparent hole, so the region you pick stays clean.
- Border-only recording: once recording starts the dim lifts to a single border,
  and pointer and keyboard pass through, so you can use the rest of your system
  (browse, switch workspaces, type into the recorded app) while it records.
- Stop the recording with the on-overlay Stop button or a global keybind that
  toggles it off; Esc cancels before recording starts.
- Records to a crash-safe MKV through `wf-recorder`, finalised cleanly with a
  SIGINT so the file is always playable.
- Never films its own UI: the dim, the border, and the control bar all sit
  outside the captured geometry.
- Pre-draw control bar: pick the audio source (None, System, Mic, or Mic+System),
  the output format (MP4, WebM, AV1, animated WebP, or GIF), and a countdown
  delay before you draw; on record the bar collapses to a Stop button.
- Captures system and/or microphone audio via the active `pactl` device (the
  default input is used, so selecting a different mic just works); Mic+System
  mixes both through a temporary PipeWire source that is torn down on stop.
- Optional countdown (Off, 3s, 5s, or 10s) shown as a number over the selection
  before recording starts, so the count is never in the file.
- A live REC dot and elapsed timer on the overlay while recording.
- Transcodes in the background (lanner quits at once) and fires a desktop
  notification plus copies the saved path to the clipboard when it finishes.

## Requirements

- A wlroots-based Wayland compositor (Hyprland, Sway, river, Wayfire).
- `wf-recorder` for screen capture.
- `ffmpeg` for transcoding to the final format.
- System libraries `gtk4` and `gtk4-layer-shell`.
- Optional: `notify-send` (libnotify) and `wl-clipboard` (`wl-copy`) for the
  on-finish notification and clipboard copy; recording works without them.

On Arch Linux:

```bash
sudo pacman -S wf-recorder ffmpeg gtk4 gtk4-layer-shell
```

## Build

```bash
git clone https://github.com/tidynest/lanner.git
cd lanner
cargo build --release
```

The binary is at `target/release/lanner`.

## Usage

Run it; a control bar appears at the top. Pick your audio source, output format,
and optional countdown delay, drag a rectangle over the area you want, then press
Enter. If a delay is set, a countdown shows over the selection first; then
recording starts. While recording you can use your system normally; the region
keeps capturing. Stop with the on-overlay Stop button, or by running lanner again
(the second invocation toggles the recording off). Press Esc to cancel before
recording starts.

```bash
lanner
```

Recordings are written to `~/Videos/lanner-<timestamp>.mkv`, then transcoded to
your chosen format (e.g. `.webm`) beside it. The MKV is kept as the crash-safe
original. The transcode runs in the background, so lanner exits the moment you
stop; the final file appears in `~/Videos` a moment later.

Bind it to a key in Hyprland (`~/.config/hypr/hyprland.conf`) so one press starts
and the next stops:

```
bind = SUPER_SHIFT, R, exec, /path/to/lanner
```

At a full-screen selection there is no room for the on-screen controls, so use
the keybind to stop.

## Roadmap

- [x] M1: layer-shell spotlight overlay
- [x] M2: rubber-band selection with a transparent hole
- [x] M3: region recording to MKV via `wf-recorder`
- [x] M4: stop methods (Stop button, keybind toggle), input passthrough, and border-only recording
- [x] M5: transcode to MP4, WebM, GIF, animated WebP, AV1
- [x] M6: pre-draw control bar (audio source and output format toggles)
- [x] M7: countdown with a user-set delay
- [x] M8: niceties (desktop notification, clipboard path, REC indicator and timer)

## Architecture

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the module layout and the
full data flow from launch to saved file.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) and the
[Code of Conduct](CODE_OF_CONDUCT.md). Security reports go through
[SECURITY.md](SECURITY.md).

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).
