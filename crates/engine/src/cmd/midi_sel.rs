//! Selection-scoped MIDI operations. The `event.quantize/transpose/change_velocity/
//! change_duration` commands take an optional `indices: [n]` (note positions inside one clip, as
//! listed by `midi.notes`); without it they act on whole clips exactly as before. Also hosts
//! `event.fix_overlaps`.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_midi::{Note, Sequence};
use soundcraft_model::ClipContent;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "event.fix_overlaps",
        "Fix Overlaps",
        ["Event", "MIDI Operations"],
        None,
        "{clips?, indices?: [n]} (trims each note to the next same-pitch, same-channel note; drops zero-length ones) -> {removed, indices}",
        has_selection,
        fix_overlaps
    )]
}

/// The `indices` parameter: `None` when absent (act on whole clips), otherwise the note positions.
pub fn indices_param(p: &Value) -> Option<Vec<usize>> {
    let a = p.get("indices")?.as_array()?;
    Some(a.iter().filter_map(Value::as_u64).filter_map(|x| usize::try_from(x).ok()).collect())
}

fn valid_sorted(idx: &[usize], len: usize) -> Vec<usize> {
    let mut v: Vec<usize> = idx.iter().copied().filter(|&i| i < len).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// Applies `f` to the notes at `indices` (all notes when `None`), then re-sorts. Returns the new
/// positions of the notes that were selected (empty when `indices` is `None`).
pub fn apply(seq: &mut Sequence, indices: Option<&[usize]>, f: impl FnOnce(&mut [Note])) -> Vec<usize> {
    let Some(idx) = indices else {
        f(&mut seq.notes);
        seq.sort();
        return Vec::new();
    };
    let sel = valid_sorted(idx, seq.notes.len());
    let mut sub: Vec<Note> = sel.iter().filter_map(|&i| seq.notes.get(i).copied()).collect();
    f(&mut sub);
    for (&i, n) in sel.iter().zip(sub) {
        if let Some(slot) = seq.notes.get_mut(i) {
            *slot = n;
        }
    }
    let perm = seq.sort_tracked();
    let mut out: Vec<usize> = sel.iter().filter_map(|&i| perm.get(i).copied()).collect();
    out.sort_unstable();
    out
}

/// Shared body of the scoped `event.*` note operations: acts on `indices` of the first clip, or on
/// every clip from `clips` / the selection.
pub fn run(e: &mut Engine, p: &Value, f: impl Fn(&mut [Note])) -> Result<Value> {
    let sel = indices_param(p);
    let mut ids = clip_ids_param(e, p);
    if sel.is_some() {
        ids.truncate(1);
    }
    let s = e.session_mut();
    let (mut n, mut moved) = (0, Vec::new());
    for id in ids {
        if let Some(c) = s.find_clip_mut(id)
            && let ClipContent::Midi { sequence } = &mut c.content
        {
            moved = apply(sequence, sel.as_deref(), &f);
            n += sequence.notes.len();
        }
    }
    Ok(json!({"notes": n, "indices": moved}))
}

fn fix_overlaps(e: &mut Engine, p: &Value) -> Result<Value> {
    let sel = indices_param(p);
    let mut ids = clip_ids_param(e, p);
    if sel.is_some() {
        ids.truncate(1);
    }
    let s = e.session_mut();
    let (mut removed, mut kept_sel) = (0usize, Vec::new());
    for id in ids {
        let Some(c) = s.find_clip_mut(id) else { continue };
        let ClipContent::Midi { sequence } = &mut c.content else { continue };
        let all: Vec<usize> = (0..sequence.notes.len()).collect();
        let sel_idx = valid_sorted(sel.as_deref().unwrap_or(&all), sequence.notes.len());
        let mut sub: Vec<Note> = sel_idx.iter().filter_map(|&i| sequence.notes.get(i).copied()).collect();
        let drop_sub = soundcraft_midi::ops::trim_overlaps(&mut sub);
        let dropped: Vec<usize> = drop_sub.iter().filter_map(|&k| sel_idx.get(k).copied()).collect();
        for (&i, n) in sel_idx.iter().zip(sub) {
            if let Some(slot) = sequence.notes.get_mut(i) {
                *slot = n;
            }
        }
        let mut i = 0usize;
        sequence.retain_notes(|_| {
            let keep = dropped.binary_search(&i).is_err();
            i += 1;
            keep
        });
        removed += dropped.len();
        // `dropped` is ascending (sorted by `trim_overlaps`, mapped through the ascending `sel_idx`).
        kept_sel = sel_idx.iter().filter(|i| dropped.binary_search(i).is_err()).map(|&i| i - dropped.partition_point(|&d| d < i)).collect();
    }
    Ok(json!({"removed": removed, "indices": kept_sel}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn setup() -> (Engine, u64) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        let clip = e.execute("midi.new_clip", &json!({"track": t.0})).unwrap()["clip"].as_u64().unwrap();
        for (pitch, start, vel) in [(60, 10, 100), (64, 490, 100), (67, 970, 100)] {
            e.execute("midi.note_add", &json!({"clip": clip, "pitch": pitch, "start_ticks": start, "velocity": vel, "length_ticks": 300})).unwrap();
        }
        (e, clip)
    }

    fn notes(e: &mut Engine, clip: u64) -> Vec<(u64, i64, u64)> {
        let v = e.execute("midi.notes", &json!({"clip": clip})).unwrap();
        v["notes"]
            .as_array()
            .unwrap()
            .iter()
            .map(|n| (n["pitch"].as_u64().unwrap(), n["start"].as_i64().unwrap(), n["velocity"].as_u64().unwrap()))
            .collect()
    }

    fn lengths(e: &mut Engine, clip: u64) -> Vec<i64> {
        let v = e.execute("midi.notes", &json!({"clip": clip})).unwrap();
        v["notes"].as_array().unwrap().iter().map(|n| n["length"].as_i64().unwrap()).collect()
    }

    #[test]
    fn transpose_only_the_selected_notes_and_undo() {
        let (mut e, clip) = setup();
        let before = notes(&mut e, clip);
        let r = e.execute("event.transpose", &json!({"clip": clip, "indices": [1], "semitones": 2})).unwrap();
        assert_eq!(r["indices"], json!([1]));
        let after = notes(&mut e, clip);
        assert_eq!((after[0].0, after[1].0, after[2].0), (60, 66, 67));
        assert!(e.undo());
        assert_eq!(notes(&mut e, clip), before);
    }

    #[test]
    fn quantize_selection_leaves_others() {
        let (mut e, clip) = setup();
        e.execute("event.quantize", &json!({"clip": clip, "indices": [2], "grid": "1/4"})).unwrap();
        let n = notes(&mut e, clip);
        assert_eq!(n[0].1, 10, "unselected note untouched");
        assert_eq!(n[2].1, 960, "selected note snapped");
        assert!(e.undo());
        assert_eq!(notes(&mut e, clip)[2].1, 970);
    }

    #[test]
    fn velocity_and_duration_scoped() {
        let (mut e, clip) = setup();
        e.execute("event.change_velocity", &json!({"clip": clip, "indices": [0, 2], "set": 40})).unwrap();
        let v: Vec<u64> = notes(&mut e, clip).iter().map(|n| n.2).collect();
        assert_eq!(v, vec![40, 100, 40]);
        e.execute("event.change_duration", &json!({"clip": clip, "indices": [1], "set": 77})).unwrap();
        assert_eq!(lengths(&mut e, clip), vec![300, 77, 300]);
    }

    #[test]
    fn without_indices_acts_on_the_whole_clip() {
        let (mut e, clip) = setup();
        e.execute("event.transpose", &json!({"clip": clip, "semitones": 1})).unwrap();
        let p: Vec<u64> = notes(&mut e, clip).iter().map(|n| n.0).collect();
        assert_eq!(p, vec![61, 65, 68]);
        e.execute("event.transpose", &json!({"clip": clip, "indices": [], "semitones": 5})).unwrap();
        let p: Vec<u64> = notes(&mut e, clip).iter().map(|n| n.0).collect();
        assert_eq!(p, vec![61, 65, 68], "empty selection changes nothing");
    }

    #[test]
    fn fix_overlaps_command_trims_drops_and_undoes() {
        let (mut e, clip) = setup();
        e.execute("midi.note_add", &json!({"clip": clip, "pitch": 60, "start_ticks": 200, "length_ticks": 300})).unwrap();
        e.execute("midi.note_add", &json!({"clip": clip, "pitch": 60, "start_ticks": 200, "length_ticks": 100})).unwrap();
        let before = notes(&mut e, clip);
        let r = e.execute("event.fix_overlaps", &json!({"clip": clip})).unwrap();
        assert_eq!(r["removed"], 1);
        let n = notes(&mut e, clip);
        assert_eq!(n.len(), before.len() - 1);
        assert_eq!(lengths(&mut e, clip)[0], 190, "first note trimmed to the next same-pitch start");
        assert!(e.undo());
        assert_eq!(notes(&mut e, clip), before);
    }

    #[test]
    fn legato_scoped_to_selection() {
        let (mut e, clip) = setup();
        e.execute("event.change_duration", &json!({"clip": clip, "indices": [0, 1], "legato": 0})).unwrap();
        assert_eq!(lengths(&mut e, clip), vec![480, 300, 300], "only the selected pair is joined");
    }
}
