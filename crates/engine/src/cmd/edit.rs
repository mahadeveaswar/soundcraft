//! Edit menu commands.

use super::more_util::toggle_flag;
use super::*;
use crate::edit as ops;
use crate::{Clipboard, cmd};
use serde_json::json;
use soundcraft_model::{EditMode, Fade, FadeShape, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(noundo "edit.undo", "Undo", ["Edit"], Some("Cmd+Z"), "{}", can_undo, |e, _| Ok(json!({"undone": e.undo()}))),
        cmd!(noundo "edit.redo", "Redo", ["Edit"], Some("Cmd+Shift+Z"), "{}", can_redo, |e, _| Ok(json!({"redone": e.redo()}))),
        cmd!(noundo "edit.restore_last_selection", "Restore Last Selection", ["Edit"], Some("Cmd+Alt+Z"), "{}", has_last_selection, restore_selection),
        cmd!("edit.cut", "Cut", ["Edit"], Some("Cmd+X"), "{tracks?, start?, end?}", has_range, |e, p| cut_copy(e, p, true, "edit.cut")),
        cmd!(noundo "edit.copy", "Copy", ["Edit"], Some("Cmd+C"), "{tracks?, start?, end?}", has_range, |e, p| cut_copy(e, p, false, "edit.copy")),
        cmd!("edit.paste", "Paste", ["Edit"], Some("Cmd+V"), "{tracks?, at?}", has_clipboard, paste),
        cmd!("edit.clear", "Clear", ["Edit"], Some("Cmd+B"), "{tracks?, start?, end?}", has_range, clear),
        cmd!("edit.cut_clip_gain", "Clip Gain", ["Edit", "Cut Special"], Some("Cmd+Ctrl+X"), "{}", has_range, |e, p| {
            super::edit_more::copy_clip_gain(e, p)?;
            reset_clip_gain(e, p, "edit.cut_clip_gain")
        }),
        cmd!("edit.clear_clip_gain", "Clip Gain", ["Edit", "Clear Special"], Some("Cmd+Ctrl+B"), "{}", has_range, |e, p| reset_clip_gain(
            e,
            p,
            "edit.clear_clip_gain"
        )),
        cmd!("edit.cut_all_automation", "All Automation", ["Edit", "Cut Special"], None, "{tracks?, start?, end?}", has_range, |e, p| {
            super::edit_more::cut_all_automation(e, p)
        }),
        cmd!("edit.clear_all_automation", "All Automation", ["Edit", "Clear Special"], None, "{}", has_range, |e, p| clear_automation(e, p, None)),
        cmd!("edit.clear_pan_automation", "Pan Automation", ["Edit", "Clear Special"], None, "{}", has_range, |e, p| clear_automation(
            e,
            p,
            Some("pan")
        )),
        cmd!("edit.clear_plugin_automation", "Plugin Automation", ["Edit", "Clear Special"], None, "{}", has_range, |e, p| clear_automation(
            e,
            p,
            Some("plugin")
        )),
        cmd!(
            "edit.paste_repeat_to_fill",
            "Repeat to Fill Selection",
            ["Edit", "Paste Special"],
            Some("Cmd+Alt+V"),
            "{}",
            has_clipboard,
            repeat_to_fill
        ),
        cmd!(noundo "edit.select_all", "Select All", ["Edit"], Some("Cmd+A"), "{}", always, select_all),
        cmd!(noundo "edit.select", "Set Edit Selection", [], None, "{tracks?: [id|name], start?, end?, clips?: [id], sources?: [id] (Clip List files), exact?: bool (ignore edit groups)}", always, select),
        cmd!(noundo "edit.select_none", "Deselect All", [], None, "{}", always, |e, _| {
            let s = e.session_mut();
            s.edit.selected_tracks.clear();
            s.edit.selected_clips.clear();
            s.edit.selected_sources.clear();
            s.edit.selection.end = s.edit.selection.start;
            Ok(json!({}))
        }),
        cmd!(noundo "edit.selection_to_timeline", "Change Timeline to Match Edit", ["Edit", "Selection"], Some("Alt+Shift+5"), "{}", always, |e, _| {
            let s = e.session_mut();
            s.edit.timeline_selection = s.edit.selection;
            Ok(json!({}))
        }),
        cmd!(noundo "edit.timeline_to_selection", "Change Edit to Match Timeline", ["Edit", "Selection"], Some("Alt+Shift+6"), "{}", always, |e, _| {
            let s = e.session_mut();
            s.edit.selection = s.edit.timeline_selection;
            Ok(json!({}))
        }),
        cmd!(noundo "edit.move_selection_left", "Move Edit Left", ["Edit", "Selection"], Some("Cmd+Alt+L"), "{}", always, |e, _| shift_selection(e, -1)),
        cmd!(noundo "edit.move_selection_right", "Move Edit Right", ["Edit", "Selection"], Some("Cmd+Alt+'"), "{}", always, |e, _| shift_selection(e, 1)),
        cmd!(noundo "edit.halve_selection", "Halve Edit", ["Edit", "Selection"], None, "{}", always, |e, _| scale_selection(e, 0.5)),
        cmd!(noundo "edit.double_selection", "Double Edit", ["Edit", "Selection"], None, "{}", always, |e, _| scale_selection(e, 2.0)),
        cmd!(noundo "edit.extend_selection_up", "Extend Edit Up", ["Edit", "Selection"], Some("Ctrl+Shift+P"), "{}", has_tracks, |e, _| extend_tracks(e, -1)),
        cmd!(noundo "edit.extend_selection_down", "Extend Edit Down", ["Edit", "Selection"], Some("Ctrl+Shift+;"), "{}", has_tracks, |e, _| extend_tracks(e, 1)),
        cmd!(noundo "edit.remove_selection_top", "Remove Edit from Top", ["Edit", "Selection"], None, "{}", has_tracks, |e, _| {
            let s = e.session_mut();
            if !s.edit.selected_tracks.is_empty() {
                s.edit.selected_tracks.remove(0);
            }
            Ok(json!({}))
        }),
        cmd!(noundo "edit.remove_selection_bottom", "Remove Edit from Bottom", ["Edit", "Selection"], None, "{}", has_tracks, |e, _| {
            e.session_mut().edit.selected_tracks.pop();
            Ok(json!({}))
        }),
        cmd!("edit.duplicate", "Duplicate", ["Edit"], Some("Cmd+D"), "{tracks?, start?, end?}", has_range, duplicate),
        cmd!("edit.repeat", "Repeat...", ["Edit"], Some("Alt+R"), "{count: n}", has_range, repeat),
        cmd!("edit.shift", "Shift...", ["Edit"], Some("Alt+H"), "{by: samples|{seconds}, later?: bool}", has_range, shift),
        cmd!("edit.insert_silence", "Insert Silence", ["Edit"], Some("Cmd+Shift+E"), "{tracks?, start?, end?}", has_range, insert_silence),
        cmd!("edit.separate", "At Selection", ["Edit", "Separate"], Some("Cmd+E"), "{tracks?, at?|start/end}", has_selection, separate),
        cmd!("edit.separate_on_grid", "On Grid", ["Edit", "Separate"], None, "{tracks?, start?, end?}", has_range, separate_on_grid),
        cmd!("edit.separate_at_transients", "At Transients", ["Edit", "Separate"], None, "{sensitivity?: 0..1}", has_range, separate_at_transients),
        cmd!("edit.heal", "Heal Separation", ["Edit"], Some("Cmd+H"), "{tracks?, start?, end?}", has_selection, heal),
        cmd!("edit.trim_to_selection", "To Selection", ["Edit", "Trim"], Some("Cmd+T"), "{tracks?, start?, end?}", has_range, trim_to_selection),
        cmd!("edit.trim_start_to_insertion", "Start to Insertion", ["Edit", "Trim"], Some("Alt+Shift+7"), "{tracks?, at?}", has_selection, |e, p| {
            trim_edge(e, p, true)
        }),
        cmd!("edit.trim_end_to_insertion", "End to Insertion", ["Edit", "Trim"], Some("Alt+Shift+8"), "{tracks?, at?}", has_selection, |e, p| {
            trim_edge(e, p, false)
        }),
        cmd!("edit.trim_to_file_boundaries", "To File Boundaries", ["Edit", "Trim"], None, "{clips?}", has_selection, trim_to_file),
        cmd!("edit.trim_to_fill_selection", "To Fill Selection", ["Edit", "Trim"], None, "{}", has_range, trim_to_fill),
        cmd!("edit.mute_clips", "Mute", ["Edit"], Some("Cmd+M"), "{clips?}", has_selection, mute_clips),
        cmd!(
            "edit.copy_to_new_playlist",
            "New Playlist",
            ["Edit", "Copy Selection to..."],
            Some("Alt+Shift+Down"),
            "{}",
            has_range,
            copy_to_new_playlist
        ),
        cmd!(
            "edit.strip_silence",
            "Strip Silence",
            ["Edit"],
            Some("Cmd+U"),
            "{threshold_db?: -48, min_length_ms?: 50, pad_before_ms?: 5, pad_after_ms?: 20}",
            has_range,
            strip_silence
        ),
        cmd!(
            "edit.fades_create",
            "Create...",
            ["Edit", "Fades"],
            Some("Cmd+F"),
            "{shape?: linear|equal power|s-curve|exponential|logarithmic, tracks?, start?, end?} or {clip: id, side: in|out, length: samples, shape?, curvature?: -1..1}",
            has_range,
            fades_create
        ),
        cmd!("edit.fades_delete", "Delete", ["Edit", "Fades"], None, "{tracks?, start?, end?}", has_selection, fades_delete),
        cmd!("edit.fade_to_start", "Fade to Start", ["Edit", "Fades"], Some("Cmd+Ctrl+D"), "{}", has_selection, |e, p| fade_to(e, p, true)),
        cmd!("edit.fade_to_end", "Fade to End", ["Edit", "Fades"], Some("Cmd+Ctrl+G"), "{}", has_selection, |e, p| fade_to(e, p, false)),
        cmd!("edit.nudge", "Nudge", [], Some("Plus/Minus"), "{direction: 1|-1, count?: n, next?: bool (next larger value)}", has_selection, nudge),
        cmd!(
            noundo "edit.mode",
            "Edit Mode",
            [],
            Some("F1-F4, Alt+1-4, `"),
            "{mode: shuffle|slip|spot|grid|relative_grid, snap_to_grid?: bool (Shuffle, Slip and Spot)}",
            always,
            set_mode
        ),
        cmd!(
            noundo "edit.snap_to_grid",
            "Snap to Grid",
            [],
            Some("Shift+F4"),
            "{value?: bool} (toggles when omitted) — in Shuffle, Slip and Spot the Edit cursor and selections snap to the grid",
            always,
            snap_to_grid
        ),
        cmd!(
            noundo "edit.shuffle_lock",
            "Shuffle Lock",
            [],
            None,
            "{value?: bool} (toggles when omitted) — keeps Shuffle mode from being chosen",
            always,
            shuffle_lock
        ),
        cmd!(noundo "edit.tool", "Edit Tool", [], Some("F5-F10"), "{tool: zoom|trim|selector|grabber|scrubber|pencil|smart}", always, |e, p| {
            let t = str_param(p, "tool").and_then(soundcraft_model::Tool::from_id).ok_or_else(|| bad("edit.tool", "unknown tool"))?;
            e.session_mut().edit.tool = t;
            Ok(json!({"tool": t}))
        }),
        cmd!(
            "edit.move_clips",
            "Move Clips",
            [],
            None,
            "{clips?: [id], by?: samples, to?: position for the first clip that can move (Grid modes snap its sync point), track?: where the first clip's track goes; the others keep their distance}",
            has_selection,
            move_clips
        ),
        cmd!(
            "edit.trim_clip",
            "Trim Clip",
            [],
            None,
            "{clip: id, start?: position, end?: position} — moves the clip's edges like the Trim tool, in the current Edit mode",
            always,
            trim_clip
        ),
        cmd!("edit.consolidate", "Consolidate", ["Edit"], Some("Alt+Shift+3"), "{tracks?, start?, end?}", has_range, consolidate),
    ]
}

