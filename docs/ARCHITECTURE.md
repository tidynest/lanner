# Architecture and data flow

`lanner` is a single-crate binary. This document is the source of truth for how
data moves through the program.

## Modules

| Module | Responsibility |
| :--- | :--- |
| `main.rs` | Entry point: initialises logging, runs the lockfile toggle check (a second invocation stops the first and exits), then launches the app. |
| `app.rs` | GTK application, the layer-shell overlay window, the control bar, key handling, the lockfile watch, and phase wiring. |
| `controls.rs` | The pre-draw pickers (audio source, output format, countdown delay) as segmented radio groups, plus the shared `Settings` they write. |
| `overlay.rs` | The Cairo spotlight (dim, transparent hole, border) and the border-only recording draw, the rubber-band gesture, the selection state, and the per-phase surface input region. |
| `recorder.rs` | Spawns and stops `wf-recorder`, formats the capture geometry, builds the output path, and returns the finalised MKV path on stop. |
| `transcode.rs` | Pure, unit-tested `ffmpeg` argv builders (MP4, WebM, AV1, WebP, GIF) plus a detached runner that converts the MKV to the chosen format in the background. |
| `lockfile.rs` | Single-instance PID lock in `$XDG_RUNTIME_DIR`; the basis of the keybind toggle. |

## Flow

1. **Launch.** `main.rs` initialises `tracing`. If a live lockfile already
   exists, this invocation is a toggle: it deletes the lock (which the running
   instance is watching) and exits. Otherwise it claims the lock and `app.rs`
   builds a fullscreen overlay surface on the wlroots overlay layer.
2. **Select.** The control bar is up with the pre-draw pickers (`controls.rs`):
   audio source and output format, written to a shared `Settings`. `overlay.rs`
   uses a `GestureDrag` to update a shared rectangle; the draw function dims the
   whole surface and clears a transparent hole at the current selection. The
   window key controller is capture-phase, so Enter and Esc reach the overlay
   even while a picker button holds focus. Esc cancels.
3. **Record (Enter).** If a countdown delay is set, Enter first enters a
   counting-down phase: `overlay.rs` draws the number over the selection and a
   one-second `glib` timer ticks it down; `wf-recorder` is not spawned until it
   reaches zero, so the count is never filmed. Then `recorder.rs` formats the
   rectangle as a `wf-recorder` geometry string (`X,Y WxH`); if an audio source
   was chosen it resolves the
   `pactl` device (System = the default sink's `.monitor`, Mic = the default
   source) and adds `--audio`. It spawns `wf-recorder`, writing
   `~/Videos/lanner-<timestamp>.mkv`. The overlay then switches to the recording
   phase:
   - the pre-draw pickers hide and the bar collapses to the Stop button;
   - the dim is dropped, leaving only the region border;
   - the keyboard is handed to the filmed app (`KeyboardMode::None`);
   - the surface input region shrinks to just the control bar, so pointer events
     everywhere else pass through.
   You can browse, switch workspaces, and type into the recorded app while the
   region records.
4. **Stop.** Any of: the on-overlay Stop button, or a second invocation via the
   keybind. The second invocation deletes the lock; a 150 ms timer in the running
   instance sees it gone and stops. Either path calls into `recorder.rs`, which
   sends `SIGINT` so `wf-recorder` flushes and finalises the MKV, then waits for
   the process to exit.
5. **Transcode.** On stop, `recorder.rs` returns the MKV path and `app.rs` reads
   the chosen format from `Settings` and calls `transcode::spawn`, which shells
   out to `ffmpeg` to convert the MKV beside the original (e.g.
   `lanner-<ts>.webm`). It is detached: `spawn` does not wait, so the app quits
   and frees the overlay at once while `ffmpeg` finishes in the background
   (reparented to init). The MKV is kept as the crash-safe original. A completion
   notification (and the GIF audio picker being greyed out) round out the UX; the
   notification is a later milestone.

## Phases and input regions

The `locked` flag (shared from `overlay.rs`) marks the recording phase. It drives
two things: the draw function paints the dim only while not recording, and the
selection drag is frozen while recording, or while a countdown is in progress (a
`countdown` cell holding `Some(n)`), so the hole cannot move away from the fixed
capture geometry. That same `countdown` cell drives the on-overlay number. During recording the window input region is set to the
control bar's rectangle alone, which is what lets the rest of the screen stay
interactive.

## Key invariant

The dim and border are drawn only outside the selection, the control bar is
auto-placed clear of it, and `wf-recorder` captures only the selection geometry,
so the overlay is never part of the recording. The hole uses Cairo
`Operator::Clear` for true transparency, and the accent border is stroked one
pixel outside the captured rectangle. At a full-screen selection there is no room
for the bar, so it hides and you stop with the keybind.

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
