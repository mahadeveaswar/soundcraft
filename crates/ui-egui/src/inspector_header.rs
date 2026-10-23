//! The Inspector header card: a rounded glossy band in the track colour carrying the colour chip
//! (opens the colour popup), the editable name and a type badge, then evenly spaced mute / solo /
//! record / monitor toggles. Rename runs `track.rename`; the buttons run the same commands as the
//! Mix strip.

use crate::SoundApp;
use crate::clip_paint::{self, ClipPaint, ClipState, darken};
use crate::inspector_sections::{PAD, acting_tracks, contrast_text};
use crate::theme::{Tokens, bold, regular, rgb};
use crate::track_palette_ui::{self, Target};
use crate::widgets::{raised_key, shade};
use egui::{Align, Align2, Id, Key, Layout, Rect, RichText, Sense, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2};
use serde_json::json;
use soundcraft_model::{Track, TrackKind};

/// The badge shown for a track kind.
pub(crate) fn badge(kind: TrackKind) -> &'static str {
    match kind {
        TrackKind::Audio => "Audio",
        TrackKind::Aux => "Bus",
        TrackKind::Master => "Master",
        TrackKind::Midi => "MIDI",
        TrackKind::Instrument => "Instrument",
        TrackKind::Vca => "VCA",
        TrackKind::Folder => "Folder",
        TrackKind::Video => "Video",
    }
}

/// Which of mute, solo, record, monitor a kind shows.
pub(crate) fn buttons(kind: TrackKind) -> (bool, bool, bool, bool) {
    let rec = kind.has_playlist() && kind != TrackKind::Video;
    (true, kind != TrackKind::Master, rec, rec)
}

const BAND_H: f32 = 38.0;
const TOGGLE_H: f32 = 24.0;

