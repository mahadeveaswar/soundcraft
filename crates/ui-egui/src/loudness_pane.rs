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

/// How often the pane asks the engine for a fresh reading (the meter itself moves slowly).
const REFRESH_SECS: f64 = 0.1;

/// Whether a reading taken at `last` is still fresh at `now`.
pub(crate) fn is_fresh(last: Option<f64>, now: f64) -> bool {
    last.is_some_and(|l| now >= l && now - l < REFRESH_SECS)
}

/// The latest `mix.loudness` reply, asked for about ten times a second instead of every frame
/// (a reset or a new session forces the next frame to ask again).
fn reading(app: &mut SoundApp, ui: &Ui) -> serde_json::Value {
    let id = egui::Id::new("loudness-reading");
    let now = ui.input(|i| i.time);
    let cached: Option<(f64, serde_json::Value)> = ui.data(|d| d.get_temp(id));
    if let Some((at, reply)) = cached
        && is_fresh(Some(at), now)
    {
        ui.ctx().request_repaint_after_secs((REFRESH_SECS - (now - at)).max(0.01) as f32);
        return reply;
    }
    let reply = app.engine.execute("mix.loudness", &json!({})).unwrap_or_default();
    ui.data_mut(|d| d.insert_temp(id, (now, reply.clone())));
    ui.ctx().request_repaint_after_secs(REFRESH_SECS as f32);
    reply
}

pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let r = ui.max_rect();
    ui.painter().rect_filled(r, 0.0, t.strip_bg);
    let reply = reading(app, ui);
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
        ui.data_mut(|d| d.remove::<(f64, serde_json::Value)>(egui::Id::new("loudness-reading")));
    }
}

#[cfg(test)]
mod tests {
    use super::is_fresh;

    #[test]
    fn a_reading_is_reused_for_a_tenth_of_a_second() {
        assert!(!is_fresh(None, 5.0));
        assert!(is_fresh(Some(5.0), 5.05));
        assert!(!is_fresh(Some(5.0), 5.11));
        assert!(!is_fresh(Some(5.0), 4.0), "a clock that went back asks again");
    }
}
