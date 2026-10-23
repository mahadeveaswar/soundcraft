//! Automation lane to MIDI clip conversion: `automation.to_clip` writes a controller lane (CC,
//! pitch bend, channel pressure) of an instrument or MIDI track into the controller events of the
//! MIDI clips under a range (creating a clip when none is there), `automation.merge_into_clip`
//! writes it into one named clip. Stepped lanes write held values; continuous lanes are sampled on
//! a fixed grid and thinned. The same sampler feeds `file.export_midi` with `lanes`.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_midi::{CtrlEvent, CtrlKind, Sequence};
use soundcraft_model::{AutoParam, AutomationLane, Clip, ClipContent, ClipId, Session, Track, TrackId};
use soundcraft_time::{Samples, TICKS_PER_QUARTER};

/// Sampling grid for continuous lanes: a 32nd note.
const GRID_TICKS: i64 = TICKS_PER_QUARTER / 8;
/// Most grid samples one conversion takes (the grid widens beyond this).
const MAX_SAMPLES: i64 = 32_768;
/// Largest deviation (controller steps) thinning may introduce.
const TOLERANCE: f64 = 0.5;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "automation.to_clip",
            "Automation to MIDI Clip",
            [],
            None,
            "{track?, param: \"cc1\"|\"pitch_bend\"|\"pressure\"|..., start?, end? (default: the lane's first and last point), clear_lane?: true} -> {clips, created, events, removed_points}. Writes the lane into the controller events of the MIDI clips under the range (a clip is created when none is there) and, with `clear_lane`, removes the converted points. Refuses locked lanes and non-MIDI parameters",
            has_tracks,
            to_clip
        ),
        cmd!(
            "automation.merge_into_clip",
            "Merge Automation into Clip",
            [],
            None,
            "{track?, param, clip, start?, end? (default: the clip), clear_lane?: false} -> {events, removed_points}. Replaces the clip's controller events of that parameter inside the range with the lane",
            has_tracks,
            merge_into_clip
        ),
    ]
}

/// The controller kind a lane parameter writes.
fn ctrl_kind(param: &AutoParam) -> Option<CtrlKind> {
    match param {
        AutoParam::MidiCc(n) if *n < 128 => Some(CtrlKind::Cc(*n)),
        AutoParam::PitchBend => Some(CtrlKind::PitchBend),
        AutoParam::ChannelPressure => Some(CtrlKind::ChannelPressure),
        _ => None,
    }
}

/// The MIDI channel (0-15) the track sends on: its override (`midi.channel.<id>`, 1-16) or 0.
pub(crate) fn track_channel(s: &Session, t: TrackId) -> u8 {
    let v = s.edit.value(&format!("midi.channel.{}", t.0), 1.0);
    (v.clamp(1.0, 16.0) as u8).saturating_sub(1)
}

/// Write `track`'s MIDI controller lanes into `seq` (absolute ticks, the track's channel). Each
/// lane with points replaces the events of its controller between its first and last point.
pub(crate) fn write_lane_events(s: &Session, track: &Track, seq: &mut Sequence) {
    let channel = track_channel(s, track.id);
    for lane in track.automation.iter().filter(|l| l.param.is_midi()) {
        let (Some(kind), Some(first), Some(last)) = (ctrl_kind(&lane.param), lane.points.first(), lane.points.last()) else { continue };
        let events = lane_events(s, lane, first.at, last.at);
        let (Some(lo), Some(hi)) = (events.first().map(|e| e.0), events.last().map(|e| e.0)) else { continue };
        seq.ctrls.retain(|c| !(c.kind == kind && c.tick >= lo && c.tick <= hi));
        seq.ctrls.extend(events.into_iter().map(|(tick, value)| CtrlEvent { tick, channel, kind, value }));
    }
}

/// Drop sampled points that lie within `TOLERANCE` of a straight line between kept ones (a swing
/// filter: one pass, no recursion). The first and last point always stay.
fn thin(points: Vec<(i64, i32)>) -> Vec<(i64, i32)> {
    if points.len() <= 2 {
        return points;
    }
    let mut out = Vec::with_capacity(points.len());
    let mut anchor = points[0];
    out.push(anchor);
    let (mut lo, mut hi) = (f64::NEG_INFINITY, f64::INFINITY);
    let mut prev = anchor;
    for &p in &points[1..] {
        let dt = (p.0 - anchor.0) as f64;
        let (s_lo, s_hi) = ((f64::from(p.1) - TOLERANCE - f64::from(anchor.1)) / dt, (f64::from(p.1) + TOLERANCE - f64::from(anchor.1)) / dt);
        let (n_lo, n_hi) = (lo.max(s_lo), hi.min(s_hi));
        if n_lo > n_hi {
            out.push(prev);
            anchor = prev;
            let dt = (p.0 - anchor.0) as f64;
            lo = (f64::from(p.1) - TOLERANCE - f64::from(anchor.1)) / dt;
            hi = (f64::from(p.1) + TOLERANCE - f64::from(anchor.1)) / dt;
        } else {
            lo = n_lo;
            hi = n_hi;
        }
        prev = p;
    }
    out.push(prev);
    out
}

