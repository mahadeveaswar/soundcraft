//! The Master dock panel: the master level meter and volume fader, built from the same widgets
//! as the Mix window's master strip.

use crate::theme::{Tokens, regular};
use crate::widgets::{db_text, fader_slot, meter_scale};
use crate::{SoundApp, mix_window};
use egui::{Align2, Rect, Ui, pos2, vec2};
use soundcraft_model::TrackKind;

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let r = ui.max_rect();
    ui.painter().rect_filled(r, 0.0, t.strip_bg);
    let session = app.engine.session_arc();
    let Some(master) = session.tracks.iter().find(|x| x.kind == TrackKind::Master) else {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "No master track", regular(11.0), t.text_dim);
        return;
    };
    let pad = 8.0;
    let body = Rect::from_min_max(pos2(r.min.x + pad, r.min.y + 14.0), pos2(r.max.x - pad, r.max.y - 26.0));
    let h = body.height().max(60.0);
    let fader_r = Rect::from_min_size(body.min, vec2((body.width() * 0.4).clamp(34.0, 60.0), h));
    mix_window::strip_fader(app, ui, fader_r, master.id, master.mixer.volume_db);
    let slot = fader_slot(fader_r);
    let mr = Rect::from_min_max(pos2(fader_r.max.x + 24.0, slot.min.y - 5.0), pos2((fader_r.max.x + 24.0 + 48.0).min(body.max.x), slot.max.y + 1.0));
    meter_scale(ui.painter(), mr, mr.min.x - 1.0);
    let md = mix_window::master_display(app);
    mix_window::stereo_meters(ui, mr, &md);
    ui.painter().text(pos2(r.center().x, r.max.y - 12.0), Align2::CENTER_CENTER, db_text(master.mixer.volume_db), regular(11.0), t.counter_text);
    ui.painter().text(pos2(r.center().x, r.min.y + 7.0), Align2::CENTER_CENTER, "MASTER", regular(9.5), t.text_dim);
}
