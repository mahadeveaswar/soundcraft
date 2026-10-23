//! Cut / Copy / Paste in the piano roll and the arrange window: the keys go to the notes while the pointer is
//! over the roll and to the clips elsewhere, in the Default and the Alternative key set, and the roll menu offers
//! the same lines with the right enabled states.

use egui::{Event, Id, Key, Modifiers, PointerButton, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;
use soundcraft_model::ClipContent;
use soundcraft_ui_egui::{Services, SoundApp};

type H<'a> = Harness<'a, SoundApp>;

const CTRL: Modifiers = Modifiers { ctrl: true, command: true, alt: false, shift: false, mac_cmd: false };

fn harness(preset: &str) -> H<'static> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("ui.prefs", json!({"keymap_preset": preset})).unwrap();
    app.ui.show_midi_editor = true;
    app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_pixels_per_point(2.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    steps(&mut h, 6);
    h
}

fn steps(h: &mut H<'_>, n: usize) {
    for _ in 0..n {
        let mut raw = std::mem::take(h.input_mut());
        h.state_mut().raw_input_hook(&mut raw);
        *h.input_mut() = raw;
        h.step();
    }
}

fn roll_rect(h: &H<'_>) -> Rect {
    h.ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-roll"))).unwrap()
}

fn hover(h: &mut H<'_>, at: egui::Pos2) {
    h.input_mut().events.push(Event::PointerMoved(at));
    steps(h, 3);
}

fn chord(h: &mut H<'_>, key: Key) {
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: CTRL });
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: false, repeat: false, modifiers: CTRL });
    steps(h, 3);
}

fn keys_clip(h: &H<'_>) -> soundcraft_model::Clip {
    h.state().engine.session().track_by_name("Keys").unwrap().clips()[0].clone()
}

fn note_count(h: &H<'_>) -> usize {
    match keys_clip(h).content {
        ClipContent::Midi { sequence } => sequence.notes.len(),
        _ => 0,
    }
}

fn click(h: &mut H<'_>, at: egui::Pos2, button: PointerButton) {
    let s = &mut h.state_mut().synthetic;
    s.push(Event::PointerMoved(at));
    s.push(Event::PointerButton { pos: at, button, pressed: true, modifiers: Modifiers::NONE });
    s.push(Event::PointerButton { pos: at, button, pressed: false, modifiers: Modifiers::NONE });
    steps(h, 10);
}

fn line<'a>(h: &'a H<'_>, name: &'a str) -> Option<egui_kittest::Node<'a>> {
    h.query_all_by_label_contains(name).find(|n| {
        let label = n.accesskit_node().label().unwrap_or_default();
        label.strip_prefix(name).is_some_and(|rest| rest.is_empty() || (rest.starts_with(' ') && rest.split_whitespace().count() <= 1))
    })
}

#[test]
fn over_the_roll_copy_and_paste_work_on_notes_and_leave_the_arrange_clipboard_alone() {
    for preset in ["default", "alternative"] {
        let mut h = harness(preset);
        let before = note_count(&h);
        assert!(before >= 2, "the demo clip has notes");
        h.state_mut().midi.selected = vec![0, 1];
        let at = roll_rect(&h).center();
        hover(&mut h, at);
        chord(&mut h, Key::C);
        assert!(h.state().engine.clipboard.notes.is_some(), "{preset}: Cmd+C copied the notes");
        assert!(h.state().engine.clipboard.tracks.is_empty(), "{preset}: the arrange clipboard is untouched");
        chord(&mut h, Key::V);
        assert_eq!(note_count(&h), before + 2, "{preset}: Cmd+V pasted the notes");
        assert_eq!(h.state().midi.selected.len(), 2, "{preset}: the pasted notes are selected");
        assert!(h.state().engine.clipboard.tracks.is_empty());
        chord(&mut h, Key::X);
        assert_eq!(note_count(&h), before, "{preset}: Cmd+X removed the selected notes");
        chord(&mut h, Key::V);
        assert_eq!(note_count(&h), before + 2, "{preset}: and Cmd+V brings them back");
    }
}

#[test]
fn off_the_roll_the_same_keys_cut_copy_and_paste_clips() {
    for preset in ["default", "alternative"] {
        let mut h = harness(preset);
        h.state_mut().run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
        let clip = h.state().engine.session().track_by_name("Kick").unwrap().clips()[0].clone();
        h.state_mut().run("edit.select", json!({"clips": [clip.id.0]})).unwrap();
        let notes_before = note_count(&h);
        hover(&mut h, pos2(5.0, 500.0));
        chord(&mut h, Key::C);
        assert!(!h.state().engine.clipboard.tracks.is_empty(), "{preset}: Cmd+C copied the clip");
        assert!(h.state().engine.clipboard.notes.is_none());
        let clips = h.state().engine.session().track_by_name("Kick").unwrap().clips().len();
        h.state_mut().run("edit.select", json!({"tracks": ["Kick"], "start": clip.end() + 5_000, "end": clip.end() + 5_000})).unwrap();
        chord(&mut h, Key::V);
        assert_eq!(h.state().engine.session().track_by_name("Kick").unwrap().clips().len(), clips + 1, "{preset}: Cmd+V pasted the clip");
        assert_eq!(note_count(&h), notes_before, "{preset}: the notes were not touched");
    }
}

