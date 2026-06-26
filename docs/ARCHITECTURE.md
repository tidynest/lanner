# Architecture and data flow

`lanner` is a single-crate binary. This document is the source of truth for how
data moves through the program.

## Modules

| Module | Responsibility |
| :--- | :--- |
| `main.rs` | Entry point: initialises logging, runs the lockfile toggle check (a second invocation stops the first and exits), then launches the app. |
| `app.rs` | GTK application, the layer-shell overlay window, the control bar, key handling, the lockfile watch, and phase wiring. |
| `overlay.rs` | The Cairo spotlight (dim, transparent hole, border) and the border-only recording draw, the rubber-band gesture, the selection state, and the per-phase surface input region. |
| `recorder.rs` | Spawns and stops `wf-recorder`, formats the capture geometry, and builds the output path. |
| `lockfile.rs` | Single-instance PID lock in `$XDG_RUNTIME_DIR`; the basis of the keybind toggle. |

## Flow

1. **Launch.** `main.rs` initialises `tracing`. If a live lockfile already
   exists, this invocation is a toggle: it deletes the lock (which the running
   instance is watching) and exits. Otherwise it claims the lock and `app.rs`
   builds a fullscreen overlay surface on the wlroots overlay layer.
2. **Select.** `overlay.rs` uses a `GestureDrag` to update a shared rectangle.
   The draw function dims the whole surface and clears a transparent hole at the
   current selection. Esc cancels.
3. **Record (Enter).** `recorder.rs` formats the rectangle as a `wf-recorder`
   geometry string (`X,Y WxH`) and spawns `wf-recorder`, writing
   `~/Videos/lanner-<timestamp>.mkv`. The overlay then switches to the recording
   phase:
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
5. **Transcode (planned).** Convert the MKV to the chosen format with `ffmpeg`
   or `gifski`.

## Phases and input regions

The `locked` flag (shared from `overlay.rs`) marks the recording phase. It drives
two things: the draw function paints the dim only while not recording, and the
selection drag is frozen while recording so the hole cannot move away from the
fixed capture geometry. During recording the window input region is set to the
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
