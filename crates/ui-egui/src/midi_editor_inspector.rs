//! The inspector strip at the left of the MIDI editor (about 170 pt wide, collapsible to nothing):
//! Quantize, Transpose, Length, Velocity, Scale and Chord sections. Every button acts on the
//! selected notes (through `indices`) or, with nothing selected, on the whole clip.

use crate::SoundApp;
use crate::midi_editor_state::{MidiSnap, all_snaps};
use crate::theme::{Tokens, bold, regular};
use egui::{Rect, Ui, vec2};
use serde_json::{Value, json};
use soundcraft_midi::{ChordKind, Scale, ScaleKind, Sequence};
use soundcraft_model::{ClipId, Session};

/// Width of the strip when open.
pub const INSPECTOR_W: f32 = 170.0;

const ROOTS: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];

/// Values typed into the strip and the note-entry assists.
#[derive(Debug, Clone)]
pub struct InspectorState {
    pub q_grid: MidiSnap,
    pub swing: u8,
    pub transpose: i32,
    pub len_set: MidiSnap,
    pub len_scale: u32,
    pub legato_gap: i64,
    pub vel_set: u8,
    pub vel_add: i32,
    pub vel_min: u8,
    pub vel_max: u8,
    /// Root and kind offered when the clip has no scale yet.
    pub scale_root: u8,
    pub scale_kind: ScaleKind,
    /// Dim the rows that are not in the clip's scale.
    pub highlight_rows: bool,
    /// Snap notes drawn or dragged with the editor to the scale.
    pub snap_new: bool,
    pub chord_kind: ChordKind,
    pub chord_inversion: u8,
    /// The Draw tool places chords instead of single notes.
    pub draw_chords: bool,
}

impl Default for InspectorState {
    fn default() -> Self {
        Self {
            q_grid: MidiSnap::Div(16, crate::midi_editor_state::Flavor::Straight),
            swing: 0,
            transpose: 7,
            len_set: MidiSnap::Div(8, crate::midi_editor_state::Flavor::Straight),
            len_scale: 50,
            legato_gap: 0,
            vel_set: 100,
            vel_add: 10,
            vel_min: 40,
            vel_max: 100,
            scale_root: 0,
            scale_kind: ScaleKind::Major,
            highlight_rows: true,
            snap_new: false,
            chord_kind: ChordKind::Maj,
            chord_inversion: 0,
            draw_chords: false,
        }
    }
}

/// The scale stored for a clip, if any.
pub fn scale_of(s: &Session, clip: ClipId) -> Option<Scale> {
    Scale::from_code(s.edit.value(&soundcraft_engine::cmd::scale_key(clip), 0.0))
}

/// The pitch a newly drawn note gets: snapped to the scale when "Snap new notes" is on and the
/// clip has a scale.
pub fn entry_pitch(app: &SoundApp, clip: ClipId, pitch: i32) -> u8 {
    let p = pitch.clamp(0, 127) as u8;
    match scale_of(app.engine.session(), clip) {
        Some(sc) if app.midi.insp.snap_new => sc.snap(p),
        _ => p,
    }
}

/// How far to shift a pitch drag so the dragged note lands on a scale tone (identity without scale assist).
pub fn snap_drag(app: &SoundApp, clip: ClipId, from: u8, semitones: i64) -> i64 {
    let target = (i64::from(from) + semitones).clamp(0, 127) as u8;
    match scale_of(app.engine.session(), clip) {
        Some(sc) if app.midi.insp.snap_new => i64::from(sc.snap(target)) - i64::from(from),
        _ => semitones,
    }
}

/// Place a note, or a whole chord when "Draw chords" is on, as one undo step.
pub fn place(app: &mut SoundApp, clip: ClipId, pitch: i32, start: i64, length: i64) {
    let pitch = entry_pitch(app, clip, pitch);
    if app.midi.insp.draw_chords {
        let i = &app.midi.insp;
        let p = json!({"clip": clip.0, "root": pitch, "kind": i.chord_kind.id(), "inversion": i.chord_inversion, "start_ticks": start, "length_ticks": length, "velocity": 100});
        let _ = app.run("midi.chord_add", p);
    } else {
        let _ = app.run("midi.note_add", json!({"clip": clip.0, "pitch": pitch, "start_ticks": start, "length_ticks": length, "velocity": 100}));
    }
}

/// Run a clip operation on the selected notes (or the whole clip) and keep the selection on them.
pub fn act(app: &mut SoundApp, cid: ClipId, id: &str, mut p: Value) {
    p["clips"] = json!([cid.0]);
    p["clip"] = json!(cid.0);
    let scoped = !app.midi.selected.is_empty();
    if scoped {
        p["indices"] = json!(app.midi.selected);
    }
    if let Ok(r) = app.run(id, p)
        && scoped
        && let Some(a) = r["indices"].as_array()
    {
        app.midi.selected = a.iter().filter_map(Value::as_u64).map(|i| i as usize).collect();
    }
}