/// The lane as controller values over the inclusive sample range `[start, end]`: (absolute tick,
/// value). Stepped lanes give the value in force at `start` and each step after it; continuous
/// lanes are sampled every 32nd note (plus the end) and thinned. Empty when the lane has no points.
pub(crate) fn lane_events(s: &Session, lane: &AutomationLane, start: Samples, end: Samples) -> Vec<(i64, i32)> {
    if lane.points.is_empty() || end < start {
        return Vec::new();
    }
    let (lo, hi, def) = lane.param.range();
    let sr = s.sample_rate;
    let quant = |v: f32| -> i32 { v.round().clamp(lo, hi) as i32 };
    let (t0, t1) = (s.tempo.samples_to_ticks(start, sr), s.tempo.samples_to_ticks(end, sr));
    if lane.param.stepped() {
        let snap = |v: f32| if v >= (lo + hi) * 0.5 { hi as i32 } else { lo as i32 };
        let mut out: Vec<(i64, i32)> = vec![(t0, snap(lane.value_at(start, def)))];
        for p in lane.points.iter().filter(|p| p.at > start && p.at <= end) {
            let v = snap(p.value);
            if out.last().is_none_or(|l| l.1 != v) {
                out.push((s.tempo.samples_to_ticks(p.at, sr).max(t0), v));
            }
        }
        return out;
    }
    let span = (t1 - t0).max(0);
    let step = GRID_TICKS.max(span / MAX_SAMPLES + 1);
    let mut raw: Vec<(i64, i32)> = Vec::new();
    let mut t = t0;
    while t < t1 {
        raw.push((t, quant(lane.value_at(s.tempo.tick_to_samples(t, sr).clamp(start, end), def))));
        t = t.saturating_add(step);
    }
    raw.push((t1, quant(lane.value_at(end, def))));
    thin(raw)
}

/// Replace the controller events of `param`'s kind inside `[start, end]` (samples) of one clip with
/// the lane's. Returns the number of events written (0 when the range misses the clip).
fn write_into_clip(s: &mut Session, track: TrackId, clip: ClipId, param: &AutoParam, start: Samples, end: Samples) -> usize {
    let Some(kind) = ctrl_kind(param) else { return 0 };
    let channel = track_channel(s, track);
    let Some((c_start, c_end)) = s.track(track).and_then(|t| t.clips().iter().find(|c| c.id == clip)).map(|c| (c.start, c.end())) else { return 0 };
    // The clip's last sample belongs to the clip; its end does not.
    let (lo, hi) = (start.max(c_start), end.min(c_end.saturating_sub(1)));
    if hi < lo {
        return 0;
    }
    let events = match s.track(track).and_then(|t| t.lane(param)) {
        Some(lane) => lane_events(s, lane, lo, hi),
        None => return 0,
    };
    let base = s.tempo.samples_to_ticks(c_start, s.sample_rate);
    let (rel_lo, rel_hi) = (s.tempo.samples_to_ticks(lo, s.sample_rate) - base, s.tempo.samples_to_ticks(hi, s.sample_rate) - base);
    let Some(ClipContent::Midi { sequence }) = s.find_clip_mut(clip).map(|c| &mut c.content) else { return 0 };
    sequence.ctrls.retain(|c| !(c.kind == kind && c.tick >= rel_lo && c.tick <= rel_hi));
    let n = events.len();
    sequence.ctrls.extend(events.into_iter().map(|(tick, value)| CtrlEvent { tick: (tick - base).max(0), channel, kind, value }));
    sequence.sort();
    n
}

/// Remove the lane points in `[start, end]`; the lane goes away when it ends up empty.
fn clear_lane_range(tr: &mut Track, param: &AutoParam, start: Samples, end: Samples) -> usize {
    let Some(lane) = tr.automation.iter_mut().find(|l| &l.param == param) else { return 0 };
    let n = lane.clear_range(start, end.saturating_add(1));
    if lane.points.is_empty() {
        tr.automation.retain(|l| &l.param != param);
    }
    n
}

