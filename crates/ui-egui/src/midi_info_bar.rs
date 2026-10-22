//! The info strip above the piano roll: "No note selected", or editable fields for the selected
//! note (Start, End, Length, Pitch, Velocity, Channel). With several notes selected the fields
//! show `*` and an edit is applied to every selected note as a change relative to the first one.
//! Positions are `bar.beat.tick` (bar and beat count from 1, ticks from 0); lengths are
//! `bars.beats.ticks` counting from 0.

use crate::SoundApp;
use crate::theme::{Tokens, regular};
use egui::{Align, Layout, Rect, Ui};
use serde_json::{Value, json};
use soundcraft_midi::Note;
use soundcraft_model::ClipId;
use soundcraft_time::{BarBeat, TempoMap};

/// Height of the strip.
pub const INFO_H: f32 = 22.0;
const MAX_BAR: i64 = 100_000;

/// The editable fields, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Start,
    End,
    Length,
    Pitch,
    Velocity,
    Channel,
}

impl Field {
    const ALL: [Field; 6] = [Field::Start, Field::End, Field::Length, Field::Pitch, Field::Velocity, Field::Channel];

    fn label(self) -> &'static str {
        match self {
            Field::Start => "Start",
            Field::End => "End",
            Field::Length => "Length",
            Field::Pitch => "Pitch",
            Field::Velocity => "Velocity",
            Field::Channel => "Channel",
        }
    }

    fn width(self) -> f32 {
        match self {
            Field::Start | Field::End | Field::Length => 74.0,
            _ => 44.0,
        }
    }
}

/// `bar.beat.tick` for an absolute tick.
pub fn format_pos(tempo: &TempoMap, tick: i64) -> String {
    let bb = tempo.bar_beat_at_tick(tick);
    format!("{}.{}.{}", bb.bar, bb.beat, bb.tick)
}

fn split_numbers(text: &str) -> Option<Vec<i64>> {
    let parts: Vec<&str> = text.split(['.', ' ', ':']).filter(|p| !p.is_empty()).collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    parts.iter().map(|p| p.parse::<i64>().ok()).collect()
}

/// Parse `bar[.beat[.tick]]` into an absolute tick. Rejects bars below 1, beats outside the
/// bar's meter and ticks outside the beat.
pub fn parse_pos(tempo: &TempoMap, text: &str) -> Option<i64> {
    let n = split_numbers(text.trim())?;
    let (bar, beat, tick) = (*n.first()?, n.get(1).copied().unwrap_or(1), n.get(2).copied().unwrap_or(0));
    if !(1..=MAX_BAR).contains(&bar) {
        return None;
    }
    let start = tempo.bar_start_tick(bar);
    let meter = tempo.meter_at_tick(start);
    if !(1..=i64::from(meter.numerator)).contains(&beat) || !(0..meter.ticks_per_beat()).contains(&tick) {
        return None;
    }
    Some(tempo.tick_at_bar_beat(BarBeat { bar, beat, tick }))
}

/// `bars.beats.ticks` for a length that starts at absolute tick `at`.
pub fn format_len(tempo: &TempoMap, at: i64, len: i64) -> String {
    let m = tempo.meter_at_tick(at);
    let (per_bar, per_beat) = (m.ticks_per_bar().max(1), m.ticks_per_beat().max(1));
    let len = len.max(0);
    format!("{}.{}.{}", len / per_bar, (len % per_bar) / per_beat, len % per_beat)
}

/// Parse `bars[.beats[.ticks]]` into a length in ticks (must be above zero).
pub fn parse_len(tempo: &TempoMap, at: i64, text: &str) -> Option<i64> {
    let n = split_numbers(text.trim())?;
    if n.iter().any(|v| *v < 0) {
        return None;
    }
    let m = tempo.meter_at_tick(at);
    let (bars, beats, ticks) = (*n.first()?, n.get(1).copied().unwrap_or(0), n.get(2).copied().unwrap_or(0));
    if bars > MAX_BAR || beats >= i64::from(m.numerator) || ticks >= m.ticks_per_beat() {
        return None;
    }
    let total = bars.checked_mul(m.ticks_per_bar())?.checked_add(beats.checked_mul(m.ticks_per_beat())?)?.checked_add(ticks)?;
    (total > 0).then_some(total)
}

