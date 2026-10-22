//! The track colour popup: the extended palette grid, the default set, recent picks and a hex
//! field. Used by the Inspector (colour swatch) and the Settings > Track colours page. A pick
//! runs `track.color` (tracks) or `ui.theme_set_track_color` (a new-track palette slot);
//! "Apply to this track's clips" runs `track.color` with `apply_to_clips`, one undo step.

use crate::SoundApp;
use crate::settings_appearance::{bad_colour_message, hex_edit};
use crate::theme::{Tokens, regular, rgb};
use egui::{Color32, Id, Response, RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, vec2};
use serde_json::json;
use soundcraft_model::{TRACK_COLORS, TRACK_PALETTE_EXT, TrackId};

const COLUMNS: usize = 12;
const CELL: f32 = 18.0;
const GAP: f32 = 2.0;
/// How many recent picks are remembered.
pub(crate) const RECENT_MAX: usize = 8;

/// What a pick changes.
#[derive(Clone)]
pub(crate) enum Target {
    /// These tracks (the popup then also offers "Apply to this track's clips").
    Tracks(Vec<TrackId>),
    /// Slot `n` of the palette new tracks cycle through.
    NewTrackSlot(usize),
}

/// `c` moved to the front of `recent`, without duplicates, at most `RECENT_MAX` long.
pub(crate) fn push_recent(recent: &mut Vec<[u8; 3]>, c: [u8; 3]) {
    recent.retain(|x| *x != c);
    recent.insert(0, c);
    recent.truncate(RECENT_MAX);
}

fn hex_of(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

/// A swatch button that opens the colour popup on click.
pub(crate) fn swatch_button(app: &mut SoundApp, ui: &mut Ui, id: Id, current: [u8; 3], target: &Target, size: egui::Vec2) -> Response {
    let t = Tokens::current();
    let (r, resp) = ui.allocate_exact_size(size, Sense::click());
    ui.painter().rect(r, 2.0, rgb(current), Stroke::new(1.0, t.border), StrokeKind::Inside);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Track colour"));
    let resp = resp.on_hover_text("Choose a colour");
    swatch_popup(app, &resp, id, current, target);
    resp
}

/// The colour popup under `resp` (a click on it opens the popup); for callers that paint their own swatch.
pub(crate) fn swatch_popup(app: &mut SoundApp, resp: &Response, id: Id, current: [u8; 3], target: &Target) {
    egui::Popup::menu(resp).id(id).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| contents(app, ui, id, current, target));
}

/// The colour popup body for the tracks with these ids (a context menu entry shows it): the grid,
/// recent picks, hex field and "Apply to this track's clips", all acting on every listed track.
pub(crate) fn popup(app: &mut SoundApp, ui: &mut Ui, tracks: &[u64]) {
    let ids: Vec<TrackId> = tracks.iter().map(|t| TrackId(*t)).collect();
    let current = ids.first().and_then(|id| app.engine.session().track(*id)).map_or([128, 128, 128], |t| t.color);
    let id = Id::new(("track-colour-popup", tracks.first().copied().unwrap_or(0)));
    contents(app, ui, id, current, &Target::Tracks(ids));
}

/// One clickable swatch cell; returns whether it was clicked.
fn cell(ui: &mut Ui, c: [u8; 3], current: [u8; 3], size: f32) -> bool {
    let t = Tokens::current();
    let (r, resp) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let on = c == current;
    ui.painter().rect_filled(r, 2.0, rgb(c));
    let stroke = if on { Stroke::new(2.0, t.text) } else { Stroke::new(1.0, t.border) };
    ui.painter().rect_stroke(r, 2.0, stroke, StrokeKind::Inside);
    let label = format!("Colour {}", hex_of(c));
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
    resp.on_hover_text(hex_of(c)).clicked()
}

fn row(ui: &mut Ui, colors: &[[u8; 3]], current: [u8; 3]) -> Option<[u8; 3]> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(GAP, GAP);
        for c in colors {
            if cell(ui, *c, current, CELL) {
                picked = Some(*c);
            }
        }
    });
    picked
}

/// Run the pick for `target`; returns an error text, or empty.
fn apply(app: &mut SoundApp, target: &Target, c: [u8; 3]) -> String {
    let result = match target {
        Target::Tracks(ids) => {
            let tracks: Vec<u64> = ids.iter().map(|i| i.0).collect();
            app.run("track.color", json!({"tracks": tracks, "color": c}))
        }
        Target::NewTrackSlot(i) => app.run("ui.theme_set_track_color", json!({"index": i, "color": hex_of(c)})),
    };
    match result {
        Ok(_) => {
            push_recent(&mut app.ui.recent_track_colors, c);
            String::new()
        }
        Err(e) => e,
    }
}

/// The popup body.
pub(crate) fn contents(app: &mut SoundApp, ui: &mut Ui, id: Id, current: [u8; 3], target: &Target) {
    let t = Tokens::current();
    let msg_id = id.with("message");
    let mut message = ui.data_mut(|d| d.get_temp::<String>(msg_id)).unwrap_or_default();
    let mut picked = None;
    ui.spacing_mut().item_spacing = vec2(GAP, GAP);
    ui.label(RichText::new("Recent").font(regular(10.0)).color(t.text_dim));
    let recent = app.ui.recent_track_colors.clone();
    if recent.is_empty() {
        ui.label(RichText::new("Colours you pick appear here").font(regular(10.0)).color(t.text_dim));
    } else {
        picked = picked.or(row(ui, &recent, current));
    }
    ui.add_space(4.0);
    for chunk in TRACK_PALETTE_EXT.chunks(COLUMNS) {
        picked = picked.or(row(ui, chunk, current));
    }
    ui.add_space(4.0);
    ui.label(RichText::new("Default set").font(regular(10.0)).color(t.text_dim));
    for chunk in TRACK_COLORS.chunks(COLUMNS) {
        picked = picked.or(row(ui, chunk, current));
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label(RichText::new("Hex").font(regular(10.0)).color(t.text_dim));
        match hex_edit(ui, id.with("hex"), Color32::from_rgb(current[0], current[1], current[2]), 70.0) {
            Ok(Some(c)) => picked = Some([c.r(), c.g(), c.b()]),
            Ok(None) => {}
            Err(text) => message = bad_colour_message(&text),
        }
    });
    if let Target::Tracks(ids) = target {
        let label = "Apply to this track's clips";
        if ui.button(label).on_hover_text("Also give clips that carry their own colour this track's colour").clicked() {
            let tracks: Vec<u64> = ids.iter().map(|i| i.0).collect();
            message = match app.run("track.color", json!({"tracks": tracks, "color": current, "apply_to_clips": true})) {
                Ok(_) => String::new(),
                Err(e) => e,
            };
        }
    }
    if let Some(c) = picked {
        message = apply(app, target, c);
    }
    if !message.is_empty() {
        ui.label(RichText::new(&message).font(regular(10.0)).color(t.rec));
    }
    ui.data_mut(|d| d.insert_temp(msg_id, message));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_is_newest_first_deduped_and_capped() {
        let mut r = Vec::new();
        for i in 0..12u8 {
            push_recent(&mut r, [i, 0, 0]);
        }
        assert_eq!(r.len(), RECENT_MAX);
        assert_eq!(r[0], [11, 0, 0]);
        push_recent(&mut r, [8, 0, 0]);
        assert_eq!(r[0], [8, 0, 0]);
        assert_eq!(r.iter().filter(|c| **c == [8, 0, 0]).count(), 1);
    }
}