fn shuffle(s: &Session) -> bool {
    s.edit.edit_mode == EditMode::Shuffle
}

fn set_mode(e: &mut Engine, p: &Value) -> Result<Value> {
    let m = str_param(p, "mode").and_then(EditMode::from_id).ok_or_else(|| bad("edit.mode", "mode must be shuffle|slip|spot|grid|relative_grid"))?;
    if m == EditMode::Shuffle && e.session().edit.flag(ops::SHUFFLE_LOCK) {
        return Err(bad("edit.mode", "Shuffle is locked"));
    }
    let snap = bool_or(p, "snap_to_grid", false) && !m.is_grid();
    let ed = &mut e.session_mut().edit;
    ed.edit_mode = m;
    ed.set_flag(ops::SNAP_TO_GRID, snap);
    if m.is_grid() {
        ed.set_flag(ops::RELATIVE_GRID, m == EditMode::GridRelative);
    }
    Ok(json!({"mode": m, "snap_to_grid": snap}))
}

fn snap_to_grid(e: &mut Engine, p: &Value) -> Result<Value> {
    if e.session().edit.edit_mode.is_grid() {
        return Err(bad("edit.snap_to_grid", "Grid mode always snaps"));
    }
    toggle_flag(e, p, ops::SNAP_TO_GRID)
}

fn shuffle_lock(e: &mut Engine, p: &Value) -> Result<Value> {
    if shuffle(e.session()) && bool_or(p, "value", true) {
        return Err(bad("edit.shuffle_lock", "leave Shuffle mode before locking it"));
    }
    toggle_flag(e, p, ops::SHUFFLE_LOCK)
}

fn has_last_selection(e: &Engine) -> std::result::Result<(), String> {
    if e.last_selection.is_some() { Ok(()) } else { Err("no previous selection to restore".into()) }
}

/// Remember the selection `before` had when a command changed it, for Restore Last Selection.
pub(crate) fn remember_selection(e: &mut Engine, before: &Session) {
    let (old, new) = (&before.edit, &e.session().edit);
    if old.selection == new.selection && old.selected_tracks == new.selected_tracks {
        return;
    }
    e.last_selection = Some((old.selection, old.selected_tracks.clone()));
}

/// Swaps in the previous selection; `remember_selection` then keeps the one replaced, so
/// invoking it again goes back.
fn restore_selection(e: &mut Engine, _: &Value) -> Result<Value> {
    let (range, mut tracks) = e.last_selection.clone().ok_or_else(|| bad("edit.restore_last_selection", "no previous selection to restore"))?;
    tracks.retain(|t| e.session().track(*t).is_some());
    let ed = &mut e.session_mut().edit;
    ed.selection = range;
    ed.selected_tracks = tracks;
    Ok(json!({"selection": range}))
}

fn cut_copy(e: &mut Engine, p: &Value, cut: bool, id: &str) -> Result<Value> {
    let tracks = tracks_required(e, id, p)?;
    let range = range_param(e, id, p)?;
    if range.is_empty() {
        return Err(bad(id, "selection is empty"));
    }
    // Only the selected clips are taken when they alone define the range (no band, no range params).
    let only = clip_mode(e, p);
    let mut cb = Clipboard::of_range(e.session(), range);
    let follow = e.session().edit.automation_follows_edit;
    for t in &tracks {
        let mut copied = ops::copy_range(e.session(), *t, range);
        if let Some(ids) = &only {
            copied.retain(|c| ids.contains(&c.id));
        }
        cb.tracks.push(copied);
        let lanes = e
            .session()
            .track(*t)
            .map(|tr| {
                tr.automation
                    .iter()
                    .map(|l| {
                        let mut l2 = l.clone();
                        l2.points = l
                            .points
                            .iter()
                            .filter(|p| range.contains(p.at))
                            .map(|p| soundcraft_model::AutomationPoint { at: p.at - range.start, value: p.value })
                            .collect();
                        l2
                    })
                    .collect()
            })
            .unwrap_or_default();
        cb.automation.push(lanes);
    }
    cb.markers = e
        .session()
        .markers
        .iter()
        .filter(|m| m.kind == soundcraft_model::MarkerKind::Marker && range.contains(m.start))
        .map(|m| {
            let mut m = m.clone();
            m.end -= m.start;
            m.start -= range.start;
            m.end += m.start;
            m
        })
        .collect();
    if cut {
        let sh = shuffle(e.session());
        let s = e.session_mut();
        match &only {
            Some(ids) => cut_clips(s, ids, sh),
            None => {
                for t in &tracks {
                    cut_track(s, *t, range, sh, follow);
                }
            }
        }
        s.edit.selection = soundcraft_time::Range::point(range.start);
        s.edit.selected_clips.clear();
    }
    let n: usize = cb.tracks.iter().map(Vec::len).sum();
    cb.notes = e.clipboard.notes.take();
    e.clipboard = cb;
    Ok(json!({"clips": n, "length": range.len()}))
}

