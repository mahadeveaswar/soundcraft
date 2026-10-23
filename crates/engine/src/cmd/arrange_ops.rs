//! Arrange-view commands behind the context menus and the alternative key-command preset: glue, explicit
//! mute/unmute, delete, split at the playhead, locator moves and previous/next track and clip selection.
//! None of them has a default-preset shortcut (the preset overlay in the UI layer assigns keys).

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{Clip, ClipContent, Track};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "clip.glue",
            "Glue",
            ["Clip"],
            None,
            "{clips?: [id]}; joins the selected clips of each track: audio must be contiguous parts of one file, MIDI is merged into one clip",
            two_clips,
            glue
        ),
        cmd!("clip.mute", "Mute Clips", ["Clip"], None, "{clips?: [id]}; idempotent (see clip.unmute)", has_selection, |e, p| set_muted(e, p, true)),
        cmd!("clip.unmute", "Unmute Clips", ["Clip"], None, "{clips?: [id]}; idempotent", has_selection, |e, p| set_muted(e, p, false)),
        cmd!(
            "edit.delete",
            "Delete",
            ["Edit"],
            None,
            "{clips?: [id]}; removes the selected clips without closing the gap, else clears the selected range",
            has_selection,
            delete
        ),
        cmd!(
            "edit.split_at_playhead",
            "Split at Playhead",
            ["Edit", "Separate"],
            None,
            "{at?: position}; splits the selected clips (else the clips on the selected tracks) at the playhead",
            has_selection,
            split_at_playhead
        ),
        cmd!(
            "clip.split",
            "Split Clip",
            ["Clip"],
            None,
            "{clip?: id, clips?: [id], at: position}; splits only the named clips (default: the selected ones) at `at`; refuses locked clips and a position outside every clip",
            always,
            split_clips
        ),
        cmd!(noundo "transport.locate_left", "Go to Left Locator", [], None, "{}", always, |e, _| {
            let at = e.session().edit.timeline_selection.start;
            transport::locate(e, at)
        }),
        cmd!(noundo "transport.locate_right", "Go to Right Locator", [], None, "{}", always, |e, _| {
            let at = e.session().edit.timeline_selection.end;
            transport::locate(e, at)
        }),
        cmd!(noundo "transport.set_left_locator", "Set Left Locator", [], None, "{at?: position}; defaults to the playhead; the right locator moves along when it would end up before it", always, |e, p| set_locator(e, p, true)),
        cmd!(noundo "transport.set_right_locator", "Set Right Locator", [], None, "{at?: position}; defaults to the playhead; the left locator moves along when it would end up after it", always, |e, p| set_locator(e, p, false)),
        cmd!(noundo "edit.locators_to_selection", "Locators to Selection", ["Edit", "Selection"], None, "{}; sets the timeline (loop) range to the edit selection or the selected clips", has_range_or_clips, |e, _| {
            let s = e.session_mut();
            let r = effective_range(s);
            s.edit.timeline_selection = r;
            Ok(json!({"range": r}))
        }),
        cmd!(noundo "track.select_prev", "Select Previous Track", ["Track"], None, "{}", has_tracks, |e, _| step_track(e, -1)),
        cmd!(noundo "track.select_next", "Select Next Track", ["Track"], None, "{}", has_tracks, |e, _| step_track(e, 1)),
        cmd!(noundo "clip.select_prev", "Select Previous Clip", ["Clip"], None, "{}", has_tracks, |e, _| step_clip(e, -1)),
        cmd!(noundo "clip.select_next", "Select Next Clip", ["Clip"], None, "{}", has_tracks, |e, _| step_clip(e, 1)),
    ]
}

fn set_locator(e: &mut Engine, p: &Value, left: bool) -> Result<Value> {
    let cmd = if left { "transport.set_left_locator" } else { "transport.set_right_locator" };
    let at = position_param(e, cmd, p, "at")?.unwrap_or_else(|| transport::cur(e)).max(0);
    let s = e.session_mut();
    let old = s.edit.timeline_selection;
    s.edit.timeline_selection = if left { Range::new(at, old.end.max(at)) } else { Range::new(old.start.min(at), at) };
    Ok(json!({"range": s.edit.timeline_selection}))
}

