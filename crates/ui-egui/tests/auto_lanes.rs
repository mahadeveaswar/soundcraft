//! Automation lanes under tracks: row heights, breakpoint editing, locking and following.

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_model::{AutoParam, TrackId};
use soundcraft_ui_egui::auto_lanes_ui::{header_layout, lane_geometry, total_height};
use soundcraft_ui_egui::{Services, SoundApp};

fn harness() -> Harness<'static, SoundApp> {
    harness_sized(vec2(1600.0, 1000.0))
}

fn harness_sized(size: egui::Vec2) -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(size).with_step_dt(1.0 / 60.0).build_ui_state(
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

/// Press at `from`, move to `to` in `steps` frames, release; `mods` stay held for the whole drag.
fn drag(h: &mut Harness<SoundApp>, from: Pos2, to: Pos2, steps: usize, mods: Modifiers) {
    h.input_mut().events.push(Event::ModifiersChanged(mods));
    move_to(h, from);
    h.input_mut().events.push(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: mods });
    h.run_steps(2);
    for i in 1..=steps {
        let f = i as f32 / steps as f32;
        move_to(h, pos2(from.x + (to.x - from.x) * f, from.y + (to.y - from.y) * f));
    }
    h.input_mut().events.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: mods });
    h.run_steps(2);
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
}

fn pts(h: &Harness<SoundApp>, name: &str, p: &AutoParam) -> Vec<(i64, f32)> {
    h.state()
        .engine
        .session()
        .track(id(h, name))
        .and_then(|t| t.lane(p))
        .map(|l| l.points.iter().map(|x| (x.at, x.value)).collect())
        .unwrap_or_default()
}

fn set_tool(h: &mut Harness<SoundApp>, tool: &str) {
    h.state_mut().run("edit.tool", json!({"tool": tool})).unwrap();
    h.run_steps(3);
}

#[test]
fn the_pencil_draws_a_replacing_curve_in_one_undo_step() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    set_tool(&mut h, "pencil");
    let b = body(&h, "Kick", 0);
    let (from, to) = (pos2(b.min.x + 200.0, b.max.y - 10.0), pos2(b.min.x + 320.0, b.min.y + 10.0));
    let history = h.state().engine.undo_history().len();
    drag(&mut h, from, to, 12, Modifiers::NONE);
    let got = pts(&h, "Kick", &AutoParam::Volume);
    assert!(got.len() >= 2, "a drag leaves a curve: {got:?}");
    assert!(got.windows(2).all(|w| w[0].0 < w[1].0), "ordered");
    // The first and last points are anchors holding the old value; the stroke is what lies between.
    let inner = &got[1..got.len() - 1];
    assert!(inner.len() >= 2, "{got:?}");
    assert!(inner[inner.len() - 1].1 > inner[0].1 + 20.0, "values rise with the pointer: {inner:?}");
    assert_eq!(h.state().engine.undo_history().len(), history + 1, "one undo step per stroke");
    assert!(h.state_mut().engine.undo());
    assert!(pts(&h, "Kick", &AutoParam::Volume).is_empty(), "one undo restores the lane");
}

#[test]
fn the_grabber_still_moves_only_the_point_it_grabs() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    let b = body(&h, "Kick", 0);
    let spot = pos2(b.center().x + 100.0, b.min.y + b.height() * 0.3);
    click(&mut h, spot, PointerButton::Primary);
    click(&mut h, pos2(spot.x + 150.0, spot.y), PointerButton::Primary);
    let before = pts(&h, "Kick", &AutoParam::Volume);
    assert_eq!(before.len(), 2);
    drag(&mut h, spot, pos2(spot.x, spot.y + 30.0), 5, Modifiers::NONE);
    let after = pts(&h, "Kick", &AutoParam::Volume);
    assert_eq!(after.len(), 2, "no drawing with the Grabber");
    assert_eq!(after[1], before[1], "the other point is untouched");
    assert!(after[0].1 < before[0].1, "the grabbed point moved down");
}

