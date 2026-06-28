# Per-output overlays Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **Project note:** the maintainer adds code in portions himself and reviews each task. Prefer handing one task at a time with exact file and line targets over batch auto-implementation, unless he asks otherwise.

**Goal:** Dim every monitor at once and let the user draw the capture selection on any monitor, recording the output drawn on (recording stays single-output).

**Architecture:** Replace v1's single unpinned layer-shell window with one pinned window per output (`set_monitor`). All windows share one selection state tagged by output index; the draw rule punches the hole only on the active output. Keyboard is grabbed on one window (the bar window); Enter records whatever output the shared selection points at.

**Tech Stack:** Rust, gtk4 0.11 (feature `v4_12`), gtk4-layer-shell 0.8, wf-recorder, Cairo.

Spec: `docs/superpowers/specs/2026-06-28-per-output-overlays-design.md`.

**Note on GUI glue:** tasks 3-5 are gtk4 wiring; the code blocks are accurate to the design but expect to iterate against `cargo build` for borrow/clone and trait-import details. Each such task ends with a build-green checkpoint, not a unit test, because the existing overlay code is verified by the rig test, not unit tests. Tasks 1 and the geometry path stay pure and unit-tested.

---

### Task 1: `select_on` selection helper

The one piece of pure logic: a drag on output `i` claims the single shared selection, overwriting any prior (last drag wins, one selection total).

**Files:**
- Modify: `src/overlay.rs` (add helper near `normalise`, around line 90)
- Test: `src/overlay.rs` (the existing `#[cfg(test)]` module, or add one)

- [ ] **Step 1: Write the failing test**

Add to a `#[cfg(test)] mod tests` in `src/overlay.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64) -> Rect { Rect { x, y: 0.0, w: 10.0, h: 10.0 } }

    #[test]
    fn select_on_overwrites_last_wins() {
        let a = select_on(None, 0, r(1.0));
        assert_eq!(a.map(|(i, rect)| (i, rect.x)), Some((0, 1.0)));
        // a drag on a different output replaces it entirely
        let b = select_on(a, 2, r(5.0));
        assert_eq!(b.map(|(i, rect)| (i, rect.x)), Some((2, 5.0)));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p lanner select_on_overwrites_last_wins`
Expected: FAIL, `cannot find function select_on`.

- [ ] **Step 3: Write minimal implementation**

Add near `normalise` in `src/overlay.rs`:

