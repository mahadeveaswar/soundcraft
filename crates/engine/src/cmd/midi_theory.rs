//! Note-entry assistance for the MIDI Editor: a scale per clip (stored in the session's numeric
//! settings under `midi.scale.<clip>`, so no model change), snapping notes to it, and chords added
//! as one undo step.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_midi::{ChordKind, Note, Scale, ScaleKind, Sequence};
use soundcraft_model::{ClipContent, ClipId};
use soundcraft_time::TICKS_PER_QUARTER;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "midi.scale_set",
            "Set Clip Scale",
            [],
            None,
            "{clip, root: 0-11 | 'C'..'B', kind: major|minor|dorian|phrygian|lydian|mixolydian|locrian|harmonic_minor|melodic_minor|penta_major|penta_minor|blues|chromatic | null (clears)}",
            always,
            scale_set
        ),
        cmd!(
            "midi.scale_snap",
            "Snap Notes to Scale",
            [],
            None,
            "{clip, indices?: [n]} (pitches move to the nearest scale tone, ties down; needs `midi.scale_set` first) -> {indices}",
            always,
            scale_snap
        ),
        cmd!(
            "midi.chord_add",
            "Add Chord",
            [],
            None,
            "{clip, root: pitch 0-127, kind: maj|min|dim|aug|sus2|sus4|7|maj7|m7, start_ticks, length_ticks?: 960, velocity?: 100, inversion?: 0} -> {indices}",
            always,
            chord_add
        ),
        cmd!(query "midi.chord_at", "Chord at Tick", [], None, "{clip, tick} -> {chord: name | null} (the notes sounding at the tick)", always, chord_at),
    ]
}

/// The session key holding a clip's scale.
pub fn scale_key(clip: ClipId) -> String {
    format!("midi.scale.{}", clip.0)
}

fn seq_mut<'a>(e: &'a mut Engine, id: ClipId, cmd: &str) -> Result<&'a mut Sequence> {
    match e.session_mut().find_clip_mut(id).map(|c| &mut c.content) {
        Some(ClipContent::Midi { sequence }) => Ok(sequence),
        Some(_) => Err(bad(cmd, "not a MIDI clip")),
        None => Err(bad(cmd, "no such clip")),
    }
}

fn root_param(p: &Value, cmd: &str) -> Result<u8> {
    match p.get("root") {
        Some(Value::Number(n)) => n.as_u64().filter(|r| *r < 12).map(|r| r as u8).ok_or_else(|| bad(cmd, "`root` must be 0-11")),
        Some(Value::String(s)) => {
            const NAMES: [&str; 12] = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
            let t = s.trim().to_ascii_uppercase();
            NAMES.iter().position(|n| *n == t).map(|i| i as u8).ok_or_else(|| bad(cmd, "unknown root note"))
        }
        _ => Err(bad(cmd, "`root` required")),
    }
}

fn scale_set(e: &mut Engine, p: &Value) -> Result<Value> {
    const CMD: &str = "midi.scale_set";
    let id = clip_id(p, CMD)?;
    seq_mut(e, id, CMD)?;
    let key = scale_key(id);
    match p.get("kind") {
        Some(Value::Null) | None => {
            e.session_mut().edit.values.remove(&key);
            Ok(json!({"scale": null}))
        }
        Some(Value::String(k)) => {
            let kind = ScaleKind::parse(k).ok_or_else(|| bad(CMD, "unknown scale kind"))?;
            let scale = Scale::new(root_param(p, CMD)?, kind);
            e.session_mut().edit.values.insert(key, scale.to_code());
            Ok(json!({"scale": {"root": scale.root, "kind": kind.id()}}))
        }
        Some(_) => Err(bad(CMD, "`kind` must be a name or null")),
    }
}

fn scale_snap(e: &mut Engine, p: &Value) -> Result<Value> {
    const CMD: &str = "midi.scale_snap";
    let id = clip_id(p, CMD)?;
    let scale = Scale::from_code(e.session().edit.value(&scale_key(id), 0.0)).ok_or_else(|| bad(CMD, "the clip has no scale"))?;
    let sel = super::midi_sel::indices_param(p);
    let seq = seq_mut(e, id, CMD)?;
    let moved = super::midi_sel::apply(seq, sel.as_deref(), |notes| {
        for n in notes {
            n.pitch = scale.snap(n.pitch);
        }
    });
    Ok(json!({"indices": moved}))
}

fn chord_add(e: &mut Engine, p: &Value) -> Result<Value> {
    const CMD: &str = "midi.chord_add";
    let id = clip_id(p, CMD)?;
    let kind =
        str_param(p, "kind").and_then(ChordKind::parse).ok_or_else(|| bad(CMD, "`kind` must be maj, min, dim, aug, sus2, sus4, 7, maj7 or m7"))?;
    let root = i64_or(p, "root", 60).clamp(0, 127) as u8;
    let inversion = i64_or(p, "inversion", 0).clamp(0, 8) as u8;
    let start = i64_or(p, "start_ticks", 0).max(0);
    let length = i64_or(p, "length_ticks", TICKS_PER_QUARTER).max(1);
    let velocity = i64_or(p, "velocity", 100).clamp(1, 127) as u8;
    let pitches = soundcraft_midi::chord::notes(root, kind, inversion);
    let seq = seq_mut(e, id, CMD)?;
    let new: Vec<Note> = pitches.iter().map(|&pitch| Note { pitch, velocity, release_velocity: 64, channel: 0, start, length }).collect();
    let first_new = seq.notes.len();
    seq.notes.extend(new.iter().copied());
    let perm = seq.sort_tracked();
    let mut indices: Vec<usize> = (first_new..first_new + new.len()).filter_map(|i| perm.get(i).copied()).collect();
    indices.sort_unstable();
    Ok(json!({"indices": indices}))
}

