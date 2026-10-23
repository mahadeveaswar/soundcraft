//! Piano-roll Cut / Copy / Paste / Duplicate: what the keys and the menus send to the note commands
//! (`midi.notes_cut`, `midi.notes_copy`, `midi.notes_paste`, `midi.notes_duplicate`).

use crate::SoundApp;
use serde_json::{Value, json};
use soundcraft_midi::{CtrlKind, Sequence};
use soundcraft_model::{ClipContent, ClipId};

fn sequence(app: &SoundApp, clip: ClipId) -> Option<&Sequence> {
    match app.engine.session().find_clip(clip).map(|(_, c)| &c.content) {
        Some(ClipContent::Midi { sequence }) => Some(sequence),
        _ => None,
    }
}

/// Start and end tick of the selected notes.
fn span(seq: &Sequence, selected: &[usize]) -> Option<(i64, i64)> {
    let ns: Vec<_> = selected.iter().filter_map(|&i| seq.notes.get(i)).collect();
    let start = ns.iter().map(|n| n.start).min()?;
    let end = ns.iter().map(|n| n.end()).max()?;
    Some((start, end))
}

/// Controller events (all kinds in `kinds`) at ticks in `[from, to)`.
fn ctrls_in(seq: &Sequence, kinds: &[CtrlKind], from: i64, to: i64) -> Vec<usize> {
    seq.ctrls.iter().enumerate().filter(|(_, c)| c.tick >= from && c.tick < to && kinds.contains(&c.kind)).map(|(i, _)| i).collect()
}

/// Whether the note clipboard holds anything.
pub fn has_notes(app: &SoundApp) -> bool {
    app.engine.clipboard.notes.is_some()
}

/// Cut or copy the selected notes together with the events of the shown controller lanes that lie
/// under them. Returns false when no note is selected.
pub fn copy_selection(app: &mut SoundApp, clip: ClipId, cut: bool) -> bool {
    let selected = app.midi.selected.clone();
    let kinds: Vec<CtrlKind> = app.midi.lanes.iter().map(|l| l.lane.kind()).collect();
    let Some(seq) = sequence(app, clip) else { return false };
    let Some((from, to)) = span(seq, &selected) else { return false };
    let ctrls = ctrls_in(seq, &kinds, from, to);
    let id = if cut { "midi.notes_cut" } else { "midi.notes_copy" };
    app.run(id, json!({"clip": clip.0, "indices": selected, "ctrl_indices": ctrls})).is_ok()
}

/// Cut or copy one controller lane's events: those under the selected notes, or the whole lane when
/// no note is selected.
pub fn copy_lane(app: &mut SoundApp, clip: ClipId, kind: CtrlKind, cut: bool) -> bool {
    let selected = app.midi.selected.clone();
    let Some(seq) = sequence(app, clip) else { return false };
    let (from, to) = span(seq, &selected).unwrap_or((i64::MIN, i64::MAX));
    let ctrls = ctrls_in(seq, &[kind], from, to);
    if ctrls.is_empty() {
        app.ui.status = "no controller events to copy".into();
        return false;
    }
    let id = if cut { "midi.notes_cut" } else { "midi.notes_copy" };
    app.run(id, json!({"clip": clip.0, "indices": [], "ctrl_indices": ctrls})).is_ok()
}

/// Paste the note clipboard: at `tick` (relative to the clip) when given, else at the position last
/// placed in the roll, else at the playhead or insertion point.
pub fn paste(app: &mut SoundApp, clip: ClipId, tick: Option<i64>) -> bool {
    let mut p = json!({"clip": clip.0});
    if let Some(t) = tick {
        p["tick"] = json!(t);
    } else if let Some(at) = crate::midi_editor::placed_position(app.midi.play_from, app.engine.session().edit.selection) {
        p["at"] = json!(at);
    }
    app.run("midi.notes_paste", p).is_ok()
}

/// Copy the selected notes right after themselves.
pub fn duplicate(app: &mut SoundApp, clip: ClipId) -> bool {
    let selected = app.midi.selected.clone();
    !selected.is_empty() && app.run("midi.notes_duplicate", json!({"clip": clip.0, "indices": selected})).is_ok()
}

/// Keep the roll's selection in step with a note command that ran from anywhere (keys, menus,
/// the control channel): pasted notes are selected, cut notes are gone.
pub fn after_command(app: &mut SoundApp, id: &str, params: &Value, result: &Value) {
    let on_shown = params.get("clip").and_then(Value::as_u64).map(ClipId).is_some_and(|c| app.midi.clip == Some(c));
    if !on_shown {
        return;
    }
    match id {
        "midi.notes_paste" | "midi.notes_duplicate" => {
            app.midi.selected = result
                .get("indices")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).map(|x| x as usize).collect())
                .unwrap_or_default();
        }
        "midi.notes_cut" => app.midi.selected.clear(),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_span_covers_the_selected_notes_only() {
        let mut s = Sequence::default();
        for (p, start) in [(60, 0), (62, 480), (64, 960)] {
            s.notes.push(soundcraft_midi::Note { pitch: p, velocity: 100, release_velocity: 64, channel: 0, start, length: 240 });
        }
        assert_eq!(span(&s, &[1, 2]), Some((480, 1200)));
        assert_eq!(span(&s, &[]), None);
    }
}
