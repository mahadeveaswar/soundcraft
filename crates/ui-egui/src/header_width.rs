//! The Edit window's track header column width: a UI setting (`UiState.header_w`) with a drag
//! handle on the column's right edge, a command (`ui.header_width`) and a View menu reset.

use crate::SoundApp;
use crate::edit_window::HEADER_W;
use crate::theme::Tokens;
use egui::{CursorIcon, Rect, Sense, Ui, pos2, vec2};
use serde_json::{Value, json};

/// Narrowest column for the compact header, which lays out down to this width.
pub const MIN_COMPACT: f32 = 112.0;
/// Narrowest column for the classic header, whose controls sit at fixed offsets.
pub const MIN_CLASSIC: f32 = HEADER_W;
pub const MAX: f32 = 640.0;
/// Share of the canvas width the column may take, so the timeline keeps its room.
pub const MAX_SHARE: f32 = 0.45;
/// Width of the drag handle's hit area.
const HANDLE_W: f32 = 6.0;

pub fn min_width(classic: bool) -> f32 {
    if classic { MIN_CLASSIC } else { MIN_COMPACT }
}

/// `w` limited to what the header style and the canvas width allow.
pub fn clamp(w: f32, canvas_w: f32, classic: bool) -> f32 {
    let min = min_width(classic);
    let max = MAX.min(canvas_w * MAX_SHARE).max(min);
    if w.is_finite() { w.clamp(min, max) } else { HEADER_W.clamp(min, max) }
}

fn is_classic(app: &SoundApp) -> bool {
    app.ui.header_style == crate::track_header_compact::HeaderStyle::Classic
}

/// The width in force this frame for a canvas `canvas_w` wide.
pub fn live(app: &SoundApp, canvas_w: f32) -> f32 {
    clamp(app.ui.header_w, canvas_w, is_classic(app))
}

/// `ui.header_width {value?, reset?}`: report or set the header column width (no undo, UI only).
pub(crate) fn command(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    if p.get("reset").and_then(Value::as_bool) == Some(true) {
        app.ui.header_w = HEADER_W;
    } else if let Some(v) = p.get("value") {
        let w = v.as_f64().filter(|n| n.is_finite()).ok_or("value must be a number of points")? as f32;
        app.ui.header_w = w.clamp(min_width(is_classic(app)), MAX);
    }
    Ok(json!({"value": app.ui.header_w, "default": HEADER_W}))
}

/// The drag handle on the right edge of the header column (`edge_x`), spanning `y0..y1`.
/// Dragging is position based (press origin plus pointer), double-click resets.
pub(crate) fn handle(app: &mut SoundApp, ui: &mut Ui, edge_x: f32, y0: f32, y1: f32, canvas_w: f32, current: f32) {
    let t = Tokens::current();
    let r = Rect::from_min_max(pos2(edge_x - HANDLE_W / 2.0, y0), pos2(edge_x + HANDLE_W / 2.0, y1));
    let resp = ui.interact(r, ui.id().with("header_width_handle"), Sense::click_and_drag());
    let key = egui::Id::new("header_width_drag");
    if resp.drag_started()
        && let Some(p) = ui.input(|i| i.pointer.press_origin())
    {
        ui.data_mut(|d| d.insert_temp(key, (p.x, current)));
    }
    if resp.dragged()
        && let (Some(p), Some((x0, w0))) = (resp.interact_pointer_pos(), ui.data(|d| d.get_temp::<(f32, f32)>(key)))
    {
        let w = clamp(w0 + (p.x - x0), canvas_w, is_classic(app));
        if (w - app.ui.header_w).abs() > 0.01 {
            app.ui.header_w = w;
            ui.ctx().request_repaint();
        }
    }
    if resp.double_clicked() {
        app.ui.header_w = HEADER_W;
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        let col = if resp.dragged() { t.accent } else { t.accent.gamma_multiply(0.6) };
        ui.painter().rect_filled(Rect::from_center_size(pos2(edge_x, (y0 + y1) / 2.0), vec2(2.0, y1 - y0)), 0.0, col);
    }
    resp.on_hover_text("Drag to change the track header width (double-click: reset)");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_respects_style_minimum_cap_and_window_share() {
        assert_eq!(clamp(50.0, 2000.0, false), MIN_COMPACT);
        assert_eq!(clamp(50.0, 2000.0, true), MIN_CLASSIC);
        assert_eq!(clamp(900.0, 3000.0, false), MAX);
        assert_eq!(clamp(900.0, 1000.0, false), 450.0);
        assert_eq!(clamp(300.0, 1000.0, false), 300.0);
        // A tiny window never pushes the column below its minimum.
        assert_eq!(clamp(300.0, 100.0, false), MIN_COMPACT);
        assert_eq!(clamp(f32::NAN, 2000.0, false), HEADER_W);
    }

    #[test]
    fn stored_default_and_the_constant_give_bit_identical_widths() {
        for classic in [false, true] {
            for canvas in [900.0_f32, 1600.0, 3000.0] {
                let stored = 136.0_f64 as f32; // what `ui.header_width {value: 136}` stores
                assert_eq!(clamp(stored, canvas, classic).to_bits(), clamp(HEADER_W, canvas, classic).to_bits());
            }
        }
    }
}
