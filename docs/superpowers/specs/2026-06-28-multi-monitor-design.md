# Multi-monitor support (focused-output v1)

- Date: 2026-06-28
- Status: approved design, pre-implementation
- Issue: [#9](https://github.com/tidynest/lanner/issues/9)
- Scope: record the focused monitor correctly (origin translation + HiDPI), one output per recording.

## Problem

lanner records the wrong region when the active monitor is not at layout origin
`(0,0)` or is scaled. `geometry_arg` emits the selection in output-local logical
coordinates assuming origin `(0,0)` and scale 1. On a multi-monitor layout, a
selection on a secondary monitor (origin e.g. `1920,0`) is passed to
`wf-recorder -g` as if it were on the primary, capturing the wrong area. A scaled
(HiDPI) monitor is similarly unhandled.

Overlay placement is NOT the bug. The overlay is an unpinned layer-shell surface,
so the compositor already places it on the focused output. The bug is purely the
geometry translation.

## Goal and scope

In scope:

- Record whichever monitor is focused, with a correctly translated and scaled
  `-g` geometry and the right output passed to wf-recorder.
- Focus is resolved by the compositor (Hyprland: keybind or pointer, whichever
  fired most recently). lanner does not compute focus itself.
- Correct on: a secondary monitor at non-zero origin; a scaled monitor; and the
  existing single-monitor case (regression).

Out of scope (deferred):

- Per-output overlays (a spotlight on every monitor at once). v1 keeps one
  overlay on the focused output.
- A selection spanning two outputs. wf-recorder records one output, and one
  overlay on one output makes a spanning selection impossible by construction.
- A CLI flag to force a specific monitor. Focus-based selection is enough for v1.

## Approach

Delegate focus to the compositor, read back the landed output, translate the
geometry.

1. The overlay stays unpinned (no `set_monitor`), so the compositor places it on
   the focused output. Focus was already resolved by the compositor at launch
   (by whatever fired last), so this is correct regardless of how the user got
   there. Focus is sampled once, at launch; the overlay then grabs the keyboard,
   so it cannot wander mid-selection.
2. After the overlay maps, read the `gdk::Monitor` the overlay surface is on via
   `gdk::Display::monitor_at_surface(&surface)`. From it: layout origin
   `(ox, oy)`, logical size, `scale`, and connector name (e.g. `eDP-1`,
   `HEADLESS-2`).
3. `geometry_arg` translates the output-local selection into the coordinate space
   wf-recorder expects, using `(ox, oy)` and `scale`.
4. `recorder` passes `-o <connector>` only if the spike (below) shows it is
   required to disambiguate the output.

Rejected alternatives:

- Pointer-under-cursor to find the active monitor: wrong when focus was last set
  by a keybind. Focus is keybind-or-pointer, whichever fired most recently.
- Querying `hyprctl` for the focused monitor: Hyprland-specific. The project
  targets wlroots-general (Sway, river, Wayfire).

## Spike first (de-risk the unknown)

The exact `-g` coordinate convention is the one real unknown, and the whole
translation depends on it. Before writing the translation, run a spike on the
headless rig and read off the answer.

Rig:

```
hyprctl output create headless                         # -> HEADLESS-2
hyprctl keyword monitor HEADLESS-2,1920x1080,1920,0,1  # to the right of eDP-1
```

Questions the spike answers:

1. Does an unpinned lanner overlay land on the focused output? (Focus
   HEADLESS-2, launch, observe where the dim appears.)
2. Does `wf-recorder -g "X,Y WxH"` read `X,Y` in GLOBAL layout coordinates (so a
   region on HEADLESS-2 needs `1920 + x`), and does it auto-select the output
   from the geometry, or is `-o` required?
3. On a scaled output (`hyprctl keyword monitor eDP-1,1920x1080@144,0,0,1.5`),
   does `-g` want LOGICAL or PHYSICAL pixels?

The spike result fixes the translation formula and whether `-o` is needed. The
current single-output build (origin `0,0`, scale 1) is the baseline: `-g` works
with logical coordinates there, so the open questions are only the origin offset
and the scale factor.

## Design details

### Monitor geometry (new, small)

A helper that, given the overlay's `gdk::Surface`, returns the active monitor's
geometry: origin `(i32, i32)`, logical size `(i32, i32)`, `scale`, and connector
`String`. Lives in `overlay.rs` (it already owns the surface), promoted to a tiny
`monitor.rs` only if it grows. A pure read of GDK state.

### geometry_arg (changed, still pure and unit-tested)

The signature gains the monitor geometry. Expected translation (the spike
confirms the scale rule):

```
global_x = ox + round(rect.x)
global_y = oy + round(rect.y)
w = round(rect.w); h = round(rect.h)
-> "global_x,global_y wxh"
```

Most likely `-g` is in logical layout coordinates, so NO multiplication by scale
is needed (this contradicts the original plan's "multiply by scale_factor", which
the handoff already flagged as probably wrong). The spike confirms before we
commit to the formula. `geometry_arg` stays a pure function; unit tests assert the
translation for a known `(ox, oy, scale)`.

### recorder (small change)

`start` receives the monitor info (or just the translated geometry plus the
connector) and adds `-o <connector>` only if the spike shows it is required. The
audio path is unchanged.

### overlay (no draw change)

The overlay fills the focused output and the selection is in that output's local
coordinates, exactly as today. Only the geometry handed to the recorder changes.

## Testing

Headless rig (beyond the spike):

- HEADLESS-2 at `(1920,0)`: focus it via keybind, select a region, record,
  confirm the MKV captures HEADLESS-2 at the right place (not eDP-1, not offset).
- HiDPI: eDP-1 at scale 1.5, select and record, confirm the region is correct.
- Regression: single monitor at `(0,0)` scale 1 still records correctly
  (translation is identity).

Unit tests (headless, no display):

- `geometry_arg` for origin `(1920,0)` scale 1: local `(10,20) 100x200` ->
  `"1930,20 100x200"`.
- `geometry_arg` for origin `(0,0)` scale 1.5: per the spike's scale rule.

## Risks and fallback

- If an unpinned overlay does NOT land on the focused output on some compositor,
  fall back to querying focus and pinning with `set_monitor`. Expected
  unnecessary on Hyprland; the spike verifies.
- If wf-recorder needs physical pixels on scaled outputs, the translation
  multiplies by scale. The spike decides; either way the formula is small and
  unit-tested.

## Docs to update on completion

- `docs/ARCHITECTURE.md` (Resolution and monitors) and the `geometry_arg` doc
  comment: drop the "single output at origin 0,0" caveat.
- `README.md` Limitations: narrow to "per-output overlays not yet supported"
  (single-output focused recording works, including secondary and scaled
  monitors).
- CHANGELOG.
- Update issue #9: v1 done, per-output overlays remain as a follow-up.