/// Remove exactly the clips `ids` (later ones first, so a Shuffle ripple does not move the rest). A
/// clip that overlaps an unselected one is just removed; otherwise Shuffle closes its gap.
fn cut_clips(s: &mut Session, ids: &[soundcraft_model::ClipId], sh: bool) {
    let mut found: Vec<(TrackId, soundcraft_time::Range, soundcraft_model::ClipId)> = Vec::new();
    for t in &s.tracks {
        for c in t.clips().iter().filter(|c| ids.contains(&c.id)) {
            found.push((t.id, c.range(), c.id));
        }
    }
    found.sort_by_key(|(_, r, _)| std::cmp::Reverse(r.start));
    for (t, r, id) in found {
        if let Some(pl) = s.track_mut(t).and_then(|tr| tr.playlist_mut()) {
            pl.clips.retain(|c| c.id != id);
        }
        let shared = s.track(t).is_some_and(|tr| tr.clips().iter().any(|c| c.range().overlaps(&r)));
        if sh && !shared {
            ops::clear_range(s, t, r, true);
        }
    }
}

/// Remove `r` from a track, closing the gap in Shuffle mode; automation follows when `follow`.
fn cut_track(s: &mut Session, t: TrackId, r: soundcraft_time::Range, sh: bool, follow: bool) {
    if follow && let Some(tr) = s.track_mut(t) {
        let locked = crate::edit::locked_params(tr);
        for l in tr.automation.iter_mut().filter(|l| !locked.contains(&l.param)) {
            l.clear_range(r.start, r.end);
        }
    }
    ops::clear_range(s, t, r, sh);
}

fn paste(e: &mut Engine, p: &Value) -> Result<Value> {
    let mut tracks = tracks_required(e, "edit.paste", p)?;
    let given = position_param(e, "edit.paste", p, "at")?;
    let cb = e.clipboard.clone();
    if cb.tracks.is_empty() {
        return Err(bad("edit.paste", "clipboard is empty"));
    }
    let s = e.session();
    // More clipboard tracks than targets: continue downward from the first target in track order.
    if tracks.len() < cb.tracks.len() {
        let order: Vec<TrackId> = s.tracks.iter().map(|t| t.id).collect();
        tracks.sort_by_key(|t| order.iter().position(|o| o == t).unwrap_or(usize::MAX));
        if let Some(first) = tracks.first().and_then(|t| order.iter().position(|o| o == t)) {
            let more = order[first..].iter().filter(|t| !tracks.contains(t)).take(cb.tracks.len() - tracks.len());
            let more: Vec<TrackId> = more.copied().collect();
            tracks.extend(more);
        }
    }
    // A track takes only clips of its own kind (a MIDI clip does not go onto an audio track).
    let fits = |t: &TrackId, clips: &[soundcraft_model::Clip]| {
        let midi = s.track(*t).is_some_and(|tr| tr.kind.is_midi());
        clips.iter().all(|c| matches!(c.content, soundcraft_model::ClipContent::Midi { .. }) == midi)
    };
    let pairs: Vec<bool> = tracks.iter().enumerate().map(|(i, t)| cb.tracks.get(i % cb.tracks.len()).is_some_and(|c| fits(t, c))).collect();
    if !pairs.iter().any(|ok| *ok) && !cb.automation_only && cb.tracks.iter().any(|c| !c.is_empty()) {
        let has_midi = cb.tracks.iter().flatten().any(|c| matches!(c.content, soundcraft_model::ClipContent::Midi { .. }));
        let target_midi = tracks.first().and_then(|t| s.track(*t)).is_some_and(|tr| tr.kind.is_midi());
        let (held, target) = (if has_midi { "MIDI" } else { "audio" }, if target_midi { "a MIDI track" } else { "an audio track" });
        return Err(bad("edit.paste", format!("clipboard holds {held} clips; target is {target}")));
    }
    let at = match given {
        Some(at) => at,
        None if s.edit.edit_mode == EditMode::GridRelative => grid_adapt::effective_grid(s, s.edit.selection.start)
            .line_at_or_before(s.edit.selection.start, s.sample_rate, &s.tempo, s.frame_rate)
            .saturating_add(cb.grid_offset),
        None => s.edit.selection.start,
    };
    let sh = shuffle(s) && !cb.automation_only;
    let follow = s.edit.automation_follows_edit;
    let replace = Some(s.edit.selection).filter(|r| sh && given.is_none() && !r.is_empty());
    let s = e.session_mut();
    let mut pasted = 0;
    let mut new_ids = Vec::new();
    for (i, t) in tracks.iter().enumerate() {
        let Some(clips) = cb.tracks.get(i % cb.tracks.len().max(1)) else { continue };
        if !pairs.get(i).copied().unwrap_or(false) && !clips.is_empty() {
            continue;
        }
        if let Some(r) = replace {
            cut_track(s, *t, r, true, follow);
        }
        let ids = ops::paste(s, *t, at, clips, cb.length, sh).map_err(|why| bad("edit.paste", why))?;
        pasted += ids.len();
        new_ids.extend(ids);
        if follow && let (Some(lanes), Some(tr)) = (cb.automation.get(i % cb.automation.len().max(1)), s.track_mut(*t)) {
            let locked = crate::edit::locked_params(tr);
            for l in lanes.iter().filter(|l| !locked.contains(&l.param)) {
                let dst = tr.lane_mut(&l.param);
                if !l.points.is_empty() {
                    dst.clear_range(at, at + cb.length);
                }
                for pt in &l.points {
                    dst.set_point(pt.at + at, pt.value);
                }
            }
        }
    }
    s.edit.selection = soundcraft_time::Range::new(at, at + cb.length);
    s.edit.selected_clips = new_ids;
    Ok(json!({"pasted": pasted}))
}

pub(super) fn clear(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.clear", p)?;
    let range = range_param(e, "edit.clear", p)?;
    let ids = clip_ids_param(e, p);
    let sh = shuffle(e.session());
    let s = e.session_mut();
    let mut n = 0;
    if range.is_empty() && !ids.is_empty() {
        for t in &mut s.tracks {
            if let Some(pl) = t.playlist_mut() {
                let before = pl.clips.len();
                pl.clips.retain(|c| !ids.contains(&c.id));
                n += before - pl.clips.len();
            }
        }
        s.edit.selected_clips.clear();
    } else {
        for t in &tracks {
            n += ops::clear_range(s, *t, range, sh).len();
        }
    }
    Ok(json!({"removed": n}))
}

fn reset_clip_gain(e: &mut Engine, p: &Value, _id: &str) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    for id in &ids {
        if let Some(c) = s.find_clip_mut(*id) {
            c.gain_db = 0.0;
            c.gain_env.clear();
        }
    }
    Ok(json!({"clips": ids.len()}))
}

fn clear_automation(e: &mut Engine, p: &Value, which: Option<&str>) -> Result<Value> {
    let tracks = tracks_required(e, "edit.clear_automation", p)?;
    let r = range_param(e, "edit.clear_automation", p)?;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        if let Some(tr) = s.track_mut(*t) {
            let locked = crate::edit::locked_params(tr);
            for l in tr.automation.iter_mut().filter(|l| !locked.contains(&l.param)) {
                let hit = match which {
                    None => true,
                    Some("pan") => matches!(l.param, soundcraft_model::AutoParam::Pan(_)),
                    Some(_) => matches!(l.param, soundcraft_model::AutoParam::Plugin { .. }),
                };
                if hit {
                    n += l.clear_range(r.start, r.end);
                }
            }
        }
    }
    Ok(json!({"points_removed": n}))
}

fn repeat_to_fill(e: &mut Engine, p: &Value) -> Result<Value> {
    let range = range_param(e, "edit.paste_repeat_to_fill", p)?;
    let len = e.clipboard.length.max(1);
    let tracks = tracks_required(e, "edit.paste_repeat_to_fill", p)?;
    let cb = e.clipboard.clone();
    let s = e.session_mut();
    let mut at = range.start;
    let mut n = 0;
    while at < range.end && n < 10_000 {
        for (i, t) in tracks.iter().enumerate() {
            if let Some(clips) = cb.tracks.get(i % cb.tracks.len().max(1)) {
                let trimmed: Vec<_> = clips
                    .iter()
                    .filter(|c| at + c.start < range.end)
                    .map(|c| {
                        let mut c = c.clone();
                        if at + c.end() > range.end {
                            c.trim_end_to(range.end - at);
                        }
                        c
                    })
                    .collect();
                let _ = ops::paste(s, *t, at, &trimmed, (range.end - at).min(len), false);
            }
        }
        at += len;
        n += 1;
    }
    Ok(json!({"repeats": n}))
}

fn select_all(e: &mut Engine, _: &Value) -> Result<Value> {
    let s = e.session_mut();
    s.edit.selected_tracks = s.tracks.iter().filter(|t| !t.hidden).map(|t| t.id).collect();
    let end = s.content_end();
    s.edit.selection = soundcraft_time::Range::new(0, end);
    Ok(json!({"tracks": s.edit.selected_tracks.len(), "end": end}))
}

