//! Zoom sliders of the piano roll: a horizontal one with -/+ buttons at the bottom right and a
//! vertical one at the right edge. Both map the slider position to the zoom on a log scale, so
//! equal slider steps are equal zoom ratios. The header's H-/H+/V-/V+ buttons change the same state.

use crate::SoundApp;
use crate::theme::Tokens;
use egui::{Rect, Sense, Stroke, Ui, pos2, vec2};

/// Slider position 0..=1 for a value on a log scale between `min` and `max`.
pub fn to_slider(value: f32, min: f32, max: f32) -> f32 {
    if !(value.is_finite() && min.is_finite() && max.is_finite()) || min <= 0.0 || max <= min {
        return 0.0;
    }
    ((value.clamp(min, max) / min).ln() / (max / min).ln()).clamp(0.0, 1.0)
}

/// The value for a slider position (inverse of [`to_slider`]).
pub fn from_slider(pos: f32, min: f32, max: f32) -> f32 {
    if !pos.is_finite() || min <= 0.0 || max <= min {
        return min;
    }
    (min * (max / min).powf(pos.clamp(0.0, 1.0))).clamp(min, max)
}

/// A painted slider along `track` (horizontal or vertical). Returns the new position (0 at the
/// left / bottom) while it is pressed or dragged.
fn slider(ui: &mut Ui, track: Rect, id: egui::Id, pos: f32, vertical: bool, tip: &str) -> Option<f32> {
    let t = Tokens::current();
    let resp = ui.interact(track.expand2(if vertical { vec2(4.0, 0.0) } else { vec2(0.0, 4.0) }), id, Sense::click_and_drag());
    resp.widget_info(|| egui::WidgetInfo::slider(true, f64::from(pos), tip));
    let c = track.center();
    let (a, b) = if vertical { (pos2(c.x, track.max.y), pos2(c.x, track.min.y)) } else { (pos2(track.min.x, c.y), pos2(track.max.x, c.y)) };
    ui.painter().line_segment([a, b], Stroke::new(2.0, t.text_dim.gamma_multiply(0.5)));
    let thumb = a.lerp(b, pos.clamp(0.0, 1.0));
    let active = resp.hovered() || resp.dragged();
    ui.painter().circle_filled(thumb, if active { 5.5 } else { 4.5 }, if active { t.header_text } else { t.text_dim });
    let resp = resp.on_hover_text(tip);
    if (resp.dragged() || resp.clicked())
        && let Some(p) = resp.interact_pointer_pos()
    {
        let frac = if vertical { (track.max.y - p.y) / track.height().max(1.0) } else { (p.x - track.min.x) / track.width().max(1.0) };
        return Some(frac.clamp(0.0, 1.0));
    }
    None
}

