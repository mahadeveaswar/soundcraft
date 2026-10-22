//! A locked automation lane keeps its points through every bulk automation writer; the unlocked
//! lanes the same command touches still change, and undo is never blocked.

use serde_json::{Value, json};
use soundcraft_engine::Engine;
use soundcraft_model::{AutoParam, Clip, ClipId, ShownLane, SourceId, TrackId};

fn trim_param() -> AutoParam {
    AutoParam::Plugin { slot: u8::MAX, param: "trim".into() }
}

fn run(e: &mut Engine, id: &str, p: Value) {
    e.execute(id, &p).unwrap();
}

fn new_track(e: &mut Engine) -> TrackId {
    TrackId(e.execute("track.new", &json!({"count": 1})).unwrap()["tracks"][0].as_u64().unwrap())
}

fn pts(e: &Engine, t: TrackId, p: &AutoParam) -> Vec<(i64, f32)> {
    e.session().track(t).and_then(|tr| tr.lane(p)).map(|l| l.points.iter().map(|x| (x.at, x.value)).collect()).unwrap_or_default()
}

fn lock(e: &mut Engine, t: TrackId, p: AutoParam) {
    let tr = e.session_mut().track_mut(t).unwrap();
    let mut l = ShownLane::new(p);
    l.locked = true;
    tr.shown_lanes.push(l);
}

fn put(e: &mut Engine, t: TrackId, p: &AutoParam, at: &[i64]) {
    let tr = e.session_mut().track_mut(t).unwrap();
    for a in at {
        tr.lane_mut(p).set_point(*a, -6.0);
    }
}

/// One track with volume (locked) and pan (unlocked) points at 300 and 1500.
fn setup() -> (Engine, TrackId) {
    let mut e = Engine::default();
    let t = new_track(&mut e);
    for p in [AutoParam::Volume, AutoParam::Pan(0)] {
        put(&mut e, t, &p, &[300, 1500]);
    }
    lock(&mut e, t, AutoParam::Volume);
    (e, t)
}

fn assert_undo_still_works(e: &mut Engine) {
    assert!(e.undo(), "undo is never blocked by a lock");
}

#[test]
fn cut_skips_locked_lanes() {
    let (mut e, t) = setup();
    run(&mut e, "edit.cut", json!({"tracks": [t.0], "start": 0, "end": 500}));
    assert_eq!(pts(&e, t, &AutoParam::Volume), vec![(300, -6.0), (1500, -6.0)]);
    assert_eq!(pts(&e, t, &AutoParam::Pan(0)), vec![(1500, -6.0)]);
    assert_undo_still_works(&mut e);
}

#[test]
fn paste_skips_locked_lanes() {
    let (mut e, t) = setup();
    run(&mut e, "edit.copy", json!({"tracks": [t.0], "start": 0, "end": 500}));
    run(&mut e, "edit.paste", json!({"tracks": [t.0], "at": 10_000}));
    assert_eq!(pts(&e, t, &AutoParam::Volume), vec![(300, -6.0), (1500, -6.0)]);
    assert!(pts(&e, t, &AutoParam::Pan(0)).iter().any(|p| p.0 >= 10_000), "the unlocked lane takes the paste");
    assert_undo_still_works(&mut e);
}

#[test]
fn write_to_current_skips_locked_lanes() {
    let (mut e, t) = setup();
    run(&mut e, "automation.write_to_all", json!({"tracks": [t.0], "start": 0, "end": 1000}));
    assert_eq!(pts(&e, t, &AutoParam::Volume), vec![(300, -6.0), (1500, -6.0)]);
    assert_ne!(pts(&e, t, &AutoParam::Pan(0)), vec![(300, -6.0), (1500, -6.0)], "the unlocked lane is written");
    assert_undo_still_works(&mut e);
}

#[test]
fn clip_moves_carrying_automation_skip_locked_lanes() {
    let (mut e, t) = setup();
    let s = e.session_mut();
    s.sources.push(soundcraft_model::Source {
        id: SourceId(8),
        name: "src".into(),
        path: "src.wav".into(),
        channels: 1,
        frames: 1_000_000,
        sample_rate: 48_000,
        format: soundcraft_audio_io::FileFormat::Wav,
        time_reference: 0,
        unsaved: false,
    });
    let ids: Vec<ClipId> = (0..2).map(|_| s.new_clip_id()).collect();
    s.track_mut(t).and_then(|t| t.playlist_mut()).unwrap().clips =
        vec![Clip::audio(ids[0], "a", SourceId(8), 0, 0, 1000), Clip::audio(ids[1], "b", SourceId(8), 0, 3000, 1000)];
    run(&mut e, "edit.mode", json!({"mode": "shuffle"}));
    run(&mut e, "edit.move_clips", json!({"clips": [ids[0].0], "by": 4000}));
    assert_eq!(pts(&e, t, &AutoParam::Volume).first(), Some(&(300, -6.0)), "the locked lane stays put");
    assert_ne!(pts(&e, t, &AutoParam::Pan(0)).first(), Some(&(300, -6.0)), "the unlocked lane moved with the clip");
    assert_undo_still_works(&mut e);
}

#[test]
fn trim_and_volume_helpers_skip_locked_lanes() {
    let mut e = Engine::default();
    let (a, b) = (new_track(&mut e), new_track(&mut e));
    lock(&mut e, a, trim_param());
    run(&mut e, "track.trim_automation_write", json!({"tracks": [a.0, b.0], "start": 0, "end": 1000, "db": -3.0}));
    assert!(pts(&e, a, &trim_param()).is_empty(), "locked trim lane untouched");
    assert!(!pts(&e, b, &trim_param()).is_empty(), "unlocked trim lane written");
    // Coalescing trim into volume skips a track whose volume lane is locked.
    put(&mut e, a, &trim_param(), &[10]);
    put(&mut e, b, &trim_param(), &[10]);
    put(&mut e, a, &AutoParam::Volume, &[50]);
    lock(&mut e, a, AutoParam::Volume);
    run(&mut e, "track.coalesce_trim", json!({"tracks": [a.0, b.0]}));
    assert_eq!(pts(&e, a, &AutoParam::Volume), vec![(50, -6.0)]);
    assert!(!pts(&e, a, &trim_param()).is_empty());
    assert!(pts(&e, b, &trim_param()).is_empty(), "the unlocked track was coalesced");
    assert_undo_still_works(&mut e);
}
