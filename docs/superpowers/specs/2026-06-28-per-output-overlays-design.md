# Per-output overlays (multi-monitor v2)

- Date: 2026-06-28
- Status: approved design, pre-implementation
- Issue: [#9](https://github.com/tidynest/lanner/issues/9) (kept open for this follow-up)
- Scope: dim every monitor at once and let the user draw the selection on any of
  them; the output drawn on is recorded. Recording stays single-output.

## Problem

After multi-monitor v1 (focused-output capture), lanner records the focused
monitor correctly, but only that monitor dims. On a multi-monitor setup the rest
of the desktop stays bright, so the spotlight effect is inconsistent and the
user cannot start a selection on a non-focused monitor. v1 used a single unpinned
layer-shell window that the compositor placed on the focused output.

## Goal and scope

In scope:

- One layer-shell overlay per output, so every monitor dims together.
- The user drags the selection on whichever monitor they want; that output is the
  one recorded (origin-translated geometry from v1, already correct per output).
- One active selection at a time across all monitors (last drag wins).
- While recording, the non-recording monitors undim and pass input through, so
  the rest of the desktop stays usable (same philosophy as v1's border-only
  recording on a single monitor).

Out of scope (deferred):

- Monitor hotplug mid-session. lanner is launch-select-record-quit; outputs are
  enumerated once at launch.
- A selection spanning two outputs. wf-recorder records one output and one
  overlay owns the selection, so a spanning region is impossible by construction.
- Independent simultaneous selections on multiple outputs. One region, one output
  by design.

## Approach

N pinned per-output windows share a single selection state tagged by output.
Rejected: a single surface spanning all outputs (wlroots binds a layer surface to
one output, so there is no clean cross-output surface); dim-only backdrops on the
other monitors (fails the draw-anywhere scope).

1. The coordinator enumerates `display.monitors()` once and builds one
   `ApplicationWindow` per output, each pinned with
   `LayerShell::set_monitor(Some(&monitor))`, anchored to all four edges,
   exclusive zone -1. This is the inverse of v1's single unpinned window.
2. All windows share one state object (`Rc`):
   - `active: Cell<Option<(usize, Rect)>>` - the selection: which output index
     owns it plus the rect. A new drag overwrites it (last wins, one selection
     total).
   - `locked: Cell<bool>` and `countdown: Cell<Option<u32>>`, as today but shared.
   - the existing `SharedSettings` and `recorder`.
3. Keyboard is grabbed on one window only. The bar window gets
   `KeyboardMode::Exclusive` (layer-shell exclusive means that surface receives
   all keyboard input globally) and owns the single key controller. The user
   drags on any monitor (which sets shared `active`), then Enter, read by the bar
   window, records whatever output `active` points at. The other windows are
   pointer-only (`KeyboardMode::None`). This avoids multi-grab ambiguity.
4. The bar window is the monitor under the pointer at launch (best-effort
   "focused", via `Surface::device_position` on each mapped surface), falling back
   to monitor 0. Every window holds a hidden Stop bar; on record only the
   recording output reveals its Stop bar, which may differ from the bar window.

## Design details

### New unit: per-output overlay window (`window.rs`)

`OverlayWindow` plus `build_output_overlay(app, monitor, index, shared)`,
extracted from today's inline `build_overlay` (N inline copies would bloat
`app.rs`). Each instance owns: its pinned `ApplicationWindow`, its `DrawingArea`,
the drag gesture, its monitor origin (reuse v1's `overlay::monitor_origin`), and a
hidden Stop bar. It exposes a redraw and `start_recording_ui(rect)` (reveal the
Stop bar, place it off the hole, shrink the input region to the bar).

### Drawing (`overlay.rs`, small change)

`draw_spotlight` gains an "am I the active output?" argument. The rule: dim the
whole surface; punch the transparent hole only if this window is the active
output; border-only while recording. Non-active windows during selection draw
full dim (no hole); during recording the recording output draws the border and
the others draw nothing and pass input through. `monitor_origin` is unchanged.

### Coordinator (`app.rs`, rewritten to a coordinator)

`build_overlay` shrinks to: claim the lockfile (unchanged), enumerate monitors,
build the `Vec<OverlayWindow>`, pick the bar window (pointer monitor or 0), attach
the picker bar plus exclusive keyboard plus the key controller there, present all.
`begin_recording` takes `(index, rect)`: `Recorder::start(rect, audio, origin)`
with output `index`'s origin; set `locked`; the recording window goes border-only
plus Stop bar plus shrunk input region; the other windows clear and pass through;
the bar window hands keyboard to the filmed app. `stop_and_quit`, the lockfile
watch, and the detached transcode are unchanged (already global, not per-window).

### Selection transition (pure, unit-tested)

Extract the active-selection update as a pure helper (e.g. `select_on(active, i,
rect) -> Option<(usize, Rect)>`) so last-wins is unit-tested without a display.

## Data flow

1. Launch: enumerate outputs, build N pinned windows, choose the bar window, set
   exclusive keyboard there, present all. Every monitor dims.
2. Select: a drag on window `i` writes shared `active = Some((i, rect))` and
   queues a redraw on all windows; window `i` draws the hole, the rest full dim.
   Dragging on another monitor overwrites `active`.
3. Record (Enter on the bar window): read `active`; if set, optional countdown
   drawn on output `i`, then `begin_recording(i, rect)`. Recording window goes
   border-only plus Stop bar; the others undim and pass through; keyboard goes to
   the filmed app.
4. Stop: Stop button, keybind toggle, or Esc finalizes the MKV, spawns the
   detached transcode, and quits all windows.

## Error handling

- Empty monitor list: `bail` (cannot happen on a running compositor, but explicit).
- A monitor that vanishes between enumeration and `set_monitor`: skip that window,
  log, continue with the rest.
- Pointer-monitor detection returns none: bar on window 0.
- `monitor_origin` keeps its `(0,0)` identity fallback per window.

## Testing

Unit (headless, no display):

- `select_on` overwrites the active selection when a drag starts on another
  output (last wins).
- `geometry_arg` per-output origin translation is already covered by v1 tests.

Rig (headless output, the method proven in v1):

- Create HEADLESS-2 at `(1920,0)`. Launch lanner. Confirm both monitors dim.
- Draw on HEADLESS-2, record, confirm the MKV captures HEADLESS-2.
- Draw on eDP-1, record, confirm the MKV captures eDP-1.
- While recording one output, confirm the other undims and is usable.
- Regression: a single monitor behaves exactly as today.

## Risks and fallback

- If a compositor mishandles multiple pinned layer surfaces or exclusive keyboard
  on one of several surfaces, fall back to attaching the key controller to every
  window and reading the shared `active` from whichever fires.
- If `device_position` cannot identify the pointer's monitor reliably, the bar
  defaults to monitor 0; the feature still works, the bar is just not always on
  the monitor the user starts on.

## Docs to update on completion

- `docs/ARCHITECTURE.md`: module table (new `window.rs`), the overlay flow, and
  the Resolution and monitors section (every output dims; selection on any
  output).
- `README.md` Limitations: drop "per-output overlays not yet supported"; note the
  remaining limits are spanning selections and monitor hotplug.
- CHANGELOG.
- Update issue #9: per-output overlays done; close it or narrow to the remaining
  out-of-scope items.
