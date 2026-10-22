//! Right-click "Add Track" items: shown on the empty area under the tracks and inside a track's
//! context menu. A track added from a track's menu lands right after that track.

use egui::Ui;
use serde_json::json;
use soundcraft_model::TrackId;

use crate::SoundApp;

/// The kinds offered, as (menu label, `track.new` kind id).
pub(crate) const ADD_KINDS: [(&str, &str); 5] =
    [("Audio Track", "audio"), ("Instrument Track", "instrument"), ("MIDI Track", "midi"), ("Aux Track", "aux"), ("Folder Track", "folder")];

/// Params for `track.new` for one kind, inserted after `after` when given.
pub(crate) fn add_params(kind: &str, after: Option<TrackId>) -> serde_json::Value {
    match after {
        Some(a) => json!({"kind": kind, "count": 1, "after": a.0}),
        None => json!({"kind": kind, "count": 1}),
    }
}

/// Menu items that add a track. Closes the menu after a choice.
pub(crate) fn add_track_items(app: &mut SoundApp, ui: &mut Ui, after: Option<TrackId>) {
    for (label, kind) in ADD_KINDS {
        if ui.button(label).clicked() {
            let _ = app.run("track.new", add_params(kind, after));
            ui.close();
        }
    }
    if ui.button("Add Multiple...").clicked() {
        let _ = app.dialogs.open_for_command(&app.engine, "track.new");
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_carry_the_anchor_only_when_given() {
        assert_eq!(add_params("midi", None), json!({"kind": "midi", "count": 1}));
        assert_eq!(add_params("midi", Some(TrackId(7)))["after"], json!(7));
    }
}