fn select(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = if p.get("tracks").is_some() || p.get("track").is_some() { Some(tracks_param(e, "edit.select", p)?) } else { None };
    let st = position_param(e, "edit.select", p, "start")?;
    let en = position_param(e, "edit.select", p, "end")?;
    let clips =
        p.get("clips").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(soundcraft_model::ClipId).collect::<Vec<_>>());
    let sources =
        p.get("sources").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(soundcraft_model::SourceId).collect::<Vec<_>>());
    let list_changes = clips.is_some() || sources.is_some() || st.is_some();
    let exact = bool_or(p, "exact", false);
    let s = e.session();
    let mut ed = s.edit.clone();
    if let Some(mut t) = tracks {
        // Active edit groups select all their members together.
        if !exact {
            let extra: Vec<soundcraft_model::TrackId> =
                s.groups.iter().filter(|g| g.active && g.edit && g.members.iter().any(|m| t.contains(m))).flat_map(|g| g.members.clone()).collect();
            for m in extra {
                if !t.contains(&m) {
                    t.push(m);
                }
            }
            let order: Vec<soundcraft_model::TrackId> = s.tracks.iter().map(|x| x.id).collect();
            t.sort_by_key(|x| order.iter().position(|o| o == x).unwrap_or(usize::MAX));
        }
        ed.selected_tracks = t;
    }
    match (st, en) {
        (Some(a), Some(b)) => ed.selection = soundcraft_time::Range::new(a.max(0), b.max(0)),
        (Some(a), None) => ed.selection = soundcraft_time::Range::point(a.max(0)),
        _ => {}
    }
    if let Some(c) = clips {
        // Selecting clips also selects their tracks (Grabber click), not their range.
        let mut range: Option<soundcraft_time::Range> = None;
        let mut trs = Vec::new();
        for id in &c {
            if let Some((tid, clip)) = s.find_clip(*id) {
                range = Some(range.map_or(clip.range(), |r| soundcraft_time::Range::new(r.start.min(clip.start), r.end.max(clip.end()))));
                if !trs.contains(&tid) {
                    trs.push(tid);
                }
            }
        }
        // The edit selection (locators) is left alone: it is set only by range edits, markers or explicit start/end.
        if range.is_some() {
            ed.selected_tracks = trs;
        }
        ed.selected_clips = c;
    } else if list_changes {
        ed.selected_clips.clear();
    }
    if list_changes {
        let mut known: std::collections::BTreeSet<_> = s.sources.iter().map(|f| f.id).collect();
        ed.selected_sources = sources.unwrap_or_default().into_iter().filter(|id| known.remove(id)).collect();
    }
    if ed.link_timeline_edit {
        ed.timeline_selection = ed.selection;
    }
    let out = json!({"selection": ed.selection, "tracks": ed.selected_tracks, "clips": ed.selected_clips, "sources": ed.selected_sources});
    if ed != s.edit {
        e.session_mut().edit = ed;
    }
    Ok(out)
}

fn shift_selection(e: &mut Engine, dir: i64) -> Result<Value> {
    let s = e.session_mut();
    let len = s.edit.selection.len().max(1);
    s.edit.selection = s.edit.selection.shifted(dir * len);
    if s.edit.selection.start < 0 {
        s.edit.selection = s.edit.selection.shifted(-s.edit.selection.start);
    }
    Ok(json!({"selection": s.edit.selection}))
}

fn scale_selection(e: &mut Engine, f: f64) -> Result<Value> {
    let s = e.session_mut();
    let len = soundcraft_time::to_samples(s.edit.selection.len() as f64 * f);
    s.edit.selection.end = s.edit.selection.start.saturating_add(len);
    Ok(json!({"selection": s.edit.selection}))
}

/// Adds the nearest track above (`dir` < 0) or below the selected ones that the Edit window shows.
fn extend_tracks(e: &mut Engine, dir: i64) -> Result<Value> {
    let s = e.session();
    let shown = |t: &&soundcraft_model::Track| !t.hidden && t.folder.and_then(|f| s.track(f)).is_none_or(|f| f.folder_open);
    let idxs: Vec<usize> = s.edit.selected_tracks.iter().filter_map(|t| s.track_index(*t)).collect();
    let next = match (idxs.iter().min(), idxs.iter().max()) {
        (Some(&top), _) if dir < 0 => s.tracks.iter().take(top).rev().find(shown),
        (_, Some(&bottom)) if dir > 0 => s.tracks.iter().skip(bottom + 1).find(shown),
        (None, _) => s.tracks.iter().find(shown),
        _ => None,
    }
    .map(|t| t.id);
    let s = e.session_mut();
    if let Some(t) = next {
        if dir < 0 {
            s.edit.selected_tracks.insert(0, t);
        } else {
            s.edit.selected_tracks.push(t);
        }
    }
    Ok(json!({"tracks": s.edit.selected_tracks}))
}

fn duplicate(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.duplicate", p)?;
    let range = range_param(e, "edit.duplicate", p)?;
    if range.is_empty() {
        return Err(bad("edit.duplicate", "selection is empty"));
    }
    let sh = shuffle(e.session());
    let follow = e.session().edit.automation_follows_edit;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let clips = ops::copy_range(s, *t, range);
        n += ops::paste(s, *t, range.end, &clips, range.len(), sh).map_err(|why| bad("edit.duplicate", why))?.len();
        if follow && let Some(tr) = s.track_mut(*t) {
            let locked = crate::edit::locked_params(tr);
            for l in tr.automation.iter_mut().filter(|l| !locked.contains(&l.param)) {
                let pts: Vec<_> = l.points.iter().filter(|p| range.contains(p.at)).copied().collect();
                for pt in pts {
                    l.set_point(pt.at + range.len(), pt.value);
                }
            }
        }
    }
    s.edit.selection = range.shifted(range.len());
    Ok(json!({"clips": n}))
}

fn repeat(e: &mut Engine, p: &Value) -> Result<Value> {
    let count = i64_or(p, "count", 1).clamp(1, 999);
    let tracks = tracks_required(e, "edit.repeat", p)?;
    let range = range_param(e, "edit.repeat", p)?;
    if range.is_empty() {
        return Err(bad("edit.repeat", "selection is empty"));
    }
    let sh = shuffle(e.session());
    let s = e.session_mut();
    for t in &tracks {
        let clips = ops::copy_range(s, *t, range);
        for k in 1..=count {
            let at = range.start.saturating_add(range.len().saturating_mul(k));
            ops::paste(s, *t, at, &clips, range.len(), sh).map_err(|why| bad("edit.repeat", why))?;
        }
    }
    Ok(json!({"repeats": count}))
}

fn shift(e: &mut Engine, p: &Value) -> Result<Value> {
    let by = position_param(e, "edit.shift", p, "by")?.ok_or_else(|| bad("edit.shift", "`by` is required"))?;
    let later = bool_or(p, "later", true);
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let n = ops::move_clips(s, &ids, if later { by } else { -by }, &[]);
    Ok(json!({"moved": n}))
}

fn insert_silence(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.insert_silence", p)?;
    let range = range_param_exact(e, "edit.insert_silence", p)?;
    let sh = shuffle(e.session());
    let follow = e.session().edit.automation_follows_edit;
    let s = e.session_mut();
    for t in &tracks {
        if sh {
            ops::make_room(s, *t, range.start, range.len()).map_err(|why| bad("edit.insert_silence", why))?;
        } else {
            ops::clear_range(s, *t, range, false);
            if follow && let Some(tr) = s.track_mut(*t) {
                let locked = crate::edit::locked_params(tr);
                for l in tr.automation.iter_mut().filter(|l| !locked.contains(&l.param)) {
                    l.clear_range(range.start, range.end);
                }
            }
        }
    }
    Ok(json!({"inserted": range.len()}))
}

fn separate(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.separate", p)?;
    let points: Vec<i64> = if let Some(at) = position_param(e, "edit.separate", p, "at")? {
        vec![at]
    } else {
        let r = range_param_exact(e, "edit.separate", p)?;
        if r.is_empty() { vec![r.start] } else { vec![r.start, r.end] }
    };
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        for at in &points {
            n += ops::separate_at(s, *t, *at);
        }
    }
    Ok(json!({"separated": n}))
}

fn separate_on_grid(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.separate_on_grid", p)?;
    let r = range_param(e, "edit.separate_on_grid", p)?;
    let s = e.session_mut();
    let lines = grid_adapt::effective_grid(s, r.start).lines(r.start, r.end, s.sample_rate, &s.tempo, s.frame_rate, 10_000);
    let mut n = 0;
    for t in &tracks {
        for at in &lines {
            n += ops::separate_at(s, *t, *at);
        }
    }
    Ok(json!({"separated": n}))
}

