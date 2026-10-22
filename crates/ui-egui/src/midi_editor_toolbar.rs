//! Header groups of the piano roll added on top of the original toolbar: the snap dropdown, the
//! clip selector and the quantize strength. Each is a plain function that draws into whatever
//! `Ui` it is given, so the same code serves the header and the overflow menu.

use crate::SoundApp;
use crate::midi_editor_state::{MidiSnap, all_snaps};
use egui::Ui;
use serde_json::json;
use soundcraft_model::{ClipContent, ClipId, Session, TrackId};

/// A short name for a tick count that is a plain note division ("1/16"), else the tick count.
pub fn ticks_name(ticks: i64) -> String {
    let whole = soundcraft_time::TICKS_PER_QUARTER * 4;
    if ticks > 0 && whole % ticks == 0 { format!("1/{}", whole / ticks) } else { format!("{ticks} ticks") }
}

/// Snap dropdown with every choice, plus the note-colour switch.
pub fn snap_group(app: &mut SoundApp, ui: &mut Ui, session_grid: i64) {
    let text =
        if app.midi.snap == MidiSnap::Grid { format!("Snap: Grid {}", ticks_name(session_grid)) } else { format!("Snap: {}", app.midi.snap.label()) };
    ui.menu_button(text, |ui| {
        for s in all_snaps() {
            let tip = if s == MidiSnap::Grid { "Follow the session edit grid" } else { "Snap notes to this value" };
            if toggle_label(ui, app.midi.snap == s, &s.label()).on_hover_text(tip).clicked() {
                app.midi.snap = s;
                ui.close();
            }
        }
        ui.separator();
        let single = app.engine.session().edit.flag("midi.single_colour");
        let mut by_velocity = !single;
        if ui.checkbox(&mut by_velocity, "Colour notes by velocity").clicked() {
            let _ = app.run("view.midi_single_colour", json!({}));
        }
    })
    .response
    .on_hover_text("Snap value for placing and moving notes");
}

/// MIDI clips on `track`, as (id, name).
pub fn clips_on_track(s: &Session, track: TrackId) -> Vec<(ClipId, String)> {
    s.track(track)
        .map(|t| t.clips().iter().filter(|c| matches!(c.content, ClipContent::Midi { .. })).map(|c| (c.id, c.name.clone())).collect())
        .unwrap_or_default()
}

/// Clip selector: the MIDI clips on the edited clip's track; choosing one selects it.
pub fn clip_group(app: &mut SoundApp, ui: &mut Ui, current: ClipId, name: &str) {
    let track = app.engine.session().find_clip(current).map(|(t, _)| t);
    let clips = track.map(|t| clips_on_track(app.engine.session(), t)).unwrap_or_default();
    ui.menu_button(format!("Clip: {name}"), |ui| {
        for (id, n) in &clips {
            if ui.selectable_label(*id == current, n).clicked() {
                let _ = app.run("edit.select", json!({"clips": [id.0]}));
                ui.close();
            }
        }
    })
    .response
    .on_hover_text("Switch to another MIDI clip on this track");
}

/// Quantize strength 0..=100 next to the Quantize button.
pub fn strength_field(app: &mut SoundApp, ui: &mut Ui) {
    ui.add(egui::DragValue::new(&mut app.midi.quant_strength).range(0..=100).suffix("%").speed(0.5))
        .on_hover_text("Quantize strength: how far notes move toward the grid");
}

/// A selectable label whose selected state stays readable in both themes (the stock selected
/// text colour is dark blue, which disappears on the blue selection fill of the Light theme).
pub fn toggle_label(ui: &mut egui::Ui, on: bool, text: &str) -> egui::Response {
    ui.scope(|ui| {
        if on && crate::theme::is_light() {
            ui.visuals_mut().selection.stroke.color = egui::Color32::WHITE;
        }
        ui.selectable_label(on, text)
    })
    .inner
}
