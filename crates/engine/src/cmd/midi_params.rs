//! Per-track MIDI playback parameters (transpose, velocity offset, channel override) and the
//! audition release tail. All of them are numeric settings saved with the session
//! (`rtp.<track>.transpose`, `rtp.<track>.velocity`, `midi.channel.<track>`,
//! `options.audition_tail_ms`), so no session field changes and old sessions load unchanged. The
//! mix engine reads them at the start of every block.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{Session, TrackId};

/// Largest transpose, in semitones, either way.
pub const MAX_TRANSPOSE: f64 = 48.0;
/// Largest velocity offset either way.
pub const MAX_VELOCITY: f64 = 127.0;
/// Default and largest audition release tail, in milliseconds.
pub const DEFAULT_TAIL_MS: f64 = 300.0;
pub const MAX_TAIL_MS: f64 = 2000.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "track.midi_params",
            "MIDI Track Parameters",
            [],
            None,
            "{tracks?, transpose?: -48..48 semitones, velocity?: -127..127 offset, channel?: 1..16 | null (clear)} -> {tracks: [{track, transpose, velocity, channel}]}. Non-destructive: the notes in the clips are untouched. Changing transpose or channel while notes sound releases them first",
            has_tracks,
            midi_params
        ),
        cmd!(query "track.midi_params_get", "Get MIDI Track Parameters", [], None, "{tracks?} -> {tracks: [{track, transpose, velocity, channel}]}", always, midi_params_get),
        cmd!(
            noundo "options.audition_tail_ms",
            "Audition Release Tail",
            [],
            None,
            "{value?: 0..2000} -> {value} (milliseconds an auditioned note's release keeps sounding; default 300; no value reads it)",
            always,
            audition_tail
        ),
    ]
}

/// One track's parameters as JSON.
pub fn params_of(s: &Session, t: TrackId) -> Value {
    let channel = s.edit.values.get(&format!("midi.channel.{}", t.0)).copied().filter(|c| c.is_finite() && (1.0..=16.0).contains(c));
    json!({
        "track": t.0,
        "transpose": s.edit.value(&format!("rtp.{}.transpose", t.0), 0.0).clamp(-127.0, 127.0),
        "velocity": s.edit.value(&format!("rtp.{}.velocity", t.0), 0.0).clamp(-127.0, 127.0),
        "channel": channel,
    })
}

fn number(p: &Value, key: &str, cmd: &str, lo: f64, hi: f64) -> Result<Option<f64>> {
    match p.get(key) {
        None => Ok(None),
        Some(v) => {
            let n = v.as_f64().filter(|n| n.is_finite()).ok_or_else(|| bad(cmd, format!("`{key}` must be a number")))?;
            Ok(Some(n.round().clamp(lo, hi)))
        }
    }
}

fn midi_params(e: &mut Engine, p: &Value) -> Result<Value> {
    const C: &str = "track.midi_params";
    let tracks = tracks_required(e, C, p)?;
    for t in &tracks {
        if !e.session().track(*t).is_some_and(|tr| tr.kind.is_midi()) {
            return Err(bad(C, "only instrument and MIDI tracks have MIDI parameters"));
        }
    }
    let transpose = number(p, "transpose", C, -MAX_TRANSPOSE, MAX_TRANSPOSE)?;
    let velocity = number(p, "velocity", C, -MAX_VELOCITY, MAX_VELOCITY)?;
    // `channel: null` clears the override.
    let channel = match p.get("channel") {
        None => None,
        Some(Value::Null) => Some(None),
        Some(_) => Some(number(p, "channel", C, 1.0, 16.0)?),
    };
    if transpose.is_none() && velocity.is_none() && channel.is_none() {
        return Err(bad(C, "give at least one of `transpose`, `velocity`, `channel`"));
    }
    let s = e.session_mut();
    for t in &tracks {
        let mut put = |key: String, v: Option<f64>| match v {
            Some(v) if v != 0.0 => {
                s.edit.values.insert(key, v);
            }
            _ => {
                s.edit.values.remove(&key);
            }
        };
        if let Some(v) = transpose {
            put(format!("rtp.{}.transpose", t.0), Some(v));
        }
        if let Some(v) = velocity {
            put(format!("rtp.{}.velocity", t.0), Some(v));
        }
        if let Some(c) = channel {
            put(format!("midi.channel.{}", t.0), c);
        }
    }
    let s = e.session();
    Ok(json!({"tracks": tracks.iter().map(|t| params_of(s, *t)).collect::<Vec<_>>()}))
}