#[test]
fn the_draw_button_and_the_command_modifier_draw_with_any_tool() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    set_tool(&mut h, "grabber");
    let t = id(&h, "Kick");
    let b = body(&h, "Kick", 0);
    let (from, to) = (pos2(b.min.x + 200.0, b.max.y - 10.0), pos2(b.min.x + 300.0, b.min.y + 10.0));
    h.state_mut().run("automation.lane_draw", json!({"track": t.0, "index": 0})).unwrap();
    h.run_steps(3);
    drag(&mut h, from, to, 10, Modifiers::NONE);
    let drawn = pts(&h, "Kick", &AutoParam::Volume);
    assert!(drawn.len() >= 2, "Draw on: a drag draws with the Grabber: {drawn:?}");
    h.state_mut().run("automation.lane_draw", json!({"track": t.0, "index": 0, "draw": false})).unwrap();
    h.state_mut().run("automation.clear", json!({"track": t.0, "param": "volume", "start": 0, "end": 1_000_000_000})).unwrap();
    h.run_steps(3);
    assert!(pts(&h, "Kick", &AutoParam::Volume).is_empty());
    drag(&mut h, from, to, 10, Modifiers { command: true, ctrl: !cfg!(target_os = "macos"), mac_cmd: cfg!(target_os = "macos"), ..Modifiers::NONE });
    assert!(pts(&h, "Kick", &AutoParam::Volume).len() >= 2, "the command modifier draws for one stroke");
    drag(&mut h, from, to, 10, Modifiers::NONE);
    assert!(!h.state().engine.undo_history().is_empty());
}

#[test]
fn shift_drag_erases_the_swept_range_and_a_locked_lane_refuses_the_pencil() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    set_tool(&mut h, "pencil");
    let t = id(&h, "Kick");
    let b = body(&h, "Kick", 0);
    let (tl, zoom) = (h.state().edit_layout.timeline, h.state().engine.session().edit.zoom);
    let at_x = |x: f32| zoom.scroll + ((x - tl[0]) as f64 * zoom.samples_per_px) as i64;
    for i in 0..40 {
        let value = if i % 2 == 0 { -40.0 } else { 0.0 };
        h.state_mut()
            .run("automation.set_point", json!({"track": t.0, "param": "volume", "at": at_x(b.min.x + 200.0 + i as f32 * 5.0), "value": value}))
            .unwrap();
    }
    let (from_at, to_at) = (at_x(b.min.x + 250.0), at_x(b.min.x + 350.0));
    let inside = |h: &Harness<SoundApp>| pts(h, "Kick", &AutoParam::Volume).iter().filter(|p| p.0 > from_at + 40_000 && p.0 < to_at - 40_000).count();
    assert!(inside(&h) > 5);
    let history = h.state().engine.undo_history().len();
    drag(&mut h, pos2(b.min.x + 250.0, b.center().y), pos2(b.min.x + 350.0, b.center().y), 10, Modifiers::SHIFT);
    assert_eq!(inside(&h), 0, "Shift+drag removed the swept points");
    assert!(pts(&h, "Kick", &AutoParam::Volume).len() > 10, "points outside the sweep stay");
    assert_eq!(h.state().engine.undo_history().len(), history + 1, "the erase is one undo step");
    h.state_mut().run("automation.lane_lock", json!({"track": t.0, "index": 0})).unwrap();
    h.run_steps(3);
    let locked = pts(&h, "Kick", &AutoParam::Volume);
    drag(&mut h, pos2(b.min.x + 450.0, b.max.y - 15.0), pos2(b.min.x + 520.0, b.min.y + 8.0), 8, Modifiers::NONE);
    assert_eq!(pts(&h, "Kick", &AutoParam::Volume), locked, "a locked lane is not drawn on");
    assert_eq!(h.state().ui.status, "Lane is locked");
}

#[test]
fn drawing_on_a_mute_lane_only_writes_off_and_on() {
    let mut h = harness();
    show(&mut h, "Kick", "mute");
    set_tool(&mut h, "pencil");
    let b = body(&h, "Kick", 0);
    drag(&mut h, pos2(b.min.x + 200.0, b.max.y - 8.0), pos2(b.min.x + 320.0, b.min.y + 8.0), 12, Modifiers::NONE);
    let got = pts(&h, "Kick", &AutoParam::Mute);
    assert!(got.len() >= 2 && got.iter().all(|(_, v)| *v == 0.0 || *v == 1.0), "{got:?}");
    assert!(got.iter().any(|(_, v)| *v == 1.0));
    assert!(got.windows(2).all(|w| w[0].1 != w[1].1), "a stepped stroke keeps only the changes, not a point per sample: {got:?}");
}

