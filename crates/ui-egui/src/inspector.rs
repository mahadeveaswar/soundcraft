//! The Inspector dock panel: the selected track's header (colour, name, type, mute/solo/record/
//! monitor) and collapsible sections chosen by track kind (routing, instrument, inserts, sends,
//! channel, notes). The sections and header live in `inspector_sections.rs` and
//! `inspector_header.rs`; every change runs the same commands as the Mix strips, and nothing here
//! touches the session directly.

use crate::SoundApp;
use crate::inspector_header;
use crate::inspector_sections::{self, inspector_sections};
use crate::theme::{Tokens, regular};
use egui::{Align2, Ui};
use soundcraft_model::TrackId;

/// The track the Inspector shows: the first selected track that still exists.
pub(crate) fn shown_track(app: &SoundApp) -> Option<TrackId> {
    let s = app.engine.session();
    s.edit.selected_tracks.iter().copied().find(|id| s.tracks.iter().any(|t| t.id == *id))
}

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let r = ui.max_rect();
    ui.painter().rect_filled(r, 0.0, t.strip_bg);
    let session = app.engine.session_arc();
    let Some(track) = shown_track(app).and_then(|id| session.tracks.iter().find(|x| x.id == id)) else {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "No track selected", regular(11.0), t.text_dim);
        return;
    };
    egui::ScrollArea::vertical().id_salt("inspector-scroll").auto_shrink([false, false]).show(ui, |ui| {
        inspector_header::show(app, ui, track);
        for s in inspector_sections(track.kind) {
            if inspector_sections::header(app, ui, *s) {
                inspector_sections::body(app, ui, *s, track);
            }
        }
    });
}
