//! Note audition: sound a note on a track's instrument without editing the document.

use super::*;
use crate::cmd;
use serde_json::json;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        noundo "midi.audition",
        "Audition Note",
        [],
        None,
        "{track, pitch: 0..127, velocity?: 100, on: bool} (presses or releases a live note on the track's instrument; no document change)",
        always,
        audition
    )]
}

fn audition(e: &mut Engine, p: &Value) -> Result<Value> {
    const ID: &str = "midi.audition";
    let track = track_param(e, ID, p, "track")?.ok_or_else(|| bad(ID, "`track` required"))?;
    let pitch = p.get("pitch").and_then(Value::as_u64).ok_or_else(|| bad(ID, "`pitch` required (0..127)"))?;
    let pitch = u8::try_from(pitch).ok().filter(|p| *p <= 127).ok_or_else(|| bad(ID, "`pitch` must be 0..127"))?;
    let on = p.get("on").and_then(Value::as_bool).ok_or_else(|| bad(ID, "`on` required (true or false)"))?;
    let velocity = match p.get("velocity") {
        None | Some(Value::Null) => 100,
        Some(v) => v.as_u64().ok_or_else(|| bad(ID, "`velocity` must be 1..127"))?.clamp(1, 127) as u8,
    };
    e.transport_requests.push(crate::TransportRequest::Audition { track, pitch, velocity, on });
    Ok(json!({"track": track.0, "pitch": pitch, "velocity": velocity, "on": on}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TransportRequest;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn engine() -> (Engine, u64) {
        let mut e = Engine::default();
        let t = e.session_mut().add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some("Keys"));
        e.transport_requests.clear();
        (e, t.0)
    }

    #[test]
    fn a_valid_call_pushes_one_request_and_no_undo_entry() {
        let (mut e, t) = engine();
        let label_before = e.undo_label().map(str::to_string);
        let rev = e.revision;
        e.execute("midi.audition", &json!({"track": t, "pitch": 60, "on": true})).unwrap();
        assert_eq!(
            e.transport_requests,
            vec![TransportRequest::Audition { track: soundcraft_model::TrackId(t), pitch: 60, velocity: 100, on: true }]
        );
        assert_eq!(e.undo_label().map(str::to_string), label_before, "no undo entry");
        assert_eq!(e.revision, rev, "the document is untouched");
    }

    #[test]
    fn velocity_and_track_name_are_honoured() {
        let (mut e, t) = engine();
        e.execute("midi.audition", &json!({"track": "Keys", "pitch": 127, "velocity": 64, "on": false})).unwrap();
        assert_eq!(
            e.transport_requests,
            vec![TransportRequest::Audition { track: soundcraft_model::TrackId(t), pitch: 127, velocity: 64, on: false }]
        );
    }

    #[test]
    fn bad_input_is_an_error_and_pushes_nothing() {
        let (mut e, t) = engine();
        for p in [
            json!({"track": 999_999, "pitch": 60, "on": true}),
            json!({"track": "Nope", "pitch": 60, "on": true}),
            json!({"pitch": 60, "on": true}),
            json!({"track": t, "pitch": 128, "on": true}),
            json!({"track": t, "pitch": -1, "on": true}),
            json!({"track": t, "on": true}),
            json!({"track": t, "pitch": 60}),
            json!({"track": t, "pitch": 60, "on": "yes"}),
            json!({"track": t, "pitch": 60, "velocity": "loud", "on": true}),
        ] {
            assert!(e.execute("midi.audition", &p).is_err(), "{p}");
        }
        assert!(e.transport_requests.is_empty());
    }
}
