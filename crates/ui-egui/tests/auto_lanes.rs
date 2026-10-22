//! Automation lanes under tracks: row heights, breakpoint editing, locking and following.

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;
use soundcraft_model::{AutoParam, TrackId};
use soundcraft_ui_egui::auto_lanes_ui::{header_layout, lane_geometry, total_height};
use soundcraft_ui_egui::{Services, SoundApp};

fn harness() -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    h
}

fn id(h: &Harness<SoundApp>, name: &str) -> TrackId {
    h.state().engine.session().track_by_name(name).map(|t| t.id).unwrap()
}

fn show(h: &mut Harness<SoundApp>, name: &str, param: &str) {
    let t = id(h, name);
    h.state_mut().run("automation.lane_show", json!({"track": t.0, "param": param})).unwrap();
    h.run_steps(4);
}

/// The body rectangle of lane `index` of `name`.
fn body(h: &Harness<SoundApp>, name: &str, index: usize) -> Rect {
    let t = id(h, name);
    let row = h.state().edit_layout.rows.iter().find(|(i, _)| *i == t.0).unwrap().1;
    let tl = h.state().edit_layout.timeline;
    let track = h.state().engine.session().track(t).unwrap().clone();
    lane_geometry(Rect::from_min_max(pos2(row[0], row[1]), pos2(row[2], row[3])), tl[0], &track)[index].body
}

fn move_to(h: &mut Harness<SoundApp>, p: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(p));
    h.run_steps(2);
}

fn button(h: &mut Harness<SoundApp>, p: Pos2, b: PointerButton, pressed: bool) {
    h.input_mut().events.push(Event::PointerButton { pos: p, button: b, pressed, modifiers: Modifiers::NONE });
    h.run_steps(2);
}

fn click(h: &mut Harness<SoundApp>, p: Pos2, b: PointerButton) {
    move_to(h, p);
    button(h, p, b, true);
    button(h, p, b, false);
}

fn points(h: &Harness<SoundApp>, name: &str, p: &AutoParam) -> usize {
    h.state().engine.session().track(id(h, name)).and_then(|t| t.lane(p).map(|l| l.points.len())).unwrap_or(0)
}

#[test]
fn rows_grow_by_the_lane_heights_and_later_tracks_move_down() {
    let mut h = harness();
    let rows = |h: &Harness<SoundApp>| h.state().edit_layout.rows.clone();
    let before = rows(&h);
    let content_before = h.state().edit_layout.content_h;
    show(&mut h, "Kick", "volume");
    show(&mut h, "Kick", "pan");
    show(&mut h, "Snare", "mute");
    let after = rows(&h);
    let kick = h.state().engine.session().track(id(&h, "Kick")).unwrap().clone();
    let snare = h.state().engine.session().track(id(&h, "Snare")).unwrap().clone();
    let grow = |i: usize| (after[i].1[3] - after[i].1[1]) - (before[i].1[3] - before[i].1[1]);
    assert!((grow(0) - total_height(&kick)).abs() < 0.01 && total_height(&kick) > 100.0);
    assert!((grow(1) - total_height(&snare)).abs() < 0.01);
    assert!((after[2].1[1] - before[2].1[1] - total_height(&kick) - total_height(&snare)).abs() < 0.01, "the third track moved down by both");
    assert!(h.state().edit_layout.content_h > content_before + 150.0, "the scroll range includes the lanes");
}

#[test]
fn clicking_a_lane_adds_a_point_right_click_deletes_and_drag_is_one_undo_step() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    show(&mut h, "Snare", "pan");
    let vol = AutoParam::Volume;
    let b = body(&h, "Kick", 0);
    let spot = pos2(b.center().x + 100.0, b.min.y + b.height() * 0.3);
    click(&mut h, spot, PointerButton::Primary);
    assert_eq!(points(&h, "Kick", &vol), 1, "a click adds a breakpoint");
    assert_eq!(points(&h, "Snare", &AutoParam::Pan(0)), 0, "other lanes are untouched");
    let value = h.state().engine.session().track(id(&h, "Kick")).unwrap().lane(&vol).unwrap().points[0].value;
    assert!(value > -30.0 && value < 12.0, "value from the height: {value}");
    // Drag it to the lower right.
    let history = h.state().engine.undo_history().len();
    let to = pos2(spot.x + 80.0, b.min.y + b.height() * 0.8);
    move_to(&mut h, spot);
    button(&mut h, spot, PointerButton::Primary, true);
    for i in 1..=6 {
        move_to(&mut h, pos2(spot.x + 80.0 * i as f32 / 6.0, spot.y + (to.y - spot.y) * i as f32 / 6.0));
    }
    button(&mut h, to, PointerButton::Primary, false);
    let moved = h.state().engine.session().track(id(&h, "Kick")).unwrap().lane(&vol).unwrap().points[0];
    assert_eq!(points(&h, "Kick", &vol), 1);
    assert!(moved.value < value, "dragged down: {} < {value}", moved.value);
    assert_eq!(h.state().engine.undo_history().len(), history + 1, "one undo step for the whole drag");
    assert!(h.state_mut().engine.undo());
    let back = h.state().engine.session().track(id(&h, "Kick")).unwrap().lane(&vol).unwrap().points[0];
    assert!((back.value - value).abs() < 1e-4 && back.at != moved.at, "undo restores the point");
    // Right-click on the point removes it.
    let t = h.state().edit_layout.timeline;
    let x = t[0] + ((back.at - h.state().engine.session().edit.zoom.scroll) as f64 / h.state().engine.session().edit.zoom.samples_per_px) as f32;
    let range = (-60.0f32, 12.0f32);
    let y = b.max.y - 4.0 - ((back.value - range.0) / (range.1 - range.0)) * (b.height() - 8.0);
    click(&mut h, pos2(x, y), PointerButton::Secondary);
    assert_eq!(points(&h, "Kick", &vol), 0, "right-click deletes the point");
}

