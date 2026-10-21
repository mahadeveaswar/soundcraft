//! MIDI controller lanes (CC, pitch bend, aftertouch) in a MIDI clip.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_midi::{CtrlEvent, CtrlKind, Sequence};
use soundcraft_model::{ClipContent, ClipId};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "midi.ctrl_list", "List MIDI Controller Events", [], None, "{clip, lane?: \"cc\"|\"pitch_bend\"|\"pressure\", cc?, channel?}", always, ctrl_list),
        cmd!(
            "midi.ctrl_set",
            "Set MIDI Controller Points",
            [],
            None,
            "{clip, lane: \"cc\"|\"pitch_bend\"|\"pressure\", cc?: 1, channel?: 0, points: [{tick, value}]} (replaces an event at the same tick)",
            always,
            ctrl_set
        ),
        cmd!(
            "midi.ctrl_clear",
            "Clear MIDI Controller Range",
            [],
            None,
            "{clip, lane, cc?, channel?, start_ticks?: 0, end_ticks?: end of clip}",
            always,
            ctrl_clear
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

/// The lane named by `lane` (+ `cc`), with the value range its events accept.
fn lane(p: &Value, cmd: &str) -> Result<(CtrlKind, i32, i32)> {
    match p.get("lane").and_then(Value::as_str) {
        Some("cc") => Ok((CtrlKind::Cc(i64_or(p, "cc", 1).clamp(0, 127) as u8), 0, 127)),
        Some("pitch_bend") => Ok((CtrlKind::PitchBend, -8192, 8191)),
        Some("pressure") => Ok((CtrlKind::ChannelPressure, 0, 127)),
        _ => Err(bad(cmd, "`lane` must be cc, pitch_bend or pressure")),
    }
}

fn same_lane(a: CtrlKind, b: CtrlKind) -> bool {
    a == b
}

fn ctrl_list(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = clip_id(p, "midi.ctrl_list")?;
    let want = if p.get("lane").is_some() { Some(lane(p, "midi.ctrl_list")?.0) } else { None };
    let ch = p.get("channel").and_then(Value::as_i64);
    match e.session().find_clip(id).map(|(_, c)| &c.content) {
        Some(ClipContent::Midi { sequence }) => {
            let events: Vec<&CtrlEvent> = sequence
                .ctrls
                .iter()
                .filter(|c| want.is_none_or(|k| same_lane(k, c.kind)) && ch.is_none_or(|ch| i64::from(c.channel) == ch))
                .collect();
            Ok(json!({"events": events}))
        }
        _ => Err(bad("midi.ctrl_list", "not a MIDI clip")),
    }
}

fn ctrl_set(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = clip_id(p, "midi.ctrl_set")?;
    let (kind, lo, hi) = lane(p, "midi.ctrl_set")?;
    let channel = i64_or(p, "channel", 0).clamp(0, 15) as u8;
    let points = p.get("points").and_then(Value::as_array).ok_or_else(|| bad("midi.ctrl_set", "`points` required"))?;
    let mut new = Vec::with_capacity(points.len().min(65_536));
    for pt in points.iter().take(65_536) {
        let tick = pt.get("tick").and_then(Value::as_i64).ok_or_else(|| bad("midi.ctrl_set", "each point needs `tick`"))?;
        let value = pt.get("value").and_then(Value::as_i64).ok_or_else(|| bad("midi.ctrl_set", "each point needs `value`"))?;
        new.push(CtrlEvent { tick: tick.max(0), channel, kind, value: value.clamp(i64::from(lo), i64::from(hi)) as i32 });
    }
    let seq = seq_mut(e, id, "midi.ctrl_set")?;
    seq.ctrls.retain(|c| !(c.channel == channel && same_lane(c.kind, kind) && new.iter().any(|n| n.tick == c.tick)));
    let added = new.len();
    seq.ctrls.extend(new);
    seq.sort();
    Ok(json!({"set": added, "events": seq.ctrls.len()}))
}

fn ctrl_clear(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = clip_id(p, "midi.ctrl_clear")?;
    let (kind, _, _) = lane(p, "midi.ctrl_clear")?;
    let channel = i64_or(p, "channel", 0).clamp(0, 15) as u8;
    let (start, end) = (i64_or(p, "start_ticks", 0), i64_or(p, "end_ticks", i64::MAX));
    let seq = seq_mut(e, id, "midi.ctrl_clear")?;
    let before = seq.ctrls.len();
    seq.ctrls.retain(|c| !(c.channel == channel && same_lane(c.kind, kind) && c.tick >= start && c.tick < end));
    Ok(json!({"deleted": before - seq.ctrls.len()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn clip(e: &mut Engine) -> u64 {
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        e.execute("midi.new_clip", &json!({"track": t.0})).unwrap()["clip"].as_u64().unwrap()
    }

    #[test]
    fn set_replaces_same_tick_and_clamps() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        e.execute("midi.ctrl_set", &json!({"clip": c, "lane": "cc", "cc": 1, "points": [{"tick": 0, "value": 10}, {"tick": 480, "value": 999}]}))
            .unwrap();
        e.execute("midi.ctrl_set", &json!({"clip": c, "lane": "cc", "cc": 1, "points": [{"tick": 0, "value": 50}]})).unwrap();
        let l = e.execute("midi.ctrl_list", &json!({"clip": c, "lane": "cc", "cc": 1})).unwrap();
        let ev = l["events"].as_array().unwrap();
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0]["value"], 50);
        assert_eq!(ev[1]["value"], 127);
    }

    #[test]
    fn lanes_are_independent_and_clear_is_scoped() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        e.execute("midi.ctrl_set", &json!({"clip": c, "lane": "cc", "cc": 64, "points": [{"tick": 0, "value": 127}, {"tick": 960, "value": 0}]}))
            .unwrap();
        e.execute("midi.ctrl_set", &json!({"clip": c, "lane": "cc", "cc": 11, "points": [{"tick": 0, "value": 90}]})).unwrap();
        e.execute("midi.ctrl_set", &json!({"clip": c, "lane": "pitch_bend", "points": [{"tick": 0, "value": -99999}]})).unwrap();
        let r = e.execute("midi.ctrl_clear", &json!({"clip": c, "lane": "cc", "cc": 64})).unwrap();
        assert_eq!(r["deleted"], 2);
        assert_eq!(e.execute("midi.ctrl_list", &json!({"clip": c})).unwrap()["events"].as_array().map(Vec::len), Some(2));
        let pb = e.execute("midi.ctrl_list", &json!({"clip": c, "lane": "pitch_bend"})).unwrap();
        assert_eq!(pb["events"][0]["value"], -8192);
    }

    #[test]
    fn bad_input_is_an_error_not_a_panic() {
        let mut e = Engine::default();
        let c = clip(&mut e);
        assert!(e.execute("midi.ctrl_set", &json!({"clip": c, "lane": "nope", "points": []})).is_err());
        assert!(e.execute("midi.ctrl_set", &json!({"clip": c, "lane": "cc", "points": [{"tick": 1}]})).is_err());
        assert!(e.execute("midi.ctrl_set", &json!({"clip": 99999, "lane": "cc", "points": []})).is_err());
    }
}
