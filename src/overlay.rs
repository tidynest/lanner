//! The spotlight surface: a DrawingArea that dims the screen and punches a clear
//! hole while selecting, then shows only the region border while recording.

use std::cell::Cell;
use std::rc::Rc;

use gtk4::prelude::*;
use gtk4::{DrawingArea, GestureDrag, cairo, gdk};

/// A selection rectangle in widget (logical) coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// What `build_surface` returns: the drawing area, the live selection rect (None
/// until a drag starts), and the lock that freezes selection while recording.
type Surface = (DrawingArea, Rc<Cell<Option<Rect>>>, Rc<Cell<bool>>);

/// Build the spotlight DrawingArea with live rubber-band selection.
pub fn build_surface() -> Surface {
    let area = DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);

    // Shared state: the live rectangle (None until a drag starts) and the
    // point where the current drag began.
    let rect: Rc<Cell<Option<Rect>>> = Rc::new(Cell::new(None));
    let start: Rc<Cell<(f64, f64)>> = Rc::new(Cell::new((0.0, 0.0)));

    // Set true once recording starts: freezes the selection drag so it can't
    // move the hole away from the fixed capture geometry.
    let locked: Rc<Cell<bool>> = Rc::new(Cell::new(false));

    {
        let rect = rect.clone();
        let locked = locked.clone();
        area.set_draw_func(move |_, cr, w, h| {
            if let Err(e) = draw_spotlight(cr, w, h, rect.get(), locked.get()) {
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
        let locked = locked.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            if locked.get() {
                return;
            }
            let (sx, sy) = start.get();
            rect.set(Some(normalise(sx, sy, sx + dx, sy + dy)));
            area.queue_draw();
        });
    }
    area.add_controller(drag);
    (area, rect, locked)
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

/// Paint the spotlight. During selection: dim the surround, punch a clear hole,
/// stroke an accent border. While recording: border only, no dim, so the rest of
/// the screen stays visible and usable. Cairo errors bubble to the caller.
fn draw_spotlight(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    rect: Option<Rect>,
    recording: bool,
) -> Result<(), cairo::Error> {
    if !recording {
        // Dim the whole surface. NB: cr.paint() drops semi-transparent content on
        // this GTK4/Wayland stack, so fill an explicit rectangle instead.
        cr.set_source_rgba(0.02, 0.03, 0.06, 0.55);
        cr.rectangle(0.0, 0.0, f64::from(w), f64::from(h));
        cr.fill()?;
    }

    if let Some(r) = rect {
        if !recording {
            // True transparent hole: Clear writes zero alpha (not a lighter dim).
            cr.set_operator(cairo::Operator::Clear);
            cr.rectangle(r.x, r.y, r.w, r.h);
            cr.fill()?;
            cr.set_operator(cairo::Operator::Over);
        }

        // Accent border 1px OUTSIDE the hole so it's never inside the capture.
        cr.set_source_rgba(0.40, 0.78, 1.0, 0.95);
        cr.set_line_width(2.0);
        cr.rectangle(r.x - 1.0, r.y - 1.0, r.w + 2.0, r.h + 2.0);
        cr.stroke()?;
    }
    Ok(())
}

/// While recording, only the control bar catches pointer events; the region and
/// the rest of the screen pass straight through to whatever is underneath. A
/// zero-size rect (bar hidden, e.g. full-screen) clears the region entirely, so
/// everything passes through.
pub fn set_input_to_bar(surface: &gdk::Surface, x: i32, y: i32, w: i32, h: i32) {
    let region =
        cairo::Region::create_rectangle(&cairo::RectangleInt::new(x, y, w.max(0), h.max(0)));
    surface.set_input_region(Some(&region));
}