fn separate_at_transients(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.separate_at_transients", p)?;
    let r = range_param(e, "edit.separate_at_transients", p)?;
    let sens = f32_or(p, "sensitivity", 0.5).clamp(0.0, 1.0);
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let hits = crate::io::transients_in(s, *t, r, sens);
        for at in hits {
            n += ops::separate_at(s, *t, at);
        }
    }
    Ok(json!({"separated": n}))
}

fn heal(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.heal", p)?;
    let mut r = range_param(e, "edit.heal", p)?;
    if r.is_empty() {
        r = soundcraft_time::Range::new(r.start - 1, r.start + 1);
    }
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::heal(s, *t, r)).sum();
    Ok(json!({"healed": n}))
}

fn trim_to_selection(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.trim_to_selection", p)?;
    let r = range_param(e, "edit.trim_to_selection", p)?;
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::trim_to(s, *t, r)).sum();
    Ok(json!({"trimmed": n}))
}

fn trim_edge(e: &mut Engine, p: &Value, start: bool) -> Result<Value> {
    let tracks = tracks_required(e, "edit.trim", p)?;
    let at = position_param(e, "edit.trim", p, "at")?.unwrap_or(e.session().edit.selection.start);
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| if start { ops::trim_start_to(s, *t, at) } else { ops::trim_end_to(s, *t, at) }).sum();
    Ok(json!({"trimmed": n}))
}

fn trim_to_file(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut n = 0;
    for id in ids {
        if let Some((frames, lo, hi)) = trim_limits(s, id)
            && let Some(c) = s.find_clip_mut(id)
        {
            let off = c.source_offset();
            let new_start = c.start - off;
            c.trim_start_to(new_start.max(lo));
            c.trim_end_to((new_start + frames).min(hi));
            n += 1;
        }
    }
    Ok(json!({"trimmed": n}))
}

fn trim_to_fill(e: &mut Engine, p: &Value) -> Result<Value> {
    let r = range_param(e, "edit.trim_to_fill_selection", p)?;
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let mut n = 0;
    for id in ids {
        if let Some((frames, lo, hi)) = trim_limits(s, id)
            && let Some(c) = s.find_clip_mut(id)
        {
            let file_start = c.start - c.source_offset();
            c.trim_start_to(r.start.max(file_start).max(lo));
            c.trim_end_to(r.end.min(file_start + frames).min(hi));
            n += 1;
        }
    }
    Ok(json!({"trimmed": n}))
}

/// How far a trim may extend an audio clip: its source length in frames, and the earliest
/// start and latest end before it would run over the clip before or after it on the playlist
/// (an extension never overwrites, like the trim tool). An overlap the clip already has with
/// a neighbour (a crossfade) is kept rather than trimmed away.
fn trim_limits(s: &Session, id: ClipId) -> Option<(Samples, Samples, Samples)> {
    let (track, c) = s.find_clip(id)?;
    let frames = c.source().and_then(|src| s.source(src)).map(|src| i64::try_from(src.frames).unwrap_or(i64::MAX))?;
    let others = || s.track(track).into_iter().flat_map(|t| t.clips()).filter(|x| x.id != id);
    let lo = others().filter(|x| x.start < c.start).map(|x| x.end()).max().map_or(Samples::MIN, |end| end.min(c.start));
    let hi = others().filter(|x| x.start > c.start).map(|x| x.start).min().map_or(Samples::MAX, |start| start.max(c.end()));
    Some((frames, lo, hi))
}

fn mute_clips(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let any_unmuted = ids.iter().any(|id| s.find_clip(*id).is_some_and(|(_, c)| !c.muted));
    for id in &ids {
        if let Some(c) = s.find_clip_mut(*id) {
            c.muted = any_unmuted;
        }
    }
    Ok(json!({"muted": any_unmuted, "clips": ids.len()}))
}

fn copy_to_new_playlist(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.copy_to_new_playlist", p)?;
    let r = range_param(e, "edit.copy_to_new_playlist", p)?;
    let s = e.session_mut();
    for t in &tracks {
        let clips = ops::copy_range(s, *t, r);
        let mut fresh = Vec::new();
        for mut c in clips {
            s.reid_clip(&mut c);
            c.start += r.start;
            fresh.push(c);
        }
        if let Some(tr) = s.track_mut(*t) {
            let name = format!("{}.{:02}", tr.name, tr.playlists.len() + 1);
            tr.playlists.push(soundcraft_model::Playlist { name, clips: fresh });
            tr.active_playlist = tr.playlists.len() - 1;
        }
    }
    Ok(json!({"tracks": tracks.len()}))
}

fn strip_silence(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.strip_silence", p)?;
    let r = range_param(e, "edit.strip_silence", p)?;
    let thr = f32_or(p, "threshold_db", -48.0).clamp(-96.0, 0.0);
    let sr = e.session().sample_rate;
    let ms = |k: &str, d: f64| sr.samples(f64_or(p, k, d).clamp(0.0, 10_000.0) / 1000.0);
    let (min_len, pre, post) = (ms("min_length_ms", 50.0), ms("pad_before_ms", 5.0), ms("pad_after_ms", 20.0));
    let s = e.session_mut();
    let mut kept = 0;
    for t in &tracks {
        kept += crate::io::strip_silence(s, *t, r, thr, min_len, pre, post)?;
    }
    Ok(json!({"clips": kept}))
}

fn fades_create(e: &mut Engine, p: &Value) -> Result<Value> {
    let shape = str_param(p, "shape")
        .map_or(Some(FadeShape::EqualPower), FadeShape::from_id)
        .ok_or_else(|| bad("edit.fades_create", "unknown fade shape"))?;
    if p.get("clip").is_some() || p.get("side").is_some() {
        let curvature = match p.get("curvature") {
            None => 0.0,
            Some(value) => value
                .as_f64()
                .filter(|v| v.is_finite() && (-1.0..=1.0).contains(v))
                .ok_or_else(|| bad("edit.fades_create", "curvature must be a finite number from -1 to 1"))? as f32,
        };
        if ["track", "tracks", "clips", "start", "end"].iter().any(|key| p.get(key).is_some()) {
            return Err(bad("edit.fades_create", "clip fades cannot also specify tracks or an edit range"));
        }
        let id = p.get("clip").and_then(Value::as_u64).map(soundcraft_model::ClipId).ok_or_else(|| bad("edit.fades_create", "clip must be an id"))?;
        let fade_in = match str_param(p, "side") {
            Some("in") => true,
            Some("out") => false,
            _ => return Err(bad("edit.fades_create", "side must be in or out")),
        };
        let length = p
            .get("length")
            .and_then(Value::as_i64)
            .filter(|len| *len > 0)
            .ok_or_else(|| bad("edit.fades_create", "length must be a positive sample count"))?;
        let (_, clip) = e.session().find_clip(id).ok_or_else(|| bad("edit.fades_create", "clip not found"))?;
        let available = clip.length.min(clip.end().saturating_sub(clip.start));
        if available <= 0 {
            return Err(bad("edit.fades_create", "clip has no length"));
        }
        if p.get("shape").is_some_and(|v| !v.is_string()) {
            return Err(bad("edit.fades_create", "shape must be a fade shape name"));
        }
        let fade = Fade { len: length.min(available), shape, curvature };
        let clip = e.session_mut().find_clip_mut(id).ok_or_else(|| bad("edit.fades_create", "clip not found"))?;
        if fade_in {
            clip.fade_in = fade;
        } else {
            clip.fade_out = fade;
        }
        return Ok(json!({"fades": 1}));
    }
    if p.get("curvature").is_some() {
        return Err(bad("edit.fades_create", "curvature requires a clip and side"));
    }
    let tracks = tracks_required(e, "edit.fades_create", p)?;
    let r = range_param(e, "edit.fades_create", p)?;
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::create_fades(s, *t, r, shape)).sum();
    if n == 0 {
        return Err(bad("edit.fades_create", "the selection must cover a clip start, end or the boundary between two clips"));
    }
    Ok(json!({"fades": n}))
}

fn fades_delete(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.fades_delete", p)?;
    let r = range_param(e, "edit.fades_delete", p)?;
    let s = e.session_mut();
    let n: usize = tracks.iter().map(|t| ops::delete_fades(s, *t, r)).sum();
    Ok(json!({"deleted": n}))
}

fn fade_to(e: &mut Engine, p: &Value, to_start: bool) -> Result<Value> {
    let tracks = tracks_required(e, "edit.fade_to", p)?;
    let at = e.session().edit.selection.start;
    let s = e.session_mut();
    let mut n = 0;
    for t in &tracks {
        let range = s.track(*t).and_then(|tr| {
            tr.clips()
                .iter()
                .find(|c| c.range().contains(at))
                .map(|c| if to_start { soundcraft_time::Range::new(c.start, at) } else { soundcraft_time::Range::new(at, c.end()) })
        });
        if let Some(r) = range {
            n += ops::create_fades(s, *t, r, FadeShape::EqualPower);
        }
    }
    Ok(json!({"fades": n}))
}

