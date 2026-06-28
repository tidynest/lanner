//! GTK4 layer-shell overlay: spotlight region selection, then border-only
//! recording with a floating Stop bar.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use anyhow::{Result, bail};
use gtk4::{
    Align, Application, ApplicationWindow, Box, Button, CssProvider, EventControllerKey, Label,
    Orientation, Overlay,
    gdk::{Display, Key},
    glib::{ExitCode, Propagation},
    prelude::*,
};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::controls::{self, Settings, SharedSettings};
use crate::overlay::Rect;
use crate::recorder::Recorder;
use crate::transcode;

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

    let (surface, rect, locked, countdown) = crate::overlay::build_surface();

    // gtk4::Overlay stacks the control bar on top of the spotlight DrawingArea
    // (which stays on the draw/input base). The bar shows the pre-draw pickers
    // during selection, then swaps to a Stop button while recording.
    let overlay = Overlay::new();
    overlay.set_child(Some(&surface));

    // Shared user choices (audio source + output format), written by the picker
    // buttons and read at record-start (audio) and stop (format).
    let settings: SharedSettings = Rc::new(RefCell::new(Settings::default()));

    // Visible from launch (top-centre) so audio/format can be chosen before the
    // region is drawn. On record we hide the pickers, reveal Stop, and the bar
    // moves clear of the capture hole via place_off_hole.
    let bar = Box::new(Orientation::Horizontal, 12);
    bar.add_css_class("control-bar");
    bar.set_halign(Align::Center);
    bar.set_valign(Align::Start);
    bar.set_margin_top(PAD);

    let pickers = controls::build_pickers(&settings);
    bar.append(&pickers);

    // REC indicator + elapsed timer, shown only while recording.
    let rec_label = Label::new(None);
    rec_label.add_css_class("rec-label");
    rec_label.set_visible(false);
    bar.append(&rec_label);

    let stop = Button::with_label("\u{23f9}  Stop");
    stop.add_css_class("stop-btn");
    stop.set_visible(false);
    bar.append(&stop);
    overlay.add_overlay(&bar);

    window.set_child(Some(&overlay));

    let recorder: Rc<RefCell<Option<Recorder>>> = Rc::new(RefCell::new(None));
    {
        let recorder = recorder.clone();
        let app = app.clone();
        let settings = settings.clone();
        stop.connect_clicked(move |_| stop_and_quit(&recorder, &app, &settings));
    }

    // Watch our own lockfile; a second invocation deletes it to request a stop.
    {
        let recorder = recorder.clone();
        let app = app.clone();
        let settings = settings.clone();
        gtk4::glib::timeout_add_local(std::time::Duration::from_millis(150), move || {
            if crate::lockfile::is_held() {
                gtk4::glib::ControlFlow::Continue
            } else {
                stop_and_quit(&recorder, &app, &settings);
                gtk4::glib::ControlFlow::Break
            }
        });
    }

    // The record-start sequence: spawn wf-recorder, swap the bar to Stop, and
    // shrink the input region. Shared by the immediate path and the countdown
    // timer's final tick, so the start logic lives in one place.
    let begin_recording: Rc<dyn Fn(Rect)> = {
        let recorder = recorder.clone();
        let settings = settings.clone();
        let locked = locked.clone();
        let surface = surface.clone();
        let win = window.clone();
        let pickers = pickers.clone();
        let stop = stop.clone();
        let rec_label = rec_label.clone();
        let bar = bar.clone();
        let overlay = overlay.clone();
        Rc::new(move |r: Rect| {
            let audio = settings.borrow().audio;
            let origin = win
                .surface()
                .map(|s| crate::overlay::monitor_origin(&s))
                .unwrap_or((0, 0));
            match Recorder::start(r, audio, origin) {
                Ok(rec) => {
                    *recorder.borrow_mut() = Some(rec);
                    locked.set(true);
                    surface.queue_draw(); // repaint: drop the dim, keep the border
                    win.set_keyboard_mode(KeyboardMode::None); // hand keys to the filmed app

                    // Swap the bar from pre-draw pickers to the Stop button, and
                    // from centred to margin-positioned so place_off_hole can
                    // steer it clear of the capture. Measure AFTER the swap: a
                    // hidden widget measures 0, emptying the input region.
                    pickers.set_visible(false);
                    stop.set_visible(true);
                    rec_label.set_visible(true);
                    rec_label.set_markup(&rec_markup(0));
                    start_rec_timer(&rec_label);
                    bar.set_halign(Align::Start);
                    bar.set_margin_start(PAD);
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
        })
    };

    let keys = EventControllerKey::new();
    // Capture phase: the overlay's Enter (record) and Esc (cancel) must win over
    // any focused picker button, else a focused toggle eats the key first.
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    let app = app.clone();
    let surface = surface.clone();
    let settings = settings.clone();
    keys.connect_key_pressed(move |_, key, _, _| match key {
        Key::Return => {
            // Ignore Enter if already recording or already counting down.
            if recorder.borrow().is_none()
                && countdown.get().is_none()
                && let Some(r) = rect.get()
            {
                let secs = settings.borrow().countdown_secs;
                if secs == 0 {
                    begin_recording(r);
                } else {
                    // Counting down: keep the dim + hole up and draw N, ticking
                    // once a second. wf-recorder is not spawned until 0, so the
                    // count is never filmed.
                    countdown.set(Some(secs));
                    surface.queue_draw();
                    let begin = begin_recording.clone();
                    let countdown = countdown.clone();
                    let surface = surface.clone();
                    gtk4::glib::timeout_add_seconds_local(1, move || match countdown.get() {
                        Some(v) if v > 1 => {
                            countdown.set(Some(v - 1));
                            surface.queue_draw();
                            gtk4::glib::ControlFlow::Continue
                        }
                        _ => {
                            countdown.set(None);
                            surface.queue_draw();
                            begin(r);
                            gtk4::glib::ControlFlow::Break
                        }
                    });
                }
            }
            Propagation::Stop
        }
        Key::Escape => {
            stop_and_quit(&recorder, &app, &settings);
            Propagation::Stop
        }
        _ => Propagation::Proceed,
    });
    window.add_controller(keys);

    window.present();
}

/// Stop any active recording (finalises the MKV) and quit. Shared by Esc and
/// the STOP button so the stop path lives in exactly one place.
fn stop_and_quit(
    recorder: &Rc<RefCell<Option<Recorder>>>,
    app: &Application,
    settings: &SharedSettings,
) {
    if let Some(rec) = recorder.borrow_mut().take() {
        let mkv = rec.stop();
        let format = settings.borrow().format;
        // Transcode in the background so the app quits and frees the overlay at
        // once; ffmpeg finishes detached. MKV kept if it fails. A completion
        // notification is M8.
        if let Err(e) = transcode::spawn(format, &mkv) {
            tracing::error!(
                "could not start transcode, keeping MKV {}: {e}",
                mkv.display()
            );
        }
    }
    app.quit();
}

/// Pango markup for the REC indicator: a red dot plus MM:SS elapsed.
fn rec_markup(secs: u32) -> String {
    format!(
        "<span foreground='#ff5d6c'>\u{23fa}</span> {:02}:{:02}",
        secs / 60,
        secs % 60
    )
}

/// Tick the REC timer once a second, updating `label` with the elapsed time.
/// Runs until the app quits (one recording per launch).
fn start_rec_timer(label: &Label) {
    let label = label.clone();
    let secs = Rc::new(Cell::new(0u32));
    gtk4::glib::timeout_add_seconds_local(1, move || {
        let s = secs.get() + 1;
        secs.set(s);
        label.set_markup(&rec_markup(s));
        gtk4::glib::ControlFlow::Continue
    });
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
        .bar-caption {
            color: rgba(220, 230, 245, 0.65);
            font-size: 12px;
            font-weight: 600;
        }
        .seg {
            background: rgba(40, 48, 66, 0.85);
            color: #e6ecf5;
            border: 1px solid rgba(102, 199, 255, 0.18);
            padding: 4px 12px;
        }
        .seg:checked {
            background: linear-gradient(180deg, #5db4ff, #3d86e2);
            color: #ffffff;
            border-color: rgba(102, 199, 255, 0.6);
        }
        .rec-label {
            color: #e6ecf5;
            font-weight: 600;
            margin-right: 4px;
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
    fn rec_markup_formats_mm_ss() {
        assert!(rec_markup(0).ends_with(" 00:00"));
        assert!(rec_markup(75).ends_with(" 01:15"));
        assert!(rec_markup(3599).ends_with(" 59:59"));
    }

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