fn two_clips(e: &Engine) -> std::result::Result<(), String> {
    if clip_ids_param(e, &json!({})).len() >= 2 { Ok(()) } else { Err("select two or more clips".into()) }
}

fn has_range_or_clips(e: &Engine) -> std::result::Result<(), String> {
    let s = e.session();
    if !s.edit.selection.is_empty() || !s.edit.selected_clips.is_empty() {
        Ok(())
    } else {
        Err("make an edit selection or select clips first".into())
    }
}

// ---- glue ----------------------------------------------------------------------------------

fn glue(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let mut groups: Vec<(TrackId, Vec<Clip>)> = Vec::new();
    {
        let s = e.session();
        for id in ids {
            let Some((t, c)) = s.find_clip(id) else { continue };
            match groups.iter_mut().find(|(g, _)| *g == t) {
                Some((_, v)) => v.push(c.clone()),
                None => groups.push((t, vec![c.clone()])),
            }
        }
    }
    groups.retain(|(_, v)| v.len() >= 2);
    if groups.is_empty() {
        return Err(bad("clip.glue", "select two or more clips on one track"));
    }
    for (_, v) in &mut groups {
        v.sort_by_key(|c| c.start);
        check_glue(v)?;
    }
    let s = e.session_mut();
    let mut glued = 0;
    let mut selected = Vec::new();
    for (t, v) in &groups {
        let (Some(first), Some(last)) = (v.first(), v.last()) else { continue };
        let range = Range::new(first.start, v.iter().map(Clip::end).max().unwrap_or(last.end()));
        if first.is_audio() {
            crate::edit::heal(s, *t, range);
        } else {
            crate::edit::consolidate_midi(s, *t, range);
        }
        glued += 1;
        if let Some(c) = s.track(*t).and_then(|tr| tr.clips().iter().find(|c| c.start == range.start)) {
            selected.push(c.id);
        }
    }
    if !selected.is_empty() {
        s.edit.selected_clips = selected;
    }
    Ok(json!({"glued": glued}))
}

/// Refuse what glue cannot do without rendering: say why, change nothing.
fn check_glue(v: &[Clip]) -> Result<()> {
    let name = |c: &Clip| c.name.clone();
    if let Some(c) = v.iter().find(|c| crate::edit::locked(c)) {
        return Err(bad("clip.glue", format!("clip `{}` is locked", name(c))));
    }
    if v.iter().all(Clip::is_audio) {
        for pair in v.windows(2) {
            let [a, b] = pair else { continue };
            let continues = match (&a.content, &b.content) {
                (ClipContent::Audio { source: sa, offset: oa }, ClipContent::Audio { source: sb, offset: ob }) => {
                    sa == sb && oa.checked_add(a.length) == Some(*ob)
                }
                _ => false,
            };
            if a.end() != b.start || !continues || (a.gain_db - b.gain_db).abs() >= 1e-6 {
                return Err(bad(
                    "clip.glue",
                    format!(
                        "`{}` and `{}` are not contiguous parts of the same audio file with the same gain; use Consolidate to render them into one clip",
                        name(a),
                        name(b)
                    ),
                ));
            }
        }
        return Ok(());
    }
    if v.iter().all(|c| matches!(c.content, ClipContent::Midi { .. })) {
        if let Some(c) = v.iter().find(|c| c.muted) {
            return Err(bad("clip.glue", format!("clip `{}` is muted: unmute it first, or its notes would be lost", name(c))));
        }
        return Ok(());
    }
    Err(bad("clip.glue", "only audio clips or only MIDI clips can be glued together"))
}

// ---- mute, delete, split -----------------------------------------------------------------------

fn set_muted(e: &mut Engine, p: &Value, muted: bool) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut changed = 0;
    for id in &ids {
        if let Some(c) = s.find_clip_mut(*id)
            && c.muted != muted
        {
            c.muted = muted;
            changed += 1;
        }
    }
    Ok(json!({"muted": muted, "clips": ids.len(), "changed": changed}))
}