fn nudge(e: &mut Engine, p: &Value) -> Result<Value> {
    let dir = i64_or(p, "direction", 1).signum();
    let count = i64_or(p, "count", 1).clamp(1, 1000);
    let s = e.session();
    let value = if bool_or(p, "next", false) { s.edit.nudge.stepped(true) } else { s.edit.nudge };
    let step = value.step_samples(s.edit.selection.start, s.sample_rate, &s.tempo, s.frame_rate) * count * if dir == 0 { 1 } else { dir };
    let ids = clip_ids_param(e, p);
    let s = e.session_mut();
    let n = if ids.is_empty() { 0 } else { ops::move_clips(s, &ids, step, &[]) };
    s.edit.selection = s.edit.selection.shifted(step);
    if s.edit.selection.start < 0 {
        s.edit.selection = s.edit.selection.shifted(-s.edit.selection.start);
    }
    Ok(json!({"moved": n, "by": step}))
}

fn move_clips(e: &mut Engine, p: &Value) -> Result<Value> {
    let ids = clip_ids_param(e, p);
    if ids.is_empty() {
        return Err(bad("edit.move_clips", "no clips"));
    }
    let s = e.session();
    let dest = match track_param(e, "edit.move_clips", p, "track")? {
        Some(to) => ids
            .iter()
            .find_map(|c| s.find_clip(*c))
            .and_then(|(from, _)| ops::destinations(s, &ids, from, to))
            .ok_or_else(|| bad("edit.move_clips", "the clips can't land there: past the last track, or on a track that can't hold them"))?,
        None => Vec::new(),
    };
    let first = ops::first_movable(s, &ids);
    let delta = match (position_param(e, "edit.move_clips", p, "by")?, position_param(e, "edit.move_clips", p, "to")?) {
        (Some(by), _) => by,
        (None, Some(to)) => to.saturating_sub(first.map_or(0, |c| c.start)),
        _ => 0,
    };
    let mode = s.edit.edit_mode;
    let delta = match first {
        Some(c) if mode.is_grid() && p.get("to").is_some() => ops::grid_delta(s, c, delta, mode == EditMode::GridRelative),
        _ => delta,
    };
    let s = e.session_mut();
    let n = if mode == EditMode::Shuffle {
        ops::shuffle_clips(s, &ids, delta, &dest).map_err(|why| bad("edit.move_clips", why))?
    } else {
        ops::move_clips(s, &ids, delta, &dest)
    };
    s.edit.selected_clips = ids;
    Ok(json!({"moved": n, "by": delta}))
}

fn trim_clip(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = clip_id(p, "edit.trim_clip")?;
    let start = position_param(e, "edit.trim_clip", p, "start")?;
    let end = position_param(e, "edit.trim_clip", p, "end")?;
    if start.is_none() && end.is_none() {
        return Err(bad("edit.trim_clip", "pass `start` and/or `end`"));
    }
    let sh = shuffle(e.session());
    let c = ops::trim_clip(e.session_mut(), id, start, end, sh).ok_or_else(|| bad("edit.trim_clip", "no such clip, or it is edit-locked"))?;
    Ok(json!({"start": c.start, "end": c.end()}))
}

