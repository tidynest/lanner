//! The spotlight surface: a DrawingArea that dims the screen and punches a
//! transparent hole where the user drags out a selection rectangle.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{DrawingArea, GestureDrag, cairo};

/// A selection rectangle in widget (logical) coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Build the spotlight DrawingArea with live rubber-band selection.
pub fn build_surface() -> (DrawingArea, Rc<Cell<Option<Rect>>>) {
    let area = DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);

    // Shared state: the live rectangle (None until a drag starts) and the
    // point where the current drag began.
    let rect: Rc<Cell<Option<Rect>>> = Rc::new(Cell::new(None));
    let start: Rc<Cell<(f64, f64)>> = Rc::new(Cell::new((0.0, 0.0)));

    {
        let rect = rect.clone();
        area.set_draw_func(move |_, cr, w, h| {
            if let Err(e) = draw_spotlight(cr, w, h, rect.get()) {
                tracing::warn!("spotlight draw failed: {e}");
            }
        });
    }

    let drag = GestureDrag::new();
    {
        let start = start.clone();
        drag.connect_drag_begin(move |_, x, y| start.set((x, y)));
    }
    {
        let rect = rect.clone();
        let area = area.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let (sx, sy) = start.get();
            rect.set(Some(normalise(sx, sy, sx + dx, sy + dy)));
            area.queue_draw();
        });
    }
    area.add_controller(drag);

    (area, rect)
}

/// Normalise two corner points into a positive-size rectangle.
fn normalise(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
    Rect {
        x: x0.min(x1),
        y: y0.min(y1),
        w: (x1 - x0).abs(),
        h: (y1 - y0).abs(),
    }
}

/// Paint the dim and the spotlight hole. Cairo errors bubble to the caller.
fn draw_spotlight(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    rect: Option<Rect>,
) -> Result<(), cairo::Error> {
    // Dim the whole surface. NB: cr.paint() drops semi-transparent content on
    // this GTK4/Wayland stack, so fill an explicit rectangle instead.
    cr.set_source_rgba(0.02, 0.03, 0.06, 0.55);
    cr.rectangle(0.0, 0.0, f64::from(w), f64::from(h));
    cr.fill()?;

    if let Some(r) = rect {
        // True transparent hole: Clear writes zero alpha (not a lighter dim).
        cr.set_operator(cairo::Operator::Clear);
        cr.rectangle(r.x, r.y, r.w, r.h);
        cr.fill()?;
        cr.set_operator(cairo::Operator::Over);

        // Accent border 1px OUTSIDE the hole so it's never inside the capture.
        cr.set_source_rgba(0.40, 0.78, 1.0, 0.95);
        cr.set_line_width(2.0);
        cr.rectangle(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0);
        cr.stroke()?;
    }
    Ok(())
}
