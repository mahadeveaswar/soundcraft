//! The catalogue-driven context menus: what each surface offers, key text from the active key set,
//! disabled lines, and that a line runs its command. Menu lines are found by their labels.

use egui::{Event, Id, Modifiers, PointerButton, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp};

type H<'a> = Harness<'a, SoundApp>;

fn harness(app: SoundApp) -> H<'static> {
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

fn demo() -> H<'static> {
    harness(SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default()))
}

fn step(h: &mut H<'_>) {
    let mut raw = std::mem::take(h.input_mut());
    h.state_mut().raw_input_hook(&mut raw);
    *h.input_mut() = raw;
    h.step();
}

fn steps(h: &mut H<'_>, n: usize) {
    for _ in 0..n {
        step(h);
    }
}

fn click(h: &mut H<'_>, at: egui::Pos2, button: PointerButton) {
    let s = &mut h.state_mut().synthetic;
    s.push(Event::PointerMoved(at));
    s.push(Event::PointerButton { pos: at, button, pressed: true, modifiers: Modifiers::NONE });
    s.push(Event::PointerButton { pos: at, button, pressed: false, modifiers: Modifiers::NONE });
    steps(h, 10);
}

/// The menu line whose label is `name`, optionally followed by one key-text word (lines carry their key text
/// after the label), if such a menu is open.
fn line<'a>(h: &'a H<'_>, name: &'a str) -> Option<egui_kittest::Node<'a>> {
    h.query_all_by_label_contains(name).find(|n| {
        let label = n.accesskit_node().label().unwrap_or_default();
        label.strip_prefix(name).is_some_and(|rest| rest.is_empty() || (rest.starts_with(' ') && rest.split_whitespace().count() <= 1))
    })
}

fn label_of(n: &egui_kittest::Node<'_>) -> String {
    n.accesskit_node().label().unwrap_or_default()
}

fn kick_clips(h: &H<'_>) -> usize {
    h.state().engine.session().track_by_name("Kick").map_or(0, |t| t.clips().len())
}

#[test]
fn clip_menu_offers_the_arrange_lines_with_the_active_keys() {
    let mut h = demo();
    h.state_mut().run("ui.prefs", json!({"keymap_preset": "alternative"})).unwrap();
    steps(&mut h, 2);
    click(&mut h, pos2(880.0, 240.0), PointerButton::Secondary);
    for name in [
        "Cut",
        "Copy",
        "Paste",
        "Delete",
        "Split at Playhead",
        "Split Range",
        "Glue",
        "Duplicate",
        "Repeat...",
        "Mute",
        "Unmute",
        "Lock / Unlock",
        "Time Lock / Unlock",
        "Fades",
        "Process",
        "Quantize to Grid",
        "Loop...",
        "Unloop",
        "Group",
        "Ungroup",
        "Bring to Front",
        "Send to Back",
        "Colour",
        "Rename...",
        "Render Clip Gain",
        "Bounce (Consolidate)",
    ] {
        assert!(line(&h, name).is_some(), "the clip menu has `{name}`");
    }
    let glue = line(&h, "Glue").unwrap();
    assert!(label_of(&glue).contains("Alt+G"), "Glue shows its key: {}", label_of(&glue));
    assert!(glue.accesskit_node().is_disabled(), "Glue needs two clips, only one is selected");
    assert!(label_of(&line(&h, "Split at Playhead").unwrap()).contains("Alt+X"));
    assert!(!line(&h, "Mute").unwrap().accesskit_node().is_disabled());
}

#[test]
fn the_menu_follows_the_key_set() {
    let mut h = demo();
    click(&mut h, pos2(880.0, 240.0), PointerButton::Secondary);
    assert!(label_of(&line(&h, "Cut").unwrap()).contains("Ctrl+X"), "the Default set shows its own keys");
    assert!(!label_of(&line(&h, "Glue").unwrap()).contains("Alt+G"));
}

#[test]
fn split_at_playhead_in_the_clip_menu_splits_the_clip() {
    let mut h = demo();
    let clip = h.state().engine.session().track_by_name("Kick").unwrap().clips()[0].clone();
    let before = kick_clips(&h);
    h.state_mut().run("transport.locate", json!({"at": clip.start + clip.length / 2})).unwrap();
    click(&mut h, pos2(880.0, 240.0), PointerButton::Secondary);
    let at = line(&h, "Split at Playhead").unwrap().rect().center();
    click(&mut h, at, PointerButton::Primary);
    assert_eq!(kick_clips(&h), before + 1);
}

#[test]
fn an_empty_lane_offers_paste_selection_markers_and_add_track() {
    let mut h = demo();
    // Scan the Kick lane for a spot without a clip.
    let mut found = false;
    for x in (300..1560).step_by(60) {
        click(&mut h, pos2(x as f32, 240.0), PointerButton::Secondary);
        if line(&h, "Select None").is_some() {
            found = true;
            break;
        }
        click(&mut h, pos2(1200.0, 8.0), PointerButton::Primary);
    }
    assert!(found, "a right-click on empty lane opens its menu");
    for name in ["Paste", "Select All", "Select None", "Locators to Selection", "Add Marker", "Add Track"] {
        assert!(line(&h, name).is_some(), "the empty-lane menu has `{name}`");
    }
    assert!(line(&h, "Glue").is_none(), "no clip lines on an empty lane");
}

