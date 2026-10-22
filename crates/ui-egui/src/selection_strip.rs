//! The Standard toolbar's selection strip: what is selected right now. Notes in the MIDI editor,
//! one clip, several clips, a timeline selection or tracks. The timeline selection's start and
//! length and a single clip's name can be edited here.

use crate::SoundApp;
use crate::midi_info_bar::{Field, field_text, text_field};
use crate::theme::{Tokens, regular};
use egui::Ui;
use serde_json::json;
use soundcraft_midi::Note;
use soundcraft_model::{ClipId, Session};
use soundcraft_time::{Samples, TimeFormat};

/// Height of the strip.
pub const HEIGHT: f32 = 20.0;
const MAX_NAMES: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub enum Summary {
    Nothing,
    /// Notes selected in the MIDI editor: how many, and the lead note's fields.
    Notes {
        count: usize,
        start: String,
        length: String,
        pitch: String,
        velocity: String,
    },
    Clip {
        id: ClipId,
        name: String,
        start: String,
        end: String,
        length: String,
        offset: Option<String>,
        gain_db: f32,
    },
    Clips {
        count: usize,
        start: String,
        end: String,
    },
    Range {
        start: Samples,
        end: Samples,
        start_text: String,
        end_text: String,
        length_text: String,
    },
    Tracks {
        count: usize,
        names: String,
    },
}

/// `len` samples as a duration in the main counter's format.
pub fn length_text(s: &Session, start: Samples, len: Samples) -> String {
    soundcraft_time::format_length(start, len, s.edit.main_counter, s.sample_rate, &s.tempo, s.frame_rate)
}

/// Parse a typed length: the counter text for a position that far from sample 0.
pub fn parse_length(s: &Session, text: &str) -> Option<Samples> {
    let fmt = s.edit.main_counter;
    let at = |t: &str| s.parse_position(t, fmt).ok();
    let len = at(text)? - at(&s.format_position(0, fmt))?;
    (len >= 0).then_some(len)
}

/// What the strip shows. `notes` are the selected notes of the focused MIDI editor (lead first)
/// with the clip's start tick.
pub fn summary(s: &Session, notes: &[Note], base_tick: i64) -> Summary {
    let fmt: TimeFormat = s.edit.main_counter;
    let pos = |at: Samples| s.format_position(at, fmt);
    if let Some(lead) = notes.first() {
        let f = |field| field_text(&s.tempo, base_tick, lead, field);
        return Summary::Notes {
            count: notes.len(),
            start: f(Field::Start),
            length: f(Field::Length),
            pitch: f(Field::Pitch),
            velocity: f(Field::Velocity),
        };
    }
    let clips: Vec<_> = s.edit.selected_clips.iter().filter_map(|id| s.find_clip(*id).map(|(_, c)| c)).collect();
    if let [c] = clips.as_slice() {
        let offset = match &c.content {
            soundcraft_model::ClipContent::Audio { offset, .. } | soundcraft_model::ClipContent::Video { offset, .. } => {
                Some(length_text(s, 0, *offset))
            }
            soundcraft_model::ClipContent::Midi { .. } => None,
        };
        return Summary::Clip {
            id: c.id,
            name: c.name.clone(),
            start: pos(c.start),
            end: pos(c.end()),
            length: length_text(s, c.start, c.length),
            offset,
            gain_db: c.gain_db,
        };
    }
    if clips.len() > 1 {
        let start = clips.iter().map(|c| c.start).min().unwrap_or(0);
        let end = clips.iter().map(|c| c.end()).max().unwrap_or(0);
        return Summary::Clips { count: clips.len(), start: pos(start), end: pos(end) };
    }
    let r = s.edit.selection;
    if !r.is_empty() {
        return Summary::Range {
            start: r.start,
            end: r.end,
            start_text: pos(r.start),
            end_text: pos(r.end),
            length_text: length_text(s, r.start, r.len()),
        };
    }
    let names: Vec<&str> = s.edit.selected_tracks.iter().filter_map(|id| s.track(*id).map(|t| t.name.as_str())).collect();
    if names.is_empty() {
        return Summary::Nothing;
    }
    let mut shown = names.iter().take(MAX_NAMES).copied().collect::<Vec<_>>().join(", ");
    if names.len() > MAX_NAMES {
        shown.push_str(", ...");
    }
    Summary::Tracks { count: names.len(), names: shown }
}

fn pair(ui: &mut Ui, label: &str, value: &str) {
    let t = Tokens::current();
    ui.label(egui::RichText::new(label).font(regular(11.0)).color(t.text_dim));
    ui.label(egui::RichText::new(value).font(regular(11.0)).color(t.text));
    ui.add_space(10.0);
}