#[test]
fn the_track_view_pencil_is_one_undo_step() {
    let mut h = harness();
    let t = id(&h, "Kick");
    h.state_mut().run("track.view", json!({"tracks": [t.0], "view": "volume"})).unwrap();
    set_tool(&mut h, "pencil");
    h.run_steps(3);
    let row = h.state().edit_layout.rows.iter().find(|(i, _)| *i == t.0).unwrap().1;
    let tl = h.state().edit_layout.timeline;
    let (y0, y1) = (row[3] - 8.0, row[1] + 8.0);
    let history = h.state().engine.undo_history().len();
    drag(&mut h, pos2(tl[0] + 200.0, y0), pos2(tl[0] + 320.0, y1), 12, Modifiers::NONE);
    let got = pts(&h, "Kick", &AutoParam::Volume);
    assert!(got.len() >= 2, "{got:?}");
    assert_eq!(h.state().engine.undo_history().len(), history + 1, "one undo step, not one per point");
    assert!(h.state_mut().engine.undo());
    assert!(pts(&h, "Kick", &AutoParam::Volume).is_empty());
}

#[test]
fn alt_pencil_draws_one_straight_ramp_in_one_undo_step() {
    let mut h = harness();
    show(&mut h, "Kick", "volume");
    set_tool(&mut h, "pencil");
    let b = body(&h, "Kick", 0);
    let (from, to) = (pos2(b.min.x + 200.0, b.max.y - 10.0), pos2(b.min.x + 400.0, b.min.y + 10.0));
    let history = h.state().engine.undo_history().len();
    // A wiggling path: the ramp ignores everything but the two ends.
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::ALT));
    move_to(&mut h, from);
    h.input_mut().events.push(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::ALT });
    h.run_steps(2);
    for (dx, dy) in [(40.0, -60.0), (80.0, 0.0), (120.0, -10.0), (200.0, -10.0)] {
        move_to(&mut h, pos2(from.x + dx, (from.y + dy).clamp(b.min.y, b.max.y)));
    }
    move_to(&mut h, to);
    h.input_mut().events.push(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::ALT });
    h.run_steps(2);
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.run_steps(2);
    let got = pts(&h, "Kick", &AutoParam::Volume);
    // Anchors hold the old value at both ends; the ramp is what lies between them.
    let inner = &got[1..got.len().saturating_sub(1)];
    assert!(inner.len() >= 3, "{got:?}");
    let (a, z) = (inner[0], inner[inner.len() - 1]);
    for p in inner {
        let f = (p.0 - a.0) as f64 / (z.0 - a.0) as f64;
        let want = f64::from(a.1) + f64::from(z.1 - a.1) * f;
        assert!((f64::from(p.1) - want).abs() < 0.5, "straight line: {p:?} vs {want}");
    }
    assert!(z.1 > a.1 + 20.0, "rises with the pointer");
    assert_eq!(h.state().engine.undo_history().len(), history + 1, "one undo step");
    assert!(h.state_mut().engine.undo());
    assert!(pts(&h, "Kick", &AutoParam::Volume).is_empty());
}

#[test]
fn alt_pencil_on_a_stepped_lane_only_holds_off_and_on() {
    let mut h = harness();
    show(&mut h, "Kick", "mute");
    set_tool(&mut h, "pencil");
    let b = body(&h, "Kick", 0);
    drag(&mut h, pos2(b.min.x + 200.0, b.max.y - 8.0), pos2(b.min.x + 320.0, b.min.y + 8.0), 12, Modifiers::ALT);
    let got = pts(&h, "Kick", &AutoParam::Mute);
    assert!(got.len() >= 2 && got.iter().all(|(_, v)| *v == 0.0 || *v == 1.0), "{got:?}");
    assert!(got.windows(2).all(|w| w[0].1 != w[1].1), "repeated holds are thinned away: {got:?}");
}

#[test]
fn midi_controller_lane_menu_converts_the_lane() {
    let mut h = harness_sized(vec2(1600.0, 1800.0));
    let tid = id(&h, "Keys");
    h.state_mut().run("automation.lane_show", json!({"track": tid.0, "param": "cc1"})).unwrap();
    for (at, v) in [(0, 10.0), (48_000, 100.0)] {
        h.state_mut().run("automation.set_point", json!({"track": tid.0, "param": "cc1", "at": at, "value": v})).unwrap();
    }
    h.run_steps(4);
    let b = body(&h, "Keys", 0);
    let head = pos2(b.min.x - 4.0, b.center().y + 12.0);
    click(&mut h, head, PointerButton::Secondary);
    h.run_steps(3);
    h.get_by_label("Convert to MIDI Clip Events").click();
    h.run_steps(4);
    assert!(
        h.state().engine.session().track(tid).unwrap().clips().iter().any(|c| matches!(c.content, soundcraft_model::ClipContent::Midi { .. })),
        "a clip was written"
    );
}
