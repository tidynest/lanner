//! GTK4 layer-shell overlay: spotlight region selection, then border-only
//! recording with a floating Stop bar. One pinned overlay window per output;
//! the selection lives in shared state, drawn on whichever monitor owns it.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use anyhow::{Result, bail};
use gtk4::{
    Align, Application, Box, Button, CssProvider, EventControllerKey, Label, Orientation,
    gdk::{Display, Key, Monitor},
    glib::{ExitCode, Propagation},
    prelude::*,
};
use gtk4_layer_shell::{KeyboardMode, LayerShell};

use crate::controls::{self, Settings, SharedSettings};
use crate::overlay::Rect;
use crate::recorder::Recorder;
use crate::transcode;

const APP_ID: &str = "dev.lanner.Lanner";
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
    let Some(display) = Display::default() else {
        tracing::error!("no display");
        return;
    };
    // n_items + item + downcast: relies only on ListModelExt (no iter::<T>).
    let list = display.monitors();
    let monitors: Vec<Monitor> = (0..list.n_items())
        .filter_map(|i| list.item(i))
        .filter_map(|o| o.downcast::<Monitor>().ok())
        .collect();
    if monitors.is_empty() {
        tracing::error!("no monitors found");
        return;
    }

    // One pinned overlay window per output; all share the selection state.
    let shared = crate::window::Shared::default();
    let windows = Rc::new(
        monitors
            .iter()
            .enumerate()
            .map(|(i, m)| crate::window::build_output_overlay(app, m, i, &shared))
            .collect::<Vec<_>>(),
    );

    // Shared user choices (audio source + output format), written by the picker
    // buttons and read at record-start (audio) and stop (format).
    let settings: SharedSettings = Rc::new(RefCell::new(Settings::default()));

    // Control bar: pre-draw pickers + REC label + Stop, built once. Hosted by the
    // bar window's overlay and reparented onto the recording output if different.
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

    // Present every window so its surface exists (device_position needs it), then
    // pick the bar window: the monitor under the pointer at launch, else 0.
    for w in windows.iter() {
        w.window.present();
    }
    let bar_idx = pointer_window(&windows);
    windows[bar_idx].overlay.add_overlay(&bar);
    windows[bar_idx]
        .window
        .set_keyboard_mode(KeyboardMode::Exclusive); // exclusive grab = all keys

    // The record-start sequence: spawn wf-recorder on output `idx`, swap that
    // output's bar to Stop, shrink its input region, and clear the rest. Shared by
    // the immediate path and the countdown timer's final tick.
    let begin_recording: Rc<dyn Fn(usize, Rect)> = {
        let recorder = recorder.clone();
        let settings = settings.clone();
        let shared = shared.clone();
        let windows = windows.clone();
        let bar = bar.clone();
        let pickers = pickers.clone();
        let stop = stop.clone();
        let rec_label = rec_label.clone();
        Rc::new(move |idx: usize, r: Rect| {
            let audio = settings.borrow().audio;
            let origin = windows[idx].origin;
            match Recorder::start(r, audio, origin) {
                Ok(rec) => {
                    *recorder.borrow_mut() = Some(rec);
                    shared.locked.set(true);
                    shared.redraw_all(); // recording output -> border; others -> blank
                    windows[bar_idx]
                        .window
                        .set_keyboard_mode(KeyboardMode::None);

                    // Move the bar onto the recording output if it lives elsewhere.
                    if idx != bar_idx {
                        windows[bar_idx].overlay.remove_overlay(&bar);
                        windows[idx].overlay.add_overlay(&bar);
                    }

                    // Swap pickers -> Stop + REC, then place the bar clear of the
                    // hole on the recording output. Measure AFTER the swap: a
                    // hidden widget measures 0, emptying the input region.
                    pickers.set_visible(false);
                    stop.set_visible(true);
                    rec_label.set_visible(true);
                    rec_label.set_markup(&rec_markup(0));
                    start_rec_timer(&rec_label);
                    bar.set_halign(Align::Start);
                    bar.set_margin_start(PAD);
                    let rec_win = &windows[idx];
                    let bw = bar.measure(Orientation::Horizontal, -1).1;
                    let bh = bar.measure(Orientation::Vertical, -1).1;
                    let bar_rect = match place_off_hole(
                        (bar.margin_start(), bar.margin_top()),
                        (bw, bh),
                        (rec_win.overlay.width(), rec_win.overlay.height()),
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
                    if let Some(gdk_surface) = rec_win.window.surface() {
                        crate::overlay::set_input_to_bar(
                            &gdk_surface,
                            bar_rect.0,
                            bar_rect.1,
                            bar_rect.2,
                            bar_rect.3,
                        );
                    }

                    // Every other output: undim + full passthrough.
                    for w in windows.iter() {
                        if w.index != idx {
                            w.clear_passthrough();
                        }
                    }
                }
                Err(e) => tracing::error!("{e:#}"),
            }
        })
    };

    // Keyboard on the bar window only (exclusive grab receives all keys). Capture
    // phase: Enter (record) and Esc (cancel) win over a focused picker button.
    let keys = EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    {
        let app = app.clone();
        let settings = settings.clone();
        let shared = shared.clone();
        let recorder = recorder.clone();
        keys.connect_key_pressed(move |_, key, _, _| match key {
            Key::Return => {
                // Ignore Enter if already recording, counting down, or no selection.
                if recorder.borrow().is_none()
                    && shared.countdown.get().is_none()
                    && let Some((idx, r)) = shared.active.get()
                {
                    let secs = settings.borrow().countdown_secs;
                    if secs == 0 {
                        begin_recording(idx, r);
                    } else {
                        // Counting down: keep the dim + hole up and draw N, ticking
                        // once a second. wf-recorder is not spawned until 0, so the
                        // count is never filmed.
                        shared.countdown.set(Some(secs));
                        shared.redraw_all();
                        let begin = begin_recording.clone();
                        let shared = shared.clone();
                        gtk4::glib::timeout_add_seconds_local(1, move || {
                            match shared.countdown.get() {
                                Some(v) if v > 1 => {
                                    shared.countdown.set(Some(v - 1));
                                    shared.redraw_all();
                                    gtk4::glib::ControlFlow::Continue
                                }
                                _ => {
                                    shared.countdown.set(None);
                                    shared.redraw_all();
                                    begin(idx, r);
                                    gtk4::glib::ControlFlow::Break
                                }
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
    }
    windows[bar_idx].window.add_controller(keys);
}

/// Index of the window whose surface currently contains the pointer, else 0.
/// Best-effort "focused" output for placing the control bar at launch.
fn pointer_window(windows: &[crate::window::OverlayWindow]) -> usize {
    let Some(seat) = Display::default().and_then(|d| d.default_seat()) else {
        return 0;
    };
    let Some(pointer) = seat.pointer() else {
        return 0;
    };
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
