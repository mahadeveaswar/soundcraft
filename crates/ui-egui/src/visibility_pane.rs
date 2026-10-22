//! The Visibility dock panel: every track with a show/hide check box, and a "shown / total" counter.
//! Changes run `track.hide`, so they are undoable and hide the track in the Edit and Mix windows.

use crate::SoundApp;
use crate::theme::{Tokens, bold, regular, rgb};
use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::json;

/// `(id, name, shown, colour)` of every track, in session order.
fn rows(app: &SoundApp) -> Vec<(u64, String, bool, [u8; 3])> {
    app.engine.session().tracks.iter().map(|t| (t.id.0, t.name.clone(), !t.hidden, t.color)).collect()
}

/// The counter text: shown tracks over all tracks.
pub(crate) fn counter(app: &SoundApp) -> String {
    let s = app.engine.session();
    format!("{} / {}", s.tracks.iter().filter(|t| !t.hidden).count(), s.tracks.len())
}

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.panel_bg);
    let (head, _) = ui.allocate_exact_size(vec2(ui.available_width(), 22.0), Sense::hover());
    ui.painter().rect_filled(head, 0.0, t.panel_bg2);
    ui.painter().text(pos2(head.min.x + 8.0, head.center().y), Align2::LEFT_CENTER, "SHOWN", bold(10.5), t.header_text);
    ui.painter().text(pos2(head.max.x - 8.0, head.center().y), Align2::RIGHT_CENTER, counter(app), bold(10.5), t.counter_text);
    ui.painter().line_segment([pos2(head.min.x, head.max.y), pos2(head.max.x, head.max.y)], Stroke::new(1.0, t.border));
    let rows = rows(app);
    egui::ScrollArea::vertical().id_salt("visibility_scroll").auto_shrink([false, false]).show(ui, |ui| {
        for (id, name, shown, color) in rows {
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click());
            resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, true, shown, &name));
            if resp.hovered() {
                ui.painter().rect_filled(r, 0.0, t.row_selected);
            }
            let boxr = Rect::from_center_size(pos2(r.min.x + 12.0, r.center().y), vec2(11.0, 11.0));
            ui.painter().rect_stroke(boxr, 2.0, Stroke::new(1.0, t.text_dim), egui::StrokeKind::Inside);
            if shown {
                ui.painter().rect_filled(boxr.shrink(2.5), 1.0, t.visible_dot);
            }
            ui.painter().rect_filled(Rect::from_min_size(pos2(r.min.x + 24.0, r.min.y + 3.0), vec2(4.0, 12.0)), 0.0, rgb(color));
            ui.painter().text(pos2(r.min.x + 34.0, r.center().y), Align2::LEFT_CENTER, &name, regular(11.5), if shown { t.text } else { t.text_dim });
            if resp.clicked() {
                let _ = app.run("track.hide", json!({"tracks": [id], "hidden": shown}));
            }
        }
    });
}