/// A small painted -/+ button; returns true when clicked.
fn step_button(ui: &mut Ui, r: Rect, id: egui::Id, plus: bool, label: &str) -> bool {
    let t = Tokens::current();
    let resp = ui.interact(r, id, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    if resp.hovered() {
        ui.painter().rect_filled(r, 3.0, t.row_selected);
    }
    let c = r.center();
    let s = Stroke::new(1.3, t.header_text);
    ui.painter().line_segment([c + vec2(-3.5, 0.0), c + vec2(3.5, 0.0)], s);
    if plus {
        ui.painter().line_segment([c + vec2(0.0, -3.5), c + vec2(0.0, 3.5)], s);
    }
    resp.on_hover_text(label).clicked()
}

/// Draw both sliders. `h` is the bottom-right strip, `v` the right-edge column. `ppt`/`ppt_range`
/// is the horizontal zoom now in effect; `roll_h` keeps the vertical view steady while zooming.
pub fn show(app: &mut SoundApp, ui: &mut Ui, h: Rect, v: Rect, ppt: f32, ppt_range: (f32, f32), roll_h: f32, row_range: (f32, f32), step: f32) {
    let btn = vec2(14.0, h.height().min(14.0));
    let mid = h.center().y;
    let minus = Rect::from_center_size(pos2(h.min.x + btn.x / 2.0, mid), btn);
    let plus = Rect::from_center_size(pos2(h.max.x - btn.x / 2.0, mid), btn);
    let track = Rect::from_min_max(pos2(minus.max.x + 4.0, h.min.y), pos2(plus.min.x - 4.0, h.max.y));
    if step_button(ui, minus, ui.id().with("hz_minus"), false, "Zoom out horizontally") {
        app.midi.zoom_h = Some(1.0 / step);
    }
    if step_button(ui, plus, ui.id().with("hz_plus"), true, "Zoom in horizontally") {
        app.midi.zoom_h = Some(step);
    }
    if let Some(pos) = slider(ui, track, ui.id().with("hz_slider"), to_slider(ppt, ppt_range.0, ppt_range.1), false, "Horizontal zoom") {
        app.midi.ppt = Some(from_slider(pos, ppt_range.0, ppt_range.1));
        ui.ctx().request_repaint();
    }
    // Vertical: "+" (taller rows) at the top, "-" at the bottom, the slider between.
    let top = Rect::from_center_size(pos2(v.center().x, v.min.y + btn.y / 2.0), btn);
    let bottom = Rect::from_center_size(pos2(v.center().x, v.max.y - btn.y / 2.0), btn);
    let vtrack = Rect::from_min_max(pos2(v.min.x, top.max.y + 4.0), pos2(v.max.x, bottom.min.y - 4.0));
    let old = app.midi.row_h;
    let mut new = old;
    if step_button(ui, top, ui.id().with("vz_plus"), true, "Taller piano-roll rows") {
        new = crate::midi_editor::step_row_h(old, 2.0);
    }
    if step_button(ui, bottom, ui.id().with("vz_minus"), false, "Shorter piano-roll rows") {
        new = crate::midi_editor::step_row_h(old, -2.0);
    }
    if let Some(pos) = slider(ui, vtrack, ui.id().with("vz_slider"), to_slider(old, row_range.0, row_range.1), true, "Vertical zoom") {
        new = from_slider(pos, row_range.0, row_range.1);
    }
    if (new - old).abs() > f32::EPSILON {
        app.midi.row_h = new;
        if app.midi.v_manual && old > 0.0 {
            app.midi.scroll = (app.midi.scroll + roll_h / 2.0) / old * new - roll_h / 2.0;
        }
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_mapping_is_monotonic_and_round_trips() {
        let (min, max) = (0.01_f32, 1.0_f32);
        let mut last = -1.0;
        for i in 0..=20 {
            let pos = i as f32 / 20.0;
            let v = from_slider(pos, min, max);
            assert!(v > last, "monotonic at {pos}");
            last = v;
            assert!((to_slider(v, min, max) - pos).abs() < 1e-4, "round trip at {pos}");
        }
        assert_eq!(from_slider(0.0, min, max), min);
        assert!((from_slider(1.0, min, max) - max).abs() < 1e-6);
        assert!((from_slider(0.5, min, max) - 0.1).abs() < 1e-5, "halfway is the geometric mean");
    }

    #[test]
    fn mapping_clamps_and_rejects_nonsense() {
        assert_eq!(to_slider(5.0, 6.0, 32.0), 0.0);
        assert_eq!(to_slider(99.0, 6.0, 32.0), 1.0);
        assert_eq!(to_slider(f32::NAN, 6.0, 32.0), 0.0);
        assert_eq!(to_slider(10.0, 0.0, 32.0), 0.0);
        assert_eq!(from_slider(f32::NAN, 6.0, 32.0), 6.0);
        assert_eq!(from_slider(7.0, 6.0, 32.0), 32.0);
        assert_eq!(from_slider(-1.0, 6.0, 32.0), 6.0);
    }
}