/// A pitch typed as a note name ("C3", "f#2", "Bb-1") or a number 0..=127.
pub fn parse_pitch(text: &str) -> Option<u8> {
    let t = text.trim();
    if let Ok(n) = t.parse::<i64>() {
        return u8::try_from(n).ok().filter(|p| *p <= 127);
    }
    let mut chars = t.chars();
    let class = match chars.next()?.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    };
    let rest = chars.as_str();
    let (shift, rest) = match rest.chars().next() {
        Some('#') => (1, rest.get(1..)?),
        Some('b') => (-1, rest.get(1..)?),
        _ => (0, rest),
    };
    let octave: i64 = rest.parse().ok()?;
    let pitch = octave.checked_add(2)?.checked_mul(12)?.checked_add(class + shift)?;
    u8::try_from(pitch).ok().filter(|p| *p <= 127)
}

/// A velocity or other plain number: `+5` / `-5` is a change, anything else a new value.
fn parse_number(text: &str, current: i64) -> Option<i64> {
    let t = text.trim();
    let n: i64 = t.strip_prefix('+').unwrap_or(t).parse().ok()?;
    if t.starts_with('+') || (t.starts_with('-') && t.len() > 1) { current.checked_add(n) } else { Some(n) }
}

/// What a note shows in a field.
pub fn field_text(tempo: &TempoMap, base_tick: i64, n: &Note, f: Field) -> String {
    let abs = base_tick.saturating_add(n.start);
    match f {
        Field::Start => format_pos(tempo, abs),
        Field::End => format_pos(tempo, abs.saturating_add(n.length)),
        Field::Length => format_len(tempo, abs, n.length),
        Field::Pitch => soundcraft_midi::note_name(n.pitch),
        Field::Velocity => n.velocity.to_string(),
        Field::Channel => (u16::from(n.channel) + 1).to_string(),
    }
}

/// Turn typed text into `midi.notes_adjust` params (without `clip` and `indices`), relative to
/// the `lead` note. Invalid text is an error message, never a panic.
pub fn adjust_params(tempo: &TempoMap, base_tick: i64, lead: &Note, f: Field, text: &str) -> Result<Value, String> {
    let abs = base_tick.saturating_add(lead.start);
    let bad = |what: &str| format!("not a valid {what}: `{}`", text.trim());
    match f {
        Field::Start => {
            let new = parse_pos(tempo, text).ok_or_else(|| bad("start position"))?;
            let rel = new - base_tick;
            if rel < 0 {
                return Err("the note would start before the clip".into());
            }
            Ok(json!({"start_delta": rel - lead.start}))
        }
        Field::End => {
            let new = parse_pos(tempo, text).ok_or_else(|| bad("end position"))?;
            let len = new - abs;
            if len <= 0 {
                return Err("the end must be after the start".into());
            }
            Ok(json!({"length_delta": len - lead.length}))
        }
        Field::Length => {
            let len = parse_len(tempo, abs, text).ok_or_else(|| bad("length"))?;
            Ok(json!({"length_delta": len - lead.length}))
        }
        Field::Pitch => {
            let p = match parse_pitch(text) {
                Some(p) => i64::from(p),
                None => parse_number(text, i64::from(lead.pitch)).filter(|p| (0..=127).contains(p)).ok_or_else(|| bad("pitch"))?,
            };
            Ok(json!({"pitch_delta": p - i64::from(lead.pitch)}))
        }
        Field::Velocity => {
            let v = parse_number(text, i64::from(lead.velocity)).filter(|v| (1..=127).contains(v)).ok_or_else(|| bad("velocity (1-127)"))?;
            Ok(json!({"velocity_delta": v - i64::from(lead.velocity)}))
        }
        Field::Channel => {
            let c = text.trim().parse::<i64>().ok().filter(|c| (1..=16).contains(c)).ok_or_else(|| bad("channel (1-16)"))?;
            Ok(json!({"channel": c - 1}))
        }
    }
}

