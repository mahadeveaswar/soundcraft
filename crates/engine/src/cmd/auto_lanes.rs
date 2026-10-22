//! Automation lanes shown under a track in the Edit window (view state saved with the session).
//!
//! Showing and hiding a lane is an undoable edit. Switching a lane's parameter, locking it,
//! following and resizing it are view commands (no undo step, no dirty flag), so a lane that
//! follows a knob never splits the undo step of the knob's own gesture.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{AutoParam, ShownLane, Track};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "automation.lane_show",
            "Show Automation Lane",
            [],
            None,
            "{track?, param?: volume|pan|mute|send_a_level|plugin:a:<id>|cc<n>|pitch_bend|pressure (MIDI controllers on instrument and MIDI tracks only; default: the first of volume, pan, mute not shown)}",
            has_tracks,
            lane_show
        ),
        cmd!("automation.lane_hide", "Hide Automation Lane", [], None, "{track?, param}", has_tracks, lane_hide),
        cmd!(
            view "automation.lane_set_param",
            "Change Automation Lane Parameter",
            [],
            None,
            "{track?, index, param} (a parameter another lane of the track shows is refused)",
            has_tracks,
            lane_set_param
        ),
        cmd!(view "automation.lane_lock", "Lock Automation Lane", [], None, "{track?, index, locked?: toggles}", has_tracks, lane_lock),
        cmd!(
            view "automation.lane_follow",
            "Automation Lane Follows Touched Parameter",
            [],
            None,
            "{track?, index, on?: toggles}",
            has_tracks,
            lane_follow
        ),
        cmd!(view "automation.lane_height", "Automation Lane Height", [], None, "{track?, index, height: 36..320}", has_tracks, lane_height),
        cmd!(
            query "automation.lanes_list",
            "List Automation Lanes",
            [],
            None,
            "{track?} -> lanes: [{index, param, label, points, value, unit, min, max, height, locked, follow, draw, toggle}]",
            always,
            lanes_list
        ),
    ]
}

const DEFAULTS: [AutoParam; 3] = [AutoParam::Volume, AutoParam::Pan(0), AutoParam::Mute];

fn track_of(e: &Engine, c: &str, p: &Value) -> Result<TrackId> {
    match track_param(e, c, p, "track")? {
        Some(t) => Ok(t),
        None => e.target_tracks().first().copied().ok_or_else(|| bad(c, "no track: pass `track` or select one")),
    }
}

fn param_of(p: &Value, c: &str) -> Result<AutoParam> {
    let s = str_param(p, "param").ok_or_else(|| bad(c, "`param` required"))?;
    AutoParam::parse(s).ok_or_else(|| bad(c, format!("unknown automation param `{s}`")))
}

fn index_of(t: &Track, p: &Value, c: &str) -> Result<usize> {
    let i = p.get("index").and_then(Value::as_u64).ok_or_else(|| bad(c, "`index` required"))?;
    let i = usize::try_from(i).unwrap_or(usize::MAX);
    if i < t.shown_lanes.len() { Ok(i) } else { Err(bad(c, format!("no lane {i} (the track shows {})", t.shown_lanes.len()))) }
}

/// The track and lane index a command addresses.
fn target(e: &Engine, c: &str, p: &Value) -> Result<(TrackId, usize)> {
    let id = track_of(e, c, p)?;
    let i = index_of(e.session().track(id).ok_or_else(|| bad(c, "no such track"))?, p, c)?;
    Ok((id, i))
}