fn delete(e: &mut Engine, p: &Value) -> Result<Value> {
    let explicit = p.get("clips").is_some() || p.get("clip").is_some();
    if !explicit && e.session().edit.selected_clips.is_empty() {
        return edit::clear(e, p);
    }
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut removed = 0;
    for t in &mut s.tracks {
        if let Some(pl) = t.playlist_mut() {
            let before = pl.clips.len();
            pl.clips.retain(|c| !(ids.contains(&c.id) && !crate::edit::locked(c)));
            removed += before - pl.clips.len();
        }
    }
    s.edit.selected_clips.retain(|c| s.tracks.iter().any(|t| t.clips().iter().any(|x| x.id == *c)));
    Ok(json!({"removed": removed}))
}

fn split_at_playhead(e: &mut Engine, p: &Value) -> Result<Value> {
    let at = match position_param(e, "edit.split_at_playhead", p, "at")? {
        Some(at) => at,
        None => transport::cur(e),
    };
    let s = e.session();
    let mut tracks: Vec<TrackId> = s.edit.selected_clips.iter().filter_map(|c| s.find_clip(*c).map(|(t, _)| t)).collect();
    if tracks.is_empty() {
        tracks = e.target_tracks();
    }
    tracks.dedup();
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| crate::edit::separate_at(s, *t, at)).sum();
    Ok(json!({"separated": n, "at": at}))
}

/// Split the named clips (not every clip on their tracks) at one position.
fn split_clips(e: &mut Engine, p: &Value) -> Result<Value> {
    let Some(at) = position_param(e, "clip.split", p, "at")? else {
        return Err(bad("clip.split", "`at` is required"));
    };
    let ids = clip_ids_param(e, p);
    let mut targets: Vec<(TrackId, Vec<ClipId>)> = Vec::new();
    {
        let s = e.session();
        let mut inside = false;
        for id in &ids {
            let Some((t, c)) = s.find_clip(*id) else { continue };
            if at <= c.start || at >= c.end() {
                continue;
            }
            inside = true;
            if crate::edit::locked(c) {
                return Err(bad("clip.split", format!("clip `{}` is locked", c.name)));
            }
            match targets.iter_mut().find(|(g, _)| *g == t) {
                Some((_, v)) => v.push(*id),
                None => targets.push((t, vec![*id])),
            }
        }
        if !inside {
            return Err(bad("clip.split", "the position is not inside any of the clips"));
        }
    }
    let s = e.session_mut();
    let mut n = 0;
    for (t, v) in &targets {
        n += crate::edit::separate_clips_at(s, *t, at, |c| v.contains(&c.id));
    }
    Ok(json!({"separated": n, "at": at}))
}

// ---- previous / next selection ---------------------------------------------------------------------

fn shown_tracks(s: &soundcraft_model::Session) -> Vec<&Track> {
    s.tracks.iter().filter(|t| !t.hidden && t.kind != soundcraft_model::TrackKind::Master).collect()
}

fn step_track(e: &mut Engine, dir: i64) -> Result<Value> {
    let s = e.session();
    let shown: Vec<TrackId> = shown_tracks(s).iter().map(|t| t.id).collect();
    if shown.is_empty() {
        return Err(bad("track.select", "there are no tracks to select"));
    }
    let cur = s.edit.selected_tracks.first().and_then(|t| shown.iter().position(|x| x == t));
    let last = shown.len() - 1;
    let at = match (cur, dir < 0) {
        (None, true) => last,
        (None, false) => 0,
        (Some(i), true) => i.saturating_sub(1),
        (Some(i), false) => (i + 1).min(last),
    };
    let Some(id) = shown.get(at).copied() else { return Ok(json!({})) };
    let s = e.session_mut();
    s.edit.selected_tracks = vec![id];
    s.edit.selected_clips.clear();
    Ok(json!({"track": id}))
}

