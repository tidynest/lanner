//! GTK4 layer-shell overlay: spotlight region selection, then border-only
//! recording with a floating Stop bar.

use std::{cell::RefCell, rc::Rc};

use anyhow::{Result, bail};
use gtk4::{
    Align, Application, ApplicationWindow, Box, Button, CssProvider, EventControllerKey,
    Orientation, Overlay,
    gdk::{Display, Key},
    glib::{ExitCode, Propagation},
    prelude::*,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::overlay::Rect;
use crate::recorder::Recorder;

const APP_ID: &str = "dev.lanner.Lanner";
const NAMESPACE: &str = "lanner";
const PAD: i32 = 10; // distance from edge of screen to control bar

/// Build the GTK application and run it. Returns once the overlay closes.
pub fn run() -> Result<()> {
    gtk4::init()?;

    if !gtk4_layer_shell::is_supported() {
        bail!(
            "Needs a wlroots compositor with wlr-layer-shell support (Hyprland, Sway, river, Wayfire, etc.)"
        );
    }

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build_overlay);

    match app.run() {
        ExitCode::SUCCESS => Ok(()),
        code => bail!("GTK exited with {code:?}"),
    }
}

fn build_overlay(app: &Application) {
    let window = ApplicationWindow::builder().application(app).build();

    // Layer-shell config MUST come before the window is realised (present()).
    window.init_layer_shell();
    window.set_layer(Layer::Overlay);
    window.set_namespace(Some(NAMESPACE));
    for edge in [Edge::Left, Edge::Right, Edge::Top, Edge::Bottom] {
        window.set_anchor(edge, true); // all four anchored -> stretched fullscreen
    }
    window.set_exclusive_zone(-1); // ignore other panels' reserved space
    window.set_keyboard_mode(KeyboardMode::Exclusive); // so Esc reaches us

    let (surface, rect, locked) = crate::overlay::build_surface();

    // gtk4::Overlay stacks a floating control bar on top of the spotlight
    // DrawingArea (which stays on the draw/input base). Bar is hidden until we
    // actually record, so it never blocks selection.
    let overlay = Overlay::new();
    overlay.set_child(Some(&surface));

    let bar = Box::new(Orientation::Horizontal, 0);
    bar.add_css_class("control-bar");
    bar.set_halign(Align::Start);
    bar.set_valign(Align::Start);
    bar.set_margin_start(PAD);
    bar.set_margin_top(PAD);
    bar.set_visible(false);

    let stop = Button::with_label("\u{23f9}  Stop");
    stop.add_css_class("stop-btn");
    bar.append(&stop);
    overlay.add_overlay(&bar);

    window.set_child(Some(&overlay));

    let recorder: Rc<RefCell<Option<Recorder>>> = Rc::new(RefCell::new(None));
    {
        let recorder = recorder.clone();
        let app = app.clone();
        stop.connect_clicked(move |_| stop_and_quit(&recorder, &app));
    }

    // Watch our own lockfile; a second invocation deletes it to request a stop.
    {
        let recorder = recorder.clone();
        let app = app.clone();
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            if crate::lockfile::is_held() {
                gtk4::glib::ControlFlow::Continue
            } else {
                stop_and_quit(&recorder, &app);
                gtk4::glib::ControlFlow::Break
            }
        });
    }

    let keys = EventControllerKey::new();
    let app = app.clone();
    let win = window.clone();
    let bar = bar.clone();
    let overlay = overlay.clone();
    let locked = locked.clone();
    let surface = surface.clone();
    keys.connect_key_pressed(move |_, key, _, _| match key {
        Key::Return => {
            if recorder.borrow().is_none()
                && let Some(r) = rect.get()
            {
                match Recorder::start(r) {
                    Ok(rec) => {
                        *recorder.borrow_mut() = Some(rec);
                        locked.set(true);
                        surface.queue_draw(); // repaint: drop the dim, keep the border
                        win.set_keyboard_mode(KeyboardMode::None); // hand keys to the filmed app
                        // Show the bar BEFORE measuring: a hidden widget measures
                        // 0, which would make the input region empty (bar dead).
                        bar.set_visible(true);
                        let bw = bar.measure(Orientation::Horizontal, -1).1;
                        let bh = bar.measure(Orientation::Vertical, -1).1;
                        let bar_rect = match place_off_hole(
                            (bar.margin_start(), bar.margin_top()),
                            (bw, bh),
                            (overlay.width(), overlay.height()),
                            r,
                            PAD,
                        ) {
                            Some((x, y)) => {
                                bar.set_margin_start(x);
                                bar.set_margin_top(y);
                                (x, y, bw, bh)
                            }
                            None => {
                                bar.set_visible(false);
                                tracing::info!("full-screen capture: stop with the keybind");
                                (0, 0, 0, 0)
                            }
                        };
                        if let Some(gdk_surface) = win.surface() {
                            crate::overlay::set_input_to_bar(
                                &gdk_surface,
                                bar_rect.0,
                                bar_rect.1,
                                bar_rect.2,
                                bar_rect.3,
                            );
                        }
                    }
                    Err(e) => tracing::error!("{e:#}"),
                }
            }
            Propagation::Stop
        }
        Key::Escape => {
            stop_and_quit(&recorder, &app);
            Propagation::Stop
        }
        _ => Propagation::Proceed,
    });
    window.add_controller(keys);

    window.present();
}

