//! Piano-roll clipboard: cut, copy, paste and duplicate notes (and controller events) inside MIDI
//! clips. The note clipboard (`Engine::clipboard.notes`) is separate from the arrange clipboard,
//! so copying notes does not disturb a copied clip, and the other way round.

use super::*;
use crate::{NoteClip, cmd};
use serde_json::json;
use soundcraft_midi::{CtrlEvent, Note, Sequence};
use soundcraft_model::{ClipContent, ClipId};

/// Ticks are clamped to this magnitude so hostile positions cannot overflow later sums.
const TICK_LIMIT: i64 = i64::MAX / 8;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "midi.notes_copy",
            "Copy Notes",
            [],
            None,
            "{clip, indices: [n], ctrl_indices?: [n]} (puts the notes and the listed controller events on the note clipboard) -> {notes, ctrls}",
            always,
            |e, p| copy(e, p, false, "midi.notes_copy")
        ),
        cmd!(
            "midi.notes_cut",
            "Cut Notes",
            [],
            None,
            "{clip, indices: [n], ctrl_indices?: [n]} (copy, then remove them from the clip) -> {notes, ctrls}",
            always,
            |e, p| copy(e, p, true, "midi.notes_cut")
        ),
        cmd!(
            "midi.notes_paste",
            "Paste Notes",
            [],
            None,
            "{clip, tick?, at?, semitones?: 0} (pastes the note clipboard; the first event lands at `tick` relative to the clip start, else at `at` samples, else at the playhead or insertion point when it is inside the clip, else at the clip start; the clip grows to hold the notes) -> {indices: [n], ctrls: n}",
            always,
            paste
        ),
        cmd!(
            "midi.notes_duplicate",
            "Duplicate Notes",
            [],
            None,
            "{clip, indices: [n]} (copies the notes right after the last selected note ends; the note clipboard is untouched) -> {indices: [n]}",
            always,
            duplicate
        ),
    ]
}

fn seq_of<'a>(e: &'a Engine, id: ClipId, cmd: &str) -> Result<&'a Sequence> {
    match e.session().find_clip(id).map(|(_, c)| &c.content) {
        Some(ClipContent::Midi { sequence }) => Ok(sequence),
        Some(_) => Err(bad(cmd, "not a MIDI clip")),
        None => Err(bad(cmd, "no such clip")),
    }
}

fn index_list(p: &Value, key: &str) -> Vec<usize> {
    let mut v: Vec<usize> = p
        .get(key)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_u64).filter_map(|x| usize::try_from(x).ok()).collect())
        .unwrap_or_default();
    v.sort_unstable();
    v.dedup();
    v
}

/// Snapshot of the listed notes and controller events, relative to the earliest of them.
fn snapshot(seq: &Sequence, notes: &[usize], ctrls: &[usize]) -> Option<NoteClip> {
    let ns: Vec<(Note, bool)> = notes.iter().filter_map(|&i| seq.notes.get(i).map(|n| (*n, seq.is_muted(i)))).collect();
    let cs: Vec<CtrlEvent> = ctrls.iter().filter_map(|&i| seq.ctrls.get(i).copied()).collect();
    let origin = ns.iter().map(|(n, _)| n.start).chain(cs.iter().map(|c| c.tick)).min()?;
    let end = ns.iter().map(|(n, _)| n.end()).chain(cs.iter().map(|c| c.tick)).max().unwrap_or(origin);
    Some(NoteClip {
        notes: ns.into_iter().map(|(n, m)| (Note { start: n.start - origin, ..n }, m)).collect(),
        ctrls: cs.into_iter().map(|c| CtrlEvent { tick: c.tick - origin, ..c }).collect(),
        span_ticks: end.saturating_sub(origin).max(0),
    })
}

fn copy(e: &mut Engine, p: &Value, cut: bool, id: &str) -> Result<Value> {
    let clip = clip_id(p, id)?;
    let notes = index_list(p, "indices");
    let ctrls = index_list(p, "ctrl_indices");
    let seq = seq_of(e, clip, id)?;
    let nc = snapshot(seq, &notes, &ctrls).ok_or_else(|| bad(id, "select notes first"))?;
    let counts = json!({"notes": nc.notes.len(), "ctrls": nc.ctrls.len()});
    if cut && let Some(ClipContent::Midi { sequence }) = e.session_mut().find_clip_mut(clip).map(|c| &mut c.content) {
        let mut i = 0usize;
        sequence.retain_notes(|_| {
            let keep = notes.binary_search(&i).is_err();
            i += 1;
            keep
        });
        let mut j = 0usize;
        sequence.ctrls.retain(|_| {
            let keep = ctrls.binary_search(&j).is_err();
            j += 1;
            keep
        });
    }
    e.clipboard.notes = Some(nc);
    Ok(counts)
}