fn step_clip(e: &mut Engine, dir: i64) -> Result<Value> {
    let pos = transport::cur(e);
    let s = e.session();
    let from_clip = s.edit.selected_clips.first().and_then(|c| s.find_clip(*c)).map(|(t, c)| (t, Some(c.id)));
    let from_track = || s.edit.selected_tracks.first().filter(|t| s.track(**t).is_some()).map(|t| (*t, None));
    let Some((track, current)) = from_clip.or_else(from_track) else {
        return Err(bad("clip.select", "select a track or a clip first"));
    };
    let mut clips: Vec<(Samples, ClipId)> = s.track(track).map(|t| t.clips().iter().map(|c| (c.start, c.id)).collect()).unwrap_or_default();
    clips.sort_unstable();
    let idx = match current {
        Some(id) => clips.iter().position(|(_, c)| *c == id).and_then(|i| if dir < 0 { i.checked_sub(1) } else { Some(i + 1) }),
        None if dir < 0 => clips.iter().rposition(|(start, _)| *start < pos),
        None => clips.iter().position(|(start, _)| *start >= pos),
    };
    let target = idx.and_then(|i| clips.get(i)).map(|(_, c)| *c).or(current);
    let Some(id) = target else { return Ok(json!({"clip": null})) };
    let s = e.session_mut();
    s.edit.selected_tracks = vec![track];
    s.edit.selected_clips = vec![id];
    Ok(json!({"clip": id}))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, Source, SourceId, TrackKind};

    /// One mono track with a 60,000-frame source, cut into clips given as (start, length, offset).
    fn audio(parts: &[(Samples, Samples, Samples)]) -> (Engine, TrackId, Vec<ClipId>) {
        let mut s = soundcraft_model::Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        s.sources.push(Source {
            id: SourceId(500),
            name: "src".into(),
            path: "src.wav".into(),
            channels: 1,
            frames: 60_000,
            sample_rate: 48_000,
            format: soundcraft_audio_io::FileFormat::Wav,
            time_reference: 0,
            unsaved: false,
        });
        let mut ids = Vec::new();
        for (start, len, off) in parts {
            let id = s.new_clip_id();
            ids.push(id);
            crate::edit::place_clip(&mut s, t, Clip::audio(id, "Take", SourceId(500), *off, *start, *len));
        }
        (Engine::new(s), t, ids)
    }

    fn pick(e: &mut Engine, ids: &[ClipId]) {
        let raw: Vec<u64> = ids.iter().map(|c| c.0).collect();
        e.execute("edit.select", &json!({"clips": raw})).unwrap();
    }

    #[test]
    fn glue_joins_contiguous_audio_and_undoes() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1000, 500, 1000)]);
        pick(&mut e, &ids);
        assert_eq!(e.execute("clip.glue", &json!({})).unwrap()["glued"], 1);
        let clips = e.session().track(t).unwrap().clips().to_vec();
        assert_eq!(clips.iter().map(|c| (c.start, c.length)).collect::<Vec<_>>(), vec![(0, 1500)]);
        assert_eq!(e.session().edit.selected_clips, vec![clips[0].id]);
        assert!(e.undo());
        assert_eq!(e.session().track(t).unwrap().clips().len(), 2);
    }

    #[test]
    fn glue_refuses_a_gap_or_another_file_position_and_changes_nothing() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1200, 500, 1000)]);
        pick(&mut e, &ids);
        let err = e.execute("clip.glue", &json!({})).unwrap_err().to_string();
        assert!(err.contains("Consolidate"), "{err}");
        let (mut e2, t2, ids2) = audio(&[(0, 1000, 0), (1000, 500, 4000)]);
        pick(&mut e2, &ids2);
        assert!(e2.execute("clip.glue", &json!({})).is_err());
        assert_eq!(e.session().track(t).unwrap().clips().len(), 2);
        assert_eq!(e2.session().track(t2).unwrap().clips().len(), 2);
    }

    #[test]
    fn glue_needs_two_clips_and_merges_midi() {
        let (mut e, _, ids) = audio(&[(0, 1000, 0)]);
        pick(&mut e, &ids);
        assert!(e.execute("clip.glue", &json!({})).is_err());
        assert!(find_command("clip.glue").is_some_and(|c| (c.enabled)(&e).is_err()));

        let mut s = soundcraft_model::Session::default();
        let t = s.add_track(TrackKind::Instrument, ChannelFormat::Stereo, None);
        let q = soundcraft_time::TICKS_PER_QUARTER;
        let note = |start: i64| soundcraft_midi::Note { pitch: 60, velocity: 100, release_velocity: 64, channel: 0, start, length: q };
        let one = soundcraft_midi::Sequence { notes: vec![note(0)], ..Default::default() };
        let mut ids = Vec::new();
        for (i, start) in [0, 96_000].into_iter().enumerate() {
            let id = s.new_clip_id();
            ids.push(id);
            crate::edit::place_clip(&mut s, t, Clip::midi(id, format!("M{i}"), start, 48_000, one.clone()));
        }
        let mut e = Engine::new(s);
        pick(&mut e, &ids);
        e.execute("clip.glue", &json!({})).unwrap();
        let clips = e.session().track(t).unwrap().clips().to_vec();
        assert_eq!(clips.len(), 1);
        assert_eq!((clips[0].start, clips[0].end()), (0, 144_000));
        assert!(matches!(&clips[0].content, ClipContent::Midi { sequence } if sequence.notes.len() == 2));
    }

    #[test]
    fn mute_and_unmute_are_idempotent() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1000, 500, 1000)]);
        pick(&mut e, &ids);
        assert_eq!(e.execute("clip.mute", &json!({})).unwrap()["changed"], 2);
        assert_eq!(e.execute("clip.mute", &json!({})).unwrap()["changed"], 0);
        assert!(e.session().track(t).unwrap().clips().iter().all(|c| c.muted));
        assert_eq!(e.execute("clip.unmute", &json!({})).unwrap()["changed"], 2);
        assert_eq!(e.execute("clip.unmute", &json!({})).unwrap()["changed"], 0);
        assert!(e.session().track(t).unwrap().clips().iter().all(|c| !c.muted));
        e.execute("clip.mute", &json!({})).unwrap();
        assert!(e.undo());
        assert!(e.session().track(t).unwrap().clips().iter().all(|c| !c.muted));
    }

    #[test]
    fn delete_removes_selected_clips_without_closing_the_gap_and_skips_locked() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1000, 500, 1000), (2000, 500, 2000)]);
        pick(&mut e, &ids[..2]);
        e.session_mut().find_clip_mut(ids[1]).unwrap().edit_locked = true;
        assert_eq!(e.execute("edit.delete", &json!({})).unwrap()["removed"], 1);
        let starts: Vec<_> = e.session().track(t).unwrap().clips().iter().map(|c| c.start).collect();
        assert_eq!(starts, vec![1000, 2000], "later clips stay where they were");
        assert!(e.undo());
        assert_eq!(e.session().track(t).unwrap().clips().len(), 3);
    }

    #[test]
    fn split_at_playhead_splits_selected_clips_and_ignores_the_others() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1000, 1000, 1000)]);
        pick(&mut e, &ids[..1]);
        e.execute("transport.locate", &json!({"at": 400})).unwrap();
        assert_eq!(e.execute("edit.split_at_playhead", &json!({})).unwrap()["separated"], 1);
        let starts: Vec<_> = e.session().track(t).unwrap().clips().iter().map(|c| c.start).collect();
        assert_eq!(starts, vec![0, 400, 1000]);
        assert!(e.undo());
        assert_eq!(e.session().track(t).unwrap().clips().len(), 2);
    }

    #[test]
    fn split_clips_splits_only_the_named_clip_and_undoes() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1000, 1000, 5000)]);
        let id0 = ids[0].0;
        assert_eq!(e.execute("clip.split", &json!({"clip": id0, "at": 700})).unwrap()["separated"], 1);
        let starts: Vec<_> = e.session().track(t).unwrap().clips().iter().map(|c| c.start).collect();
        assert_eq!(starts, vec![0, 700, 1000], "only the named clip is split");
        assert!(e.undo());
        assert_eq!(e.session().track(t).unwrap().clips().len(), 2);
    }

    #[test]
    fn split_clips_refuses_locked_and_outside_positions() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0)]);
        let id0 = ids[0].0;
        assert!(e.execute("clip.split", &json!({"clip": id0, "at": 1500})).is_err());
        assert!(e.execute("clip.split", &json!({"clip": id0})).is_err());
        e.session_mut().find_clip_mut(ids[0]).unwrap().edit_locked = true;
        assert!(e.execute("clip.split", &json!({"clip": id0, "at": 400})).is_err());
        assert_eq!(e.session().track(t).unwrap().clips().len(), 1);
    }

    #[test]
    fn glue_mute_and_delete_accept_explicit_clips_without_a_selection() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1000, 500, 1000)]);
        let raw: Vec<u64> = ids.iter().map(|c| c.0).collect();
        e.execute("clip.mute", &json!({"clips": [raw[0]]})).unwrap();
        assert!(e.session().track(t).unwrap().clips().iter().any(|c| c.muted));
        e.execute("clip.unmute", &json!({"clips": [raw[0]]})).unwrap();
        assert_eq!(e.execute("clip.glue", &json!({"clips": raw})).unwrap()["glued"], 1);
        assert_eq!(e.session().track(t).unwrap().clips().len(), 1);
    }

    #[test]
    fn locators_and_neighbour_selection() {
        let (mut e, t, ids) = audio(&[(0, 1000, 0), (1000, 500, 1000), (2000, 500, 2000)]);
        e.execute("edit.select", &json!({"tracks": [t.0], "start": 100, "end": 900})).unwrap();
        e.execute("edit.locators_to_selection", &json!({})).unwrap();
        e.execute("transport.locate_right", &json!({})).unwrap();
        assert_eq!(e.session().edit.playhead, 900);
        e.execute("transport.locate_left", &json!({})).unwrap();
        assert_eq!(e.session().edit.playhead, 100);
        pick(&mut e, &ids[..1]);
        e.execute("clip.select_next", &json!({})).unwrap();
        assert_eq!(e.session().edit.selected_clips, vec![ids[1]]);
        e.execute("clip.select_next", &json!({})).unwrap();
        e.execute("clip.select_next", &json!({})).unwrap();
        assert_eq!(e.session().edit.selected_clips, vec![ids[2]], "stops at the last clip");
        e.execute("clip.select_prev", &json!({})).unwrap();
        assert_eq!(e.session().edit.selected_clips, vec![ids[1]]);
    }

    #[test]
    fn locators_can_be_set_one_at_a_time() {
        let (mut e, _, _) = audio(&[(0, 1000, 0)]);
        e.execute("transport.set_left_locator", &json!({"at": 200})).unwrap();
        e.execute("transport.set_right_locator", &json!({"at": 800})).unwrap();
        assert_eq!(e.session().edit.timeline_selection, Range::new(200, 800));
        e.execute("transport.set_left_locator", &json!({"at": 900})).unwrap();
        assert_eq!(e.session().edit.timeline_selection, Range::new(900, 900), "the right locator never ends before the left");
    }

    #[test]
    fn select_prev_next_track_walks_the_shown_tracks() {
        let mut e = crate::demo::demo_engine();
        e.execute("edit.select_none", &json!({})).unwrap();
        e.execute("track.select_next", &json!({})).unwrap();
        let first = e.session().edit.selected_tracks.clone();
        assert_eq!(first.len(), 1);
        e.execute("track.select_next", &json!({})).unwrap();
        assert_ne!(e.session().edit.selected_tracks, first);
        e.execute("track.select_prev", &json!({})).unwrap();
        assert_eq!(e.session().edit.selected_tracks, first);
        e.execute("track.select_prev", &json!({})).unwrap();
        assert_eq!(e.session().edit.selected_tracks, first, "clamps at the top");
    }
}