```rust
/// Claim the single shared selection for output `i`. One selection exists at a
/// time across all monitors; a new drag overwrites any prior one (last wins).
pub fn select_on(_prev: Option<(usize, Rect)>, i: usize, rect: Rect) -> Option<(usize, Rect)> {
    Some((i, rect))
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p lanner select_on_overwrites_last_wins`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/overlay.rs
git commit -m "feat: select_on helper for the shared per-output selection"
```

---

### Task 2: `draw_spotlight` gains an active-output flag

Each window draws dim everywhere; it punches the transparent hole only when it is the active output. Border-only while recording (unchanged).

**Files:**
- Modify: `src/overlay.rs:95-131` (`draw_spotlight` signature and the hole branch)

- [ ] **Step 1: Rename to `pub fn draw_for`, add the active flag, change the hole condition**

`window.rs` calls this across modules, so rename `draw_spotlight` -> `pub fn draw_for` and add `active: bool` after `rect` (line 95):

```rust
pub fn draw_for(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    rect: Option<Rect>,
    active: bool,
    recording: bool,
    countdown: Option<u32>,
) -> Result<(), cairo::Error> {
```

Then guard the hole + border + countdown block so a non-active window only dims. The `if let Some(r) = rect` block (lines 111-129) becomes:

```rust
    if let Some(r) = rect {
        if !active {
            return Ok(()); // not the active output: dim only, no hole or border
        }
        if !recording {
            cr.set_operator(cairo::Operator::Clear);
            cr.rectangle(r.x, r.y, r.w, r.h);
            cr.fill()?;
            cr.set_operator(cairo::Operator::Over);
        }
        cr.set_source_rgba(0.40, 0.78, 1.0, 0.95);
        cr.set_line_width(2.0);
        cr.rectangle(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0);
        cr.stroke()?;
        if let Some(n) = countdown {
            draw_countdown(cr, r, n)?;
        }
    }
    Ok(())
```

(The full-surface dim above this block already runs for every non-recording window, which is what dims the inactive monitors.)

- [ ] **Step 2: Build (call site is updated in Task 3)**

Run: `cargo build -p lanner`
Expected: errors at the old `draw_spotlight(...)` call in `build_surface` (renamed + missing `active`). `build_surface` is deleted in Task 3, so this task does not build or commit alone; it commits together with Tasks 3 and 4.

---

### Task 3: `Shared` state and the per-output `OverlayWindow` (`window.rs`)

Extract the per-output window. Each instance pins to one monitor, draws from shared state, and its drag writes the shared selection tagged with its index.

**Files:**
- Create: `src/window.rs`
- Modify: `src/main.rs` (add `mod window;`)
- Modify: `src/overlay.rs` (make `build_surface` per-output, or replace its use; see below)

- [ ] **Step 1: Define `Shared` and `OverlayWindow` in `src/window.rs`**

```rust
//! One pinned layer-shell overlay window per output. All windows share the
//! selection state; each draws the hole only when it is the active output.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea, GestureDrag, gdk};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::overlay::{Rect, draw_for, normalise, select_on};

const NAMESPACE: &str = "lanner";

/// Selection state shared by every output window (single source of truth).
#[derive(Clone)]
pub struct Shared {
    /// The selection: which output index owns it and the rect, or None.
    pub active: Rc<Cell<Option<(usize, Rect)>>>,
    pub locked: Rc<Cell<bool>>,
    pub countdown: Rc<Cell<Option<u32>>>,
    /// All output drawing areas, so any drag can redraw every window.
    pub areas: Rc<std::cell::RefCell<Vec<DrawingArea>>>,
}

impl Shared {
    pub fn new() -> Self {
        Self {
            active: Rc::new(Cell::new(None)),
            locked: Rc::new(Cell::new(false)),
            countdown: Rc::new(Cell::new(None)),
            areas: Rc::new(std::cell::RefCell::new(Vec::new())),
        }
    }
    /// Redraw every output window (used after any selection change).
    pub fn redraw_all(&self) {
        for a in self.areas.borrow().iter() {
            a.queue_draw();
        }
    }
}

/// A single output's overlay window plus the data the coordinator needs.
pub struct OverlayWindow {
    pub window: ApplicationWindow,
    pub area: DrawingArea,
    pub index: usize,
    pub origin: (i32, i32),
}

/// Build one pinned, fullscreen overlay window for `monitor` at `index`.
pub fn build_output_overlay(
    app: &Application,
    monitor: &gdk::Monitor,
    index: usize,
    shared: &Shared,
) -> OverlayWindow {
    let window = ApplicationWindow::builder().application(app).build();
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace(Some(NAMESPACE));
    window.set_monitor(Some(monitor)); // pin to THIS output (inverse of v1)
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, true);
    }
    window.set_exclusive_zone(-1);
    window.set_keyboard_mode(KeyboardMode::None); // bar window overrides later

    let area = DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);

    // Draw from shared state: am I the active output?
    {
        let shared = shared.clone();
        area.set_draw_func(move |_, cr, w, h| {
            let active = shared.active.get();
            let is_active = active.map(|(i, _)| i == index).unwrap_or(false);
            let rect = active.map(|(_, r)| r);
            if let Err(e) =
                draw_for(cr, w, h, rect, is_active, shared.locked.get(), shared.countdown.get())
            {
                tracing::warn!("spotlight draw failed: {e}");
            }
        });
    }

    // Drag on this window claims the shared selection for `index`.
    let drag = GestureDrag::new();
    let start: Rc<Cell<(f64, f64)>> = Rc::new(Cell::new((0.0, 0.0)));
    {
        let start = start.clone();
        drag.connect_drag_begin(move |_, x, y| start.set((x, y)));
    }
    {
        let shared = shared.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            if shared.locked.get() || shared.countdown.get().is_some() {
                return;
            }
            let (sx, sy) = start.get();
            let rect = normalise(sx, sy, sx + dx, sy + dy);
            shared.active.set(select_on(shared.active.get(), index, rect));
            shared.redraw_all();
        });
    }
    area.add_controller(drag);

    window.set_child(Some(&area));
    shared.areas.borrow_mut().push(area.clone());

    let origin = crate::overlay::monitor_origin(monitor);
    OverlayWindow { window, area, index, origin }
}
```

If clippy flags `new_without_default` on `Shared::new` (the gate runs clippy), add `#[derive(Default)]` alongside `Clone` (all fields' `Default` match `new`) and either keep `new` or call `Shared::default()` in Task 4.