/// Put `nc` into `clip` with its first event at `at` ticks (clamped to the clip start), shifted by
/// `semitones`. Grows the clip to hold the result. Returns the new notes' indices.
fn insert(e: &mut Engine, clip: ClipId, nc: &NoteClip, at: i64, semitones: i64, id: &str) -> Result<Vec<usize>> {
    seq_of(e, clip, id)?;
    let at = at.clamp(0, TICK_LIMIT);
    let s = e.session_mut();
    let (tempo, sr) = (s.tempo.clone(), s.sample_rate);
    let Some(c) = s.find_clip_mut(clip) else { return Err(bad(id, "no such clip")) };
    let clip_start = c.start;
    let ClipContent::Midi { sequence } = &mut c.content else { return Err(bad(id, "not a MIDI clip")) };
    let first_new = sequence.notes.len();
    for (n, muted) in &nc.notes {
        let pitch = (i64::from(n.pitch).saturating_add(semitones)).clamp(0, 127) as u8;
        sequence.notes.push(Note { pitch, start: n.start.clamp(0, TICK_LIMIT).saturating_add(at), ..*n });
        sequence.set_muted(sequence.notes.len().saturating_sub(1), *muted);
    }
    for ev in &nc.ctrls {
        sequence.ctrls.push(CtrlEvent { tick: ev.tick.clamp(0, TICK_LIMIT).saturating_add(at), ..*ev });
    }
    let end_tick = sequence.end_tick();
    let order = sequence.sort_tracked();
    let indices: Vec<usize> = (first_new..first_new + nc.notes.len()).filter_map(|old| order.get(old).copied()).collect();
    // Grow the clip when the pasted material ends past its end.
    let base = tempo.samples_to_ticks(clip_start, sr);
    let end = tempo.tick_to_samples(base.saturating_add(end_tick), sr);
    if end > c.end() {
        c.length = end - c.start;
    }
    Ok(indices)
}

fn paste(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "midi.notes_paste";
    let clip = clip_id(p, id)?;
    seq_of(e, clip, id)?;
    let nc = e.clipboard.notes.clone().ok_or_else(|| bad(id, "no notes on the clipboard"))?;
    let semitones = i64_or(p, "semitones", 0).clamp(-127, 127);
    let at_ticks = |e: &Engine, at: Samples| {
        let s = e.session();
        let clip_start = s.find_clip(clip).map_or(0, |(_, c)| c.start);
        s.tempo.samples_to_ticks(at, s.sample_rate) - s.tempo.samples_to_ticks(clip_start, s.sample_rate)
    };
    let tick = if let Some(t) = p.get("tick").and_then(Value::as_i64) {
        t
    } else if let Some(at) = position_param(e, id, p, "at")? {
        at_ticks(e, at)
    } else {
        // The playhead (while playing) or the insertion point, when it falls inside the clip.
        let (clip_start, clip_end) = e.session().find_clip(clip).map_or((0, 0), |(_, c)| (c.start, c.end()));
        let play = if e.transport.playing { e.transport.position } else { e.session().edit.selection.start };
        if play >= clip_start && play < clip_end { at_ticks(e, play) } else { 0 }
    };
    let indices = insert(e, clip, &nc, tick, semitones, id)?;
    Ok(json!({"indices": indices, "ctrls": nc.ctrls.len()}))
}

