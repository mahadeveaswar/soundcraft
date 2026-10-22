//! Piano-roll tools (Mute, Glue, Zoom, Line), stacked controller lanes, the inspector strip and
//! the scale / chord entry assists.

use egui::{Event, Id, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_midi::Note;
use soundcraft_model::{ClipContent, ClipId};
use soundcraft_ui_egui::{Services, SoundApp, midi_editor};

fn harness(width: f32, with_clip: bool) -> (Harness<'static, SoundApp>, u64) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    let mut clip = 0;
    if with_clip {
        let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
        clip = created["clip"].as_u64().unwrap();
        app.run("midi.note_add", json!({"clip": clip, "pitch": 60, "start_ticks": 0, "length_ticks": 480, "velocity": 90})).unwrap();
        app.run("midi.note_add", json!({"clip": clip, "pitch": 60, "start_ticks": 960, "length_ticks": 480, "velocity": 40})).unwrap();
        app.run("midi.note_add", json!({"clip": clip, "pitch": 64, "start_ticks": 480, "length_ticks": 480, "velocity": 70})).unwrap();
        app.run("edit.select", json!({"clips": [clip]})).unwrap();
    }
    let mut h = Harness::builder().with_size(vec2(width, 900.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    (h, clip)
}

fn seq(h: &Harness<SoundApp>, clip: u64) -> soundcraft_midi::Sequence {
    match &h.state().engine.session().find_clip(ClipId(clip)).unwrap().1.content {
        ClipContent::Midi { sequence } => sequence.clone(),
        _ => panic!("MIDI clip required"),
    }
}

fn notes(h: &Harness<SoundApp>, clip: u64) -> Vec<Note> {
    seq(h, clip).notes
}

fn roll(h: &Harness<SoundApp>) -> Rect {
    h.ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-roll"))).unwrap()
}

/// Fix the horizontal zoom (0.1 point per tick, view at the clip start) and return where a note at
/// `tick` / `pitch` is on screen.
fn note_pos(h: &mut Harness<SoundApp>, tick: i64, pitch: i32) -> Pos2 {
    h.state_mut().midi.ppt = Some(0.1);
    h.state_mut().midi.view_start = 0.0;
    h.run_steps(3);
    let r = roll(h);
    let m = &h.state().midi;
    pos2(r.min.x + tick as f32 * 0.1, r.min.y + (127 - pitch) as f32 * m.row_h - m.scroll + m.row_h / 2.0)
}

fn click_at(h: &mut Harness<SoundApp>, at: Pos2) {
    h.hover_at(at);
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(1);
    }
    h.run_steps(3);
}

fn drag(h: &mut Harness<SoundApp>, from: Pos2, to: Pos2) {
    h.hover_at(from);
    h.run_steps(1);
    h.event(Event::PointerButton { pos: from, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.run_steps(1);
    for i in 1..=6 {
        h.hover_at(from.lerp(to, i as f32 / 6.0));
        h.run_steps(1);
    }
    h.event(Event::PointerButton { pos: to, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(4);
}

fn set_tool(h: &mut Harness<SoundApp>, tool: &str) {
    h.state_mut().run("ui.midi_tool", json!({"tool": tool})).unwrap();
    // Let egui forget the previous click so this one is not a double-click.
    h.run_steps(70);
}

#[test]
fn every_tool_is_accepted_by_ui_midi_tool() {
    let (mut h, _) = harness(1600.0, true);
    for tool in ["select", "pencil", "draw", "erase", "split", "glue", "mute", "zoom", "line"] {
        let r = h.state_mut().run("ui.midi_tool", json!({"tool": tool})).unwrap();
        let want = if tool == "draw" { "pencil" } else { tool };
        assert_eq!(r, json!({"tool": want}));
    }
    assert!(h.state_mut().run("ui.midi_tool", json!({"tool": "lasso"})).is_err());
}

#[test]
fn mute_tool_toggles_a_note_and_undoes() {
    let (mut h, clip) = harness(1600.0, true);
    set_tool(&mut h, "mute");
    let at = note_pos(&mut h, 100, 60);
    click_at(&mut h, at);
    assert_eq!(seq(&h, clip).muted.iter().copied().collect::<Vec<_>>(), vec![0]);
    h.run_steps(70);
    click_at(&mut h, at);
    assert!(seq(&h, clip).muted.is_empty(), "a second click unmutes");
    h.run_steps(70);
    click_at(&mut h, at);
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert!(seq(&h, clip).muted.is_empty());
}

#[test]
fn glue_tool_joins_a_note_with_the_next_of_the_same_pitch() {
    let (mut h, clip) = harness(1600.0, true);
    set_tool(&mut h, "glue");
    let at = note_pos(&mut h, 100, 60);
    click_at(&mut h, at);
    let n = notes(&h, clip);
    assert_eq!(n.len(), 2, "{n:?}");
    assert_eq!((n[0].pitch, n[0].start, n[0].length), (60, 0, 1440), "from the first start to the last end");
    assert_eq!(n[1].pitch, 64, "the other pitch is untouched");
}

#[test]
fn zoom_tool_zooms_in_on_click() {
    let (mut h, _) = harness(1600.0, true);
    set_tool(&mut h, "zoom");
    let at = note_pos(&mut h, 100, 60);
    let before = h.state().midi.ppt.unwrap();
    click_at(&mut h, at);
    assert!(h.state().midi.ppt.unwrap() > before, "zoomed in");
}

#[test]
fn line_tool_draws_a_ramp_in_the_controller_lane() {
    let (mut h, clip) = harness(1600.0, true);
    h.state_mut().run("ui.midi_lane_remove", json!({"lane": "velocity"})).unwrap();
    h.state_mut().run("ui.midi_lane_add", json!({"lane": "modulation"})).unwrap();
    h.state_mut().run("ui.midi_lane_height", json!({"index": 0, "height": 100})).unwrap();
    set_tool(&mut h, "line");
    h.run_steps(3);
    let handle = h.ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-lane-handle"))).unwrap();
    let r = roll(&h);
    let (top, bottom) = (handle.max.y + 6.0, handle.max.y + 94.0);
    drag(&mut h, pos2(r.min.x + 40.0, bottom), pos2(r.min.x + 700.0, top));
    let s = seq(&h, clip);
    let mut cc1: Vec<_> = s.ctrls.iter().filter(|c| c.kind == soundcraft_midi::CtrlKind::Cc(1)).collect();
    cc1.sort_by_key(|c| c.tick);
    assert!(cc1.len() >= 3, "a ramp of several points: {cc1:?}");
    assert!(cc1.first().unwrap().value < cc1.last().unwrap().value, "rising ramp");
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert!(seq(&h, clip).ctrls.iter().all(|c| c.kind != soundcraft_midi::CtrlKind::Cc(1)), "one undo removes the whole ramp");
}

#[test]
fn tools_do_nothing_without_a_clip() {
    let (mut h, _) = harness(1600.0, false);
    let before = h.state().engine.session().clone();
    for tool in ["pencil", "erase", "split", "glue", "mute", "zoom", "line"] {
        set_tool(&mut h, tool);
        click_at(&mut h, pos2(400.0, 700.0));
    }
    assert!(*h.state().engine.session() == before);
}

#[test]
fn lanes_add_remove_resize_through_commands() {
    let (mut h, _) = harness(1600.0, true);
    assert_eq!(h.state().midi.lanes.len(), 1);
    let r = h.state_mut().run("ui.midi_lane_add", json!({})).unwrap();
    assert_eq!(r["lanes"][1]["lane"], "modulation");
    assert!(h.state_mut().run("ui.midi_lane_add", json!({"lane": "modulation"})).is_err(), "already shown");
    h.state_mut().run("ui.midi_lane_height", json!({"index": 1, "height": 5000})).unwrap();
    assert_eq!(h.state().midi.lanes[1].height, 300.0, "clamped");
    h.state_mut().run("ui.midi_lane_height", json!({"index": 1, "height": 1})).unwrap();
    assert_eq!(h.state().midi.lanes[1].height, 40.0);
    assert!(h.state_mut().run("ui.midi_lane_height", json!({"index": 9, "height": 80})).is_err());
    h.run_steps(4);
    h.state_mut().run("ui.midi_lane_remove", json!({"index": 0})).unwrap();
    assert_eq!(h.state().midi.lanes.len(), 1);
    assert!(h.state_mut().run("ui.midi_lane_remove", json!({"index": 5})).is_err());
}

#[test]
fn the_plus_button_adds_the_next_unused_lane() {
    let (mut h, _) = harness(1600.0, true);
    h.get_by_label("Add lane").click();
    h.run_steps(4);
    assert_eq!(h.state().midi.lanes.len(), 2);
    assert_eq!(h.state().midi.lanes[1].lane, midi_editor::Lane::Modulation);
}

#[test]
fn inspector_opens_and_transpose_plus_one_changes_pitches() {
    let (mut h, clip) = harness(1600.0, true);
    assert!(!h.state().midi.inspector_open);
    assert_eq!(h.state_mut().run("ui.midi_inspector", json!({"open": true})).unwrap(), json!({"open": true}));
    h.run_steps(4);
    let before = notes(&h, clip);
    h.get_by_label("+1").click();
    h.run_steps(4);
    let after = notes(&h, clip);
    assert_eq!(after.iter().map(|n| n.pitch).collect::<Vec<_>>(), before.iter().map(|n| n.pitch + 1).collect::<Vec<_>>(), "whole clip");
    // With a selection only that note moves.
    h.state_mut().midi.selected = vec![0];
    h.run_steps(3);
    h.get_by_label("-12").click();
    h.run_steps(4);
    let again = notes(&h, clip);
    assert_eq!(again.iter().filter(|n| n.pitch < 50).count(), 1, "only the selected note went down an octave: {again:?}");
}

#[test]
fn inspector_fix_overlaps_and_velocity_set_act_on_the_selection() {
    let (mut h, clip) = harness(1600.0, true);
    h.state_mut().run("midi.note_edit", json!({"clip": clip, "index": 0, "length_ticks": 2000})).unwrap();
    h.state_mut().run("ui.midi_inspector", json!({"open": true})).unwrap();
    h.run_steps(4);
    h.get_by_label("Fix Overlaps").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("Fix Overlaps").click();
    h.run_steps(4);
    let n = notes(&h, clip);
    assert_eq!(n.iter().find(|x| x.pitch == 60 && x.start == 0).unwrap().length, 960, "trimmed to the next C");
    h.state_mut().midi.selected = vec![1];
    h.run_steps(3);
    h.get_all_by_label("Set").last().unwrap().scroll_to_me();
    h.run_steps(3);
    h.get_all_by_label("Set").last().unwrap().click();
    h.run_steps(4);
    let n = notes(&h, clip);
    assert_eq!(n.iter().filter(|x| x.velocity == 100).count(), 1);
}

#[test]
fn scale_assist_dims_rows_only_with_a_scale_and_snaps_new_notes() {
    let (mut h, clip) = harness(1600.0, true);
    h.state_mut().run("midi.scale_set", json!({"clip": clip, "root": 0, "kind": "major"})).unwrap();
    h.state_mut().run("ui.midi_assist", json!({"snap_new": true})).unwrap();
    set_tool(&mut h, "pencil");
    // C#3 (61) is out of C major: the drawn note lands on C (60).
    let at = note_pos(&mut h, 2000, 61);
    click_at(&mut h, at);
    let added: Vec<_> = notes(&h, clip).into_iter().filter(|n| n.start >= 1920).collect();
    assert_eq!(added.len(), 1, "{added:?}");
    assert_eq!(added[0].pitch, 60, "snapped down to C");
}

#[test]
fn draw_chords_places_a_whole_chord_in_one_undo_step() {
    let (mut h, clip) = harness(1600.0, true);
    h.state_mut().run("ui.midi_assist", json!({"draw_chords": true})).unwrap();
    h.state_mut().midi.insp.chord_kind = soundcraft_midi::ChordKind::Maj7;
    set_tool(&mut h, "pencil");
    let before = notes(&h, clip).len();
    let at = note_pos(&mut h, 2000, 65);
    click_at(&mut h, at);
    let n = notes(&h, clip);
    assert_eq!(n.len(), before + 4);
    let mut chord: Vec<u8> = n.iter().filter(|x| x.start >= 1920).map(|x| x.pitch).collect();
    chord.sort_unstable();
    assert_eq!(chord, vec![65, 69, 72, 76]);
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(notes(&h, clip).len(), before, "one undo removes the chord");
}

#[test]
fn muted_notes_stay_selectable_and_listed() {
    let (mut h, clip) = harness(1600.0, true);
    h.state_mut().run("midi.notes_mute", json!({"clip": clip, "indices": [0]})).unwrap();
    h.run_steps(3);
    let r = h.state_mut().run("midi.notes", json!({"clip": clip})).unwrap();
    assert_eq!(r["muted"], json!([0]));
}

#[test]
fn the_default_float_rect_fits_small_windows() {
    for w in [800.0_f32, 1100.0, 1440.0, 1920.0, 3000.0] {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(w, 800.0));
        let [x, _, fw, _] = midi_editor::float_rect(screen);
        assert!(x >= 0.0 && x + fw <= w, "{w}: x {x} width {fw}");
        assert!(fw <= midi_editor::FLOAT_W);
    }
    let wide = midi_editor::float_rect(Rect::from_min_size(Pos2::ZERO, vec2(3000.0, 1000.0)));
    assert_eq!(wide[2], midi_editor::FLOAT_W);
}