- [ ] **Step 2: Expose the helpers `window.rs` imports from `overlay.rs`**

In `src/overlay.rs`: make `normalise` and `select_on` `pub` (and `draw_for` is already `pub` from Task 2).

Change `monitor_origin` to take the monitor directly. We now hold the `gdk::Monitor` at build time, so the v1 `monitor_at_surface` indirection is gone. Replace the whole `monitor_origin(surface)` fn with:

```rust
/// Origin of `monitor` in global logical layout coords. Adding it shifts an
/// output-local selection onto a secondary monitor; wf-recorder reads `-g` in
/// these (global logical) coords.
pub fn monitor_origin(monitor: &gdk::Monitor) -> (i32, i32) {
    let g = monitor.geometry();
    (g.x(), g.y())
}
```

`build_surface` (the old single-window builder) and its `Surface` type alias are replaced by `build_output_overlay`; delete them once `app.rs` (Task 4) no longer calls them. `set_input_to_bar` stays (Task 5 reuses it).

- [ ] **Step 3: Register the module**

In `src/main.rs`, add with the other `mod` lines:

```rust
mod window;
```

- [ ] **Step 4: Build green**

Run: `cargo build -p lanner`
Expected: builds once `app.rs` is updated (Task 4). If you are doing tasks strictly in order, this task and Task 4 compile together; commit them as one.

---

### Task 4: Coordinator rewrite in `app.rs`

`build_overlay` becomes: enumerate outputs, build the window vec, pick the bar window (pointer monitor or 0), put the picker bar + exclusive keyboard + key controller there, present all.

**Files:**
- Modify: `src/app.rs:48-247` (`build_overlay`)

- [ ] **Step 1: Enumerate monitors and build the windows**

Replace the top of `build_overlay` (the single-window creation, old lines 49-61) with:

```rust
fn build_overlay(app: &Application) {
    let Some(display) = Display::default() else {
        tracing::error!("no display");
        return;
    };
    // n_items + item + downcast: relies only on ListModelExt (no iter::<T> trait import).
    let list = display.monitors();
    let monitors: Vec<gdk::Monitor> = (0..list.n_items())
        .filter_map(|i| list.item(i))
        .filter_map(|o| o.downcast::<gdk::Monitor>().ok())
        .collect();
    if monitors.is_empty() {
        tracing::error!("no monitors found");
        return;
    }

    let shared = crate::window::Shared::new();
    let settings: SharedSettings = Rc::new(RefCell::new(Settings::default()));
    let recorder: Rc<RefCell<Option<Recorder>>> = Rc::new(RefCell::new(None));

    let windows: Vec<crate::window::OverlayWindow> = monitors
        .iter()
        .enumerate()
        .map(|(i, m)| crate::window::build_output_overlay(app, m, i, &shared))
        .collect();
    // Shared into both the key controller and begin_recording, so wrap in Rc.
    let windows = Rc::new(windows);
```

- [ ] **Step 2: Pick the bar window (pointer monitor, fallback 0)**

After presenting (windows must be mapped for `device_position`), choose the bar index. Add a helper in `app.rs`:

```rust
/// Index of the window whose surface currently contains the pointer, else 0.
fn pointer_window(windows: &[crate::window::OverlayWindow]) -> usize {
    let Some(seat) = Display::default().and_then(|d| d.default_seat()) else {
        return 0;
    };
    let Some(pointer) = seat.pointer() else { return 0 };
    windows
        .iter()
        .position(|w| {
            w.window
                .surface()
                .and_then(|s| s.device_position(&pointer))
                .is_some()
        })
        .unwrap_or(0)
}
```

