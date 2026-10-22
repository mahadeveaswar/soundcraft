//! The tool palette of the piano roll (Select, Draw, Erase, Split, Glue, Mute, Zoom, Line) with
//! icons drawn in code, plus the small pure helpers behind the Glue, Mute and Zoom tools.

use crate::SoundApp;
use crate::midi_editor::MidiTool;
use crate::theme::Tokens;
use egui::{Color32, Painter, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_midi::Note;
use soundcraft_model::ClipId;

const BTN: f32 = 22.0;
const GAP: f32 = 2.0;

/// Display name and tooltip of a tool.
pub fn describe(tool: MidiTool, mac: bool) -> (&'static str, String) {
    let alt = if mac { "Option" } else { "Alt" };
    match tool {
        MidiTool::Select => ("Select", "Click to select, drag to move or resize, double-click empty space to add a note".into()),
        MidiTool::Pencil => ("Draw", "Click or drag to draw notes".into()),
        MidiTool::Erase => ("Erase", "Click or drag across notes to delete them".into()),
        MidiTool::Split => ("Split", "Click a note to cut it at the grid line".into()),
        MidiTool::Glue => ("Glue tool", "Click a note to join it with the next note of the same pitch".into()),
        MidiTool::Mute => ("Mute", "Click a note to mute or unmute it".into()),
        MidiTool::Zoom => ("Zoom", format!("Click to zoom in, {alt}-click to zoom out")),
        MidiTool::Line => ("Line", "Drag in a controller lane to draw a straight ramp".into()),
    }
}

/// The icon of a tool, drawn inside `r`.
pub fn draw_icon(p: &Painter, r: Rect, tool: MidiTool, ink: Color32) {
    let c = r.center();
    let s = Stroke::new(1.4, ink);
    match tool {
        MidiTool::Select => {
            let pts = vec![
                c + vec2(-4.0, -6.0),
                c + vec2(-4.0, 5.0),
                c + vec2(-1.5, 2.5),
                c + vec2(0.5, 6.5),
                c + vec2(2.5, 5.5),
                c + vec2(0.5, 1.5),
                c + vec2(4.5, 1.5),
            ];
            p.add(egui::Shape::closed_line(pts, Stroke::new(1.5, ink)));
        }
        MidiTool::Pencil => {
            p.line_segment([c + vec2(-4.5, 5.0), c + vec2(4.0, -4.5)], Stroke::new(3.0, ink));
            p.line_segment([c + vec2(-5.5, 6.0), c + vec2(-4.5, 3.5)], Stroke::new(1.4, ink));
        }
        MidiTool::Erase => {
            let body = Rect::from_center_size(c, vec2(12.0, 7.0));
            p.rect_stroke(body, 1.5, s, StrokeKind::Inside);
            p.line_segment([body.center_top(), body.center_bottom()], s);
        }
        MidiTool::Split => {
            let body = Rect::from_center_size(c, vec2(13.0, 6.0));
            p.rect_stroke(body, 1.0, s, StrokeKind::Inside);
            p.line_segment([c + vec2(0.0, -7.0), c + vec2(0.0, 7.0)], Stroke::new(1.6, ink));
        }
        MidiTool::Glue => {
            p.rect_filled(Rect::from_center_size(c + vec2(-5.0, 0.0), vec2(6.0, 6.0)), 1.0, ink);
            p.rect_filled(Rect::from_center_size(c + vec2(5.0, 0.0), vec2(6.0, 6.0)), 1.0, ink);
            p.line_segment([c + vec2(-3.0, 0.0), c + vec2(3.0, 0.0)], Stroke::new(2.0, ink));
        }
        MidiTool::Mute => {
            let body = Rect::from_center_size(c, vec2(13.0, 6.0));
            p.rect_stroke(body, 1.0, s, StrokeKind::Inside);
            p.line_segment([c + vec2(-6.0, 6.0), c + vec2(6.0, -6.0)], s);
        }
        MidiTool::Zoom => {
            p.circle_stroke(c + vec2(-1.5, -1.5), 4.5, s);
            p.line_segment([c + vec2(1.8, 1.8), c + vec2(6.0, 6.0)], Stroke::new(2.0, ink));
        }
        MidiTool::Line => {
            p.line_segment([c + vec2(-5.5, 5.0), c + vec2(5.5, -5.0)], s);
            p.circle_filled(c + vec2(-5.5, 5.0), 2.0, ink);
            p.circle_filled(c + vec2(5.5, -5.0), 2.0, ink);
        }
    }
}

/// One painted palette button; returns true when clicked.
fn icon_button(ui: &mut Ui, r: Rect, id: egui::Id, selected: bool, label: &str, tip: &str, paint: impl FnOnce(&Painter, Rect, Color32)) -> bool {
    let t = Tokens::current();
    let resp = ui.interact(r, id, Sense::click());
    resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, label));
    if selected {
        ui.painter().rect_filled(r, 3.0, t.row_selected);
        ui.painter().rect_stroke(r, 3.0, Stroke::new(1.0, t.header_text.gamma_multiply(0.6)), StrokeKind::Inside);
    } else if resp.hovered() {
        ui.painter().rect_filled(r, 3.0, t.row_selected.gamma_multiply(0.5));
    }
    paint(ui.painter(), r, if selected || resp.hovered() { t.header_text } else { t.text_dim });
    resp.on_hover_text(tip).clicked()
}