fn consolidate(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_required(e, "edit.consolidate", p)?;
    let r = range_param(e, "edit.consolidate", p)?;
    if r.is_empty() {
        return Err(bad("edit.consolidate", "selection is empty"));
    }
    let midi = tracks.iter().filter(|&&t| crate::edit::consolidate_midi(e.session_mut(), t, r)).count();
    let mut n = 0;
    for t in tracks {
        if crate::io::consolidate(e, "edit.consolidate", t, r)? {
            n += 1;
        }
    }
    Ok(json!({"consolidated": n + midi}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, Clip, ClipContent, TrackId, TrackKind};

    #[test]
    fn selecting_a_clip_leaves_the_edit_selection_alone_and_cut_copy_use_the_clip() {
        let mut e = crate::demo::demo_engine();
        assert!(e.session().edit.selection.is_empty());
        assert_eq!(e.session().edit.selection.start, 0);
        let clip = e.session().track_by_name("Kick").unwrap().clips()[0].clone();
        let before = e.session().edit.selection;
        e.execute("edit.select", &json!({"clips": [clip.id.0]})).unwrap();
        assert_eq!(e.session().edit.selection, before, "clip select must not move the locators");
        assert_eq!(e.session().edit.selected_clips, vec![clip.id]);
        assert!(!e.session().edit.selected_tracks.is_empty());
        assert_eq!(effective_range(e.session()), clip.range());
        e.execute("edit.copy", &json!({})).unwrap();
        assert_eq!(e.session().edit.selection, before);
        e.execute("edit.cut", &json!({})).unwrap();
        assert!(e.session().find_clip(clip.id).is_none(), "cut removed the selected clip");
        assert!(e.session().edit.selection.is_empty(), "cut leaves a cursor, never a band");
    }

    #[test]
    fn a_non_empty_selection_wins_over_selected_clips() {
        let mut e = crate::demo::demo_engine();
        let clip = e.session().track_by_name("Kick").unwrap().clips()[0].clone();
        e.execute("edit.select", &json!({"clips": [clip.id.0]})).unwrap();
        e.execute("edit.select", &json!({"start": 10, "end": 20, "clips": [clip.id.0]})).unwrap();
        assert_eq!(effective_range(e.session()), soundcraft_time::Range::new(10, 20));
    }

    #[test]
    fn targeted_fade_curvature_saves_reopens_and_undoes_without_changing_lengths() {
        for side in ["in", "out"] {
            let mut e = crate::demo::demo_engine();
            let clip = e.session().track_by_name("Kick").unwrap().clips()[0].clone();
            e.execute("edit.fades_create", &json!({"clip": clip.id.0, "side": side, "length": clip.length / 2, "shape": "s-curve"})).unwrap();
            let before = e.session().clone();
            let undo_len = e.undo_history().len();
            e.execute(
                "edit.fades_create",
                &json!({"clip": clip.id.0, "side": side, "length": clip.length / 2, "shape": "s-curve", "curvature": -0.65}),
            )
            .unwrap();
            let mut expected = before.clone();
            let c = expected.find_clip_mut(clip.id).unwrap();
            if side == "in" {
                c.fade_in.curvature = -0.65;
            } else {
                c.fade_out.curvature = -0.65;
            }
            assert_eq!(e.session(), &expected);
            let reopened = Session::from_json(&e.session().to_json().unwrap()).unwrap();
            let reopened = reopened.find_clip(clip.id).unwrap().1;
            let stored = expected.find_clip(clip.id).unwrap().1;
            assert_eq!(reopened.fade_in, stored.fade_in);
            assert_eq!(reopened.fade_out, stored.fade_out);
            assert_eq!(e.undo_history().len(), undo_len + 1);
            assert!(e.undo());
            assert_eq!(e.session(), &before);
            assert!(e.redo());
            assert_eq!(e.session(), &expected);
        }
    }

    #[test]
    fn targeted_fades_fill_only_the_requested_side_and_undo_once() {
        for side in ["in", "out"] {
            let mut e = crate::demo::demo_engine();
            let clip = e.session().track_by_name("Kick").unwrap().clips()[0].clone();
            e.execute("edit.select_none", &json!({})).unwrap();
            let before = e.session().clone();
            let undo_len = e.undo_history().len();
            assert_eq!(
                e.execute("edit.fades_create", &json!({"clip": clip.id.0, "side": side, "length": i64::MAX, "shape": "s-curve"})).unwrap(),
                json!({"fades": 1})
            );
            let mut expected = before.clone();
            let c = expected.find_clip_mut(clip.id).unwrap();
            let fade = Fade { len: clip.length, shape: FadeShape::SCurve, curvature: 0.0 };
            if side == "in" {
                c.fade_in = fade;
            } else {
                c.fade_out = fade;
            }
            assert_eq!(e.session(), &expected);
            assert_eq!(e.undo_history().len(), undo_len + 1);
            assert!(e.undo());
            assert_eq!(e.session(), &before);
            assert!(e.redo());
            assert_eq!(e.session(), &expected);
        }
    }

    #[test]
    fn targeted_fades_reject_invalid_parameters_without_an_edit() {
        let mut e = crate::demo::demo_engine();
        let id = e.session().track_by_name("Kick").unwrap().clips()[0].id.0;
        for params in [
            json!({"clip": id, "side": "in"}),
            json!({"clip": id, "length": 10}),
            json!({"side": "out", "length": 10}),
            json!({"clip": "bad", "side": "in", "length": 10}),
            json!({"clip": u64::MAX, "side": "in", "length": 10}),
            json!({"clip": id, "side": "bad", "length": 10}),
            json!({"clip": id, "side": "in", "length": 0}),
            json!({"clip": id, "side": "in", "length": -1}),
            json!({"clip": id, "side": "in", "length": 1.5}),
            json!({"clip": id, "side": "in", "length": "10"}),
            json!({"clip": id, "side": "in", "length": u64::MAX}),
            json!({"clip": id, "side": "in", "length": 10, "shape": "bad"}),
            json!({"clip": id, "side": "in", "length": 10, "shape": null}),
            json!({"clip": id, "side": "in", "length": 10, "start": 0}),
            json!({"clip": id, "side": "in", "length": 10, "end": 100}),
            json!({"clip": id, "side": "in", "length": 10, "tracks": []}),
            json!({"clip": id, "side": "in", "length": 10, "curvature": 2}),
            json!({"clip": id, "side": "in", "length": 10, "curvature": -2}),
            json!({"clip": id, "side": "in", "length": 10, "curvature": 1e100}),
            json!({"clip": id, "side": "in", "length": 10, "curvature": null}),
            json!({"clip": id, "side": "in", "length": 10, "curvature": "0.5"}),
            json!({"start": 0, "end": 100, "curvature": 0.5}),
        ] {
            let before = e.session().clone();
            let history = e.undo_history();
            assert!(e.execute("edit.fades_create", &params).is_err(), "{params}");
            assert_eq!(e.session(), &before, "{params}");
            assert_eq!(e.undo_history(), history, "{params}");
        }
        e.session_mut().find_clip_mut(soundcraft_model::ClipId(id)).unwrap().length = 0;
        let before = e.session().clone();
        assert!(e.execute("edit.fades_create", &json!({"clip": id, "side": "in", "length": 1})).is_err());
        assert_eq!(e.session(), &before);
    }

    #[test]
    fn targeted_fades_support_one_sample_clips() {
        let mut e = crate::demo::demo_engine();
        let id = e.session().track_by_name("Kick").unwrap().clips()[0].id;
        e.session_mut().find_clip_mut(id).unwrap().length = 1;
        for side in ["in", "out"] {
            e.execute("edit.fades_create", &json!({"clip": id.0, "side": side, "length": 1})).unwrap();
            let clip = e.session().find_clip(id).unwrap().1;
            assert_eq!(if side == "in" { clip.fade_in.len } else { clip.fade_out.len }, 1);
        }
    }

    #[test]
    fn selecting_what_is_already_selected_leaves_the_document_alone() {
        let mut e = Engine::default();
        let ids = e.execute("track.new", &json!({"count": 2})).unwrap()["tracks"].clone();
        let sel = json!({"tracks": [ids[0]], "start": 100, "end": 200});
        e.execute("edit.select", &sel).unwrap();
        let rev = e.revision;
        assert_eq!(e.execute("edit.select", &sel).unwrap()["tracks"], json!([ids[0]]));
        assert_eq!(e.revision, rev);
        e.execute("edit.select", &json!({"tracks": [ids[0]], "start": 100, "end": 300})).unwrap();
        assert_ne!(e.revision, rev);
    }

    fn keys() -> (Engine, TrackId, impl Fn(i64) -> i64) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        let (tempo, sr) = (e.session().tempo.clone(), e.session().sample_rate);
        (e, t, move |q| tempo.tick_to_samples(q * soundcraft_time::TICKS_PER_QUARTER, sr))
    }

    fn note(start: i64, length: i64, pitch: u8) -> soundcraft_midi::Note {
        let q = soundcraft_time::TICKS_PER_QUARTER;
        soundcraft_midi::Note { pitch, velocity: 100, release_velocity: 64, channel: 0, start: start * q, length: length * q }
    }

    #[test]
    fn consolidating_an_empty_midi_selection_makes_an_empty_clip() {
        let (mut e, t, at) = keys();
        e.execute("edit.select", &json!({"tracks": [t.0], "start": at(4), "end": at(12)})).unwrap();
        assert_eq!(e.execute("edit.consolidate", &json!({})).unwrap()["consolidated"], 1);
        let clips = e.session().track(t).unwrap().clips().to_vec();
        assert_eq!(clips.iter().map(|c| (c.start, c.end())).collect::<Vec<_>>(), vec![(at(4), at(12))]);
        assert!(matches!(&clips[0].content, ClipContent::Midi { sequence } if sequence.notes.is_empty()));
    }

    #[test]
    fn consolidating_midi_merges_the_clips_as_they_sound() {
        let (mut e, t, at) = keys();
        let s = e.session_mut();
        let cc = soundcraft_midi::CtrlEvent { tick: 0, channel: 0, kind: soundcraft_midi::CtrlKind::Cc(1), value: 64 };
        let a = soundcraft_midi::Sequence { notes: vec![note(0, 1, 60), note(3, 2, 62), note(5, 1, 63)], ctrls: vec![], ..Default::default() };
        let b = soundcraft_midi::Sequence { notes: vec![note(0, 1, 64)], ctrls: vec![], ..Default::default() };
        let c = soundcraft_midi::Sequence { notes: vec![note(1, 1, 65)], ctrls: vec![cc], ..Default::default() };
        for (start, end, seq, muted) in [(0, 4, a, false), (6, 8, b, true), (8, 12, c, false)] {
            let id = s.new_clip_id();
            let mut clip = Clip::midi(id, "Keys", at(start), at(end) - at(start), seq);
            clip.muted = muted;
            ops::place_clip(s, t, clip);
        }
        e.execute("edit.select", &json!({"tracks": [t.0], "start": at(2), "end": at(10)})).unwrap();
        e.execute("edit.consolidate", &json!({})).unwrap();
        let clips = e.session().track(t).unwrap().clips().to_vec();
        assert_eq!(clips.iter().map(|c| (c.start, c.end())).collect::<Vec<_>>(), vec![(0, at(2)), (at(2), at(10)), (at(10), at(12))]);
        let ClipContent::Midi { sequence } = &clips[1].content else { panic!("not MIDI") };
        assert_eq!(sequence.notes, vec![note(1, 1, 62), note(7, 1, 65)]);
        assert_eq!(sequence.ctrls, vec![soundcraft_midi::CtrlEvent { tick: 6 * soundcraft_time::TICKS_PER_QUARTER, ..cc }]);
    }

    /// A 60,000-frame source with `clips` placed on one track as (start, length, source offset).
    fn trim_session(clips: &[(Samples, Samples, Samples)]) -> (Engine, TrackId, Vec<ClipId>) {
        use soundcraft_model::{ChannelFormat, Clip, TrackKind};
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Audio, ChannelFormat::Mono, None);
        let buf = soundcraft_audio_io::AudioBuffer { sample_rate: s.sample_rate.hz(), channels: vec![vec![0.25; 60_000]] };
        let src = crate::io::add_source(&mut s, "src", buf, None, soundcraft_audio_io::FileFormat::Wav);
        let mut ids = Vec::new();
        for &(start, len, off) in clips {
            let id = s.new_clip_id();
            ops::place_clip(&mut s, t, Clip::audio(id, "c", src, off, start, len));
            ids.push(id);
        }
        (Engine::new(s), t, ids)
    }

    fn bounds(e: &Engine, t: TrackId) -> Vec<(Samples, Samples, Samples)> {
        e.session().track(t).unwrap().clips().iter().map(|c| (c.start, c.end(), c.source_offset())).collect()
    }

    #[test]
    fn trim_to_file_boundaries_stops_at_the_neighbours() {
        let (mut e, t, ids) = trim_session(&[(0, 10_000, 0), (20_000, 10_000, 20_000), (40_000, 5_000, 0)]);
        e.execute("edit.trim_to_file_boundaries", &json!({"clips": [ids[1].0]})).unwrap();
        assert_eq!(bounds(&e, t), vec![(0, 10_000, 0), (10_000, 40_000, 10_000), (40_000, 45_000, 0)]);
    }

    #[test]
    fn trim_to_fill_selection_stops_at_the_neighbours() {
        let (mut e, t, ids) = trim_session(&[(0, 10_000, 0), (20_000, 10_000, 20_000), (40_000, 5_000, 0)]);
        e.execute("edit.trim_to_fill_selection", &json!({"clips": [ids[1].0], "start": 5_000, "end": 45_000})).unwrap();
        assert_eq!(bounds(&e, t), vec![(0, 10_000, 0), (10_000, 40_000, 10_000), (40_000, 45_000, 0)]);
    }

    #[test]
    fn trims_without_neighbours_expand_fully() {
        let (mut e, t, ids) = trim_session(&[(20_000, 10_000, 20_000)]);
        e.execute("edit.trim_to_file_boundaries", &json!({"clips": [ids[0].0]})).unwrap();
        assert_eq!(bounds(&e, t), vec![(0, 60_000, 0)]);
        let (mut e, t, ids) = trim_session(&[(20_000, 10_000, 20_000)]);
        e.execute("edit.trim_to_fill_selection", &json!({"clips": [ids[0].0], "start": 5_000, "end": 45_000})).unwrap();
        assert_eq!(bounds(&e, t), vec![(5_000, 45_000, 5_000)]);
    }

    #[test]
    fn trim_to_file_boundaries_keeps_an_existing_crossfade() {
        let (mut e, t, ids) = trim_session(&[(0, 10_000, 0), (10_000, 10_000, 10_000)]);
        e.execute("edit.fades_create", &json!({"tracks": [t], "start": 9_500, "end": 10_500})).unwrap();
        let faded = bounds(&e, t);
        assert_eq!((faded[0].1, faded[1].0), (10_500, 9_500), "the fixture needs an overlapping crossfade");
        e.execute("edit.trim_to_file_boundaries", &json!({"clips": [ids[1].0]})).unwrap();
        assert_eq!(bounds(&e, t), vec![faded[0], (9_500, 60_000, 9_500)]);
    }

    #[test]
    fn restore_last_selection_swaps_back_to_the_previous_range_and_tracks() {
        let mut e = Engine::default();
        assert!(e.execute("edit.restore_last_selection", &json!({})).is_err(), "nothing to restore yet");
        let ids = e.execute("track.new", &json!({"count": 2})).unwrap()["tracks"].clone();
        e.execute("edit.select", &json!({"tracks": [ids[0]], "start": 1_000, "end": 4_000})).unwrap();
        e.execute("edit.select", &json!({"tracks": [ids[1]], "start": 7_000, "end": 9_000})).unwrap();
        let tracks = |e: &Engine| e.session().edit.selected_tracks.iter().map(|t| t.0).collect::<Vec<_>>();
        e.execute("edit.restore_last_selection", &json!({})).unwrap();
        assert_eq!(e.session().edit.selection, soundcraft_time::Range::new(1_000, 4_000));
        assert_eq!(json!(tracks(&e)), json!([ids[0]]));
        e.execute("edit.restore_last_selection", &json!({})).unwrap();
        assert_eq!(e.session().edit.selection, soundcraft_time::Range::new(7_000, 9_000));
        assert_eq!(json!(tracks(&e)), json!([ids[1]]));
    }
    // ---- clipboard behaviour (arrange) ----

    fn two_tracks() -> (Engine, TrackId, TrackId) {
        let mut e = Engine::default();
        let ids = e.execute("track.new", &json!({"count": 2, "kind": "audio"})).unwrap()["tracks"].clone();
        let (a, b) = (TrackId(ids[0].as_u64().unwrap()), TrackId(ids[1].as_u64().unwrap()));
        for (t, at) in [(a, 1_000), (b, 1_000)] {
            let s = e.session_mut();
            let id = s.new_clip_id();
            crate::edit::place_clip(s, t, Clip::audio(id, "c", soundcraft_model::SourceId(0), 0, at, 4_000));
        }
        (e, a, b)
    }

    fn starts(e: &Engine, t: TrackId) -> Vec<(i64, i64)> {
        e.session().track(t).unwrap().clips().iter().map(|c| (c.start, c.length)).collect()
    }

    #[test]
    fn an_audio_clip_copies_and_pastes_at_the_insertion_point_and_at_a_given_position() {
        let (mut e, a, _) = two_tracks();
        let clip = e.session().track(a).unwrap().clips()[0].id;
        e.execute("edit.select", &json!({"clips": [clip.0]})).unwrap();
        e.execute("edit.copy", &json!({})).unwrap();
        e.execute("edit.select", &json!({"tracks": [a.0], "start": 20_000, "end": 20_000})).unwrap();
        e.execute("edit.paste", &json!({})).unwrap();
        assert_eq!(starts(&e, a), vec![(1_000, 4_000), (20_000, 4_000)]);
        assert_eq!(e.session().edit.selected_clips.len(), 1, "the pasted clip is selected");
        assert_ne!(e.session().edit.selected_clips[0], clip);
        e.execute("edit.paste", &json!({"tracks": [a.0], "at": 40_000})).unwrap();
        assert_eq!(starts(&e, a).len(), 3);
        assert_eq!(starts(&e, a)[2].0, 40_000);
    }

    #[test]
    fn paste_lands_on_another_track_and_undo_restores() {
        let (mut e, a, b) = two_tracks();
        let clip = e.session().track(a).unwrap().clips()[0].id;
        e.execute("edit.select", &json!({"clips": [clip.0]})).unwrap();
        e.execute("edit.copy", &json!({})).unwrap();
        let before = e.session().clone();
        e.execute("edit.paste", &json!({"tracks": [b.0], "at": 30_000})).unwrap();
        assert_eq!(starts(&e, b).len(), 2);
        assert_eq!(starts(&e, a).len(), 1);
        e.undo();
        assert_eq!(e.session().tracks, before.tracks);
    }

    #[test]
    fn a_two_track_clipboard_pasted_to_one_target_continues_on_the_next_track() {
        let (mut e, a, b) = two_tracks();
        e.execute("edit.select", &json!({"tracks": [a.0, b.0], "start": 1_000, "end": 5_000})).unwrap();
        e.execute("edit.copy", &json!({})).unwrap();
        e.execute("edit.paste", &json!({"tracks": [a.0], "at": 20_000})).unwrap();
        assert_eq!(starts(&e, a).len(), 2);
        assert_eq!(starts(&e, b).len(), 2, "the second clipboard track lands on the track below");
    }

    #[test]
    fn pasting_the_wrong_clip_kind_or_an_empty_clipboard_is_an_error_without_an_undo_step() {
        let (mut e, a, _) = two_tracks();
        let steps = e.undo_history().len();
        assert!(e.execute("edit.paste", &json!({"tracks": [a.0], "at": 0})).is_err(), "empty clipboard");
        assert_eq!(e.undo_history().len(), steps);
        let m = TrackId(e.execute("track.new", &json!({"kind": "midi"})).unwrap()["tracks"][0].as_u64().unwrap());
        let steps = e.undo_history().len();
        let clip = e.session().track(a).unwrap().clips()[0].id;
        e.execute("edit.select", &json!({"clips": [clip.0]})).unwrap();
        e.execute("edit.copy", &json!({})).unwrap();
        let err = e.execute("edit.paste", &json!({"tracks": [m.0], "at": 0})).unwrap_err().to_string();
        assert!(err.contains("audio clips") && err.contains("MIDI track"), "{err}");
        assert_eq!(e.undo_history().len(), steps);
    }

    #[test]
    fn a_midi_clip_cuts_and_pastes_with_equal_notes() {
        let mut e = Engine::default();
        let t = TrackId(e.execute("track.new", &json!({"kind": "midi"})).unwrap()["tracks"][0].as_u64().unwrap());
        let clip = e.execute("midi.new_clip", &json!({"track": t.0, "start": 0, "end": 48_000})).unwrap()["clip"].as_u64().unwrap();
        e.execute("midi.note_add", &json!({"clip": clip, "pitch": 64, "start_ticks": 240, "length_ticks": 240})).unwrap();
        let notes = |e: &Engine| {
            e.session()
                .track(t)
                .unwrap()
                .clips()
                .iter()
                .map(|c| match &c.content {
                    ClipContent::Midi { sequence } => sequence.notes.clone(),
                    _ => Vec::new(),
                })
                .collect::<Vec<_>>()
        };
        let before = notes(&e);
        e.execute("edit.select", &json!({"clips": [clip]})).unwrap();
        e.execute("edit.cut", &json!({})).unwrap();
        assert!(e.session().track(t).unwrap().clips().is_empty());
        assert!(e.session().edit.selected_clips.is_empty(), "cut clears the clip selection");
        e.execute("edit.paste", &json!({"tracks": [t.0], "at": 0})).unwrap();
        assert_eq!(notes(&e), before);
    }

    #[test]
    fn cutting_selected_clips_leaves_an_unselected_overlapping_clip_alone() {
        let (mut e, a, b) = two_tracks();
        let (ca, cb) = (e.session().track(a).unwrap().clips()[0].id, e.session().track(b).unwrap().clips()[0].id);
        e.execute("edit.select", &json!({"clips": [ca.0]})).unwrap();
        e.execute("edit.select", &json!({"clips": [ca.0]})).unwrap();
        e.execute("edit.cut", &json!({})).unwrap();
        assert!(starts(&e, a).is_empty());
        assert!(e.session().find_clip(cb).is_some(), "the other track's overlapping clip stays");
        e.execute("edit.paste", &json!({"tracks": [a.0], "at": 9_000})).unwrap();
        assert_eq!(starts(&e, a), vec![(9_000, 4_000)]);
        assert_eq!(starts(&e, b).len(), 1, "the paste added nothing to the other track");
    }
}