- [ ] **Step 3: Present all, then attach the bar + keyboard to the bar window**

```rust
    for w in &windows {
        w.window.present();
    }
    let bar_idx = pointer_window(&windows);
    let bar_window = &windows[bar_idx].window;
    bar_window.set_keyboard_mode(KeyboardMode::Exclusive);
```

Then build the existing control bar (pickers + Stop) exactly as today, but parent it on `bar_window` via a `gtk4::Overlay` wrapping `windows[bar_idx].area`. The pickers/`Settings` wiring from old lines 71-95 is unchanged; only the parent window differs.

- [ ] **Step 4: Move the key controller onto the bar window**

The `EventControllerKey` block (old lines 195-244) stays, but: it reads `shared.active.get()` instead of the old `rect`, and on Enter calls `begin_recording` (Task 5) with the `(index, rect)` from `shared.active`. Add it to `bar_window`, not a generic `window`.

```rust
    keys.connect_key_pressed(move |_, key, _, _| match key {
        Key::Return => {
            if recorder.borrow().is_none()
                && shared.countdown.get().is_none()
                && let Some((idx, r)) = shared.active.get()
            {
                let secs = settings.borrow().countdown_secs;
                if secs == 0 {
                    begin_recording(idx, r);
                } else {
                    shared.countdown.set(Some(secs));
                    shared.redraw_all();
                    // ... existing per-second tick, calling begin_recording(idx, r) at 0,
                    //     and shared.redraw_all() each tick instead of surface.queue_draw()
                }
            }
            Propagation::Stop
        }
        Key::Escape => { stop_and_quit(&recorder, &app, &settings); Propagation::Stop }
        _ => Propagation::Proceed,
    });
    bar_window.add_controller(keys);
}
```

- [ ] **Step 5: Build green and commit Tasks 2-4 together**

Run: `cargo build -p lanner && cargo fmt`
Expected: builds. Launch once on your single monitor to confirm it still dims, selects, and records (regression).

```bash
git add src/window.rs src/overlay.rs src/app.rs src/main.rs
git commit -m "feat: per-output overlay windows with a shared selection"
```

---

### Task 5: Recording-phase fan-out

On record, the recording output goes border-only + Stop bar; the others undim and pass input through.

**Files:**
- Modify: `src/app.rs` (`begin_recording`, now `Fn(usize, Rect)`)
- Modify: `src/window.rs` (add `OverlayWindow::clear_passthrough` and reuse the v1 Stop-bar placement on the recording window)

- [ ] **Step 1: `begin_recording` takes `(idx, rect)`**

Signature becomes `Rc<dyn Fn(usize, Rect)>`. Inside:

```rust
Rc::new(move |idx: usize, r: Rect| {
    let audio = settings.borrow().audio;
    let origin = windows[idx].origin; // origin of the recording output
    match Recorder::start(r, audio, origin) {
        Ok(rec) => {
            *recorder.borrow_mut() = Some(rec);
            shared.locked.set(true);
            shared.redraw_all(); // recording output -> border; others -> blank
            bar_window.set_keyboard_mode(KeyboardMode::None);
            // recording window: reveal Stop bar, place off hole, shrink input region
            //   (reuse v1 place_off_hole + set_input_to_bar, parented on windows[idx])
            // every OTHER window: clear + passthrough
            for w in windows.iter() {
                if w.index != idx {
                    w.clear_passthrough();
                }
            }
        }
        Err(e) => tracing::error!("{e:#}"),
    }
})
```

`windows` here is the `Rc<Vec<OverlayWindow>>` from Task 4, cloned into this closure. The Stop-bar reveal, `place_off_hole`, and `set_input_to_bar` are the existing v1 functions, now targeting `windows[idx]`.

- [ ] **Step 2: `clear_passthrough` in `window.rs`**

