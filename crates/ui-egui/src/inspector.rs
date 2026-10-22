//! The Inspector dock panel: the selected track's name, colour, volume fader and its inserts and
//! sends, drawn with the same slot widgets as the Edit window's header columns and the Mix strips.
//! Every change runs the same commands as those places; nothing here touches the session directly.

use crate::SoundApp;
use crate::mix_window;
use crate::theme::{Tokens, bold, regular, rgb};
use crate::widgets::{db_text, fader_slot};
use egui::{Align2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use soundcraft_model::{INSERT_SLOTS, SEND_SLOTS, Track, TrackId};

const SLOT_H: f32 = 18.0;
const FADER_H: f32 = 140.0;
const PAD: f32 = 8.0;

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
        header(ui, track);
        fader_block(app, ui, track);
        section(ui, "INSERTS");
        slots(app, ui, track, INSERT_SLOTS, mix_window::insert_slot);
        section(ui, "SENDS");
        slots(app, ui, track, SEND_SLOTS, mix_window::send_slot);
    });
}

/// The name and a swatch of the track colour.
fn header(ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
    let swatch = Rect::from_min_size(pos2(r.min.x + PAD, r.min.y + 5.0), vec2(14.0, 16.0));
    ui.painter().rect(swatch, 2.0, rgb(track.color), Stroke::new(1.0, t.border), StrokeKind::Inside);
    let name_clip = Rect::from_min_max(pos2(swatch.max.x + 6.0, r.min.y), pos2(r.max.x - PAD, r.max.y));
    ui.painter().with_clip_rect(name_clip).text(pos2(name_clip.min.x, r.center().y), Align2::LEFT_CENTER, &track.name, bold(12.0), t.text);
    resp.on_hover_text(format!("{} ({:?})", track.name, track.kind));
}

/// The volume fader with its dB readout beside it.
fn fader_block(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), FADER_H + 24.0), Sense::hover());
    let fader_r = Rect::from_min_size(pos2(r.min.x + PAD, r.min.y + 4.0), vec2(((r.width() - 2.0 * PAD) * 0.4).clamp(34.0, 60.0), FADER_H));
    mix_window::strip_fader(app, ui, fader_r, track.id, track.mixer.volume_db);
    let slot = fader_slot(fader_r);
    let x = fader_r.max.x + 14.0;
    ui.painter().text(pos2(x, slot.min.y + 8.0), Align2::LEFT_CENTER, "VOLUME", regular(9.5), t.text_dim);
    ui.painter().text(pos2(x, slot.min.y + 26.0), Align2::LEFT_CENTER, db_text(track.mixer.volume_db), bold(13.0), t.counter_text);
}

fn section(ui: &mut Ui, text: &str) {
    let t = Tokens::current();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
    ui.painter().text(pos2(r.min.x + PAD, r.center().y), Align2::LEFT_CENTER, text, regular(9.5), t.section_label);
}

/// `count` slot rows on the slot well, drawn by the Mix window's slot widgets.
fn slots(app: &mut SoundApp, ui: &mut Ui, track: &Track, count: usize, slot: fn(&mut SoundApp, &mut Ui, &Track, usize, Rect)) {
    let t = Tokens::current();
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), count as f32 * SLOT_H + 3.0), Sense::hover());
    ui.painter().rect_filled(r.shrink2(vec2(PAD - 4.0, 0.0)), 0.0, t.slot_well);
    for i in 0..count {
        let cell = Rect::from_min_size(pos2(r.min.x + PAD, r.min.y + 1.0 + i as f32 * SLOT_H), vec2(r.width() - 2.0 * PAD, 15.0));
        slot(app, ui, track, i, cell);
    }
}