fn lane_show(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lane_show";
    let id = track_of(e, c, p)?;
    let requested = if str_param(p, "param").is_some() { Some(param_of(p, c)?) } else { None };
    let tr = e.session_mut().track_mut(id).ok_or_else(|| bad(c, "no such track"))?;
    if let Some(x) = &requested {
        super::mix::check_param_for_track(c, tr, x)?;
    }
    let param = match requested {
        Some(x) if tr.shown_lanes.iter().any(|l| l.param == x) => return Err(bad(c, "that lane is already shown")),
        Some(x) => x,
        None => DEFAULTS
            .iter()
            .find(|d| !tr.shown_lanes.iter().any(|l| &l.param == *d))
            .cloned()
            .ok_or_else(|| bad(c, "volume, pan and mute are all shown; pass `param`"))?,
    };
    tr.shown_lanes.push(ShownLane::new(param.clone()));
    Ok(json!({"index": tr.shown_lanes.len() - 1, "param": param.id()}))
}

fn lane_hide(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lane_hide";
    let id = track_of(e, c, p)?;
    let param = param_of(p, c)?;
    let tr = e.session_mut().track_mut(id).ok_or_else(|| bad(c, "no such track"))?;
    let i = tr.shown_lanes.iter().position(|l| l.param == param).ok_or_else(|| bad(c, "that lane is not shown"))?;
    tr.shown_lanes.remove(i);
    Ok(json!({"index": i}))
}

fn lane_set_param(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lane_set_param";
    let (id, i) = target(e, c, p)?;
    let param = param_of(p, c)?;
    let tr = e.session().track(id).ok_or_else(|| bad(c, "no such track"))?;
    super::mix::check_param_for_track(c, tr, &param)?;
    if tr.shown_lanes.iter().enumerate().any(|(j, l)| j != i && l.param == param) {
        return Err(bad(c, "another lane of this track already shows that parameter"));
    }
    let tr = e.session_mut().track_mut(id).ok_or_else(|| bad(c, "no such track"))?;
    if let Some(l) = tr.shown_lanes.get_mut(i) {
        l.param = param.clone();
    }
    Ok(json!({"index": i, "param": param.id()}))
}

fn lane_lock(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lane_lock";
    let (id, i) = target(e, c, p)?;
    let want = p.get("locked").and_then(Value::as_bool);
    let tr = e.session_mut().track_mut(id).ok_or_else(|| bad(c, "no such track"))?;
    let Some(l) = tr.shown_lanes.get_mut(i) else { return Err(bad(c, "no such lane")) };
    l.locked = want.unwrap_or(!l.locked);
    Ok(json!({"index": i, "locked": l.locked}))
}

fn lane_follow(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lane_follow";
    let (id, i) = target(e, c, p)?;
    let want = p.get("on").and_then(Value::as_bool);
    let tr = e.session_mut().track_mut(id).ok_or_else(|| bad(c, "no such track"))?;
    let Some(l) = tr.shown_lanes.get_mut(i) else { return Err(bad(c, "no such lane")) };
    l.follow = want.unwrap_or(!l.follow);
    Ok(json!({"index": i, "on": l.follow}))
}

fn lane_height(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lane_height";
    let (id, i) = target(e, c, p)?;
    let h = p.get("height").and_then(Value::as_f64).filter(|h| h.is_finite()).ok_or_else(|| bad(c, "`height` required"))?;
    let h = ShownLane::clamp_height(h as f32);
    let tr = e.session_mut().track_mut(id).ok_or_else(|| bad(c, "no such track"))?;
    if let Some(l) = tr.shown_lanes.get_mut(i) {
        l.height = h;
    }
    Ok(json!({"index": i, "height": h}))
}

fn lanes_list(e: &mut Engine, p: &Value) -> Result<Value> {
    let c = "automation.lanes_list";
    let id = track_of(e, c, p)?;
    let at = e.transport.position;
    let tr = e.session().track(id).ok_or_else(|| bad(c, "no such track"))?;
    let lanes: Vec<Value> = tr
        .shown_lanes
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let fixed = super::mix::current_value(tr, &l.param);
            let lane = tr.lane(&l.param);
            let value = lane.map_or(fixed, |a| a.value_at(at, fixed));
            let (min, max, _) = l.param.range();
            let (min, max) = if min <= f32::MIN || max >= f32::MAX { (Value::Null, Value::Null) } else { (json!(min), json!(max)) };
            json!({
                "index": i, "param": l.param.id(), "label": l.param.label(), "points": lane.map_or(0, |a| a.points.len()),
                "value": value, "unit": unit_of(&l.param), "min": min, "max": max, "height": l.height, "locked": l.locked, "follow": l.follow, "draw": l.draw, "toggle": l.param.is_toggle(),
            })
        })
        .collect();
    Ok(json!({"track": id.0, "lanes": lanes}))
}