fn chord_at(e: &mut Engine, p: &Value) -> Result<Value> {
    const CMD: &str = "midi.chord_at";
    let id = clip_id(p, CMD)?;
    let tick = i64_or(p, "tick", 0);
    let seq = seq_mut(e, id, CMD)?;
    Ok(json!({"chord": chord_name_at(seq, tick)}))
}

/// The name of the chord sounding at `tick` (3 or more pitch classes), by the same naming as Extract Chords.
pub fn chord_name_at(seq: &Sequence, tick: i64) -> Option<String> {
    let mut weights = [0.0f64; 12];
    let mut bass: Option<u8> = None;
    for n in seq.notes.iter().filter(|n| n.start <= tick && tick < n.end()) {
        if let Some(w) = weights.get_mut(usize::from(n.pitch % 12)) {
            *w += 1.0;
        }
        bass = Some(bass.map_or(n.pitch, |b| b.min(n.pitch)));
    }
    super::event_more::chord_name(&weights, usize::from(bass? % 12))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn clip(e: &mut Engine) -> u64 {
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        e.execute("midi.new_clip", &json!({"track": t.0})).unwrap()["clip"].as_u64().unwrap()
    }

    fn pitches(e: &mut Engine, c: u64) -> Vec<u64> {
        let r = e.execute("midi.notes", &json!({"clip": c})).unwrap();
        r["notes"].as_array().unwrap().iter().map(|n| n["pitch"].as_u64().unwrap()).collect()
    }

    #[test]
    fn chord_add_is_one_undo_step_with_the_right_pitches() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        let r = e.execute("midi.chord_add", &json!({"clip": c, "root": 60, "kind": "maj7", "start_ticks": 0, "length_ticks": 960})).unwrap();
        assert_eq!(r["indices"], json!([0, 1, 2, 3]));
        assert_eq!(pitches(&mut e, c), vec![60, 64, 67, 71]);
        assert!(e.undo());
        assert!(pitches(&mut e, c).is_empty(), "one undo removes all four notes");
        e.execute("midi.chord_add", &json!({"clip": c, "root": 60, "kind": "maj7", "start_ticks": 0, "inversion": 1})).unwrap();
        assert_eq!(pitches(&mut e, c), vec![64, 67, 71, 72]);
        assert!(e.execute("midi.chord_add", &json!({"clip": c, "root": 60, "kind": "bogus", "start_ticks": 0})).is_err());
    }

    #[test]
    fn chord_at_names_the_sounding_notes() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        e.execute("midi.chord_add", &json!({"clip": c, "root": 60, "kind": "min", "start_ticks": 0, "length_ticks": 960})).unwrap();
        assert_eq!(e.execute("midi.chord_at", &json!({"clip": c, "tick": 100})).unwrap()["chord"], "Cm");
        assert!(e.execute("midi.chord_at", &json!({"clip": c, "tick": 2000})).unwrap()["chord"].is_null());
    }

    #[test]
    fn scale_set_snap_and_clear() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        for pitch in [61, 63, 66] {
            e.execute("midi.note_add", &json!({"clip": c, "pitch": pitch, "start_ticks": pitch * 10})).unwrap();
        }
        assert!(e.execute("midi.scale_snap", &json!({"clip": c})).is_err(), "no scale yet");
        let r = e.execute("midi.scale_set", &json!({"clip": c, "root": "C", "kind": "major"})).unwrap();
        assert_eq!(r["scale"], json!({"root": 0, "kind": "major"}));
        e.execute("midi.scale_snap", &json!({"clip": c, "indices": [0]})).unwrap();
        assert_eq!(pitches(&mut e, c), vec![60, 63, 66], "only the selected note moved");
        e.execute("midi.scale_snap", &json!({"clip": c})).unwrap();
        assert_eq!(pitches(&mut e, c), vec![60, 62, 65]);
        e.execute("midi.scale_set", &json!({"clip": c, "root": 0, "kind": null})).unwrap();
        assert!(e.execute("midi.scale_snap", &json!({"clip": c})).is_err());
        assert!(e.execute("midi.scale_set", &json!({"clip": c, "root": 12, "kind": "major"})).is_err());
        assert!(e.execute("midi.scale_set", &json!({"clip": c, "root": 0, "kind": "nope"})).is_err());
    }

    #[test]
    fn a_session_without_scale_data_has_no_scale() {
        let e = Engine::default();
        assert!(Scale::from_code(e.session().edit.value("midi.scale.1", 0.0)).is_none());
    }
}
