//! Freehand automation drawing: `automation.draw` replaces a swept range of a lane with the given
//! points, `automation.erase_range` removes a range, `automation.lane_draw` toggles a lane's draw
//! mode. Locked lanes refuse every edit made here and in the point commands (one helper).

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{AutoParam, Track};
use std::collections::BTreeMap;

/// Most points one `automation.draw` call accepts.
pub const MAX_DRAW_POINTS: usize = 8192;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "automation.draw",
            "Draw Automation",
            [],
            None,
            "{track?, param, points: [{at, value}] (at most 8192), replace?: true (replace the points in the swept range, keeping the curve outside)} -> {written, removed}",
            has_tracks,
            draw
        ),
        cmd!(
            "automation.erase_range",
            "Erase Automation Range",
            [],
            None,
            "{track?, param, start, end} (the curve outside the range is kept) -> {removed}",
            has_tracks,
            erase_range
        ),
        cmd!(view "automation.lane_draw", "Automation Lane Draw Mode", [], None, "{track?, index, draw?: toggles}", has_tracks, lane_draw),
    ]
}

/// True when a shown lane of `param` on the track is locked.
pub(crate) fn lane_locked(tr: &Track, param: &AutoParam) -> bool {
    tr.shown_lanes.iter().any(|l| l.locked && &l.param == param)
}

/// Err("lane is locked") when the lane for `param` is locked.
pub(crate) fn refuse_locked(cmd: &str, tr: &Track, param: &AutoParam) -> Result<()> {
    if lane_locked(tr, param) { Err(bad(cmd, "lane is locked")) } else { Ok(()) }
}

/// The value limits the engine enforces for a lane (a plugin parameter's own range).
fn value_range(tr: &Track, param: &AutoParam) -> (f32, f32) {
    if let AutoParam::Plugin { slot, param: id } = param
        && let Some(Some(ins)) = tr.mixer.inserts.get(usize::from(*slot))
        && let Some(info) = super::clap::plugin_info(&ins.plugin)
        && let Some(pi) = info.params.iter().find(|x| x.id == id.as_str())
    {
        return (pi.min, pi.max);
    }
    let (lo, hi, _) = param.range();
    (lo, hi)
}

/// Switch-like lanes (mute, CC64-69, plugin toggles) only hold their limits.
fn is_stepped(tr: &Track, param: &AutoParam) -> bool {
    if param.stepped() {
        return true;
    }
    if let AutoParam::Plugin { slot, param: id } = param
        && let Some(Some(ins)) = tr.mixer.inserts.get(usize::from(*slot))
        && let Some(info) = super::clap::plugin_info(&ins.plugin)
        && let Some(pi) = info.params.iter().find(|x| x.id == id.as_str())
    {
        return matches!(pi.unit, soundcraft_dsp::Unit::Toggle);
    }
    false
}

fn point_position(v: &Value) -> Option<i64> {
    let at = v.get("at")?;
    let n = at.as_i64().or_else(|| at.as_f64().filter(|f| f.is_finite()).map(|f| f.clamp(0.0, MAX_POSITION as f64) as i64))?;
    Some(n.clamp(0, MAX_POSITION))
}

/// Sorted, de-duplicated (the last value wins), clamped (and for stepped lanes snapped) points.
fn clean_points(tr: &Track, param: &AutoParam, raw: &[Value]) -> Vec<(i64, f32)> {
    let (lo, hi) = value_range(tr, param);
    let stepped = is_stepped(tr, param);
    let mut by_at: BTreeMap<i64, f32> = BTreeMap::new();
    for v in raw {
        let (Some(at), Some(value)) = (point_position(v), v.get("value").and_then(Value::as_f64).filter(|x| x.is_finite())) else { continue };
        let value = (value as f32).clamp(lo, hi);
        by_at.insert(at, if stepped { if value >= (lo + hi) * 0.5 { hi } else { lo } } else { value });
    }
    let mut out: Vec<(i64, f32)> = Vec::with_capacity(by_at.len());
    for (at, value) in by_at {
        // A step happens right at its point: the old value holds until one sample before.
        if stepped
            && let Some(&(prev_at, prev)) = out.last()
            && prev != value
            && at - 1 > prev_at
        {
            out.push((at - 1, prev));
        }
        out.push((at, value));
    }
    out
}