/// The track and parameter every command here works on, validated: a MIDI track, a controller
/// parameter, a lane with points, not locked.
fn target(e: &Engine, c: &str, p: &Value) -> Result<(TrackId, AutoParam)> {
    let tracks = tracks_required(e, c, p)?;
    let t = *tracks.first().ok_or_else(|| bad(c, "no track: pass `track` or select one"))?;
    let name = str_param(p, "param").ok_or_else(|| bad(c, "`param` required"))?;
    let param = AutoParam::parse(name).ok_or_else(|| bad(c, format!("unknown automation param `{name}`")))?;
    if !param.is_midi() || ctrl_kind(&param).is_none() {
        return Err(bad(c, format!("`{}` is not a MIDI controller; only CC, pitch bend and pressure lanes convert to clips", param.id())));
    }
    let tr = e.session().track(t).ok_or_else(|| bad(c, "no such track"))?;
    super::mix::check_param_for_track(c, tr, &param)?;
    super::auto_draw::refuse_locked(c, tr, &param)?;
    if tr.lane(&param).is_none_or(|l| l.points.is_empty()) {
        return Err(bad(c, format!("the `{}` lane has no points", param.id())));
    }
    Ok((t, param))
}

fn range_of(e: &Engine, c: &str, p: &Value, default: (Samples, Samples)) -> Result<(Samples, Samples)> {
    let start = position_param(e, c, p, "start")?.unwrap_or(default.0).max(0);
    let end = position_param(e, c, p, "end")?.unwrap_or(default.1).max(0);
    if end < start {
        return Err(bad(c, "`end` must not be before `start`"));
    }
    Ok((start, end))
}

fn to_clip(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.to_clip";
    let (t, param) = target(e, c, p)?;
    let lane_range = e
        .session()
        .track(t)
        .and_then(|tr| tr.lane(&param))
        .and_then(|l| Some((l.points.first()?.at, l.points.last()?.at)))
        .ok_or_else(|| bad(c, "the lane has no points"))?;
    let (start, end) = range_of(e, c, p, lane_range)?;
    let clear = bool_or(p, "clear_lane", true);
    let s = e.session_mut();
    let mut clips: Vec<ClipId> = s
        .track(t)
        .map(|tr| {
            tr.clips().iter().filter(|c| matches!(c.content, ClipContent::Midi { .. }) && c.start <= end && c.end() > start).map(|c| c.id).collect()
        })
        .unwrap_or_default();
    let created = clips.is_empty();
    if created {
        let id = s.new_clip_id();
        let len = end.saturating_sub(start).saturating_add(1).max(1);
        crate::edit::place_clip(s, t, Clip::midi(id, "Automation", start, len, Sequence::default()));
        clips.push(id);
    }
    let events: usize = clips.iter().map(|id| write_into_clip(s, t, *id, &param, start, end)).sum();
    let removed = if clear { s.track_mut(t).map_or(0, |tr| clear_lane_range(tr, &param, start, end)) } else { 0 };
    Ok(json!({"clips": clips, "created": created, "events": events, "removed_points": removed}))
}

