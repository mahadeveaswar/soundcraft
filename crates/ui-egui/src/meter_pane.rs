//! The Meters dock panel: one peak meter per shown track plus the main mix, side by side, with the
//! track name under each. Levels come from the same meter state the Mix window reads.

use crate::SoundApp;
use crate::theme::{Tokens, regular};
use crate::widgets::{meter, meter_scale};
use egui::{Align2, Rect, Ui, pos2, vec2};
use soundcraft_model::TrackKind;

const LANE_W: f32 = 34.0;

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let r = ui.max_rect();
    ui.painter().rect_filled(r, 0.0, t.strip_bg);
    let session = app.engine.session_arc();
    let body_top = r.min.y + 10.0;
    let body_bottom = r.max.y - 18.0;
    if body_bottom - body_top < 30.0 {
        return;
    }
    meter_scale(ui.painter(), Rect::from_min_max(pos2(r.min.x + 22.0, body_top), pos2(r.min.x + 24.0, body_bottom)), r.min.x + 21.0);
    let mut x = r.min.x + 30.0;
    let mut lanes: Vec<(String, crate::MeterDisplay)> = session
        .tracks
        .iter()
        .filter(|x| !x.hidden && x.kind != TrackKind::Master)
        .map(|x| (x.name.clone(), app.meters.get(&x.id).copied().unwrap_or_default()))
        .collect();
    lanes.push(("Main".into(), crate::mix_window::master_display(app)));
    for (name, d) in lanes {
        let cell = Rect::from_min_max(pos2(x, body_top), pos2(x + LANE_W - 6.0, body_bottom));
        x += LANE_W;
        if cell.max.x > r.max.x {
            break;
        }
        let w = cell.width() / 2.0 - 1.0;
        meter(ui, Rect::from_min_size(cell.min, vec2(w, cell.height())), d.level[0], d.hold[0], d.clip);
        meter(ui, Rect::from_min_size(pos2(cell.min.x + w + 2.0, cell.min.y), vec2(w, cell.height())), d.level[1], d.hold[1], d.clip);
        let label: String = name.chars().take(4).collect();
        ui.painter().text(pos2(cell.center().x, r.max.y - 8.0), Align2::CENTER_CENTER, label, regular(9.0), t.text_dim);
    }
}
