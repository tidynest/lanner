//! One pinned layer-shell overlay window per output. Every window dims its
//! output; the selection lives in shared state tagged by output index, so a drag
//! on any monitor claims it and only that output punches the hole. The control
//! bar is hosted by one window's `Overlay` and reparented onto the recording
//! output when that differs.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea, GestureDrag, Overlay, gdk};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::overlay::{Rect, draw_for, normalise, select_on};

const NAMESPACE: &str = "lanner";

/// Selection state shared by every output window (single source of truth).
#[derive(Clone, Default)]
pub struct Shared {
    /// The selection: which output index owns it and the rect, or None.
    pub active: Rc<Cell<Option<(usize, Rect)>>>,
    pub locked: Rc<Cell<bool>>,
    pub countdown: Rc<Cell<Option<u32>>>,
    /// Every output's drawing area, so any drag can redraw all windows.
    pub areas: Rc<RefCell<Vec<DrawingArea>>>,
}

impl Shared {
    /// Redraw every output window (used after any selection or phase change).
    pub fn redraw_all(&self) {
        for a in self.areas.borrow().iter() {
            a.queue_draw();
        }
    }
}

/// A single output's overlay window plus what the coordinator needs.
pub struct OverlayWindow {
    pub window: ApplicationWindow,
    pub area: DrawingArea,
    pub overlay: Overlay,
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
    window.set_keyboard_mode(KeyboardMode::None); // the bar window overrides later

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
            if let Err(e) = draw_for(
                cr,
                w,
                h,
                rect,
                is_active,
                shared.locked.get(),
                shared.countdown.get(),
            ) {
                tracing::warn!("spotlight draw failed: {e}");
            }
        });
    }

    // A drag on this window claims the shared selection for `index`.
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
                return; // frozen while recording or counting down
            }
            let (sx, sy) = start.get();
            let rect = normalise(sx, sy, sx + dx, sy + dy);
            shared
                .active
                .set(select_on(shared.active.get(), index, rect));
            shared.redraw_all();
        });
    }
    area.add_controller(drag);

    // gtk::Overlay wraps the drawing area so the control bar can sit on top of
    // (and be reparented to) this window.
    let overlay = Overlay::new();
    overlay.set_child(Some(&area));
    window.set_child(Some(&overlay));
    shared.areas.borrow_mut().push(area.clone());

    let origin = crate::overlay::monitor_origin(monitor);
    OverlayWindow {
        window,
        area,
        overlay,
        index,
        origin,
    }
}

impl OverlayWindow {
    /// Undim and let all input through: empty input region, nothing drawn. Used
    /// on the non-recording outputs while a recording is live (`draw_for` returns
    /// blank for a non-active output once `locked` is set).
    pub fn clear_passthrough(&self) {
        if let Some(surface) = self.window.surface() {
            let empty = gtk4::cairo::Region::create();
            surface.set_input_region(Some(&empty));
        }
        self.area.queue_draw();
    }
}