fn duplicate(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "midi.notes_duplicate";
    let clip = clip_id(p, id)?;
    let notes = index_list(p, "indices");
    let seq = seq_of(e, clip, id)?;
    let nc = snapshot(seq, &notes, &[]).ok_or_else(|| bad(id, "select notes first"))?;
    let after = notes.iter().filter_map(|&i| seq.notes.get(i)).map(Note::end).max().unwrap_or(0);
    let indices = insert(e, clip, &nc, after, 0, id)?;
    Ok(json!({"indices": indices}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_midi::CtrlKind;

    fn setup() -> (Engine, u64, u64, TrackId) {
        let mut e = Engine::default();
        let ids = e.execute("track.new", &json!({"count": 2, "kind": "midi"})).unwrap()["tracks"].clone();
        let t = TrackId(ids[0].as_u64().unwrap());
        let a = e.execute("midi.new_clip", &json!({"track": ids[0], "start": 0, "end": 96_000})).unwrap()["clip"].as_u64().unwrap();
        let b = e.execute("midi.new_clip", &json!({"track": ids[1], "start": 0, "end": 96_000})).unwrap()["clip"].as_u64().unwrap();
        (e, a, b, t)
    }

    fn seq(e: &Engine, clip: u64) -> Sequence {
        match &e.session().find_clip(ClipId(clip)).unwrap().1.content {
            ClipContent::Midi { sequence } => sequence.clone(),
            _ => panic!("not midi"),
        }
    }

    #[test]
    fn copy_notes_in_one_clip_and_paste_into_a_clip_on_another_track() {
        let (mut e, a, b, _) = setup();
        for (pitch, start) in [(60, 480), (64, 960)] {
            e.execute("midi.note_add", &json!({"clip": a, "pitch": pitch, "start_ticks": start, "length_ticks": 240})).unwrap();
        }
        e.execute("midi.notes_copy", &json!({"clip": a, "indices": [0, 1]})).unwrap();
        assert_eq!(seq(&e, a).notes.len(), 2, "copy leaves the source alone");
        let r = e.execute("midi.notes_paste", &json!({"clip": b, "tick": 1_920})).unwrap();
        assert_eq!(r["indices"], json!([0, 1]));
        let s = seq(&e, b);
        assert_eq!(s.notes.iter().map(|n| (n.pitch, n.start, n.length)).collect::<Vec<_>>(), vec![(60, 1_920, 240), (64, 2_400, 240)]);
        let r = e.execute("midi.notes_paste", &json!({"clip": b, "tick": 0, "semitones": 12})).unwrap();
        assert_eq!(r["indices"].as_array().unwrap().len(), 2);
        assert_eq!(seq(&e, b).notes.first().map(|n| n.pitch), Some(72));
    }

    #[test]
    fn cut_then_paste_restores_notes_with_mute_flags_and_controller_events_in_one_undo_step() {
        let (mut e, a, _, _) = setup();
        for (pitch, start) in [(60, 0), (62, 480), (67, 960)] {
            e.execute("midi.note_add", &json!({"clip": a, "pitch": pitch, "start_ticks": start, "length_ticks": 240})).unwrap();
        }
        e.execute("midi.notes_mute", &json!({"clip": a, "indices": [1], "muted": true})).unwrap();
        {
            let ClipContent::Midi { sequence } = &mut e.session_mut().find_clip_mut(ClipId(a)).unwrap().content else { panic!() };
            sequence.ctrls.push(CtrlEvent { tick: 500, channel: 0, kind: CtrlKind::Cc(1), value: 90 });
        }
        let before = seq(&e, a);
        let steps = e.undo_history().len();
        e.execute("midi.notes_cut", &json!({"clip": a, "indices": [0, 1], "ctrl_indices": [0]})).unwrap();
        assert_eq!(e.undo_history().len(), steps + 1);
        let cut = seq(&e, a);
        assert_eq!(cut.notes.len(), 1);
        assert!(cut.ctrls.is_empty());
        assert!(cut.muted.is_empty());
        e.execute("midi.notes_paste", &json!({"clip": a, "tick": 0})).unwrap();
        let back = seq(&e, a);
        assert_eq!(back.notes, before.notes);
        assert_eq!(back.muted, before.muted);
        assert_eq!(back.ctrls, before.ctrls);
        e.undo();
        assert_eq!(seq(&e, a), cut);
    }

    #[test]
    fn paste_past_the_clip_end_extends_the_clip_and_survives_edit_copy() {
        let (mut e, a, _, t) = setup();
        e.execute("midi.note_add", &json!({"clip": a, "pitch": 60, "start_ticks": 0, "length_ticks": 480})).unwrap();
        e.execute("midi.notes_copy", &json!({"clip": a, "indices": [0]})).unwrap();
        let before = e.session().find_clip(ClipId(a)).unwrap().1.length;
        e.execute("midi.notes_paste", &json!({"clip": a, "tick": 100_000})).unwrap();
        assert!(e.session().find_clip(ClipId(a)).unwrap().1.length > before, "clip grew");
        e.execute("edit.select", &json!({"tracks": [t.0], "start": 0, "end": 1_000})).unwrap();
        e.execute("edit.copy", &json!({})).unwrap();
        assert!(e.clipboard.notes.is_some(), "an arrange copy keeps the note clipboard");
        assert!(e.execute("midi.notes_paste", &json!({"clip": a, "tick": 0})).is_ok());
    }

    #[test]
    fn duplicate_lands_after_the_last_note_and_leaves_the_clipboard_alone() {
        let (mut e, a, _, _) = setup();
        e.execute("midi.note_add", &json!({"clip": a, "pitch": 60, "start_ticks": 0, "length_ticks": 480})).unwrap();
        assert!(e.execute("midi.notes_copy", &json!({"clip": a, "indices": []})).is_err());
        let r = e.execute("midi.notes_duplicate", &json!({"clip": a, "indices": [0]})).unwrap();
        assert_eq!(r["indices"], json!([1]));
        assert_eq!(seq(&e, a).notes.iter().map(|n| n.start).collect::<Vec<_>>(), vec![0, 480]);
        assert!(e.clipboard.notes.is_none());
        assert!(e.execute("midi.notes_paste", &json!({"clip": a})).is_err(), "nothing copied yet");
    }
}