#[test]
fn the_roll_menu_has_cut_copy_paste_duplicate_with_their_enabled_states() {
    let mut h = harness("default");
    let roll = roll_rect(&h);
    click(&mut h, pos2(roll.max.x - 40.0, roll.min.y + 6.0), PointerButton::Secondary);
    for name in ["Cut", "Copy", "Paste", "Duplicate"] {
        assert!(line(&h, name).is_some(), "the roll menu has `{name}`");
    }
    for name in ["Cut", "Copy", "Duplicate", "Paste"] {
        assert!(line(&h, name).unwrap().accesskit_node().is_disabled(), "`{name}` is disabled with nothing selected or copied");
    }
    click(&mut h, pos2(5.0, 5.0), PointerButton::Primary);
    h.state_mut().midi.selected = vec![0];
    let before = note_count(&h);
    click(&mut h, pos2(roll.max.x - 40.0, roll.min.y + 6.0), PointerButton::Secondary);
    assert!(!line(&h, "Copy").unwrap().accesskit_node().is_disabled());
    assert!(line(&h, "Paste").unwrap().accesskit_node().is_disabled(), "nothing copied yet");
    let at = line(&h, "Copy").unwrap().rect().center();
    click(&mut h, at, PointerButton::Primary);
    assert!(h.state().engine.clipboard.notes.is_some());
    click(&mut h, pos2(roll.max.x - 40.0, roll.min.y + 6.0), PointerButton::Secondary);
    let at = line(&h, "Paste").unwrap().rect().center();
    click(&mut h, at, PointerButton::Primary);
    assert_eq!(note_count(&h), before + 1, "Paste added the copied note");
}

fn raw(h: &mut H<'_>, e: Event) {
    h.input_mut().events.push(e);
    steps(h, 3);
}

#[test]
fn real_keyboard_clipboard_events_copy_cut_and_paste_notes_in_both_key_sets() {
    for preset in ["default", "alternative"] {
        let mut h = harness(preset);
        let before = note_count(&h);
        h.state_mut().midi.selected = vec![0, 1];
        let at = roll_rect(&h).center();
        hover(&mut h, at);
        raw(&mut h, Event::Copy);
        assert!(h.state().engine.clipboard.notes.is_some(), "{preset}: Copy event copied the notes");
        raw(&mut h, Event::Paste("soundcraft".into()));
        assert_eq!(note_count(&h), before + 2, "{preset}: Paste event pasted");
        raw(&mut h, Event::Cut);
        assert_eq!(note_count(&h), before, "{preset}: Cut event removed the selected notes");
        raw(&mut h, Event::Paste("foreign text".into()));
        assert_eq!(note_count(&h), before + 2, "{preset}: foreign clipboard text still pastes the internal clipboard");
    }
}

#[test]
fn a_copy_event_and_its_key_event_act_once_and_the_marker_reaches_the_os_clipboard() {
    let mut h = harness("default");
    let before = note_count(&h);
    h.state_mut().midi.selected = vec![0, 1];
    let at = roll_rect(&h).center();
    hover(&mut h, at);
    h.input_mut().events.push(Event::Copy);
    let mut marker = false;
    for _ in 0..4 {
        steps(&mut h, 1);
        marker |= h.output().platform_output.commands.iter().any(|c| matches!(c, egui::OutputCommand::CopyText(t) if t == "soundcraft"));
    }
    assert!(marker, "an internal copy puts the marker on the OS clipboard");
    h.input_mut().events.push(Event::Paste("x".into()));
    h.input_mut().events.push(Event::Key { key: Key::V, physical_key: None, pressed: true, repeat: false, modifiers: CTRL });
    steps(&mut h, 3);
    assert_eq!(note_count(&h), before + 2, "one paste, not two");
}

#[test]
fn a_paste_event_over_the_arrange_area_pastes_clips() {
    let mut h = harness("default");
    h.state_mut().run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
    let clip = h.state().engine.session().track_by_name("Kick").unwrap().clips()[0].clone();
    h.state_mut().run("edit.select", json!({"clips": [clip.id.0]})).unwrap();
    hover(&mut h, pos2(5.0, 500.0));
    raw(&mut h, Event::Copy);
    assert!(!h.state().engine.clipboard.tracks.is_empty());
    let clips = h.state().engine.session().track_by_name("Kick").unwrap().clips().len();
    h.state_mut().run("edit.select", json!({"tracks": ["Kick"], "start": clip.end() + 5_000, "end": clip.end() + 5_000})).unwrap();
    raw(&mut h, Event::Paste("soundcraft".into()));
    assert_eq!(h.state().engine.session().track_by_name("Kick").unwrap().clips().len(), clips + 1);
}