fn draw(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.draw";
    let tracks = tracks_required(e, c, p)?;
    let param = p
        .get("param")
        .and_then(Value::as_str)
        .map(|s| AutoParam::parse(s).ok_or_else(|| bad(c, format!("unknown automation param `{s}`"))))
        .transpose()?;
    let param = param.ok_or_else(|| bad(c, "`param` required"))?;
    let raw = p.get("points").and_then(Value::as_array).ok_or_else(|| bad(c, "`points` required: [{at, value}]"))?;
    if raw.len() > MAX_DRAW_POINTS {
        return Err(bad(c, format!("at most {MAX_DRAW_POINTS} points per call (got {})", raw.len())));
    }
    let replace = p.get("replace").and_then(Value::as_bool).unwrap_or(true);
    for t in &tracks {
        if let Some(tr) = e.session().track(*t) {
            super::mix::check_param_for_track(c, tr, &param)?;
            refuse_locked(c, tr, &param)?;
        }
    }
    let (mut written, mut removed) = (0usize, 0usize);
    let s = e.session_mut();
    for t in &tracks {
        let Some(tr) = s.track_mut(*t) else { continue };
        let pts = clean_points(tr, &param, raw);
        let (Some(&(min, _)), Some(&(max, _))) = (pts.first(), pts.last()) else { continue };
        let def = super::mix::current_value(tr, &param);
        let lane = tr.lane_mut(&param);
        if replace {
            let before = lane.value_at(min.saturating_sub(1), def);
            let after = lane.value_at(max.saturating_add(1), def);
            removed += lane.clear_range(min, max.saturating_add(1));
            if min > 0 {
                lane.set_point(min - 1, before);
            }
            lane.set_point(max.saturating_add(1), after);
        }
        for (at, v) in &pts {
            lane.set_point(*at, *v);
        }
        written += pts.len();
    }
    Ok(json!({"written": written, "removed": removed}))
}

fn erase_range(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.erase_range";
    let tracks = tracks_required(e, c, p)?;
    let s = str_param(p, "param").ok_or_else(|| bad(c, "`param` required"))?;
    let param = AutoParam::parse(s).ok_or_else(|| bad(c, format!("unknown automation param `{s}`")))?;
    let start = position_param(e, c, p, "start")?.ok_or_else(|| bad(c, "`start` required"))?;
    let end = position_param(e, c, p, "end")?.ok_or_else(|| bad(c, "`end` required"))?;
    let (start, end) = if start <= end { (start, end) } else { (end, start) };
    let (start, end) = (start.max(0), end.max(0));
    for t in &tracks {
        if let Some(tr) = e.session().track(*t) {
            super::mix::check_param_for_track(c, tr, &param)?;
            refuse_locked(c, tr, &param)?;
        }
    }
    if start == end {
        return Ok(json!({"removed": 0}));
    }
    let mut removed = 0;
    let s = e.session_mut();
    for t in &tracks {
        let Some(tr) = s.track_mut(*t) else { continue };
        let def = super::mix::current_value(tr, &param);
        let Some(lane) = tr.automation.iter_mut().find(|l| l.param == param) else { continue };
        let has_left = lane.points.iter().any(|pt| pt.at < start);
        let has_right = lane.points.iter().any(|pt| pt.at >= end);
        let before = lane.value_at(start.saturating_sub(1), def);
        let after = lane.value_at(end, def);
        let n = lane.clear_range(start, end);
        if n == 0 {
            continue;
        }
        removed += n;
        // Anchors keep the curve outside the range as it was.
        if has_left && start > 0 {
            lane.set_point(start - 1, before);
        }
        if has_right {
            lane.set_point(end, after);
        }
    }
    Ok(json!({"removed": removed}))
}