fn edit_pair(ui: &mut Ui, label: &str, key: &str, current: &str, width: f32) -> Option<String> {
    let t = Tokens::current();
    let lab = ui.label(egui::RichText::new(label).font(regular(11.0)).color(t.text_dim));
    let out = text_field(ui, ui.id().with(key), lab.id, current, width);
    ui.add_space(10.0);
    out
}

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let (notes, base) = selected_notes(app);
    let sum = summary(app.engine.session(), &notes, base);
    let mut run: Option<(&str, serde_json::Value)> = None;
    ui.horizontal_centered(|ui| {
        ui.add_space(10.0);
        match &sum {
            Summary::Nothing => {
                ui.label(egui::RichText::new("Nothing selected").font(regular(11.0)).color(t.text_dim));
            }
            Summary::Notes { count, start, length, pitch, velocity } => {
                pair(ui, "Notes", &count.to_string());
                pair(ui, "Start", start);
                pair(ui, "Length", length);
                pair(ui, "Pitch", pitch);
                pair(ui, "Velocity", velocity);
            }
            Summary::Clip { id, name, start, end, length, offset, gain_db } => {
                if let Some(n) = edit_pair(ui, "Clip", "clip_name", name, 120.0) {
                    run = Some(("clip.rename", json!({"clip": id.0, "name": n})));
                }
                pair(ui, "Start", start);
                pair(ui, "End", end);
                pair(ui, "Length", length);
                if let Some(o) = offset {
                    pair(ui, "Offset", o);
                }
                pair(ui, "Gain", &format!("{gain_db:.1} dB"));
            }
            Summary::Clips { count, start, end } => {
                pair(ui, "Clips", &count.to_string());
                pair(ui, "From", start);
                pair(ui, "To", end);
            }
            Summary::Range { start, end, start_text, end_text, length_text } => {
                let s = app.engine.session();
                if let Some(x) = edit_pair(ui, "Start", "sel_start", start_text, 90.0).and_then(|x| s.parse_position(&x, s.edit.main_counter).ok()) {
                    run = Some(("edit.select", json!({"start": x, "end": x.max(*end - start + x)})));
                }
                pair(ui, "End", end_text);
                if let Some(l) = edit_pair(ui, "Length", "sel_len", length_text, 90.0).and_then(|x| parse_length(s, &x)) {
                    run = Some(("edit.select", json!({"start": start, "end": start.saturating_add(l)})));
                }
            }
            Summary::Tracks { count, names } => {
                pair(ui, if *count == 1 { "Track" } else { "Tracks" }, &format!("{count}: {names}"));
            }
        }
    });
    if let Some((id, p)) = run {
        let _ = app.run(id, p);
    }
}

/// The selected notes of the open MIDI clip, lead first, and the clip's start tick.
fn selected_notes(app: &SoundApp) -> (Vec<Note>, i64) {
    let s = app.engine.session();
    let Some(cid) = app.midi.clip else { return (Vec::new(), 0) };
    let Some((_, clip)) = s.find_clip(cid) else { return (Vec::new(), 0) };
    let soundcraft_model::ClipContent::Midi { sequence } = &clip.content else { return (Vec::new(), 0) };
    let notes = app.midi.selected.iter().filter_map(|i| sequence.notes.get(*i).cloned()).collect();
    (notes, s.tempo.samples_to_ticks(clip.start, s.sample_rate))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_engine::Engine;

    fn engine_with_clips() -> (Engine, Vec<ClipId>) {
        let e = soundcraft_engine::demo::demo_engine();
        let ids: Vec<ClipId> = e.session().all_clips().map(|c| c.id).collect();
        (e, ids)
    }

    #[test]
    fn nothing_selected() {
        let mut e = Engine::default();
        e.session_mut().edit.selection = soundcraft_time::Range::default();
        assert_eq!(summary(e.session(), &[], 0), Summary::Nothing);
    }

    #[test]
    fn tracks_only() {
        let (mut e, _) = engine_with_clips();
        let tracks: Vec<_> = e.session().tracks.iter().take(2).map(|t| t.id).collect();
        let s = e.session_mut();
        s.edit.selected_clips.clear();
        s.edit.selection = soundcraft_time::Range::default();
        s.edit.selected_tracks = tracks;
        assert!(matches!(summary(e.session(), &[], 0), Summary::Tracks { count: 2, .. }));
    }

    #[test]
    fn timeline_selection_without_clips() {
        let (mut e, _) = engine_with_clips();
        let s = e.session_mut();
        s.edit.selected_clips.clear();
        s.edit.selection = soundcraft_time::Range::new(48_000, 96_000);
        match summary(e.session(), &[], 0) {
            Summary::Range { start, end, .. } => assert_eq!((start, end), (48_000, 96_000)),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn one_clip_then_several() {
        let (mut e, ids) = engine_with_clips();
        assert!(ids.len() >= 2);
        e.session_mut().edit.selected_clips = vec![ids[0]];
        assert!(matches!(summary(e.session(), &[], 0), Summary::Clip { id, .. } if id == ids[0]));
        e.session_mut().edit.selected_clips = vec![ids[0], ids[1]];
        assert!(matches!(summary(e.session(), &[], 0), Summary::Clips { count: 2, .. }));
    }

    #[test]
    fn notes_win_over_everything() {
        let (mut e, ids) = engine_with_clips();
        e.session_mut().edit.selected_clips = vec![ids[0]];
        let n = Note { pitch: 60, velocity: 100, release_velocity: 64, channel: 0, start: 0, length: 480 };
        match summary(e.session(), &[n, n], 0) {
            Summary::Notes { count, pitch, velocity, .. } => assert_eq!((count, velocity.as_str(), pitch.as_str()), (2, "100", "C3")),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn length_round_trips_in_samples_and_minsecs() {
        let mut e = Engine::default();
        for fmt in [TimeFormat::Samples, TimeFormat::MinSecs] {
            e.session_mut().edit.main_counter = fmt;
            let s = e.session();
            let text = length_text(s, 0, 96_000);
            assert_eq!(parse_length(s, &text), Some(96_000), "{fmt:?} {text}");
        }
    }
}