/// A rounded state toggle: dim when off, glossy in `on_color` when on.
fn toggle(ui: &mut Ui, r: Rect, text: &str, on: bool, on_color: egui::Color32, text_on: egui::Color32, tip: &str, label: &str) -> egui::Response {
    let t = Tokens::current();
    let resp = ui.interact(r, ui.id().with(label), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, label));
    let painter = ui.painter();
    let col = if on {
        let p = ClipPaint::from_tokens(
            &t,
            if resp.hovered() { clip_paint::lighten(on_color, 0.08) } else { on_color },
            darken(on_color, 0.35),
            ClipState::Normal,
            0,
        );
        clip_paint::body(painter, r, &p);
        text_on
    } else {
        // Off: a raised key (it sinks while pressed) rather than a flat chip.
        if resp.is_pointer_button_down_on() {
            painter.rect(r, 5.0, shade(t.button, -0.14), Stroke::new(1.0, t.button_border), StrokeKind::Inside);
        } else {
            raised_key(painter, r, [shade(t.button, 0.1), shade(t.button, -0.1)], resp.hovered());
        }
        if resp.hovered() { t.text } else { t.text_dim }
    };
    painter.text(r.center(), Align2::CENTER_CENTER, text, bold(12.0), col);
    resp.on_hover_text(tip)
}

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let id = track.id;
    let msg_id = Id::new(("inspector-rename-message", id.0));
    ui.add_space(6.0);
    let (full, _) = ui.allocate_exact_size(vec2(ui.available_width(), BAND_H), Sense::hover());
    let band = Rect::from_min_max(pos2(full.min.x + PAD - 2.0, full.min.y), pos2(full.max.x - PAD + 2.0, full.max.y));
    let base = rgb(track.color);
    clip_paint::strip(ui.painter(), band, &ClipPaint { radius: 7, ..ClipPaint::strip_from_tokens(&t, base, ClipState::Normal, 0) });
    ui.painter().rect_stroke(band, 7.0, Stroke::new(1.0, darken(base, 0.45)), StrokeKind::Inside);
    let ink = contrast_text(base, &t);
    // Colour chip.
    let chip = Rect::from_center_size(pos2(band.min.x + 17.0, band.center().y), vec2(18.0, 18.0));
    let chip_resp = ui.interact(chip, Id::new(("inspector-colour-chip", id.0)), Sense::click());
    chip_resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Track colour"));
    ui.painter().rect(chip, 5.0, darken(base, 0.2), Stroke::new(if chip_resp.hovered() { 2.0 } else { 1.5 }, ink), StrokeKind::Inside);
    let chip_resp = chip_resp.on_hover_text("Choose a colour");
    let target = Target::Tracks(acting_tracks(app, id));
    track_palette_ui::swatch_popup(app, &chip_resp, Id::new(("inspector-colour", id.0)), track.color, &target);
    // Type badge pill at the right.
    let label = badge(track.kind);
    let galley = ui.painter().layout_no_wrap(label.to_string(), bold(9.5), ink);
    let pill = Rect::from_center_size(pos2(band.max.x - 8.0 - (galley.size().x + 12.0) / 2.0, band.center().y), vec2(galley.size().x + 12.0, 16.0));
    let dark_ink = ink == t.text;
    ui.painter().rect_filled(pill, 8.0, if dark_ink { egui::Color32::from_black_alpha(70) } else { egui::Color32::from_white_alpha(70) });
    ui.painter().galley(pill.center() - galley.size() / 2.0, galley, ink);
    // Name between chip and pill.
    let name_r = Rect::from_min_max(pos2(chip.max.x + 8.0, band.min.y + 4.0), pos2(pill.min.x - 6.0, band.max.y - 4.0));
    let mut c = ui.new_child(UiBuilder::new().max_rect(name_r).layout(Layout::left_to_right(Align::Center)));
    name(app, &mut c, track, msg_id, ink);
    if let Some(m) = ui.data(|d| d.get_temp::<String>(msg_id)).filter(|m| !m.is_empty()) {
        ui.horizontal(|ui| {
            ui.add_space(PAD);
            ui.label(RichText::new(m).font(regular(10.0)).color(t.rec));
        });
    }
    ui.add_space(6.0);
    let (m, s, r, i) = buttons(track.kind);
    let n = [m, s, r, i].iter().filter(|b| **b).count();
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), TOGGLE_H), Sense::hover());
    let row = Rect::from_min_max(pos2(row.min.x + PAD - 2.0, row.min.y), pos2(row.max.x - PAD + 2.0, row.max.y));
    let gap = 5.0;
    let w = ((row.width() - gap * (n as f32 - 1.0)) / n as f32).min(if n <= 2 { 64.0 } else { f32::MAX });
    let mut k = 0.0;
    let mut next = || {
        let r = Rect::from_min_size(pos2(row.min.x + k * (w + gap), row.min.y), vec2(w, TOGGLE_H));
        k += 1.0;
        r
    };
    if m {
        let on = track.mixer.mute;
        if toggle(ui, next(), "M", on, t.mute, t.text_dark, "Mute", "Inspector: Mute").clicked() {
            let _ = app.run("mix.mute", json!({"track": id.0}));
        }
    }
    if s {
        let on = track.mixer.solo;
        if toggle(ui, next(), "S", on, t.solo, t.text_dark, "Solo", "Inspector: Solo").clicked() {
            let _ = app.run("mix.solo", json!({"track": id.0}));
        }
    }
    if r {
        let on = track.mixer.record_arm;
        if toggle(ui, next(), "R", on, t.rec, t.text, "Record enable", "Inspector: Record enable").clicked() {
            let _ = app.run("mix.record_arm", json!({"track": id.0}));
        }
    }
    if i {
        let on = track.mixer.input_monitor;
        if toggle(ui, next(), "I", on, t.accent, t.accent_text, "Input monitoring", "Inspector: Input monitor").clicked() {
            let _ = app.run("mix.input_monitor", json!({"track": id.0}));
        }
    }
    ui.add_space(8.0);
}

/// The track name: a click opens an inline editor; Enter (or leaving the field) runs
/// `track.rename`, Escape cancels. The Master track's name is fixed.
fn name(app: &mut SoundApp, ui: &mut Ui, track: &Track, msg_id: Id, ink: egui::Color32) {
    let edit_id = Id::new(("inspector-rename", track.id.0));
    let started_id = edit_id.with("started");
    let field_id = edit_id.with("field");
    let width = ui.available_width().max(40.0);
    let editing = ui.data(|d| d.get_temp::<String>(edit_id));
    let Some(mut text) = editing else {
        let (nr, resp) = ui.allocate_exact_size(vec2(width, 22.0), if track.kind == TrackKind::Master { Sense::hover() } else { Sense::click() });
        let g = ui.painter().layout_no_wrap(track.name.clone(), bold(14.0), ink);
        ui.painter().with_clip_rect(nr).galley(pos2(nr.min.x, nr.center().y - g.size().y / 2.0), g, ink);
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
    let resp = ui.add(egui::TextEdit::singleline(&mut text).id(field_id).desired_width(width).font(bold(14.0)));
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
    fn badges_are_short_words() {
        for k in TrackKind::ALL {
            assert!((3..=10).contains(&badge(k).len()));
        }
    }
}
