//! GTK4 layer-shell overlay: A fullscreen dim that quits on Esc.

use std::{cell::RefCell, rc::Rc};

use anyhow::{Result, bail};
use gtk4::{prelude::*, {Application, ApplicationWindow, CssProvider, glib}};
use gtk4_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

use crate::recorder::Recorder;

const APP_ID: &str = "dev.lanner.Lanner";
const NAMESPACE: &str = "lanner";

/// Build the GTK application and run it. Returns once the overlay closes.
pub fn run() -> Result<()> {
    gtk4::init()?;
    
    if !gtk4_layer_shell::is_supported() {
        bail!("Needs a wlroots compositor with wlr-layer-shell support (Hyprland, Sway, river, Wayfire, etc.)");
    }

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| load_css());
    app.connect_activate(build_overlay);

    match app.run() {
        glib::ExitCode::SUCCESS => Ok(()),
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

    let (surface, rect) = crate::overlay::build_surface();
    window.set_child(Some(&surface));

    let recorder: Rc<RefCell<Option<Recorder>>> = Rc::new(RefCell::new(None));

    let keys = gtk4::EventControllerKey::new();
    let app = app.clone();
    keys.connect_key_pressed(move |_, key, _, _| {
        match key {
            gtk4::gdk::Key::Return => {
                if recorder.borrow().is_none()
                    && let Some(r) = rect.get() {
                    match Recorder::start(r) {
                        Ok(rec) => *recorder.borrow_mut() = Some(rec),
                        Err(e) => tracing::error!("{e:#}"),
                    }
                }
                glib::Propagation::Stop
            }
            gtk4::gdk::Key::Escape => {
                if let Some(rec) = recorder.borrow_mut().take() {
                    rec.stop();
                }
                app.quit();
                glib::Propagation::Stop
            }
            _ => glib::Propagation::Proceed,
        }
    });
    window.add_controller(keys);

    window.present();
}

fn load_css() {
    let provider = CssProvider::new();
    provider.load_from_string("window { background: transparent; }");
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}