/// What a lane's values are measured in.
fn unit_of(p: &AutoParam) -> &'static str {
    match p {
        AutoParam::Volume | AutoParam::SendLevel(_) => "dB",
        AutoParam::Pan(_) | AutoParam::SendPan(_) => "pan",
        _ if p.stepped() => "on/off",
        AutoParam::MidiCc(_) | AutoParam::ChannelPressure => "cc",
        AutoParam::PitchBend => "bend",
        _ => "value",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, Session, TrackKind};

    fn engine() -> (Engine, u64) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        (e, t.0)
    }

    fn lanes(e: &mut Engine, t: u64) -> Vec<Value> {
        e.execute("automation.lanes_list", &json!({"track": t})).unwrap()["lanes"].as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn show_defaults_run_through_volume_pan_mute_then_refuse() {
        let (mut e, t) = engine();
        for want in ["volume", "pan", "mute"] {
            assert_eq!(e.execute("automation.lane_show", &json!({"track": t})).unwrap()["param"], want);
        }
        assert!(e.execute("automation.lane_show", &json!({"track": t})).is_err());
        assert!(e.execute("automation.lane_show", &json!({"track": t, "param": "volume"})).is_err(), "no duplicates");
        assert!(e.execute("automation.lane_show", &json!({"track": t, "param": "nonsense"})).is_err());
        assert_eq!(e.execute("automation.lane_show", &json!({"track": t, "param": "send_a_level"})).unwrap()["index"], 3);
        assert_eq!(lanes(&mut e, t).len(), 4);
    }

    #[test]
    fn hide_set_param_lock_follow_and_height() {
        let (mut e, t) = engine();
        e.execute("automation.lane_show", &json!({"track": t})).unwrap();
        e.execute("automation.lane_show", &json!({"track": t, "param": "pan"})).unwrap();
        assert!(e.execute("automation.lane_set_param", &json!({"track": t, "index": 0, "param": "pan"})).is_err());
        e.execute("automation.lane_set_param", &json!({"track": t, "index": 0, "param": "mute"})).unwrap();
        assert_eq!(lanes(&mut e, t)[0]["param"], "mute");
        assert_eq!(lanes(&mut e, t)[0]["toggle"], true);
        assert_eq!(e.execute("automation.lane_lock", &json!({"track": t, "index": 1})).unwrap()["locked"], true);
        assert_eq!(e.execute("automation.lane_lock", &json!({"track": t, "index": 1, "locked": false})).unwrap()["locked"], false);
        assert_eq!(e.execute("automation.lane_follow", &json!({"track": t, "index": 0, "on": true})).unwrap()["on"], true);
        assert_eq!(e.execute("automation.lane_height", &json!({"track": t, "index": 0, "height": 5000})).unwrap()["height"], 320.0);
        assert_eq!(e.execute("automation.lane_height", &json!({"track": t, "index": 0, "height": 1})).unwrap()["height"], 36.0);
        assert!(e.execute("automation.lane_lock", &json!({"track": t, "index": 9})).is_err());
        e.execute("automation.lane_hide", &json!({"track": t, "param": "mute"})).unwrap();
        assert_eq!(lanes(&mut e, t).len(), 1);
        assert!(e.execute("automation.lane_hide", &json!({"track": t, "param": "mute"})).is_err());
    }

    #[test]
    fn view_commands_add_no_undo_steps() {
        let (mut e, t) = engine();
        e.execute("automation.lane_show", &json!({"track": t})).unwrap();
        let before = e.undo_history().len();
        e.execute("automation.lane_set_param", &json!({"track": t, "index": 0, "param": "pan"})).unwrap();
        e.execute("automation.lane_lock", &json!({"track": t, "index": 0})).unwrap();
        e.execute("automation.lane_follow", &json!({"track": t, "index": 0})).unwrap();
        e.execute("automation.lane_height", &json!({"track": t, "index": 0, "height": 100})).unwrap();
        assert_eq!(e.undo_history().len(), before);
        assert_eq!(lanes(&mut e, t)[0]["param"], "pan");
    }

    #[test]
    fn list_reports_points_and_the_value_at_the_playhead() {
        let (mut e, t) = engine();
        e.execute("automation.lane_show", &json!({"track": t, "param": "volume"})).unwrap();
        e.execute("automation.set_point", &json!({"track": t, "param": "volume", "at": 0, "value": -6.0})).unwrap();
        let l = lanes(&mut e, t);
        assert_eq!(l[0]["points"], 1);
        assert_eq!(l[0]["value"], -6.0);
    }

    #[test]
    fn undo_restores_the_lanes_and_hiding_keeps_the_points() {
        let (mut e, t) = engine();
        e.execute("automation.lane_show", &json!({"track": t})).unwrap();
        e.execute("automation.set_point", &json!({"track": t, "param": "volume", "at": 100, "value": -3.0})).unwrap();
        e.execute("automation.lane_hide", &json!({"track": t, "param": "volume"})).unwrap();
        assert!(lanes(&mut e, t).is_empty());
        assert_eq!(e.session().track(TrackId(t)).unwrap().lane(&AutoParam::Volume).unwrap().points.len(), 1, "automation survives hiding");
        assert!(e.undo());
        assert_eq!(lanes(&mut e, t).len(), 1);
        assert!(e.undo());
        assert_eq!(e.session().track(TrackId(t)).unwrap().automation.len(), 0);
        assert!(e.undo());
        assert!(lanes(&mut e, t).is_empty());
    }

    #[test]
    fn session_json_round_trip_and_old_sessions_without_the_field_load() {
        let (mut e, t) = engine();
        e.execute("automation.lane_show", &json!({"track": t, "param": "plugin:a:threshold"})).unwrap();
        e.execute("automation.lane_lock", &json!({"track": t, "index": 0})).unwrap();
        e.execute("automation.lane_follow", &json!({"track": t, "index": 0})).unwrap();
        let v = serde_json::to_value(e.session()).unwrap();
        let back: Session = serde_json::from_value(v.clone()).unwrap();
        let l = &back.track(TrackId(t)).unwrap().shown_lanes;
        assert_eq!(l.len(), 1);
        assert!(l[0].locked && l[0].follow);
        assert_eq!(l[0].param, AutoParam::Plugin { slot: 0, param: "threshold".into() });
        // A session saved before the field existed.
        let mut old = v;
        if let Some(tracks) = old.get_mut("tracks").and_then(Value::as_array_mut) {
            for tr in tracks {
                if let Some(o) = tr.as_object_mut() {
                    o.remove("shown_lanes");
                }
            }
        }
        let back: Session = serde_json::from_value(old).unwrap();
        assert!(back.track(TrackId(t)).unwrap().shown_lanes.is_empty());
        // A lane written without the optional fields gets the defaults.
        let lane: ShownLane = serde_json::from_value(json!({"param": "Volume"})).unwrap();
        assert_eq!((lane.height, lane.locked, lane.follow), (soundcraft_model::LANE_HEIGHT_DEFAULT, false, false));
    }

    #[test]
    fn midi_params_only_on_instrument_and_midi_tracks() {
        let (mut e, t) = engine();
        assert!(e.execute("automation.lane_show", &json!({"track": t, "param": "cc11"})).is_err());
        e.execute("automation.lane_show", &json!({"track": t})).unwrap();
        assert!(e.execute("automation.lane_set_param", &json!({"track": t, "index": 0, "param": "pitch_bend"})).is_err());
        assert!(e.execute("automation.set_point", &json!({"track": t, "param": "cc1", "at": 0, "value": 5})).is_err());
        assert!(e.execute("automation.midi_value", &json!({"track": t, "param": "cc1", "value": 5})).is_err());
        assert!(e.session().track(TrackId(t)).unwrap().automation.is_empty());
        for kind in [TrackKind::Instrument, TrackKind::Midi] {
            let m = e.session_mut().add_track(kind, ChannelFormat::Stereo, None).0;
            for (i, param) in ["cc11", "cc64", "pitch_bend", "pressure"].into_iter().enumerate() {
                assert_eq!(e.execute("automation.lane_show", &json!({"track": m, "param": param})).unwrap()["index"], i);
            }
            e.execute("automation.lane_set_param", &json!({"track": m, "index": 0, "param": "cc1"})).unwrap();
            let l = lanes(&mut e, m);
            assert_eq!((l[0]["param"].as_str(), l[0]["unit"].as_str()), (Some("cc1"), Some("cc")));
            assert_eq!(l[1]["unit"], "on/off");
            assert_eq!(l[1]["toggle"], true);
            assert_eq!((l[2]["unit"].as_str(), l[2]["min"].as_f64(), l[2]["max"].as_f64()), (Some("bend"), Some(-8192.0), Some(8191.0)));
        }
    }

    #[test]
    fn midi_value_defaults_to_the_playhead() {
        let (mut e, _) = engine();
        let m = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, None).0;
        e.session_mut().edit.selection.start = 5;
        e.transport.position = 777;
        e.execute("automation.midi_value", &json!({"track": m, "param": "cc1", "value": 9})).unwrap();
        assert_eq!(e.session().track(TrackId(m)).unwrap().lane(&AutoParam::MidiCc(1)).unwrap().points[0].at, 5, "stopped: the insertion");
        e.transport.playing = true;
        e.execute("automation.midi_value", &json!({"track": m, "param": "cc1", "value": 10})).unwrap();
        let tr = e.session().track(TrackId(m)).unwrap();
        assert_eq!(tr.lane(&AutoParam::MidiCc(1)).unwrap().points.last().unwrap().at, 777, "playing: the playhead");
    }

    #[test]
    fn midi_value_writes_a_clamped_breakpoint() {
        let (mut e, _) = engine();
        let m = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, None).0;
        assert_eq!(e.execute("automation.midi_value", &json!({"track": m, "param": "cc11", "value": 300, "at": 10})).unwrap()["value"], 127.0);
        e.execute("automation.midi_value", &json!({"track": m, "param": "pitch_bend", "value": -99999, "at": 0})).unwrap();
        assert!(e.execute("automation.midi_value", &json!({"track": m, "param": "volume", "value": 1})).is_err(), "not a MIDI controller");
        assert!(e.execute("automation.midi_value", &json!({"track": m, "param": "cc11"})).is_err(), "value required");
        let tr = e.session().track(TrackId(m)).unwrap();
        assert_eq!(tr.lane(&AutoParam::MidiCc(11)).unwrap().points.len(), 1);
        assert_eq!(tr.lane(&AutoParam::PitchBend).unwrap().points[0].value, -8192.0);
        assert!(e.undo());
        assert!(e.undo());
        assert!(e.session().track(TrackId(m)).unwrap().automation.is_empty());
    }

    #[test]
    fn param_ids_round_trip() {
        for p in [
            AutoParam::Volume,
            AutoParam::Pan(0),
            AutoParam::Pan(1),
            AutoParam::Mute,
            AutoParam::SendLevel(2),
            AutoParam::SendPan(1),
            AutoParam::SendMute(0),
        ] {
            assert_eq!(AutoParam::parse(&p.id()), Some(p));
        }
        let p = AutoParam::Plugin { slot: 3, param: "gain".into() };
        assert_eq!(AutoParam::parse(&p.id()), Some(p));
    }
}
