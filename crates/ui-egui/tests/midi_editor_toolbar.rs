//! Piano-roll toolbar additions: snap, clip selector, `ui.midi_tool` / `ui.midi_snap`, the info
//! strip, the zoom sliders and the velocity colour switch.

use egui::accesskit::Role;
use egui::{Event, Id, Key, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_midi::Note;
use soundcraft_model::{ClipContent, ClipId};
use soundcraft_ui_egui::{Services, SoundApp};

fn harness(width: f32) -> (Harness<'static, SoundApp>, u64) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
    let clip = created["clip"].as_u64().unwrap();
    app.run("midi.note_add", json!({"clip": clip, "pitch": 60, "start_ticks": 0, "length_ticks": 960, "velocity": 90})).unwrap();
    app.run("midi.note_add", json!({"clip": clip, "pitch": 64, "start_ticks": 960, "length_ticks": 960, "velocity": 40})).unwrap();
    app.run("edit.select", json!({"clips": [clip]})).unwrap();
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

fn notes(h: &Harness<SoundApp>, clip: u64) -> Vec<Note> {
    match &h.state().engine.session().find_clip(ClipId(clip)).unwrap().1.content {
        ClipContent::Midi { sequence } => sequence.notes.clone(),
        _ => panic!("MIDI clip required"),
    }
}

fn roll(h: &Harness<SoundApp>) -> Rect {
    h.ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-roll"))).unwrap()
}

fn click_at(h: &mut Harness<SoundApp>, at: Pos2) {
    h.hover_at(at);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
}

#[test]
fn ui_commands_set_and_report_tool_and_snap() {
    let (mut h, _) = harness(1600.0);
    assert_eq!(h.state_mut().run("ui.midi_tool", json!({})).unwrap(), json!({"tool": "select"}));
    assert_eq!(h.state_mut().run("ui.midi_tool", json!({"tool": "pencil"})).unwrap(), json!({"tool": "pencil"}));
    assert!(h.state_mut().run("ui.midi_tool", json!({"tool": "lasso"})).is_err());
    assert_eq!(h.state().midi.tool, soundcraft_ui_egui::midi_editor::MidiTool::Pencil);
    assert_eq!(h.state_mut().run("ui.midi_snap", json!({"snap": "1/8t"})).unwrap(), json!({"snap": "1/8t"}));
    assert!(h.state_mut().run("ui.midi_snap", json!({"snap": "1/7"})).is_err());
    assert_eq!(h.state_mut().run("ui.midi_snap", json!({})).unwrap(), json!({"snap": "1/8t"}));
}

#[test]
fn snap_drives_pencil_placement_and_does_not_touch_the_session_grid() {
    let (mut h, clip) = harness(1600.0);
    h.state_mut().run("view.grid", json!({"value": "1/16"})).unwrap();
    h.state_mut().run("ui.midi_tool", json!({"tool": "pencil"})).unwrap();
    h.state_mut().run("ui.midi_snap", json!({"snap": "1/4"})).unwrap();
    h.run_steps(3);
    let grid_before = h.state().engine.session().edit.grid;
    let r = roll(&h);
    click_at(&mut h, pos2(r.left() + 140.0, r.top() + 40.0));
    let added: Vec<_> = notes(&h, clip).into_iter().filter(|n| n.pitch != 60 && n.pitch != 64).collect();
    assert_eq!(added.len(), 1, "one note drawn");
    assert_eq!(added[0].start % 960, 0, "starts on a quarter note: {}", added[0].start);
    assert_eq!(added[0].length, 960, "default length is the snap value");
    assert_eq!(h.state().engine.session().edit.grid, grid_before);
}

#[test]
fn clip_selector_lists_the_clips_on_the_track_and_switches() {
    let (mut h, clip) = harness(1600.0);
    let second = h.state_mut().run("midi.new_clip", json!({"track": "Keys", "start": 200000, "length": 48000})).unwrap()["clip"].as_u64().unwrap();
    h.state_mut().run("edit.select", json!({"clips": [clip]})).unwrap();
    h.run_steps(3);
    h.state_mut().run("clip.rename", json!({"clip": second, "name": "Second Take"})).unwrap();
    let name = "Second Take".to_string();
    h.run_steps(2);
    h.get_by_label_contains("Clip:").click();
    h.run_steps(3);
    h.get_all_by_label(&name).last().unwrap().click();
    h.run_steps(4);
    assert_eq!(h.state().engine.session().edit.selected_clips, vec![ClipId(second)]);
}

#[test]
fn snap_and_clip_selector_live_in_the_overflow_menu_when_narrow() {
    let (mut h, _) = harness(760.0);
    assert!(h.query_by_label_contains("Snap:").is_some(), "snap is the last group to give up its place");
    assert!(h.query_by_label_contains("Clip:").is_none(), "the clip selector is in the overflow menu");
    h.get_by_label("More MIDI editor actions").click();
    h.run_steps(3);
    assert!(h.query_by_label_contains("Clip:").is_some());
}

#[test]
fn info_strip_edits_velocity_with_one_undo_step() {
    let (mut h, clip) = harness(1600.0);
    h.state_mut().midi.selected = vec![0];
    h.run_steps(3);
    let before = notes(&h, clip);
    let field = h.get_by_role_and_label(Role::TextInput, "Velocity");
    field.click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, Key::A);
    h.run_steps(1);
    h.get_by_role_and_label(Role::TextInput, "Velocity").type_text("101");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(4);
    let after = notes(&h, clip);
    assert_eq!(after.iter().filter(|n| n.velocity == 101).count(), 1, "{after:?}");
    assert_eq!(after.len(), before.len());
    let undo_label = h.state().engine.undo_label().map(str::to_string);
    assert!(undo_label.is_some_and(|l| l.contains("Adjust")), "one named undo step");
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(notes(&h, clip), before);
}

#[test]
fn info_strip_rejects_invalid_input_without_changing_notes() {
    let (mut h, clip) = harness(1600.0);
    h.state_mut().midi.selected = vec![0];
    h.run_steps(3);
    let before = notes(&h, clip);
    h.get_by_role_and_label(Role::TextInput, "Velocity").click();
    h.run_steps(2);
    h.get_by_role_and_label(Role::TextInput, "Velocity").type_text("zzz");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(4);
    assert_eq!(notes(&h, clip), before);
    assert!(h.state().midi.info_error.is_some());
}

#[test]
fn zoom_sliders_buttons_and_header_buttons_share_state() {
    let (mut h, _) = harness(1600.0);
    let row_h = h.state().midi.row_h;
    h.get_by_label("Taller piano-roll rows").click();
    h.run_steps(3);
    assert_eq!(h.state().midi.row_h, row_h + 2.0, "the vertical + button");
    h.get_by_label("V-").click();
    h.run_steps(3);
    assert_eq!(h.state().midi.row_h, row_h, "the header V- undoes it: same state");
    h.get_by_label("Zoom in horizontally").click();
    h.run_steps(3);
    assert!(h.state().midi.ppt.is_some(), "the horizontal + button zooms");
    let ppt = h.state().midi.ppt.unwrap();
    h.get_by_label("H-").click();
    h.run_steps(3);
    assert!(h.state().midi.ppt.unwrap() < ppt, "the header H- shares the zoom");
    // The sliders exist and clamp.
    assert!(h.query_by_label("Horizontal zoom").is_some());
    assert!(h.query_by_label("Vertical zoom").is_some());
}

#[test]
fn velocity_colour_switch_is_a_view_flag() {
    let (mut h, _) = harness(1600.0);
    assert!(!h.state().engine.session().edit.flag("midi.single_colour"));
    h.state_mut().run("view.midi_single_colour", json!({})).unwrap();
    assert!(h.state().engine.session().edit.flag("midi.single_colour"));
    h.run_steps(3);
    h.state_mut().run("view.midi_single_colour", json!({})).unwrap();
    assert!(!h.state().engine.session().edit.flag("midi.single_colour"));
}

#[test]
fn the_roll_and_the_arrange_window_keep_separate_tools() {
    use soundcraft_model::Tool;
    use soundcraft_ui_egui::midi_editor::MidiTool;
    let (mut h, clip) = harness(1600.0);
    h.state_mut().run("ui.midi_tool", json!({"tool": "erase"})).unwrap();
    for tool in Tool::ALL {
        h.state_mut().run("edit.tool", json!({"tool": tool.id()})).unwrap();
        assert_eq!(h.state().midi.tool, MidiTool::Erase, "arrange tool {tool:?} leaves the roll tool alone");
    }
    h.state_mut().run("edit.tool", json!({"tool": "zoom"})).unwrap();
    h.state_mut().run("ui.midi_tool", json!({"tool": "pencil"})).unwrap();
    assert_eq!(h.state().engine.session().edit.tool, Tool::Zoom, "and the roll tool leaves the arrange tool alone");
    // Clicks on the roll follow the roll's own tool (Select: no edit) whatever the arrange tool is.
    h.state_mut().run("ui.midi_tool", json!({"tool": "select"})).unwrap();
    h.run_steps(3);
    let before = notes(&h, clip);
    let at = roll(&h).center() + vec2(0.0, 120.0);
    for tool in Tool::ALL {
        h.state_mut().run("edit.tool", json!({"tool": tool.id()})).unwrap();
        h.run_steps(70);
        click_at(&mut h, at);
        assert_eq!(notes(&h, clip), before, "a roll click with arrange tool {tool:?} edits nothing");
    }
}

#[test]
fn arrange_tool_keys_do_nothing_while_the_pointer_is_over_the_roll() {
    use soundcraft_model::Tool;
    let (mut h, _) = harness(1600.0);
    h.state_mut().run("edit.tool", json!({"tool": "selector"})).unwrap();
    let key = |h: &mut Harness<SoundApp>, key: Key| {
        h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
        h.run_steps(3);
    };
    h.hover_at(pos2(2.0, 2.0));
    h.run_steps(6);
    key(&mut h, Key::F6);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Trim, "keys work off the roll");
    h.hover_at(roll(&h).center());
    h.run_steps(4);
    key(&mut h, Key::F7);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Trim, "F7 over the roll leaves the arrange tool");
    h.input_mut().events.push(Event::PointerGone);
    h.run_steps(8);
    key(&mut h, Key::F5);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Zoom, "and still works elsewhere");
}