fn snap_choices() -> Vec<MidiSnap> {
    all_snaps().into_iter().filter(|s| !matches!(s, MidiSnap::Grid | MidiSnap::Off)).collect()
}

fn grid_combo(ui: &mut Ui, id: &str, value: &mut MidiSnap) {
    egui::ComboBox::from_id_salt(id).selected_text(value.label()).width(96.0).show_ui(ui, |ui| {
        for s in snap_choices() {
            ui.selectable_value(value, s, s.label());
        }
    });
}

fn button(ui: &mut Ui, text: &str, tip: &str) -> bool {
    ui.small_button(text).on_hover_text(tip).clicked()
}

/// Draw the strip into `rect`.
pub fn show(app: &mut SoundApp, ui: &mut Ui, rect: Rect, cid: ClipId, sequence: &Sequence, playhead_tick: i64, bar_ticks: i64) {
    let t = Tokens::current();
    ui.painter().rect_filled(rect, 0.0, t.panel_bg2);
    ui.painter().line_segment([rect.right_top(), rect.right_bottom()], egui::Stroke::new(1.0, t.text_dim.gamma_multiply(0.4)));
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(vec2(6.0, 4.0))).layout(egui::Layout::top_down(egui::Align::Min)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing = vec2(4.0, 3.0);
    child.spacing_mut().interact_size.y = 18.0;
    let scope = if app.midi.selected.is_empty() { "whole clip".to_string() } else { format!("{} selected", app.midi.selected.len()) };
    child.label(egui::RichText::new("INSPECTOR").font(bold(10.5)).color(t.header_text));
    child.label(egui::RichText::new(format!("Acts on: {scope}")).font(regular(10.0)).color(t.text_dim));
    egui::ScrollArea::vertical().id_salt("midi_inspector_scroll").auto_shrink([false, false]).show(&mut child, |ui| {
        ui.set_width(INSPECTOR_W - 20.0);
        egui::CollapsingHeader::new("Quantize").default_open(true).show(ui, |ui| quantize_section(app, ui, cid, bar_ticks));
        egui::CollapsingHeader::new("Transpose").default_open(true).show(ui, |ui| transpose_section(app, ui, cid));
        egui::CollapsingHeader::new("Length").default_open(true).show(ui, |ui| length_section(app, ui, cid));
        egui::CollapsingHeader::new("Velocity").default_open(true).show(ui, |ui| velocity_section(app, ui, cid));
        egui::CollapsingHeader::new("Scale").default_open(true).show(ui, |ui| scale_section(app, ui, cid));
        egui::CollapsingHeader::new("Chord").default_open(true).show(ui, |ui| chord_section(app, ui, sequence, playhead_tick));
    });
}

fn quantize_section(app: &mut SoundApp, ui: &mut Ui, cid: ClipId, bar_ticks: i64) {
    ui.horizontal(|ui| {
        ui.label("Grid");
        grid_combo(ui, "insp_qgrid", &mut app.midi.insp.q_grid);
    });
    ui.horizontal(|ui| {
        ui.label("Strength");
        ui.add(egui::DragValue::new(&mut app.midi.quant_strength).range(0..=100).suffix("%").speed(0.5));
    });
    ui.horizontal(|ui| {
        ui.label("Swing");
        ui.add(egui::DragValue::new(&mut app.midi.insp.swing).range(0..=100).suffix("%").speed(0.5));
    });
    if button(ui, "Apply", "Quantize to the chosen grid") {
        let grid = app.midi.insp.q_grid.ticks(bar_ticks / 16, bar_ticks);
        let p = json!({"grid": grid, "strength": app.midi.quant_strength, "swing": app.midi.insp.swing});
        act(app, cid, "event.quantize", p);
    }
}

fn transpose_section(app: &mut SoundApp, ui: &mut Ui, cid: ClipId) {
    ui.horizontal(|ui| {
        for (label, st) in [("-12", -12), ("-1", -1), ("+1", 1), ("+12", 12)] {
            if button(ui, label, "Transpose by this many semitones") {
                act(app, cid, "event.transpose", json!({"semitones": st}));
            }
        }
    });
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.midi.insp.transpose).range(-127..=127).suffix(" st"));
        if button(ui, "Apply", "Transpose by the value") {
            let st = app.midi.insp.transpose;
            act(app, cid, "event.transpose", json!({"semitones": st}));
        }
    });
}

fn length_section(app: &mut SoundApp, ui: &mut Ui, cid: ClipId) {
    ui.horizontal(|ui| {
        grid_combo(ui, "insp_len", &mut app.midi.insp.len_set);
        if button(ui, "Set", "Give the notes this length") {
            let ticks = app.midi.insp.len_set.ticks(0, 3840);
            act(app, cid, "event.change_duration", json!({"set": ticks}));
        }
    });
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.midi.insp.len_scale).range(5..=400).suffix("%"));
        if button(ui, "Scale", "Scale the note lengths by this percentage") {
            let k = f64::from(app.midi.insp.len_scale) / 100.0;
            act(app, cid, "event.change_duration", json!({"scale": k}));
        }
    });
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.midi.insp.legato_gap).range(0..=960).suffix(" t"))
            .on_hover_text("Gap left before the next note, in ticks");
        if button(ui, "Legato", "Stretch each note up to the next one") {
            let gap = app.midi.insp.legato_gap;
            act(app, cid, "event.change_duration", json!({"legato": gap}));
        }
    });
    if button(ui, "Fix Overlaps", "Trim notes that run into the next note of the same pitch") {
        act(app, cid, "event.fix_overlaps", json!({}));
    }
}