/// Stop any active recording (finalises the MKV) and quit. Shared by Esc and
/// the STOP button so the stop path lives in exactly one place.
fn stop_and_quit(recorder: &Rc<RefCell<Option<Recorder>>>, app: &Application) {
    if let Some(rec) = recorder.borrow_mut().take() {
        rec.stop();
    }
    app.quit();
}

/// Clamp the bar's desired top-left so the whole bar stays on screen with `pad`
/// px on every edge.
fn clamp_to_screen(
    desired: (i32, i32),
    bar: (i32, i32),
    screen: (i32, i32),
    pad: i32,
) -> (i32, i32) {
    let axis = |want: i32, size: i32, extent: i32| {
        let hi = (extent - size - pad).max(pad); // pin to `pad` if the bar can't fit
        want.clamp(pad, hi)
    };
    (
        axis(desired.0, bar.0, screen.0),
        axis(desired.1, bar.1, screen.1),
    )
}

/// AABB overlap test between the bar at `pos` (size `bar`) and the capture `hole`.
fn overlaps(pos: (i32, i32), bar: (i32, i32), hole: Rect) -> bool {
    let (x, y) = pos;
    let (bw, bh) = bar;
    let hx = hole.x as i32;
    let hy = hole.y as i32;
    let hw = hole.w as i32;
    let hh = hole.h as i32;
    x < hx + hw && x + bw > hx && y < hy + hh && y + bh > hy
}

/// Place the bar on-screen (PAD margin) AND clear of the capture `hole`, so it
/// is never filmed. Tries the dim bands below/above/right/left of the hole,
/// keeping the free axis near `desired`. Returns None when nothing fits (a
/// full-screen selection) - the caller hides the bar and relies on Esc / the
/// keybind to stop.
fn place_off_hole(
    desired: (i32, i32),
    bar: (i32, i32),
    screen: (i32, i32),
    hole: Rect,
    pad: i32,
) -> Option<(i32, i32)> {
    let (bw, bh) = bar;
    let hx = hole.x as i32;
    let hy = hole.y as i32;
    let hw = hole.w as i32;
    let hh = hole.h as i32;

    // Candidate top-left corners, one per dim band. Order = preference.
    let candidates = [
        (desired.0, hy + hh + pad), // below
        (desired.0, hy - bh - pad), // above
        (hx + hw + pad, desired.1), // right
        (hx - bw - pad, desired.1), // left
    ];
    candidates
        .into_iter()
        .map(|c| clamp_to_screen(c, bar, screen, pad))
        .find(|&p| !overlaps(p, bar, hole))
}

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_string(
        "window { background: transparent; }
        .control-bar {
            background: rgba(18, 22, 33, 0.92);
            border: 1px solid rgba(102, 199, 255, 0.35);
            border-radius: 14px;
            padding: 6px 8px;
            box-shadow: 0 6px 20px rgba(0, 0, 0, 0.45);
        }
        .stop-btn {
            background: linear-gradient(180deg, #ff5d6c, #e23b4e);
            color: #ffffff;
            font-weight: bold;
            border: none;
            border-radius: 9px;
            padding: 6px 16px;
        }
        .stop-btn:hover { background: #ff6e7b; }",
    );
    if let Some(display) = Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_keeps_bar_inside_padded_screen() {
        let screen = (1920, 1080);
        let bar = (220, 44);
        // dragged off the top-left -> snaps into the padded corner
        assert_eq!(clamp_to_screen((-100, -100), bar, screen, PAD), (PAD, PAD));
        // dragged off the bottom-right -> snaps into the far inset corner
        assert_eq!(
            clamp_to_screen((9999, 9999), bar, screen, PAD),
            (1920 - 220 - PAD, 1080 - 44 - PAD)
        );
        // already inside -> unchanged
        assert_eq!(clamp_to_screen((640, 360), bar, screen, PAD), (640, 360));
    }

    #[test]
    fn bar_dodges_hole_or_reports_no_room() {
        let screen = (1920, 1080);
        let bar = (220, 44);
        // Hole in the top-left: a clear spot must exist and not overlap it.
        let partial = Rect {
            x: 0.0,
            y: 0.0,
            w: 800.0,
            h: 500.0,
        };
        let spot = place_off_hole((10, 10), bar, screen, partial, PAD);
        assert!(spot.is_some_and(|p| !overlaps(p, bar, partial)));
        // Whole-screen hole: nowhere to go -> None (bar hides, stop via Esc).
        let full = Rect {
            x: 0.0,
            y: 0.0,
            w: 1920.0,
            h: 1080.0,
        };
        assert!(place_off_hole((10, 10), bar, screen, full, PAD).is_none());
    }
}
