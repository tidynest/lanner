//! Pre-draw control bar: audio-source and output-format pickers plus the shared
//! `Settings` they write. `app.rs` builds the bar and swaps it to a Stop button
//! once recording starts; this module supplies the picker widgets and state.

use std::{cell::RefCell, rc::Rc};

use gtk4::{Box, Label, Orientation, ToggleButton, prelude::*};

use crate::transcode::Format;

/// Audio source for the recording. `Mic+System` (a PipeWire combined source) is
/// deferred to a later milestone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Audio {
    None,
    System,
    Mic,
}

/// User choices, shared between the picker buttons (writers) and the
/// record-start and stop paths (readers).
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub audio: Audio,
    pub format: Format,
    pub countdown_secs: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            audio: Audio::None,
            format: Format::Mp4,
            countdown_secs: 0,
        }
    }
}

pub type SharedSettings = Rc<RefCell<Settings>>;

/// Build the picker section (audio, format, and countdown groups) wired to
/// `settings`. `app.rs` adds this to the bar and hides it when recording starts.
pub fn build_pickers(settings: &SharedSettings) -> Box {
    let row = Box::new(Orientation::Horizontal, 16);
    row.append(&labelled_group(
        "Audio",
        &[
            ("None", Audio::None),
            ("System", Audio::System),
            ("Mic", Audio::Mic),
        ],
        Audio::None,
        {
            let settings = settings.clone();
            move |a| settings.borrow_mut().audio = a
        },
    ));
    row.append(&labelled_group(
        "Format",
        &[
            ("MP4", Format::Mp4),
            ("WebM", Format::Webm),
            ("AV1", Format::Av1),
            ("WebP", Format::Webp),
            ("GIF", Format::Gif),
        ],
        Format::Mp4,
        {
            let settings = settings.clone();
            move |f| settings.borrow_mut().format = f
        },
    ));
    row.append(&labelled_group(
        "Delay",
        &[("Off", 0u32), ("3s", 3), ("5s", 5), ("10s", 10)],
        0,
        {
            let settings = settings.clone();
            move |s| settings.borrow_mut().countdown_secs = s
        },
    ));
    row
}

/// A captioned segmented radio group. `items` are (label, value); the button
/// matching `default` starts active. `on_pick` fires with the value whenever a
/// button becomes the active one.
fn labelled_group<T: Copy + PartialEq + 'static>(
    caption: &str,
    items: &[(&str, T)],
    default: T,
    on_pick: impl Fn(T) + 'static,
) -> Box {
    let group = Box::new(Orientation::Horizontal, 0);
    group.add_css_class("linked"); // GTK segmented-control styling

    let on_pick = Rc::new(on_pick);
    let mut first: Option<ToggleButton> = None;
    for &(label, value) in items {
        let btn = ToggleButton::with_label(label);
        btn.add_css_class("seg");
        btn.set_focus_on_click(false); // else a clicked picker steals the Enter key from the record handler
        match first {
            Some(ref f) => btn.set_group(Some(f)), // radio: one active per group
            None => first = Some(btn.clone()),
        }
        btn.set_active(value == default); // set before connect: no spurious fire
        let on_pick = on_pick.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                on_pick(value);
            }
        });
        group.append(&btn);
    }

    let wrap = Box::new(Orientation::Horizontal, 8);
    let cap = Label::new(Some(caption));
    cap.add_css_class("bar-caption");
    wrap.append(&cap);
    wrap.append(&group);
    wrap
}