```rust
impl OverlayWindow {
    /// Undim and let all input through: empty input region, nothing drawn.
    /// Used on the non-recording outputs while a recording is live.
    pub fn clear_passthrough(&self) {
        if let Some(surface) = self.window.surface() {
            let empty = gtk4::cairo::Region::create();
            surface.set_input_region(Some(&empty)); // empty region: all input passes through
        }
        self.area.queue_draw(); // draw_for returns early: locked + not active = blank
    }
}
```

For the blank draw, `draw_for` must skip the full-surface dim when `locked` is true and this window is not the active output. Adjust the dim branch in `overlay.rs` so the dim only paints when `!recording`; while recording, only the active output paints (its border). That already holds for the active window; confirm the inactive windows paint nothing when `recording` is true (the early `if !active { return }` after the dim). Move the dim under `if !recording` (it already is) so recording inactive windows are fully transparent.

- [ ] **Step 3: Build green, fmt, commit**

Run: `cargo build -p lanner && cargo fmt && cargo clippy --all-targets`
Expected: clean.

```bash
git add src/app.rs src/window.rs src/overlay.rs
git commit -m "feat: per-output recording fan-out (record one, clear the rest)"
```

---

### Task 6: Rig and regression verification

**Files:** none (manual + headless rig, the v1 method).

- [ ] **Step 1: Single-monitor regression**

Run `lanner` on your normal setup. Confirm: dims, rubber-band selection, record, Stop, transcode, notification all behave exactly as before.

- [ ] **Step 2: Headless dual-monitor rig**

```bash
hyprctl output create headless                          # -> HEADLESS-2 (auto-right)
hyprctl keyword monitor HEADLESS-2,1920x1080,1920x0,1   # position is XxY, not X,Y
```

Launch `lanner`. Confirm BOTH monitors dim. Draw on HEADLESS-2, press Enter, Stop; confirm the MKV captures HEADLESS-2 (play it back). Repeat drawing on eDP-1. While recording one, confirm the other monitor undims and is clickable.

- [ ] **Step 3: Tear down the rig**

```bash
hyprctl output remove HEADLESS-2
```

- [ ] **Step 4: Run the test suite**

Run: `cargo test -p lanner`
Expected: all pass (the v1 geometry tests + `select_on`).

---

### Task 7: Documentation

**Files:**
- Modify: `docs/ARCHITECTURE.md` (module table + Resolution and monitors section + flow)
- Modify: `README.md` (Limitations + Features)
- Modify: `CHANGELOG.md` (`[Unreleased]` -> `### Added`)
- Update: issue #9

- [ ] **Step 1: ARCHITECTURE.md** — add `window.rs` to the module table; in "Resolution and monitors", state every output gets a pinned overlay and the selection may be drawn on any output, recorded one at a time.

- [ ] **Step 2: README.md** — Features: "Dims every monitor; draw the region on any of them." Limitations: drop per-output-overlays; keep spanning selections and monitor hotplug as the remaining limits.

- [ ] **Step 3: CHANGELOG.md** — under `[Unreleased]` `### Added`:

```
- Per-output overlays: every monitor dims at once and the capture region can be
  drawn on any monitor; the output drawn on is recorded. While recording, the
  other monitors undim and pass input through.
```

- [ ] **Step 4: Commit**

```bash
git add docs/ARCHITECTURE.md README.md CHANGELOG.md
git commit -m "docs: per-output overlays"
```

- [ ] **Step 5: Issue #9** — comment that per-output overlays landed; close it or narrow to the remaining out-of-scope items (spanning selections, hotplug).

---

## Self-review notes

- Spec coverage: every-monitor dim (Task 3 draw + Task 4 enumerate), draw-anywhere (Task 3 gesture + `select_on`), one selection (Task 1), keyboard-on-one-window (Task 4), record fan-out + undim others (Task 5), pointer bar window + fallback (Task 4), per-output origin reuse (Task 5 uses `OverlayWindow.origin`), rig + regression (Task 6), docs + issue (Task 7). Covered.
- The `Shared.areas` vector is the redraw fan-out mechanism the spec implies by "queue a redraw on all windows".
- GUI glue (tasks 3-5) verified by the rig test, not unit tests, matching the existing codebase (only pure functions are unit-tested).
