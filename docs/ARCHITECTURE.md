# Architecture and data flow

`lanner` is a single-crate binary. This document is the source of truth for how
data moves through the program.

## Modules

| Module | Responsibility |
| :--- | :--- |
| `main.rs` | Entry point: initialises logging, then launches the app. |
| `app.rs` | GTK application, the layer-shell overlay window, key handling, and phase wiring. |
| `overlay.rs` | The Cairo spotlight (dim, transparent hole, border), the rubber-band gesture, and the selection state. |
| `recorder.rs` | Spawns and stops `wf-recorder`, formats the capture geometry, and builds the output path. |

## Flow

1. **Launch.** `main.rs` initialises `tracing`. `app.rs` calls
   `gtk4_layer_shell::is_supported`, then builds a fullscreen overlay surface on
   the wlroots overlay layer.
2. **Select.** `overlay.rs` uses a `GestureDrag` to update a shared rectangle.
   The draw function dims the whole surface and clears a transparent hole at the
   current selection.
3. **Record (Enter).** `app.rs` reads the rectangle. `recorder.rs` formats it as
   a `wf-recorder` geometry string (`X,Y WxH`) and spawns `wf-recorder`, writing
   `~/Videos/lanner-<timestamp>.mkv`.
4. **Stop (Esc).** `recorder.rs` sends `SIGINT` so `wf-recorder` flushes and
   finalises the MKV, then waits for the process to exit.
5. **Transcode (planned).** Convert the MKV to the chosen format with `ffmpeg`
   or `gifski`.

## Key invariant

The dim is drawn only outside the selection, and `wf-recorder` captures only the
selection geometry, so the overlay is never part of the recording. The hole uses
Cairo `Operator::Clear` for true transparency, and the accent border is stroked
one pixel outside the captured rectangle.

## Known gotcha

On this GTK4 and Wayland stack, `cairo::Context::paint()` drops semi-transparent
content. Fill an explicit rectangle instead (see the comment in `overlay.rs`).