fn velocity_section(app: &mut SoundApp, ui: &mut Ui, cid: ClipId) {
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.midi.insp.vel_set).range(1..=127));
        if button(ui, "Set", "Give the notes this velocity") {
            let v = app.midi.insp.vel_set;
            act(app, cid, "event.change_velocity", json!({"set": v}));
        }
    });
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.midi.insp.vel_add).range(-127..=127));
        if button(ui, "Add", "Add this to every velocity") {
            let v = app.midi.insp.vel_add;
            act(app, cid, "event.change_velocity", json!({"add": v}));
        }
    });
    ui.horizontal(|ui| {
        ui.add(egui::DragValue::new(&mut app.midi.insp.vel_min).range(1..=127));
        ui.label("-");
        ui.add(egui::DragValue::new(&mut app.midi.insp.vel_max).range(1..=127));
    });
    if button(ui, "Compress", "Pull every velocity into the range above") {
        let (lo, hi) = (app.midi.insp.vel_min, app.midi.insp.vel_max);
        act(app, cid, "event.change_velocity", json!({"min": lo, "max": hi}));
    }
}

fn scale_section(app: &mut SoundApp, ui: &mut Ui, cid: ClipId) {
    let current = scale_of(app.engine.session(), cid);
    let mut root = current.map_or(app.midi.insp.scale_root, |s| s.root);
    let mut kind = current.map_or(app.midi.insp.scale_kind, |s| s.kind);
    let mut changed = false;
    let mut clear = false;
    ui.horizontal(|ui| {
        ui.label("Root");
        egui::ComboBox::from_id_salt("insp_scale_root").selected_text(ROOTS.get(usize::from(root)).copied().unwrap_or("C")).width(44.0).show_ui(
            ui,
            |ui| {
                for (i, name) in ROOTS.iter().enumerate() {
                    changed |= ui.selectable_value(&mut root, i as u8, *name).changed();
                }
            },
        );
    });
    ui.horizontal(|ui| {
        ui.label("Kind");
        let text = if current.is_some() { kind.label() } else { "None" };
        egui::ComboBox::from_id_salt("insp_scale_kind").selected_text(text).width(96.0).show_ui(ui, |ui| {
            if ui.selectable_label(current.is_none(), "None").clicked() {
                clear = true;
            }
            for k in ScaleKind::ALL {
                changed |= ui.selectable_value(&mut kind, k, k.label()).changed();
            }
        });
    });
    app.midi.insp.scale_root = root;
    app.midi.insp.scale_kind = kind;
    if clear {
        let _ = app.run("midi.scale_set", json!({"clip": cid.0, "root": root, "kind": null}));
    } else if changed {
        let _ = app.run("midi.scale_set", json!({"clip": cid.0, "root": root, "kind": kind.id()}));
    }
    ui.checkbox(&mut app.midi.insp.highlight_rows, "Highlight rows").on_hover_text("Dim the rows that are not in the scale");
    ui.checkbox(&mut app.midi.insp.snap_new, "Snap new notes").on_hover_text("Drawn and dragged notes land on scale tones");
    if current.is_some() && button(ui, "Snap notes to scale", "Move out-of-scale notes to the nearest scale tone") {
        act(app, cid, "midi.scale_snap", json!({}));
    }
}

fn chord_section(app: &mut SoundApp, ui: &mut Ui, sequence: &Sequence, playhead_tick: i64) {
    ui.horizontal(|ui| {
        ui.label("Type");
        let kind = &mut app.midi.insp.chord_kind;
        egui::ComboBox::from_id_salt("insp_chord_kind").selected_text(kind.label()).width(96.0).show_ui(ui, |ui| {
            for k in ChordKind::ALL {
                ui.selectable_value(kind, k, k.label());
            }
        });
    });
    ui.horizontal(|ui| {
        ui.label("Inversion");
        ui.add(egui::DragValue::new(&mut app.midi.insp.chord_inversion).range(0..=3));
    });
    ui.checkbox(&mut app.midi.insp.draw_chords, "Draw chords").on_hover_text("The Draw tool places a chord on every click");
    let name = soundcraft_engine::cmd::chord_name_at(sequence, playhead_tick);
    ui.label(egui::RichText::new(format!("At playhead: {}", name.as_deref().unwrap_or("-"))).font(regular(10.5)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        let i = InspectorState::default();
        assert!(!i.snap_new && !i.draw_chords, "assists start off");
        assert_eq!(i.scale_kind, ScaleKind::Major);
        assert!(snap_choices().len() >= 15);
    }
}