fn lane_draw(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lane_draw";
    let id = match track_param(e, c, p, "track")? {
        Some(t) => t,
        None => e.target_tracks().first().copied().ok_or_else(|| bad(c, "no track: pass `track` or select one"))?,
    };
    let want = p.get("draw").and_then(Value::as_bool);
    let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad(c, "`index` required"))?;
    let tr = e.session_mut().track_mut(id).ok_or_else(|| bad(c, "no such track"))?;
    let l = tr.shown_lanes.get_mut(usize::try_from(i).unwrap_or(usize::MAX)).ok_or_else(|| bad(c, format!("no lane {i}")))?;
    l.draw = want.unwrap_or(!l.draw);
    Ok(json!({"index": i, "draw": l.draw}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackId, TrackKind};

    fn engine() -> (Engine, u64) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        (e, t.0)
    }

    fn pts(e: &Engine, t: u64, p: &AutoParam) -> Vec<(i64, f32)> {
        e.session().track(TrackId(t)).and_then(|tr| tr.lane(p)).map(|l| l.points.iter().map(|x| (x.at, x.value)).collect()).unwrap_or_default()
    }

    fn draw_cmd(e: &mut Engine, t: u64, param: &str, points: Value) -> Result<Value> {
        e.execute("automation.draw", &json!({"track": t, "param": param, "points": points}))
    }

    #[test]
    fn replacing_keeps_the_outside_and_anchors_at_the_old_curve() {
        let (mut e, t) = engine();
        for (at, v) in [(0, -10.0), (1000, -10.0), (2000, -20.0), (3000, -20.0)] {
            e.execute("automation.set_point", &json!({"track": t, "param": "volume", "at": at, "value": v})).unwrap();
        }
        let r = draw_cmd(&mut e, t, "volume", json!([{"at": 1200, "value": 0.0}, {"at": 1800, "value": -5.0}])).unwrap();
        assert_eq!(r["written"], 2);
        assert_eq!(r["removed"], 0);
        let got = pts(&e, t, &AutoParam::Volume);
        assert!(got.contains(&(0, -10.0)) && got.contains(&(1000, -10.0)) && got.contains(&(3000, -20.0)));
        assert!(got.iter().any(|(a, v)| *a == 1199 && (*v + 11.99).abs() < 1e-2), "anchor holds the old curve just before the stroke: {got:?}");
        assert!(got.iter().any(|(a, v)| *a == 1801 && (*v + 18.01).abs() < 1e-2), "anchor holds the old curve after the stroke: {got:?}");
        // A second pass over a wider range removes what was inside.
        let r = draw_cmd(&mut e, t, "volume", json!([{"at": 1100, "value": 1.0}, {"at": 1900, "value": 1.0}])).unwrap();
        assert!(r["removed"].as_u64().unwrap() >= 2);
        assert!(!pts(&e, t, &AutoParam::Volume).iter().any(|(a, _)| *a == 1200 || *a == 1800));
    }

    #[test]
    fn unsorted_duplicates_end_ordered_unique_and_clamped() {
        let (mut e, t) = engine();
        draw_cmd(
            &mut e,
            t,
            "volume",
            json!([{"at": 300, "value": 99.0}, {"at": 100, "value": -500.0}, {"at": 300, "value": 6.0}, {"at": -5, "value": 0.0}]),
        )
        .unwrap();
        let got = pts(&e, t, &AutoParam::Volume);
        assert!(got.windows(2).all(|w| w[0].0 < w[1].0), "{got:?}");
        assert!(got.contains(&(300, 6.0)), "the last value for an `at` wins");
        assert!(got.contains(&(100, -144.0)), "clamped to the lane range");
        assert!(got.contains(&(0, 0.0)), "negative positions go to 0");
        // Pan clamps to -1..1; non-finite values are dropped.
        draw_cmd(&mut e, t, "pan", json!([{"at": 0, "value": 5.0}, {"at": 10, "value": null}, {"at": 20, "value": "x"}])).unwrap();
        let pan = pts(&e, t, &AutoParam::Pan(0));
        assert_eq!(pan.first(), Some(&(0, 1.0)));
        assert!(!pan.iter().any(|(a, _)| *a == 10 || *a == 20), "bad values are dropped: {pan:?}");
    }

    #[test]
    fn stepped_lanes_only_hold_their_limits() {
        let (mut e, t) = engine();
        draw_cmd(&mut e, t, "mute", json!([{"at": 0, "value": 0.2}, {"at": 100, "value": 0.7}, {"at": 200, "value": 0.1}])).unwrap();
        let got = pts(&e, t, &AutoParam::Mute);
        assert!(got.iter().all(|(_, v)| *v == 0.0 || *v == 1.0), "{got:?}");
        assert!(got.contains(&(99, 0.0)) && got.contains(&(100, 1.0)), "a point one sample before the change: {got:?}");
        let m = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("K")).0;
        draw_cmd(&mut e, m, "cc64", json!([{"at": 0, "value": 30.0}, {"at": 50, "value": 100.0}])).unwrap();
        assert!(pts(&e, m, &AutoParam::MidiCc(64)).iter().all(|(_, v)| *v == 0.0 || *v == 127.0));
        assert!(draw_cmd(&mut e, t, "cc64", json!([{"at": 0, "value": 1}])).is_err(), "MIDI controllers stay off audio tracks");
    }

    #[test]
    fn merged_strokes_and_a_thin_are_one_undo_step() {
        let (mut e, t) = engine();
        let key = "lane_draw:1:volume";
        let before = e.undo_history().len();
        for i in 0..10 {
            let pts = json!([{"at": i * 100, "value": -(i as f64)}, {"at": i * 100 + 100, "value": -(i as f64) - 1.0}]);
            e.execute_merged("automation.draw", &json!({"track": t, "param": "volume", "points": pts}), key).unwrap();
        }
        e.execute_merged("automation.thin", &json!({"track": t, "param": "volume", "start": 0, "end": 1000, "tolerance": 0.01}), key).unwrap();
        e.end_merge();
        assert_eq!(e.undo_history().len(), before + 1);
        assert!(pts(&e, t, &AutoParam::Volume).len() < 12, "thinned a straight line");
        assert!(e.undo());
        assert!(pts(&e, t, &AutoParam::Volume).is_empty(), "one undo restores the original");
    }

    #[test]
    fn locked_lanes_refuse_draw_erase_and_point_commands() {
        let (mut e, t) = engine();
        e.execute("automation.lane_show", &json!({"track": t, "param": "volume"})).unwrap();
        e.execute("automation.set_point", &json!({"track": t, "param": "volume", "at": 10, "value": -3.0})).unwrap();
        e.execute("automation.lane_lock", &json!({"track": t, "index": 0})).unwrap();
        assert!(draw_cmd(&mut e, t, "volume", json!([{"at": 0, "value": 0.0}])).is_err());
        assert!(e.execute("automation.erase_range", &json!({"track": t, "param": "volume", "start": 0, "end": 100})).is_err());
        assert!(e.execute("automation.set_point", &json!({"track": t, "param": "volume", "at": 20, "value": 0.0})).is_err());
        assert!(e.execute("automation.write_range", &json!({"track": t, "param": "volume", "start": 0, "end": 5, "value": 0.0})).is_err());
        assert!(e.execute("automation.clear", &json!({"track": t, "param": "volume", "start": 0, "end": 100})).is_err());
        assert_eq!(pts(&e, t, &AutoParam::Volume), vec![(10, -3.0)], "nothing changed");
        // Other parameters are unaffected, and unlocking allows the edit again.
        assert!(draw_cmd(&mut e, t, "pan", json!([{"at": 0, "value": 0.5}])).is_ok());
        e.execute("automation.lane_lock", &json!({"track": t, "index": 0, "locked": false})).unwrap();
        assert!(draw_cmd(&mut e, t, "volume", json!([{"at": 0, "value": 0.0}])).is_ok());
        // Undo is never blocked by a lock.
        e.execute("automation.lane_lock", &json!({"track": t, "index": 0})).unwrap();
        assert!(e.undo(), "undo works with the lane locked");
    }

    #[test]
    fn too_many_points_refuse_and_leave_the_session_alone() {
        let (mut e, t) = engine();
        let many: Vec<Value> = (0..=MAX_DRAW_POINTS as i64).map(|i| json!({"at": i, "value": 0.0})).collect();
        assert!(draw_cmd(&mut e, t, "volume", json!(many)).is_err());
        assert!(pts(&e, t, &AutoParam::Volume).is_empty());
        let ok: Vec<Value> = (0..MAX_DRAW_POINTS as i64).map(|i| json!({"at": i, "value": 0.0})).collect();
        assert!(draw_cmd(&mut e, t, "volume", json!(ok)).is_ok());
    }

    #[test]
    fn huge_and_odd_positions_do_not_panic() {
        let (mut e, t) = engine();
        draw_cmd(&mut e, t, "volume", json!([{"at": i64::MAX, "value": 0.0}, {"at": i64::MIN, "value": 1.0}, {"at": 1e300, "value": 1e300}]))
            .unwrap();
        assert!(!pts(&e, t, &AutoParam::Volume).is_empty());
        e.execute("automation.erase_range", &json!({"track": t, "param": "volume", "start": i64::MAX, "end": i64::MIN})).unwrap();
        assert!(draw_cmd(&mut e, t, "volume", json!([])).is_ok(), "an empty stroke does nothing");
        assert!(e.execute("automation.draw", &json!({"track": t, "param": "volume"})).is_err(), "points required");
    }

    #[test]
    fn erase_range_keeps_the_curve_outside_and_swaps_ends() {
        let (mut e, t) = engine();
        for (at, v) in [(0, 0.0), (100, -12.0), (200, -24.0), (300, -36.0), (400, -48.0)] {
            e.execute("automation.set_point", &json!({"track": t, "param": "volume", "at": at, "value": v})).unwrap();
        }
        assert_eq!(e.execute("automation.erase_range", &json!({"track": t, "param": "volume", "start": 100, "end": 100})).unwrap()["removed"], 0);
        let r = e.execute("automation.erase_range", &json!({"track": t, "param": "volume", "start": 250, "end": 120})).unwrap();
        assert_eq!(r["removed"], 1, "only the point at 200 lies in [120, 250)");
        let got = pts(&e, t, &AutoParam::Volume);
        assert!(!got.iter().any(|(a, _)| *a == 200));
        assert!(got.iter().any(|(a, v)| *a == 119 && (*v + 14.28).abs() < 1e-2), "anchor before: {got:?}");
        assert!(got.iter().any(|(a, v)| *a == 250 && (*v + 30.0).abs() < 1e-3), "anchor after holds the old value at 250: {got:?}");
    }

    #[test]
    fn lane_draw_toggles() {
        let (mut e, t) = engine();
        e.execute("automation.lane_show", &json!({"track": t})).unwrap();
        assert_eq!(e.execute("automation.lane_draw", &json!({"track": t, "index": 0})).unwrap()["draw"], true);
        assert_eq!(e.execute("automation.lane_draw", &json!({"track": t, "index": 0})).unwrap()["draw"], false);
        assert_eq!(e.execute("automation.lane_draw", &json!({"track": t, "index": 0, "draw": true})).unwrap()["draw"], true);
        assert!(e.execute("automation.lane_draw", &json!({"track": t, "index": 4})).is_err());
    }

    #[test]
    fn scoped_thin_touches_only_one_parameter() {
        let (mut e, t) = engine();
        for i in 0..10 {
            for param in ["volume", "pan"] {
                e.execute("automation.set_point", &json!({"track": t, "param": param, "at": i * 10, "value": 0.0})).unwrap();
            }
        }
        e.execute("automation.thin", &json!({"track": t, "param": "volume", "tolerance": 0.01})).unwrap();
        assert_eq!(pts(&e, t, &AutoParam::Volume).len(), 2);
        assert_eq!(pts(&e, t, &AutoParam::Pan(0)).len(), 10);
    }
}
