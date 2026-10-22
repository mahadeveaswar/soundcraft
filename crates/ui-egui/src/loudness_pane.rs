//! The Loudness dock panel: momentary, short-term and integrated loudness (LUFS), the maximum
//! momentary and the true peak of the master bus, with a Reset button. Reads the live meter through
//! `mix.loudness`; Reset runs `mix.loudness_reset`.

use crate::SoundApp;
use crate::theme::{Tokens, bold, mono, regular};
use crate::widgets::db_text;
use egui::{Align2, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;

/// `(label, unit, value)` rows from a `mix.loudness` reply (a missing value reads as nothing measured).
pub(crate) fn rows(reply: &serde_json::Value) -> [(&'static str, &'static str, f32); 5] {
    let get = |k: &str| reply.get(k).and_then(serde_json::Value::as_f64).map_or(-144.0, |v| v as f32);
    [
        ("Momentary", "LUFS", get("momentary")),
        ("Short-term", "LUFS", get("short_term")),
        ("Integrated", "LUFS", get("integrated")),
        ("Max momentary", "LUFS", get("max_momentary")),
        ("True peak", "dBTP", get("true_peak_db")),
    ]
}

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let r = ui.max_rect();
    ui.painter().rect_filled(r, 0.0, t.strip_bg);
    let reply = app.engine.execute("mix.loudness", &json!({})).unwrap_or_default();
    let mut y = r.min.y + 14.0;
    for (label, unit, value) in rows(&reply) {
        ui.painter().text(pos2(r.min.x + 10.0, y), Align2::LEFT_CENTER, label, regular(11.0), t.text_dim);
        ui.painter().text(pos2(r.max.x - 44.0, y), Align2::RIGHT_CENTER, db_text(value), mono(13.0), t.counter_text);
        ui.painter().text(pos2(r.max.x - 10.0, y), Align2::RIGHT_CENTER, unit, regular(9.5), t.text_dim);
        y += 24.0;
    }
    let btn = Rect::from_min_size(pos2(r.min.x + 10.0, y + 4.0), vec2(70.0, 22.0));
    let resp = ui.interact(btn, ui.id().with("loudness_reset"), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Reset"));
    let resp = resp.on_hover_text("Restart the integrated, maximum and true-peak measurement");
    ui.painter().rect_filled(btn, 3.0, if resp.hovered() { t.row_selected } else { t.panel_bg2 });
    ui.painter().text(btn.center(), Align2::CENTER_CENTER, "Reset", bold(10.5), t.text);
    if resp.clicked() {
        let _ = app.run("mix.loudness_reset", json!({}));
    }
}