#[test]
fn a_locked_lane_refuses_edits() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    let t = id(&h, "Kick");
    h.state_mut().run("automation.lane_lock", json!({"track": t.0, "index": 0})).unwrap();
    h.run_steps(3);
    let b = body(&h, "Kick", 0);
    click(&mut h, pos2(b.center().x, b.center().y), PointerButton::Primary);
    assert_eq!(points(&h, "Kick", &AutoParam::Volume), 0);
    h.state_mut().run("automation.lane_lock", json!({"track": t.0, "index": 0, "locked": false})).unwrap();
    h.run_steps(3);
    click(&mut h, pos2(b.center().x, b.center().y), PointerButton::Primary);
    assert_eq!(points(&h, "Kick", &AutoParam::Volume), 1);
}

#[test]
fn a_following_lane_switches_to_the_parameter_that_moves() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    let t = id(&h, "Kick");
    h.state_mut().run("automation.lane_follow", json!({"track": t.0, "index": 0, "on": true})).unwrap();
    h.run_steps(4);
    let lane = |h: &Harness<SoundApp>| h.state().engine.session().track(t).unwrap().shown_lanes[0].param.clone();
    assert_eq!(lane(&h), AutoParam::Volume);
    h.state_mut().run("mix.pan", json!({"track": t.0, "pan": 0.4})).unwrap();
    h.run_steps(4);
    assert_eq!(lane(&h), AutoParam::Pan(0), "the lane follows the pan that moved");
    let slot = h.state().engine.session().track(t).unwrap().mixer.inserts.iter().position(Option::is_some).unwrap();
    let param = {
        let ins = h.state().engine.session().track(t).unwrap().mixer.inserts[slot].clone().unwrap();
        soundcraft_dsp::plugin_info(&ins.plugin).unwrap().params[0].id
    };
    let ins = h.state().engine.session().track(t).unwrap().mixer.inserts[slot].clone().unwrap();
    let info = soundcraft_dsp::plugin_info(&ins.plugin).unwrap().params[0].clone();
    let newv = if (info.max - info.default).abs() > 1e-3 { info.max } else { info.min };
    h.state_mut().run("mix.insert_param", json!({"track": t.0, "slot": slot, "param": param, "value": newv})).unwrap();
    h.run_steps(4);
    assert!(matches!(lane(&h), AutoParam::Plugin { .. }), "and a plugin parameter: {:?}", lane(&h));
    let recent = soundcraft_ui_egui::auto_lanes_ui::recent_params(&h.ctx, t);
    assert!(matches!(recent.first(), Some(AutoParam::Plugin { .. })), "the menu lists it first: {recent:?}");
    // With follow off nothing switches.
    h.state_mut().run("automation.lane_follow", json!({"track": t.0, "index": 0, "on": false})).unwrap();
    h.state_mut().run("mix.volume", json!({"track": t.0, "db": -9.0})).unwrap();
    h.run_steps(4);
    assert!(matches!(lane(&h), AutoParam::Plugin { .. }));
}

#[test]
fn old_sessions_without_lanes_draw_normally() {
    let h = harness();
    assert!(h.state().engine.session().tracks.iter().all(|t| t.shown_lanes.is_empty()));
    assert!(h.state().edit_layout.rows.len() >= 8);
}

#[test]
fn the_sustain_toggle_writes_127_and_0() {
    let mut h = harness();
    let t = id(&h, "Keys").0;
    h.state_mut().run("automation.lane_show", json!({"track": t, "param": "cc64"})).unwrap();
    h.run_steps(4);
    let track = h.state().engine.session().track(TrackId(t)).unwrap().clone();
    let row = h.state().edit_layout.rows.iter().find(|(i, _)| *i == t).unwrap().1;
    let tl = h.state().edit_layout.timeline;
    let geom = lane_geometry(Rect::from_min_max(pos2(row[0], row[1]), pos2(row[2], row[3])), tl[0], &track)[0];
    let value = header_layout(geom.header).value;
    let cc64 = AutoParam::MidiCc(64);
    let last = |h: &Harness<SoundApp>| {
        h.state().engine.session().track(TrackId(t)).unwrap().lane(&cc64).map(|l| l.points.iter().map(|p| p.value).collect::<Vec<_>>())
    };
    click(&mut h, value.center(), PointerButton::Primary);
    assert_eq!(last(&h), Some(vec![127.0]), "first click turns sustain on");
    click(&mut h, value.center(), PointerButton::Primary);
    assert_eq!(last(&h), Some(vec![0.0]), "second click turns it off at the same position");
    // Audio tracks keep refusing MIDI lanes.
    let kick = id(&h, "Kick");
    assert!(h.state_mut().run("automation.lane_show", json!({"track": kick.0, "param": "cc64"})).is_err());
}