/// A text field that edits a copy of `current` while it has focus. Returns the text when an edit
/// is committed (Enter or focus lost); Escape abandons it.
fn text_field(ui: &mut Ui, id: egui::Id, label: egui::Id, current: &str, width: f32) -> Option<String> {
    let editing = ui.data(|d| d.get_temp::<String>(id));
    let mut text = editing.clone().unwrap_or_else(|| current.to_string());
    let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(width).font(egui::TextStyle::Small).id(id.with("edit")));
    let resp = resp.labelled_by(label);
    if resp.changed() || resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    let mut commit = None;
    if resp.lost_focus() {
        let escaped = ui.input(|i| i.key_pressed(egui::Key::Escape));
        if editing.is_some() && !escaped && text != current {
            commit = Some(text);
        }
        ui.data_mut(|d| d.remove::<String>(id));
    }
    commit
}

/// Draw the strip into `rect`. An edit runs `midi.notes_adjust` once (one undo step).
pub fn show(app: &mut SoundApp, ui: &mut Ui, rect: Rect, cid: ClipId, notes: &[Note], base_tick: i64, tempo: &TempoMap) {
    let t = Tokens::current();
    ui.painter().rect_filled(rect, 0.0, t.panel_bg2);
    let selected: Vec<usize> = app.midi.selected.iter().copied().filter(|i| *i < notes.len()).collect();
    let Some(lead) = selected.iter().min().and_then(|i| notes.get(*i)).copied() else {
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "No note selected", regular(11.0), t.text_dim);
        return;
    };
    let multi = selected.len() > 1;
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect.shrink2(egui::vec2(8.0, 1.0))).layout(Layout::left_to_right(Align::Center)));
    child.set_clip_rect(rect.intersect(ui.clip_rect()));
    child.spacing_mut().item_spacing.x = 4.0;
    let mut error = None;
    let mut done = None;
    for f in Field::ALL {
        let caption = child.label(egui::RichText::new(f.label()).size(10.5).color(t.text_dim));
        let shown = if multi { "*".to_string() } else { field_text(tempo, base_tick, &lead, f) };
        let id = child.id().with(("midi_info", f.label()));
        if let Some(text) = text_field(&mut child, id, caption.id, &shown, f.width()) {
            match adjust_params(tempo, base_tick, &lead, f, &text) {
                Ok(mut p) => {
                    p["clip"] = json!(cid.0);
                    p["indices"] = json!(selected);
                    done = Some(p);
                }
                Err(e) => error = Some(e),
            }
        }
        child.add_space(6.0);
    }
    if let Some(p) = done {
        if let Ok(r) = app.run("midi.notes_adjust", p) {
            let moved: Vec<usize> =
                r["indices"].as_array().map(|a| a.iter().filter_map(Value::as_u64).map(|i| i as usize).collect()).unwrap_or_default();
            if !moved.is_empty() {
                app.midi.selected = moved;
            }
        }
        app.midi.info_error = None;
    } else if error.is_some() {
        app.midi.info_error = error;
    }
    if let Some(e) = &app.midi.info_error {
        child.label(egui::RichText::new(e).size(10.0).color(Tokens::current().error_text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(pitch: u8, start: i64, length: i64) -> Note {
        Note { pitch, velocity: 90, release_velocity: 64, channel: 0, start, length }
    }

    #[test]
    fn position_format_and_parse_round_trip() {
        let t = TempoMap::default();
        for tick in [0, 1, 479, 960, 3840, 3841, 3840 * 7 + 960 * 2 + 33] {
            let text = format_pos(&t, tick);
            assert_eq!(parse_pos(&t, &text), Some(tick), "{text}");
        }
        assert_eq!(format_pos(&t, 0), "1.1.0");
        assert_eq!(format_pos(&t, 3840 + 960 + 5), "2.2.5");
        assert_eq!(parse_pos(&t, "2"), Some(3840));
        assert_eq!(parse_pos(&t, "2 2 5"), Some(3840 + 965));
    }

    #[test]
    fn position_parse_rejects_bad_input() {
        let t = TempoMap::default();
        for bad in ["", " ", "0.1.0", "1.5.0", "1.0.0", "1.1.960", "1.1.-1", "a.b.c", "1.1.1.1", "999999999", "-3"] {
            assert_eq!(parse_pos(&t, bad), None, "{bad}");
        }
    }

    #[test]
    fn length_format_and_parse_round_trip_in_other_meters() {
        let mut t = TempoMap::default();
        assert_eq!(format_len(&t, 0, 3840 + 960 + 7), "1.1.7");
        assert_eq!(parse_len(&t, 0, "1.1.7"), Some(3840 + 960 + 7));
        assert_eq!(parse_len(&t, 0, "0.0.0"), None, "zero length is rejected");
        assert_eq!(parse_len(&t, 0, "0.4.0"), None, "beat count must stay within the bar");
        assert_eq!(parse_len(&t, 0, "-1"), None);
        t.set_meter(0, 3, 4).unwrap();
        assert_eq!(parse_len(&t, 0, "0.3.0"), None);
        assert_eq!(parse_len(&t, 0, "1.2.0"), Some(2880 + 1920));
        assert_eq!(format_len(&t, 0, 2880 + 1920), "1.2.0");
        for tick in [0, 5, 960, 2880, 2881, 9000] {
            assert_eq!(parse_pos(&t, &format_pos(&t, tick)), Some(tick));
        }
    }

    #[test]
    fn pitch_text_parses_names_and_numbers() {
        assert_eq!(parse_pitch("C3"), Some(60));
        assert_eq!(parse_pitch("c#3"), Some(61));
        assert_eq!(parse_pitch("Db3"), Some(61));
        assert_eq!(parse_pitch("C-2"), Some(0));
        assert_eq!(parse_pitch("G8"), Some(127));
        assert_eq!(parse_pitch("G#8"), None);
        assert_eq!(parse_pitch("72"), Some(72));
        assert_eq!(parse_pitch("128"), None);
        assert_eq!(parse_pitch("H3"), None);
        assert_eq!(parse_pitch(""), None);
        assert_eq!(parse_pitch("C9223372036854775807"), None, "huge octave must not overflow");
        assert_eq!(parse_pitch("C-9223372036854775808"), None);
        assert_eq!(parse_pitch("C99999999999999999999"), None);
        assert_eq!(parse_pos(&TempoMap::default(), "99999999999999999999.1.0"), None);
        assert_eq!(parse_pos(&TempoMap::default(), "9223372036854775807.1.0"), None);
        assert_eq!(parse_len(&TempoMap::default(), 0, "9223372036854775807.1.0"), None);
        for p in 0..=127u8 {
            assert_eq!(parse_pitch(&soundcraft_midi::note_name(p)), Some(p));
        }
    }

    #[test]
    fn typed_text_becomes_relative_adjustments_or_an_error() {
        let t = TempoMap::default();
        let n = note(60, 960, 480);
        assert_eq!(adjust_params(&t, 0, &n, Field::Velocity, "100").unwrap(), json!({"velocity_delta": 10}));
        assert_eq!(adjust_params(&t, 0, &n, Field::Velocity, "-20").unwrap(), json!({"velocity_delta": -20}));
        assert!(adjust_params(&t, 0, &n, Field::Velocity, "0").is_err());
        assert!(adjust_params(&t, 0, &n, Field::Velocity, "abc").is_err());
        assert_eq!(adjust_params(&t, 0, &n, Field::Pitch, "D3").unwrap(), json!({"pitch_delta": 2}));
        assert_eq!(adjust_params(&t, 0, &n, Field::Start, "1.3.0").unwrap(), json!({"start_delta": 960}));
        assert!(adjust_params(&t, 960, &n, Field::Start, "1.1.0").is_err(), "before the clip");
        assert_eq!(adjust_params(&t, 0, &n, Field::End, "1.3.0").unwrap(), json!({"length_delta": 480}));
        assert!(adjust_params(&t, 0, &n, Field::End, "1.1.0").is_err());
        assert_eq!(adjust_params(&t, 0, &n, Field::Length, "0.0.240").unwrap(), json!({"length_delta": -240}));
        assert_eq!(adjust_params(&t, 0, &n, Field::Channel, "16").unwrap(), json!({"channel": 15}));
        assert!(adjust_params(&t, 0, &n, Field::Channel, "17").is_err());
    }
}