/// The palette: eight tool buttons and the Glue action, left to right inside `rect`.
pub fn palette(app: &mut SoundApp, ui: &mut Ui, rect: Rect, cid: ClipId) {
    let clip = ui.clip_rect().intersect(rect);
    let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    ui.set_clip_rect(clip);
    let mut x = rect.min.x;
    for tool in MidiTool::ALL {
        let r = Rect::from_min_size(pos2(x, rect.center().y - BTN / 2.0), vec2(BTN, BTN));
        x += BTN + GAP;
        if r.max.x > rect.max.x + 0.5 {
            break;
        }
        let (label, tip) = describe(tool, ui.ctx().os().is_mac());
        let id = ui.id().with(("midi_tool", tool.id()));
        if icon_button(&mut ui, r, id, app.midi.tool == tool, label, &tip, |p, r, ink| draw_icon(p, r, tool, ink)) && app.midi.tool != tool {
            crate::midi_audition::stop(app);
            app.midi.tool = tool;
        }
    }
    let glue = Rect::from_min_size(pos2(x + 2.0, rect.center().y - 9.0), vec2(36.0, 18.0));
    if glue.max.x <= rect.max.x + 0.5 {
        let glue_id = ui.id().with("midi_glue_action");
        let clicked = icon_button(&mut ui, glue, glue_id, false, "Glue", "Join the selected notes of the same pitch into one", |p, r, ink| {
            p.text(r.center(), egui::Align2::CENTER_CENTER, "Glue", crate::theme::regular(11.0), ink);
        });
        if clicked && !app.midi.selected.is_empty() {
            let _ = app.run("midi.notes_glue", json!({"clip": cid.0, "indices": app.midi.selected}));
            app.midi.selected.clear();
        }
    }
}

/// The note that follows `index` with the same pitch and channel (the one the Glue tool joins it to).
pub fn next_same_pitch(notes: &[Note], index: usize) -> Option<usize> {
    let n = notes.get(index)?;
    notes
        .iter()
        .enumerate()
        .filter(|(j, m)| *j != index && m.pitch == n.pitch && m.channel == n.channel && (m.start, *j) > (n.start, index))
        .min_by_key(|(j, m)| (m.start, *j))
        .map(|(j, _)| j)
}

/// The notes a Mute click acts on: the whole selection when the clicked note is part of it, else
/// just the clicked note.
pub fn mute_targets(selected: &[usize], clicked: usize) -> Vec<usize> {
    if selected.contains(&clicked) { selected.to_vec() } else { vec![clicked] }
}

/// Zoom-tool click: the new horizontal zoom and first visible tick when zooming in (or out with
/// `alt`) around the pointer, which sits `pointer_dx` points right of the roll's left edge.
pub fn zoom_click(ppt: f32, view_start: f64, pointer_dx: f32, alt: bool, step: f32, min: f32, max: f32) -> (f32, f64) {
    let factor = if alt { 1.0 / step } else { step };
    let new_ppt = (ppt * factor).clamp(min, max.max(min));
    let anchor = view_start + f64::from(pointer_dx / ppt);
    (new_ppt, (anchor - f64::from(pointer_dx / new_ppt)).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(pitch: u8, start: i64) -> Note {
        Note { pitch, velocity: 100, release_velocity: 64, channel: 0, start, length: 100 }
    }

    #[test]
    fn glue_finds_the_next_note_of_the_same_pitch() {
        let notes = [n(60, 0), n(62, 50), n(60, 200), n(60, 400)];
        assert_eq!(next_same_pitch(&notes, 0), Some(2));
        assert_eq!(next_same_pitch(&notes, 2), Some(3));
        assert_eq!(next_same_pitch(&notes, 3), None);
        assert_eq!(next_same_pitch(&notes, 1), None);
        assert_eq!(next_same_pitch(&notes, 9), None);
    }

    #[test]
    fn mute_acts_on_the_selection_only_when_the_clicked_note_is_in_it() {
        assert_eq!(mute_targets(&[1, 2, 3], 2), vec![1, 2, 3]);
        assert_eq!(mute_targets(&[1, 2, 3], 7), vec![7]);
        assert_eq!(mute_targets(&[], 0), vec![0]);
    }

    #[test]
    fn zoom_click_keeps_the_tick_under_the_pointer_and_alt_zooms_out() {
        let (ppt, start) = zoom_click(0.1, 100.0, 200.0, false, 1.25, 0.01, 1.0);
        assert!((ppt - 0.125).abs() < 1e-6);
        let tick_before = 100.0 + 200.0 / 0.1_f64;
        let tick_after = start + 200.0 / f64::from(ppt);
        assert!((tick_before - tick_after).abs() < 0.01, "{tick_before} vs {tick_after}");
        let (out, _) = zoom_click(0.1, 0.0, 50.0, true, 1.25, 0.01, 1.0);
        assert!((out - 0.08).abs() < 1e-6);
        let (clamped, _) = zoom_click(1.0, 0.0, 0.0, false, 1.25, 0.01, 1.0);
        assert_eq!(clamped, 1.0);
    }
}
