//! The compact track header: the style setting, its buttons, the volume slider and its undo.

use egui::{Event, Modifiers, PointerButton, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;
use soundcraft_model::AutomationMode;
use soundcraft_ui_egui::edit_window::HEADER_W;
use soundcraft_ui_egui::track_header_compact::{HeaderFlags, HeaderLayout, HeaderStyle, layout};
use soundcraft_ui_egui::{Services, SoundApp, UiState};

fn harness(track: &str, height: &str) -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("track.height", json!({"tracks": [track], "height": height})).unwrap();
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

/// The header rectangle of `track` and the layout of its controls, in window points.
fn header(h: &Harness<SoundApp>, track: &str) -> (Rect, HeaderLayout) {
    let s = h.state().engine.session();
    let t = s.track_by_name(track).unwrap();
    let row = h.state().edit_layout.rows.iter().find(|(id, _)| *id == t.id.0).unwrap().1;
    let head = Rect::from_min_max(pos2(row[0], row[1]), pos2(row[0] + HEADER_W, row[1] + t.height.points()));
    let flags = HeaderFlags::of(t, 0);
    (head, layout(head.size(), &flags))
}

fn at(head: Rect, r: Rect) -> Pos2 {
    r.translate(head.min.to_vec2()).center()
}

fn click(h: &mut Harness<SoundApp>, p: Pos2) {
    h.hover_at(p);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
}

fn double_click(h: &mut Harness<SoundApp>, p: Pos2) {
    h.hover_at(p);
    h.run_steps(2);
    for pressed in [true, false, true, false] {
        h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(1);
    }
    h.run_steps(3);
}

fn drag(h: &mut Harness<SoundApp>, from: Pos2, to: Pos2) {
    h.hover_at(from);
    h.run_steps(2);
    h.drag_at(from);
    h.run_steps(2);
    let mid = from + (to - from) * 0.5;
    h.hover_at(mid);
    h.run_steps(2);
    h.hover_at(to);
    h.run_steps(3);
    h.drop_at(to);
    h.run_steps(3);
}

fn kick(h: &Harness<SoundApp>) -> soundcraft_model::Track {
    h.state().engine.session().track_by_name("Kick").unwrap().clone()
}

#[test]
fn header_style_command_round_trips_and_compact_is_the_default() {
    let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
    assert_eq!(app.ui.header_style, HeaderStyle::Compact);
    assert_eq!(app.run("ui.header_style", json!({})), Ok(json!({"style": "compact"})));
    assert_eq!(app.run("ui.header_style", json!({"style": "classic"})), Ok(json!({"style": "classic"})));
    assert_eq!(app.ui.header_style, HeaderStyle::Classic);
    assert!(app.run("ui.header_style", json!({"style": "tall"})).is_err());
    assert_eq!(app.ui.header_style, HeaderStyle::Classic);
    let saved = serde_json::to_value(&app.ui).unwrap();
    assert_eq!(saved["header_style"], "classic");
    let back: UiState = serde_json::from_value(saved).unwrap();
    assert_eq!(back.header_style, HeaderStyle::Classic);
    let old: UiState = serde_json::from_value(json!({"show_tracks_list": true})).unwrap();
    assert_eq!(old.header_style, HeaderStyle::Compact, "preferences saved before the setting load with the default");
    assert!(soundcraft_ui_egui::menus::UI_COMMANDS.iter().any(|c| c.0 == "ui.header_style"));
}

#[test]
fn mute_solo_record_and_automation_buttons_act_on_the_track() {
    let mut h = harness("Kick", "medium");
    let (head, lay) = header(&h, "Kick");
    assert!(!kick(&h).mixer.mute);
    click(&mut h, at(head, lay.mute.unwrap()));
    assert!(kick(&h).mixer.mute, "M mutes");
    click(&mut h, at(head, lay.solo.unwrap()));
    assert!(kick(&h).mixer.solo, "S solos");
    click(&mut h, at(head, lay.rec.unwrap()));
    assert!(kick(&h).mixer.record_arm, "the record button arms");
    click(&mut h, at(head, lay.monitor.unwrap()));
    assert!(kick(&h).mixer.input_monitor, "the monitor button switches input monitoring on");
    assert_eq!(kick(&h).mixer.automation_mode, AutomationMode::Read);
    click(&mut h, at(head, lay.write.unwrap()));
    assert_eq!(kick(&h).mixer.automation_mode, AutomationMode::Write);
    click(&mut h, at(head, lay.read.unwrap()));
    assert_eq!(kick(&h).mixer.automation_mode, AutomationMode::Read);
    click(&mut h, at(head, lay.read.unwrap()));
    assert_eq!(kick(&h).mixer.automation_mode, AutomationMode::Off, "R again turns reading off");
}

#[test]
fn edit_button_opens_the_first_plugin() {
    let mut h = harness("Kick", "medium");
    let (head, lay) = header(&h, "Kick");
    assert!(h.state().ui.plugin_windows.is_empty());
    click(&mut h, at(head, lay.edit.unwrap()));
    let id = kick(&h).id;
    assert!(h.state().ui.plugin_windows.iter().any(|(t, _)| *t == id), "the first insert window is open");
}

#[test]
fn dragging_the_volume_slider_changes_volume_in_one_undo_step() {
    let mut h = harness("Kick", "medium");
    let (head, lay) = header(&h, "Kick");
    let vol = lay.volume.expect("a medium header shows the slider");
    let before_db = kick(&h).mixer.volume_db;
    let steps_before = h.state().engine.undo_history().len();
    let start = at(head, vol) - vec2(vol.width() * 0.2, 0.0);
    drag(&mut h, start, start + vec2(30.0, 0.0));
    let after = kick(&h).mixer.volume_db;
    assert!(after > before_db + 0.5, "dragging right raises the volume: {before_db} -> {after}");
    assert_eq!(h.state().engine.undo_history().len(), steps_before + 1, "one drag is one undo step");
    assert!(h.state_mut().engine.undo());
    assert!((kick(&h).mixer.volume_db - before_db).abs() < 1e-4, "undo restores the volume");
}

#[test]
fn double_clicking_the_volume_slider_resets_to_zero_db() {
    let mut h = harness("Kick", "medium");
    let (head, lay) = header(&h, "Kick");
    let vol = lay.volume.unwrap();
    assert!(kick(&h).mixer.volume_db.abs() > 0.5, "the demo Kick starts below 0 dB");
    double_click(&mut h, at(head, vol) - vec2(vol.width() * 0.2, 0.0));
    assert!(kick(&h).mixer.volume_db.abs() < 1e-4);
}

#[test]
fn compact_headers_render_at_every_track_height() {
    for height in ["micro", "mini", "small", "medium", "large", "jumbo"] {
        let h = harness("Kick", height);
        let (head, lay) = header(&h, "Kick");
        let bounds = Rect::from_min_size(pos2(0.0, 0.0), head.size());
        for (name, r) in lay.controls() {
            assert!(bounds.contains_rect(r), "{name} outside the {height} header");
        }
        assert_eq!(lay.volume.is_some(), head.height() >= 58.0, "{height}");
    }
}

#[test]
fn classic_header_still_draws_when_selected() {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("ui.header_style", json!({"style": "classic"})).unwrap();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    assert!(!h.state().edit_layout.rows.is_empty());
}

#[test]
fn the_fold_triangle_toggles_a_folder_in_the_compact_header() {
    let mut engine = soundcraft_engine::Engine::default();
    engine.execute("track.new", &json!({"name": "Voice"})).unwrap();
    engine.execute("track.new", &json!({"name": "Bass"})).unwrap();
    engine.execute("track.move_to_new_folder", &json!({"tracks": ["Bass"], "name": "Low"})).unwrap();
    let app = SoundApp::new(engine, None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    let rows = h.state().edit_layout.rows.len();
    let (head, lay) = header(&h, "Low");
    assert!(lay.fold.is_some());
    click(&mut h, at(head, lay.fold.unwrap()));
    assert!(!h.state().engine.session().track_by_name("Low").unwrap().folder_open);
    assert_eq!(h.state().edit_layout.rows.len(), rows - 1);
    // A child header keeps clear of the guide bar of its folder.
    click(&mut h, at(head, lay.fold.unwrap()));
    assert_eq!(h.state().edit_layout.rows.len(), rows);
    let (_, child) = header(&h, "Bass");
    assert!(child.name.is_some());
}
