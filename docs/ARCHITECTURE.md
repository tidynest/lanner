# Architecture and data flow

`lanner` is a single-crate binary. This document is the source of truth for how
data moves through the program.

## Modules

| Module | Responsibility |
| :--- | :--- |
| `main.rs` | Entry point: initialises logging, runs the lockfile toggle check (a second invocation stops the first and exits), then launches the app. |
| `app.rs` | Coordinator: enumerates outputs, builds one pinned overlay window per output (`window.rs`), picks the bar window (pointer's monitor), the control bar, key handling, the lockfile watch, and the record/stop fan-out. |
| `controls.rs` | The pre-draw pickers (audio source, output format, countdown delay) as segmented radio groups, plus the shared `Settings` they write. |
| `overlay.rs` | The Cairo spotlight draw (dim, transparent hole on the active output, border), the pure selection helpers (`normalise`, `select_on`), the per-phase surface input region, and a monitor's layout origin. |
| `window.rs` | One pinned layer-shell overlay window per output and the `Shared` selection state; a drag on any output claims the single selection, tagged by output index, and redraws all windows. |
| `recorder.rs` | Spawns and stops `wf-recorder`, formats the capture geometry, builds the output path, resolves the audio device, and returns the finalised MKV path on stop. |
| `audio.rs` | The Mic+System combined source: a PipeWire null sink fed by the default mic and the default sink's monitor, recorded via its own monitor and unloaded on drop. |
| `transcode.rs` | Pure, unit-tested `ffmpeg` argv builders (MP4, WebM, AV1, WebP, GIF) plus a detached runner that converts the MKV to the chosen format in the background. |
| `lockfile.rs` | Single-instance PID lock in `$XDG_RUNTIME_DIR`; the basis of the keybind toggle. |

## Flow

1. **Launch.** `main.rs` initialises `tracing`. If a live lockfile already
   exists, this invocation is a toggle: it deletes the lock (which the running
   instance is watching) and exits. Otherwise it claims the lock and `app.rs`
   enumerates the outputs and builds one pinned fullscreen overlay window per
   monitor on the wlroots overlay layer, so every monitor dims together.
2. **Select.** The control bar (on the bar window, the monitor under the pointer
   at launch) shows the pre-draw pickers (`controls.rs`): audio source and output
   format, written to a shared `Settings`. A `GestureDrag` on any output
   (`window.rs`) writes the single shared selection, tagged with that output's
   index, and redraws every window; only the active output clears a transparent
   hole, the rest stay dimmed. The bar window holds the keyboard (capture-phase,
   so Enter and Esc win over a focused picker) and reads the shared selection on
   Enter. Esc cancels.
3. **Record (Enter).** If a countdown delay is set, Enter first enters a
   counting-down phase: `overlay.rs` draws the number over the selection and a
   one-second `glib` timer ticks it down; `wf-recorder` is not spawned until it
   reaches zero, so the count is never filmed. Then `recorder.rs` formats the
   rectangle, translated by the recording output's layout origin, as a global
   `wf-recorder` geometry string (`X,Y WxH`); if an audio source
   was chosen it resolves the
   `pactl` device (System = the default sink's `.monitor`, Mic = the default
   source, Mic+System = a temporary PipeWire null sink mixing both, held by the
   recorder and unloaded on stop) and adds `--audio`. It spawns `wf-recorder`,
   writing
   `~/Videos/lanner-<timestamp>.mkv`. The windows then switch to the recording
   phase:
   - the recording output drops its dim to just the region border; the bar moves
     onto that output (reparented if it differs from the bar window), collapses to
     the Stop button, and its input region shrinks to just the bar;
   - every other output undims entirely and passes all input through;
   - the keyboard is handed to the filmed app (`KeyboardMode::None`).
   You can browse, switch workspaces, and type into the recorded app, on any
   monitor, while the region records.
4. **Stop.** Any of: the on-overlay Stop button, or a second invocation via the
   keybind. The second invocation deletes the lock; a 150 ms timer in the running
   instance sees it gone and stops. Either path calls into `recorder.rs`, which
   sends `SIGINT` so `wf-recorder` flushes and finalises the MKV, then waits for
   the process to exit.
5. **Transcode.** On stop, `recorder.rs` returns the MKV path and `app.rs` reads
   the chosen format from `Settings` and calls `transcode::spawn`. That launches a
   detached `setsid sh` wrapper which runs `ffmpeg` (args via `"$@"`, so the GIF
   filtergraph's shell metacharacters stay literal), then fires `notify-send` and
   copies the path with `wl-copy` once it succeeds. Because it does not wait, the
   app quits and frees the overlay at once while `ffmpeg` finishes in the
   background, in its own session so the launching terminal cannot signal it. The
   MKV is kept as the crash-safe original.

## Phases and input regions

The `locked` flag (in `window::Shared`) marks the recording phase. It drives two
things: the draw function paints the dim only while not recording, and the
selection drag is frozen while recording, or while a countdown is in progress (a
`countdown` cell holding `Some(n)`), so the hole cannot move away from the fixed
capture geometry. That same `countdown` cell drives the on-overlay number. During
recording the recording output's input region is set to the control bar's
rectangle alone and every other output's region is emptied, which is what lets the
rest of the desktop stay interactive.

## Key invariant

The dim and border are drawn only outside the selection, the control bar is
auto-placed clear of it, and `wf-recorder` captures only the selection geometry,
so the overlay is never part of the recording. The hole uses Cairo
`Operator::Clear` for true transparency, and the accent border is stroked one
pixel outside the captured rectangle. At a full-screen selection there is no room
for the bar, so it hides and you stop with the keybind.

## Resolution and monitors

`wf-recorder` captures through wlr-screencopy, which copies the output's physical
framebuffer, so the recording is always at the output's native resolution - no
resolution is hardcoded in the capture path. A region on a HiDPI output is
captured at its physical pixel density automatically. Only the GIF transcode
applies a deliberate width cap, because GIF compresses far worse than video (no
interframe coding, LZW, 256 colours) and a native-resolution GIF balloons to
hundreds of MB.

The `-g` geometry is in global logical layout coordinates. `geometry_arg`
translates the output-local selection by the recording output's layout origin
(each window carries its monitor origin from `overlay::monitor_origin`), so a
secondary monitor at a non-zero origin and a scaled (HiDPI) monitor both record
correctly. wf-recorder auto-selects the output from the geometry (no `-o` needed)
and captures at the output's native resolution. Every monitor gets its own
overlay, so the selection can be drawn on any output. A single region still
cannot span outputs, since wf-recorder records one output and one overlay owns the
selection; spanning selections and monitor hotplug remain out of scope
([issue #9](https://github.com/tidynest/lanner/issues/9)).

## Known gotchas

- On this GTK4 and Wayland stack, `cairo::Context::paint()` drops semi-transparent
  content. Fill an explicit rectangle instead (see the comment in `overlay.rs`).
- A widget measures zero while hidden, so the control bar is shown before its size
  is measured for the input region; otherwise the region would be empty and the
  bar unclickable.
- The picker buttons are focusable and GTK gives the first one initial focus, so a
  focused toggle would otherwise swallow Enter (and the press would flip the radio
  back). The key controller runs in the capture phase so the overlay's Enter and
  Esc win over the focused button.
- A background `ffmpeg` that inherits the launching terminal's stdin dies on its
  first read once lanner exits, leaving a 0-byte file. The transcode runs under
  `setsid` with null stdin so it is detached from the terminal entirely.