fn merge_into_clip(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.merge_into_clip";
    let (t, param) = target(e, c, p)?;
    let clip = clip_id(p, c)?;
    let span = e
        .session()
        .track(t)
        .and_then(|tr| tr.clips().iter().find(|k| k.id == clip))
        .filter(|k| matches!(k.content, ClipContent::Midi { .. }))
        .map(|k| (k.start, k.end().saturating_sub(1)))
        .ok_or_else(|| bad(c, "`clip` must be a MIDI clip on that track"))?;
    let (start, end) = range_of(e, c, p, span)?;
    let clear = bool_or(p, "clear_lane", false);
    let s = e.session_mut();
    let events = write_into_clip(s, t, clip, &param, start, end);
    let removed = if clear && events > 0 { s.track_mut(t).map_or(0, |tr| clear_lane_range(tr, &param, start, end)) } else { 0 };
    Ok(json!({"events": events, "removed_points": removed}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn setup() -> (Engine, u64) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        (e, t.0)
    }

    fn point(e: &mut Engine, t: u64, param: &str, at: i64, value: f64) {
        e.execute("automation.set_point", &json!({"track": t, "param": param, "at": at, "value": value})).unwrap();
    }

    fn events(e: &Engine, t: u64, kind: CtrlKind) -> Vec<(i64, i32)> {
        let s = e.session();
        let mut out = Vec::new();
        if let Some(tr) = s.track(TrackId(t)) {
            for c in tr.clips() {
                if let ClipContent::Midi { sequence } = &c.content {
                    let base = s.tempo.samples_to_ticks(c.start, s.sample_rate);
                    out.extend(sequence.ctrls.iter().filter(|x| x.kind == kind).map(|x| (x.tick + base, x.value)));
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn a_ramp_becomes_a_thin_set_of_events_and_clears_the_lane() {
        let (mut e, t) = setup();
        point(&mut e, t, "cc1", 0, 0.0);
        point(&mut e, t, "cc1", 96_000, 100.0);
        let r = e.execute("automation.to_clip", &json!({"track": t, "param": "cc1"})).unwrap();
        assert_eq!(r["created"], true);
        assert_eq!(r["removed_points"], 2);
        let got = events(&e, t, CtrlKind::Cc(1));
        assert!(got.len() >= 2 && got.len() < 12, "a straight ramp is thinned: {got:?}");
        assert_eq!(got.first().map(|g| g.1), Some(0));
        assert_eq!(got.last().map(|g| g.1), Some(100));
        // Every kept event sits on the lane's line, and the lane is gone.
        let sr = e.session().sample_rate;
        for (tick, v) in &got {
            let at = e.session().tempo.tick_to_samples(*tick, sr);
            assert!((f64::from(*v) - at as f64 * 100.0 / 96_000.0).abs() <= 1.0, "{tick} {v}");
        }
        assert!(e.session().track(TrackId(t)).unwrap().lane(&AutoParam::MidiCc(1)).is_none());
    }

    #[test]
    fn round_trip_lane_to_clip_matches_the_lane_at_every_event() {
        let (mut e, t) = setup();
        for (at, v) in [(0, 10.0), (24_000, 90.0), (48_000, 20.0), (72_000, 20.0), (96_000, 120.0)] {
            point(&mut e, t, "cc11", at, v);
        }
        let lane = e.session().track(TrackId(t)).unwrap().lane(&AutoParam::MidiCc(11)).unwrap().clone();
        e.execute("automation.to_clip", &json!({"track": t, "param": "cc11", "clear_lane": false})).unwrap();
        let sr = e.session().sample_rate;
        let got = events(&e, t, CtrlKind::Cc(11));
        assert!(got.len() >= 5);
        for (tick, v) in &got {
            let at = e.session().tempo.tick_to_samples(*tick, sr);
            let want = lane.value_at(at, 0.0);
            // Thinning and linear interpolation between kept events stay within one step.
            assert!((f64::from(*v) - f64::from(want)).abs() <= 1.0, "tick {tick}: event {v} lane {want}");
        }
        assert_eq!(e.session().track(TrackId(t)).unwrap().lane(&AutoParam::MidiCc(11)).unwrap().points.len(), 5, "clear_lane false keeps the lane");
    }

    #[test]
    fn stepped_lanes_write_held_values_only() {
        let (mut e, t) = setup();
        point(&mut e, t, "cc64", 0, 127.0);
        point(&mut e, t, "cc64", 48_000, 0.0);
        point(&mut e, t, "cc64", 96_000, 127.0);
        e.execute("automation.to_clip", &json!({"track": t, "param": "cc64"})).unwrap();
        let got = events(&e, t, CtrlKind::Cc(64));
        assert_eq!(got.iter().map(|g| g.1).collect::<Vec<_>>(), vec![127, 0, 127], "{got:?}");
    }

    #[test]
    fn existing_clips_are_used_and_only_the_range_is_replaced() {
        let (mut e, t) = setup();
        let clip = e.execute("midi.new_clip", &json!({"track": t, "start": 0, "end": 192_000})).unwrap()["clip"].as_u64().unwrap();
        e.execute("midi.ctrl_set", &json!({"clip": clip, "lane": "cc", "cc": 1, "points": [{"tick": 0, "value": 5}, {"tick": 2400, "value": 6}, {"tick": 4800, "value": 7}]}))
            .unwrap();
        point(&mut e, t, "cc1", 48_000, 50.0);
        point(&mut e, t, "cc1", 96_000, 60.0);
        let r = e.execute("automation.merge_into_clip", &json!({"track": t, "param": "cc1", "clip": clip, "start": 48_000, "end": 96_000})).unwrap();
        assert!(r["events"].as_u64().unwrap() >= 2);
        let ev = e.execute("midi.ctrl_list", &json!({"clip": clip, "lane": "cc", "cc": 1})).unwrap();
        let ticks: Vec<i64> = ev["events"].as_array().unwrap().iter().map(|x| x["tick"].as_i64().unwrap()).collect();
        assert!(ticks.contains(&0) && ticks.contains(&4800), "events outside the range stay: {ticks:?}");
        assert!(!ticks.contains(&2400), "the event inside the range was replaced: {ticks:?}");
        assert_eq!(e.session().track(TrackId(t)).unwrap().clips().len(), 1, "no extra clip");
    }

    #[test]
    fn refusals_and_undo() {
        let (mut e, t) = setup();
        point(&mut e, t, "cc1", 0, 1.0);
        point(&mut e, t, "cc1", 1000, 9.0);
        assert!(e.execute("automation.to_clip", &json!({"track": t, "param": "volume"})).is_err(), "not a MIDI parameter");
        assert!(e.execute("automation.to_clip", &json!({"track": t, "param": "cc7"})).is_err(), "no such lane");
        let audio = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        assert!(e.execute("automation.to_clip", &json!({"track": audio.0, "param": "cc1"})).is_err(), "audio tracks have no controllers");
        e.execute("automation.lane_show", &json!({"track": t, "param": "cc1"})).unwrap();
        e.execute("automation.lane_lock", &json!({"track": t, "index": 0})).unwrap();
        assert!(e.execute("automation.to_clip", &json!({"track": t, "param": "cc1"})).is_err(), "locked lanes are refused");
        assert!(e.session().track(TrackId(t)).unwrap().clips().is_empty(), "nothing was created");
        e.execute("automation.lane_lock", &json!({"track": t, "index": 0, "locked": false})).unwrap();
        let before = e.undo_history().len();
        e.execute("automation.to_clip", &json!({"track": t, "param": "cc1"})).unwrap();
        assert_eq!(e.undo_history().len(), before + 1, "one undo step");
        assert!(e.undo());
        assert!(e.session().track(TrackId(t)).unwrap().clips().is_empty());
        assert_eq!(e.session().track(TrackId(t)).unwrap().lane(&AutoParam::MidiCc(1)).unwrap().points.len(), 2, "undo restores the lane");
    }

    #[test]
    fn pitch_bend_keeps_its_signed_range() {
        let (mut e, t) = setup();
        point(&mut e, t, "pitch_bend", 0, -8192.0);
        point(&mut e, t, "pitch_bend", 48_000, 8191.0);
        e.execute("automation.to_clip", &json!({"track": t, "param": "pitch_bend"})).unwrap();
        let got = events(&e, t, CtrlKind::PitchBend);
        assert_eq!(got.first().map(|g| g.1), Some(-8192));
        assert_eq!(got.last().map(|g| g.1), Some(8191));
    }

    #[test]
    fn the_track_channel_override_sets_the_event_channel() {
        let (mut e, t) = setup();
        e.session_mut().edit.values.insert(format!("midi.channel.{t}"), 5.0);
        point(&mut e, t, "cc1", 0, 1.0);
        point(&mut e, t, "cc1", 1000, 9.0);
        e.execute("automation.to_clip", &json!({"track": t, "param": "cc1"})).unwrap();
        let tr = e.session().track(TrackId(t)).unwrap();
        let ClipContent::Midi { sequence } = &tr.clips()[0].content else { panic!("midi") };
        assert!(sequence.ctrls.iter().all(|c| c.channel == 4));
    }
    #[test]
    fn export_writes_lane_events_only_when_asked() {
        let (mut e, t) = setup();
        point(&mut e, t, "cc1", 0, 10.0);
        point(&mut e, t, "cc1", 48_000, 100.0);
        let plain = soundcraft_midi::read_smf(&crate::io::export_midi_with(&e, None, false).unwrap()).unwrap();
        assert!(plain.tracks.iter().all(|tr| tr.sequence.ctrls.is_empty()), "lanes are not exported by default");
        let with = soundcraft_midi::read_smf(&crate::io::export_midi_with(&e, None, true).unwrap()).unwrap();
        let ctrls: Vec<_> = with.tracks.iter().flat_map(|tr| tr.sequence.ctrls.iter()).filter(|c| c.kind == CtrlKind::Cc(1)).collect();
        assert!(ctrls.len() >= 2, "{ctrls:?}");
        assert_eq!(ctrls.first().map(|c| c.value), Some(10));
        assert_eq!(ctrls.last().map(|c| c.value), Some(100));
        // The command forwards the flag.
        let path = std::env::temp_dir().join(format!("sc_lanes_{}.mid", std::process::id()));
        let p = path.to_string_lossy().to_string();
        e.execute("file.export_midi", &json!({"path": p, "lanes": true})).unwrap();
        let back = soundcraft_midi::read_smf(&std::fs::read(&path).unwrap()).unwrap();
        let _ = std::fs::remove_file(&path);
        assert!(back.tracks.iter().any(|tr| !tr.sequence.ctrls.is_empty()));
    }
}
