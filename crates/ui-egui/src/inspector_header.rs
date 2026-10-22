//! The Inspector header: colour swatch (opens the colour popup), inline rename, type badge and the
//! mute / solo / record / monitor buttons. Rename runs `track.rename`; the buttons run the same
//! commands as the Mix strip.

use crate::SoundApp;
use crate::inspector_sections::{PAD, acting_tracks};
use crate::theme::{Tokens, bold, regular};
use crate::track_palette_ui::{self, Target};
use crate::widgets::{rec_toggle, text_toggle};
use egui::{Id, Key, RichText, Sense, Ui, WidgetInfo, WidgetType, vec2};
use serde_json::json;
use soundcraft_model::{Track, TrackKind};

/// Three-letter badge for a track kind.
pub(crate) fn badge(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Audio => "AUD",
        TrackKind::Aux => "AUX",
        TrackKind::Master => "MST",
        TrackKind::Midi => "MID",
        TrackKind::Instrument => "INS",
        TrackKind::Vca => "VCA",
        TrackKind::Folder => "FLD",
        TrackKind::Video => "VID",
    }
}

/// Which of mute, solo, record, monitor a kind shows.
pub(crate) fn buttons(kind: TrackKind) -> (bool, bool, bool, bool) {
    let rec = kind.has_playlist() && kind != TrackKind::Video;
    (true, kind != TrackKind::Master, rec, rec)
}

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let id = track.id;
    let msg_id = Id::new(("inspector-rename-message", id.0));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.add_space(PAD);
        ui.spacing_mut().item_spacing.x = 6.0;
        let target = Target::Tracks(acting_tracks(app, id));
        track_palette_ui::swatch_button(app, ui, Id::new(("inspector-colour", id.0)), track.color, &target, vec2(14.0, 18.0));
        name(app, ui, track, msg_id);
        ui.label(RichText::new(badge(track.kind)).font(bold(9.0)).color(t.text_dim));
    });
    if let Some(m) = ui.data(|d| d.get_temp::<String>(msg_id)).filter(|m| !m.is_empty()) {
        ui.horizontal(|ui| {
            ui.add_space(PAD);
            ui.label(RichText::new(m).font(regular(10.0)).color(t.rec));
        });
    }
    ui.add_space(2.0);
    let (m, s, r, i) = buttons(track.kind);
    ui.horizontal(|ui| {
        ui.add_space(PAD);
        ui.spacing_mut().item_spacing.x = 4.0;
        let sz = vec2(24.0, 18.0);
        if m {
            let on = track.mixer.mute;
            let resp = text_toggle(ui, sz, "M", on, t.mute, "Mute");
            resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, "Inspector: Mute"));
            if resp.clicked() {
                let _ = app.run("mix.mute", json!({"track": id.0}));
            }
        }
        if s {
            let on = track.mixer.solo;
            let resp = text_toggle(ui, sz, "S", on, t.solo, "Solo");
            resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, "Inspector: Solo"));
            if resp.clicked() {
                let _ = app.run("mix.solo", json!({"track": id.0}));
            }
        }
        if r {
            let on = track.mixer.record_arm;
            let resp = rec_toggle(ui, sz, on, "Record enable");
            resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, "Inspector: Record enable"));
            if resp.clicked() {
                let _ = app.run("mix.record_arm", json!({"track": id.0}));
            }
        }
        if i {
            let on = track.mixer.input_monitor;
            let resp = text_toggle(ui, sz, "I", on, t.input, "Input monitoring");
            resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, "Inspector: Input monitor"));
            if resp.clicked() {
                let _ = app.run("mix.input_monitor", json!({"track": id.0}));
            }
        }
    });
    ui.add_space(4.0);
}

/// The track name: a click opens an inline editor; Enter (or leaving the field) runs
/// `track.rename`, Escape cancels. The Master track's name is fixed.
fn name(app: &mut SoundApp, ui: &mut Ui, track: &Track, msg_id: Id) {
    let t = Tokens::current();
    let edit_id = Id::new(("inspector-rename", track.id.0));
    let started_id = edit_id.with("started");
    let field_id = edit_id.with("field");
    let width = (ui.available_width() - 40.0).max(40.0);
    let editing = ui.data(|d| d.get_temp::<String>(edit_id));
    let Some(mut text) = editing else {
        let resp = ui.add_sized(
            vec2(width, 18.0),
            egui::Label::new(RichText::new(&track.name).font(bold(12.0)).color(t.text)).truncate().sense(if track.kind == TrackKind::Master {
                Sense::hover()
            } else {
                Sense::click()
            }),
        );
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, track.kind != TrackKind::Master, "Inspector: Track name"));
        if track.kind != TrackKind::Master && resp.clicked() {
            ui.data_mut(|d| {
                d.insert_temp(edit_id, track.name.clone());
                d.insert_temp(started_id, false);
                d.remove::<String>(msg_id);
            });
        }
        return;
    };
    let resp = ui.add(egui::TextEdit::singleline(&mut text).id(field_id).desired_width(width).font(bold(12.0)));
    let started = ui.data(|d| d.get_temp::<bool>(started_id)).unwrap_or(true);
    if !started {
        resp.request_focus();
        ui.data_mut(|d| d.insert_temp(started_id, true));
    }
    let escape = ui.input(|i| i.key_pressed(Key::Escape));
    if started && resp.lost_focus() {
        ui.data_mut(|d| d.remove::<String>(edit_id));
        if escape {
            return;
        }
        let new = text.trim().to_string();
        if new != track.name {
            let msg = match app.run("track.rename", json!({"track": track.id.0, "name": new})) {
                Ok(_) => String::new(),
                Err(e) => e,
            };
            ui.data_mut(|d| d.insert_temp(msg_id, msg));
        }
    } else {
        ui.data_mut(|d| d.insert_temp(edit_id, text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buttons_follow_the_kind() {
        assert_eq!(buttons(TrackKind::Audio), (true, true, true, true));
        assert_eq!(buttons(TrackKind::Aux), (true, true, false, false));
        assert_eq!(buttons(TrackKind::Master), (true, false, false, false));
        assert_eq!(buttons(TrackKind::Folder), (true, true, false, false));
    }

    #[test]
    fn badges_are_three_letters() {
        for k in TrackKind::ALL {
            assert_eq!(badge(k).len(), 3);
        }
    }
}
