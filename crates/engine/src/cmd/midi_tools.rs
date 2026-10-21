//! MIDI Editor tools that reshape notes: split at a tick, glue notes together.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_midi::{Note, Sequence};
use soundcraft_model::{ClipContent, ClipId};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!("midi.note_split", "Split Note", [], None, "{clip, index, tick} (tick relative to the clip start, inside the note)", always, note_split),
        cmd!(
            "midi.notes_glue",
            "Glue Notes",
            [],
            None,
            "{clip, indices: [n]} (joins same-pitch, same-channel notes into one from the first start to the last end)",
            always,
            notes_glue
        ),
    ]
}

fn seq_mut<'a>(e: &'a mut Engine, id: ClipId, cmd: &str) -> Result<&'a mut Sequence> {
    match e.session_mut().find_clip_mut(id).map(|c| &mut c.content) {
        Some(ClipContent::Midi { sequence }) => Ok(sequence),
        Some(_) => Err(bad(cmd, "not a MIDI clip")),
        None => Err(bad(cmd, "no such clip")),
    }
}

fn note_split(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = clip_id(p, "midi.note_split")?;
    let idx = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad("midi.note_split", "`index` required"))? as usize;
    let tick = p.get("tick").and_then(Value::as_i64).ok_or_else(|| bad("midi.note_split", "`tick` required"))?;
    let seq = seq_mut(e, id, "midi.note_split")?;
    let n = *seq.notes.get(idx).ok_or_else(|| bad("midi.note_split", "index out of range"))?;
    if tick <= n.start || tick >= n.end() {
        return Err(bad("midi.note_split", "`tick` must fall inside the note"));
    }
    if let Some(first) = seq.notes.get_mut(idx) {
        first.length = tick - n.start;
    }
    seq.notes.push(Note { start: tick, length: n.end() - tick, ..n });
    seq.sort();
    Ok(json!({"notes": seq.notes.len()}))
}

fn notes_glue(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = clip_id(p, "midi.notes_glue")?;
    let idx: Vec<usize> =
        p.get("indices").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|x| x as usize).collect()).unwrap_or_default();
    let seq = seq_mut(e, id, "midi.notes_glue")?;
    // Group the chosen notes by (pitch, channel); each group of two or more becomes one note.
    let mut groups: Vec<((u8, u8), Vec<usize>)> = Vec::new();
    for &i in &idx {
        if let Some(n) = seq.notes.get(i) {
            let key = (n.pitch, n.channel);
            match groups.iter_mut().find(|g| g.0 == key) {
                Some(g) => {
                    if !g.1.contains(&i) {
                        g.1.push(i);
                    }
                }
                None => groups.push((key, vec![i])),
            }
        }
    }
    let mut remove: Vec<usize> = Vec::new();
    let mut glued = 0;
    for (_, members) in groups.iter().filter(|g| g.1.len() >= 2) {
        let first = members.iter().filter_map(|&i| seq.notes.get(i)).min_by_key(|n| n.start).copied();
        let end = members.iter().filter_map(|&i| seq.notes.get(i)).map(Note::end).max();
        if let (Some(first), Some(end)) = (first, end) {
            // The earliest note keeps its place (and velocity); it grows to cover the group.
            let keep = members.iter().copied().find(|&i| seq.notes.get(i).is_some_and(|n| n.start == first.start));
            for &i in members {
                if Some(i) != keep {
                    remove.push(i);
                }
            }
            if let Some(k) = keep.and_then(|k| seq.notes.get_mut(k)) {
                k.length = (end - k.start).max(1);
            }
            glued += 1;
        }
    }
    let mut i = 0;
    seq.notes.retain(|_| {
        let keep = !remove.contains(&i);
        i += 1;
        keep
    });
    seq.sort();
    Ok(json!({"glued": glued, "removed": remove.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn clip(e: &mut Engine) -> u64 {
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        e.execute("midi.new_clip", &json!({"track": t.0})).unwrap()["clip"].as_u64().unwrap()
    }

    fn notes(e: &mut Engine, c: u64) -> Vec<(i64, i64, i64)> {
        let r = e.execute("midi.notes", &json!({"clip": c})).unwrap();
        r["notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| (n["pitch"].as_i64().unwrap(), n["start"].as_i64().unwrap(), n["length"].as_i64().unwrap()))
            .collect()
    }

    #[test]
    fn split_makes_two_notes_that_meet() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        e.execute("midi.note_add", &json!({"clip": c, "pitch": 60, "start_ticks": 0, "length_ticks": 960})).unwrap();
        e.execute("midi.note_split", &json!({"clip": c, "index": 0, "tick": 240})).unwrap();
        assert_eq!(notes(&mut e, c), vec![(60, 0, 240), (60, 240, 720)]);
    }

    #[test]
    fn split_outside_the_note_is_an_error() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        e.execute("midi.note_add", &json!({"clip": c, "pitch": 60, "start_ticks": 100, "length_ticks": 100})).unwrap();
        for tick in [100, 200, 50, 500, -5] {
            assert!(e.execute("midi.note_split", &json!({"clip": c, "index": 0, "tick": tick})).is_err());
        }
        assert!(e.execute("midi.note_split", &json!({"clip": c, "index": 9, "tick": 150})).is_err());
        assert_eq!(notes(&mut e, c).len(), 1);
    }

    #[test]
    fn glue_joins_same_pitch_only() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        for (pitch, start) in [(60, 0), (60, 480), (64, 0), (60, 960)] {
            e.execute("midi.note_add", &json!({"clip": c, "pitch": pitch, "start_ticks": start, "length_ticks": 240})).unwrap();
        }
        let before = notes(&mut e, c);
        let all: Vec<usize> = (0..before.len()).collect();
        let r = e.execute("midi.notes_glue", &json!({"clip": c, "indices": all})).unwrap();
        assert_eq!(r["glued"], 1);
        assert_eq!(notes(&mut e, c), vec![(60, 0, 1200), (64, 0, 240)]);
    }

    #[test]
    fn glue_of_one_note_or_bad_indices_changes_nothing() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        e.execute("midi.note_add", &json!({"clip": c, "pitch": 60, "start_ticks": 0, "length_ticks": 240})).unwrap();
        let r = e.execute("midi.notes_glue", &json!({"clip": c, "indices": [0, 7, 99]})).unwrap();
        assert_eq!(r["glued"], 0);
        assert_eq!(notes(&mut e, c), vec![(60, 0, 240)]);
    }
}
