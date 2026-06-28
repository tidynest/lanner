//! The spotlight surface: a DrawingArea that dims the screen and punches a clear
//! hole while selecting, then shows only the region border while recording.

use gtk4::prelude::*;
use gtk4::{cairo, gdk};

/// A selection rectangle in widget (logical) coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Claim the single shared selection for output `i`. One selection exists at a
/// time across all monitors; a new drag overwrites any prior one (last wins).
pub fn select_on(_prev: Option<(usize, Rect)>, i: usize, rect: Rect) -> Option<(usize, Rect)> {
    Some((i, rect))
}

/// Normalise two corner points into a positive-size rectangle.
pub fn normalise(x0: f64, y0: f64, x1: f64, y1: f64) -> Rect {
    Rect {
        x: x0.min(x1),
        y: y0.min(y1),
        w: (x1 - x0).abs(),
        h: (y1 - y0).abs(),
    }
}

/// Paint one output's spotlight. Every non-recording output dims its whole
/// surface; only the `active` output (the one the selection is on) punches the
/// clear hole and strokes the border. While recording, the active output shows
/// the border only and the others draw nothing, so the rest of the desktop stays
/// visible and usable. Cairo errors bubble to the caller.
pub fn draw_for(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    rect: Option<Rect>,
    active: bool,
    recording: bool,
    countdown: Option<u32>,
) -> Result<(), cairo::Error> {
    if !recording {
        // Dim the whole surface. NB: cr.paint() drops semi-transparent content on
        // this GTK4/Wayland stack, so fill an explicit rectangle instead.
        cr.set_source_rgba(0.02, 0.03, 0.06, 0.55);
        cr.rectangle(0.0, 0.0, f64::from(w), f64::from(h));
        cr.fill()?;
    }

    if let Some(r) = rect {
        if !active {
            return Ok(()); // not the active output: dim only (or blank while recording)
        }
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

        if let Some(n) = countdown {
            draw_countdown(cr, r, n)?;
        }
    }
    Ok(())
}

/// Draw the countdown number centred in the selection: white with a dark outline
/// so it stays legible over whatever shows through the transparent hole. Sized to
/// the selection and clamped so a small region still gets a readable digit.
fn draw_countdown(cr: &cairo::Context, r: Rect, n: u32) -> Result<(), cairo::Error> {
    let text = n.to_string();
    let size = (r.h * 0.5).clamp(28.0, 220.0);
    cr.select_font_face(
        "sans-serif",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Bold,
    );
    cr.set_font_size(size);

    let ext = cr.text_extents(&text)?;
    let tx = r.x + r.w / 2.0 - ext.width() / 2.0 - ext.x_bearing();
    let ty = r.y + r.h / 2.0 - ext.height() / 2.0 - ext.y_bearing();
    cr.move_to(tx, ty);
    cr.text_path(&text);

    cr.set_source_rgba(0.0, 0.0, 0.0, 0.78); // outline for contrast
    cr.set_line_width(size * 0.06);
    cr.stroke_preserve()?;
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.97); // bright fill
    cr.fill()?;
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

/// `monitor`'s layout origin in global logical coords. Adding it translates an
/// output-local selection into wf-recorder's global `-g` geometry; wf-recorder
/// reads `-g` in these (global logical) coords, captured at native resolution.
pub fn monitor_origin(monitor: &gdk::Monitor) -> (i32, i32) {
    let g = monitor.geometry();
    (g.x(), g.y())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f64) -> Rect {
        Rect {
            x,
            y: 0.0,
            w: 10.0,
            h: 10.0,
        }
    }

    #[test]
    fn select_on_overwrites_last_wins() {
        let a = select_on(None, 0, r(1.0));
        assert_eq!(a.map(|(i, rect)| (i, rect.x)), Some((0, 1.0)));
        // a drag on a different output replaces it entirely
        let b = select_on(a, 2, r(5.0));
        assert_eq!(b.map(|(i, rect)| (i, rect.x)), Some((2, 5.0)));
    }
}