#[test]
fn the_ruler_menu_offers_locators_markers_loop_and_zoom() {
    let mut h = demo();
    let mut found = false;
    for y in (60..200).step_by(6) {
        click(&mut h, pos2(900.0, y as f32), PointerButton::Secondary);
        if line(&h, "Locate Here").is_some() {
            found = true;
            break;
        }
        click(&mut h, pos2(5.0, 5.0), PointerButton::Primary);
    }
    assert!(found, "a right-click on the ruler opens its menu");
    for name in ["Add Marker", "Set Left Locator Here", "Set Right Locator Here", "Loop On/Off", "Zoom to Selection", "Zoom to Fit Session"] {
        assert!(line(&h, name).is_some(), "the ruler menu has `{name}`");
    }
    let at = line(&h, "Set Left Locator Here").unwrap().rect().center();
    click(&mut h, at, PointerButton::Primary);
    assert!(h.state().engine.session().edit.timeline_selection.start > 0, "the left locator moved to the click");
}

fn roll_rect(h: &H<'_>) -> Rect {
    h.ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-roll"))).unwrap()
}

#[test]
fn the_piano_roll_menu_offers_note_lines_and_enables_them_by_selection() {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    let mut h = harness(app);
    let roll = roll_rect(&h);
    click(&mut h, pos2(roll.max.x - 40.0, roll.min.y + 6.0), PointerButton::Secondary);
    for name in ["Delete", "Split", "Glue", "Mute / Unmute", "Quantize", "Transpose", "Velocity", "Select All"] {
        assert!(line(&h, name).is_some(), "the roll menu has `{name}`");
    }
    assert!(line(&h, "Delete").unwrap().accesskit_node().is_disabled(), "nothing is selected");
    assert!(!line(&h, "Select All").unwrap().accesskit_node().is_disabled());
    let at = line(&h, "Select All").unwrap().rect().center();
    click(&mut h, at, PointerButton::Primary);
    assert!(!h.state().midi.selected.is_empty(), "Select All selected the notes");
}

#[test]
fn the_track_menu_starts_with_mute_solo_record_and_select_clips() {
    let mut h = demo();
    // The Kick header: the left column of its row.
    let kick = h.state().engine.session().track_by_name("Kick").unwrap().id.0;
    let row = h.state().edit_layout.rows.iter().find(|(id, _)| *id == kick).map(|(_, r)| *r).unwrap();
    let header = h.state().edit_layout.timeline[0];
    let mut found = false;
    for fx in [0.15, 0.3, 0.5, 0.7, 0.85] {
        for fy in [0.25, 0.5, 0.75] {
            click(&mut h, pos2(header * fx, row[1] + (row[3] - row[1]) * fy), PointerButton::Secondary);
            if line(&h, "Select All Clips on Track").is_some() {
                found = true;
                break;
            }
            click(&mut h, pos2(1200.0, 8.0), PointerButton::Primary);
        }
        if found {
            break;
        }
    }
    assert!(found, "a right-click on a track header opens its menu");
    for name in ["Mute", "Solo", "Record Enable", "Add Track"] {
        assert!(line(&h, name).is_some(), "the track menu has `{name}`");
    }
    let at = line(&h, "Select All Clips on Track").unwrap().rect().center();
    click(&mut h, at, PointerButton::Primary);
    assert!(!h.state().engine.session().edit.selected_clips.is_empty());
}

#[test]
fn a_controller_lane_menu_clears_or_removes_the_lane_instead_of_doing_it_silently() {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    app.run("ui.midi_lane_add", json!({"lane": "modulation"})).unwrap();
    let mut h = harness(app);
    let lanes = h.state().midi.lanes.len();
    // The lane label is a drop-down at the left; the lane itself is to its right.
    let label = h.get_all_by_role(egui::accesskit::Role::ComboBox).last().map(|n| n.rect().center()).unwrap();
    let name = pos2(900.0, label.y);
    click(&mut h, name, PointerButton::Secondary);
    assert_eq!(h.state().midi.lanes.len(), lanes, "a right-click alone no longer removes the lane");
    assert!(line(&h, "Clear Lane").is_some() && line(&h, "Remove Lane").is_some());
    let at = line(&h, "Remove Lane").unwrap().rect().center();
    h.state_mut().synthetic.push(Event::PointerMoved(at));
    steps(&mut h, 4);
    assert!(line(&h, "Remove Lane").is_some(), "the menu stays open while the pointer moves onto it");
    click(&mut h, at, PointerButton::Primary);
    assert_eq!(h.state().midi.lanes.len(), lanes - 1, "status: {}", h.state().ui.status);
}

#[test]
fn a_mixer_strip_body_opens_the_track_menu() {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("window.mix", json!({"value": true})).unwrap();
    let mut h = harness(app);
    // Strips fill the window below its toolbar; scan for a spot in a strip that holds no control.
    let mut found = false;
    'scan: for y in (500..990).step_by(30) {
        for x in (30..700).step_by(45) {
            click(&mut h, pos2(x as f32, y as f32), PointerButton::Secondary);
            if line(&h, "Select All Clips on Track").is_some() {
                found = true;
                break 'scan;
            }
            click(&mut h, pos2(1590.0, 5.0), PointerButton::Primary);
        }
    }
    assert!(found || line(&h, "Select All Clips on Track").is_some());
    assert!(line(&h, "Select All Clips on Track").is_some(), "the strip body shows the track menu");
}