fn midi_params_get(e: &mut Engine, p: &Value) -> Result<Value> {
    let tracks = tracks_param(e, "track.midi_params_get", p)?;
    let s = e.session();
    Ok(json!({"tracks": tracks.iter().filter(|t| s.track(**t).is_some_and(|tr| tr.kind.is_midi())).map(|t| params_of(s, *t)).collect::<Vec<_>>()}))
}

fn audition_tail(e: &mut Engine, p: &Value) -> Result<Value> {
    const C: &str = "options.audition_tail_ms";
    if let Some(v) = number(p, "value", C, 0.0, MAX_TAIL_MS)? {
        // The default is not stored: a session that never set it has no key.
        if v == DEFAULT_TAIL_MS {
            e.session_mut().edit.values.remove("options.audition_tail_ms");
        } else {
            e.session_mut().edit.values.insert("options.audition_tail_ms".into(), v);
        }
    }
    Ok(json!({"value": e.session().edit.value("options.audition_tail_ms", DEFAULT_TAIL_MS).clamp(0.0, MAX_TAIL_MS)}))
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

    #[test]
    fn values_clamp_and_zero_clears() {
        let (mut e, t) = setup();
        let r = e.execute("track.midi_params", &json!({"track": t, "transpose": 99, "velocity": -500, "channel": 5})).unwrap();
        assert_eq!(r["tracks"][0]["transpose"], 48.0);
        assert_eq!(r["tracks"][0]["velocity"], -127.0);
        assert_eq!(r["tracks"][0]["channel"], 5.0);
        let r = e.execute("track.midi_params", &json!({"track": t, "transpose": 0, "velocity": 0, "channel": null})).unwrap();
        assert_eq!(r["tracks"][0]["transpose"], 0.0);
        assert!(r["tracks"][0]["channel"].is_null());
        assert!(e.session().edit.values.is_empty(), "defaults leave no keys behind");
        let r = e.execute("track.midi_params", &json!({"track": t, "channel": 99})).unwrap();
        assert_eq!(r["tracks"][0]["channel"], 16.0);
    }

    #[test]
    fn it_refuses_audio_tracks_and_empty_calls_and_is_one_undo_step() {
        let (mut e, t) = setup();
        let a = e.session_mut().add_track(TrackKind::Audio, ChannelFormat::Mono, Some("Vox"));
        assert!(e.execute("track.midi_params", &json!({"track": a.0, "transpose": 1})).is_err());
        assert!(e.execute("track.midi_params", &json!({"track": t})).is_err());
        assert!(e.execute("track.midi_params", &json!({"track": t, "transpose": "up"})).is_err());
        let before = e.undo_history().len();
        e.execute("track.midi_params", &json!({"track": t, "transpose": 7})).unwrap();
        assert_eq!(e.undo_history().len(), before + 1);
        assert!(e.undo());
        assert_eq!(e.execute("track.midi_params_get", &json!({"track": t})).unwrap()["tracks"][0]["transpose"], 0.0);
    }

    #[test]
    fn audition_tail_defaults_clamps_and_reads_back() {
        let mut e = Engine::default();
        assert_eq!(e.execute("options.audition_tail_ms", &json!({})).unwrap()["value"], 300.0);
        assert_eq!(e.execute("options.audition_tail_ms", &json!({"value": 5000})).unwrap()["value"], 2000.0);
        assert_eq!(e.execute("options.audition_tail_ms", &json!({"value": -3})).unwrap()["value"], 0.0);
        assert!(e.execute("options.audition_tail_ms", &json!({"value": "long"})).is_err());
        e.execute("options.audition_tail_ms", &json!({"value": 300})).unwrap();
        assert!(e.session().edit.values.is_empty());
    }
}
