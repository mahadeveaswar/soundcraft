//! Info-strip edits for the MIDI Editor: change start, length, pitch, velocity or channel of
//! several notes at once (absolute values or deltas) as one undo step. Unlike `midi.note_edit`
//! it reports where the edited notes ended up, because the sequence is re-sorted.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_midi::{Note, Sequence};
use soundcraft_model::{ClipContent, ClipId};

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "midi.notes_adjust",
        "Adjust Notes",
        [],
        None,
        "{clip, indices: [n], start_ticks?, length_ticks?, pitch?, velocity?, channel?: 0-15, start_delta?, length_delta?, pitch_delta?, velocity_delta?} → {indices: [new positions]}",
        always,
        notes_adjust
    )]
}

fn seq_mut<'a>(e: &'a mut Engine, id: ClipId, cmd: &str) -> Result<&'a mut Sequence> {
    match e.session_mut().find_clip_mut(id).map(|c| &mut c.content) {
        Some(ClipContent::Midi { sequence }) => Ok(sequence),
        Some(_) => Err(bad(cmd, "not a MIDI clip")),
        None => Err(bad(cmd, "no such clip")),
    }
}

fn opt(p: &Value, key: &str) -> Option<i64> {
    p.get(key).and_then(Value::as_i64)
}

fn sort_key(n: &Note) -> (i64, u8, u8, i64, u8) {
    (n.start, n.pitch, n.channel, n.length, n.velocity)
}

fn notes_adjust(e: &mut Engine, p: &Value) -> Result<Value> {
    const CMD: &str = "midi.notes_adjust";
    let id = clip_id(p, CMD)?;
    let wanted: Vec<usize> =
        p.get("indices").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|x| x as usize).collect()).unwrap_or_default();
    if let Some(c) = opt(p, "channel")
        && !(0..=15).contains(&c)
    {
        return Err(bad(CMD, "`channel` must be 0-15"));
    }
    let seq = seq_mut(e, id, CMD)?;
    if !wanted.iter().any(|i| *i < seq.notes.len()) {
        return Err(bad(CMD, "no notes selected"));
    }
    let mut tagged: Vec<(Note, bool, bool)> = Vec::with_capacity(seq.notes.len());
    for (i, n) in seq.notes.iter().enumerate() {
        let mut n = *n;
        let muted = seq.is_muted(i);
        let hit = wanted.contains(&i);
        if hit {
            if let Some(v) = opt(p, "start_ticks") {
                n.start = v.max(0);
            }
            if let Some(v) = opt(p, "length_ticks") {
                n.length = v.max(1);
            }
            if let Some(v) = opt(p, "pitch") {
                n.pitch = v.clamp(0, 127) as u8;
            }
            if let Some(v) = opt(p, "velocity") {
                n.velocity = v.clamp(1, 127) as u8;
            }
            if let Some(v) = opt(p, "channel") {
                n.channel = v.clamp(0, 15) as u8;
            }
            n.start = n.start.saturating_add(opt(p, "start_delta").unwrap_or(0)).max(0);
            n.length = n.length.saturating_add(opt(p, "length_delta").unwrap_or(0)).max(1);
            n.pitch = i64::from(n.pitch).saturating_add(opt(p, "pitch_delta").unwrap_or(0)).clamp(0, 127) as u8;
            n.velocity = i64::from(n.velocity).saturating_add(opt(p, "velocity_delta").unwrap_or(0)).clamp(1, 127) as u8;
        }
        tagged.push((n, hit, muted));
    }
    tagged.sort_by_key(|(n, _, _)| sort_key(n));
    let indices: Vec<usize> = tagged.iter().enumerate().filter(|(_, (_, hit, _))| *hit).map(|(i, _)| i).collect();
    seq.muted = tagged.iter().enumerate().filter(|(_, (_, _, m))| *m).filter_map(|(i, _)| u32::try_from(i).ok()).collect();
    seq.notes = tagged.into_iter().map(|(n, _, _)| n).collect();
    Ok(json!({"indices": indices}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn clip_with_notes() -> (Engine, u64) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        let clip = e.execute("midi.new_clip", &json!({"track": t.0})).unwrap()["clip"].as_u64().unwrap();
        for (pitch, start) in [(60, 0), (64, 480), (67, 960)] {
            e.execute("midi.note_add", &json!({"clip": clip, "pitch": pitch, "start_ticks": start, "velocity": 100})).unwrap();
        }
        (e, clip)
    }

    #[test]
    fn deltas_apply_to_each_note_and_report_new_positions() {
        let (mut e, clip) = clip_with_notes();
        let r = e
            .execute("midi.notes_adjust", &json!({"clip": clip, "indices": [0, 1], "start_delta": 1000, "velocity_delta": -30, "channel": 3}))
            .unwrap();
        assert_eq!(r["indices"], json!([1, 2]), "the shifted notes now sort after the untouched one");
        let n = e.execute("midi.notes", &json!({"clip": clip})).unwrap();
        assert_eq!(n["notes"][0]["pitch"], 67);
        assert_eq!(n["notes"][1]["velocity"], 70);
        assert_eq!(n["notes"][1]["start"], 1000);
        assert_eq!(n["notes"][2]["start"], 1480);
    }

    #[test]
    fn invalid_input_is_rejected_and_extremes_clamp() {
        let (mut e, clip) = clip_with_notes();
        assert!(e.execute("midi.notes_adjust", &json!({"clip": clip, "indices": [9], "velocity": 5})).is_err());
        assert!(e.execute("midi.notes_adjust", &json!({"clip": clip, "indices": [0], "channel": 16})).is_err());
        e.execute(
            "midi.notes_adjust",
            &json!({"clip": clip, "indices": [0], "pitch_delta": i64::MAX, "velocity_delta": i64::MIN, "start_delta": i64::MIN}),
        )
        .unwrap();
        let n = e.execute("midi.notes", &json!({"clip": clip})).unwrap();
        assert_eq!(n["notes"][0]["start"], 0);
        assert!(
            e.execute("midi.notes", &json!({"clip": clip})).unwrap()["notes"]
                .as_array()
                .is_some_and(|a| a.iter().any(|x| x["pitch"] == 127 && x["velocity"] == 1))
        );
    }

    #[test]
    fn one_undo_step_restores_everything() {
        let (mut e, clip) = clip_with_notes();
        let before = e.execute("midi.notes", &json!({"clip": clip})).unwrap();
        e.execute("midi.notes_adjust", &json!({"clip": clip, "indices": [0, 1, 2], "pitch_delta": 2})).unwrap();
        assert!(e.undo());
        assert_eq!(e.execute("midi.notes", &json!({"clip": clip})).unwrap(), before);
    }
}
