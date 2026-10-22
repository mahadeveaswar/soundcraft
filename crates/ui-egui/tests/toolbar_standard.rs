//! The Standard (one-row) Edit toolbar: size, style switch and the commands behind its keys.

use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::toolbar_layout::ToolbarStyle;
use soundcraft_ui_egui::{Services, SoundApp, toolbar};

fn harness(width: f32, style: ToolbarStyle) -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.toolbar_style = style;
    let mut h = Harness::builder().with_size(vec2(width, 1000.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn timeline_top(h: &Harness<SoundApp>) -> f32 {
    h.state().edit_layout.timeline[1]
}

#[test]
fn standard_is_one_34_point_row_and_classic_keeps_78() {
    let std = harness(1600.0, ToolbarStyle::Standard);
    let classic = harness(1600.0, ToolbarStyle::Classic);
    assert_eq!(toolbar::height(std.state()), 34.0);
    assert_eq!(toolbar::height(classic.state()), 78.0);
    let shift = timeline_top(&classic) - timeline_top(&std);
    // Standard is 44 pt shorter, then adds the info row and the selection strip.
    let rows = soundcraft_ui_egui::info_row::HEIGHT + soundcraft_ui_egui::selection_strip::HEIGHT;
    assert!((shift - (44.0 - rows)).abs() < 1.0, "the Standard rows should net {} pt, was {shift}", 44.0 - rows);
}

#[test]
fn the_transport_on_top_needs_the_classic_toolbar() {
    let mut h = harness(1600.0, ToolbarStyle::Standard);
    h.state_mut().run("ui.transport_position", json!({"position": "top"})).unwrap();
    assert_eq!(toolbar::effective_style(h.state()), ToolbarStyle::Classic);
    assert_eq!(toolbar::height(h.state()), 78.0);
    h.state_mut().run("ui.transport_position", json!({"position": "bottom"})).unwrap();
    assert_eq!(toolbar::effective_style(h.state()), ToolbarStyle::Standard);
}

#[test]
fn the_style_command_switches_and_rejects_unknown_names() {
    let mut h = harness(1600.0, ToolbarStyle::Standard);
    assert_eq!(h.state_mut().run("ui.toolbar_style", json!({})).unwrap()["style"], "standard");
    assert_eq!(h.state_mut().run("ui.toolbar_style", json!({"value": "classic"})).unwrap()["style"], "classic");
    h.run_steps(2);
    assert_eq!(toolbar::height(h.state()), 78.0);
    assert!(h.state_mut().run("ui.toolbar_style", json!({"value": "wide"})).is_err());
    assert!(soundcraft_ui_egui::menus::UI_COMMANDS.iter().any(|c| c.0 == "ui.toolbar_style"));
}

#[test]
fn tool_keys_set_the_tool() {
    let mut h = harness(1600.0, ToolbarStyle::Standard);
    for (label, id) in [("Pencil", "pencil"), ("Grabber", "grabber"), ("Smart Tool", "smart"), ("Trim", "trim")] {
        h.get_by_label_contains(label).click();
        h.run_steps(2);
        assert_eq!(h.state().engine.session().edit.tool.id(), id, "{label}");
    }
}

#[test]
fn the_snap_key_toggles_snap_and_leaves_grid_mode_for_the_previous_mode() {
    let mut h = harness(1600.0, ToolbarStyle::Standard);
    h.state_mut().engine.execute("edit.mode", &json!({"mode": "slip"})).unwrap();
    h.run_steps(2);
    let snap = |h: &Harness<SoundApp>| h.state().engine.session().edit.flag(soundcraft_engine::edit::SNAP_TO_GRID);
    assert!(!snap(&h));
    h.get_by_label_contains("Snap to grid").click();
    h.run_steps(2);
    assert!(snap(&h), "snap on in Slip mode");
    h.get_by_label_contains("Snap to grid").click();
    h.run_steps(2);
    assert!(!snap(&h));

    h.state_mut().engine.execute("edit.mode", &json!({"mode": "grid"})).unwrap();
    h.run_steps(2);
    h.get_by_label_contains("Snap to grid").click();
    h.run_steps(2);
    let e = &h.state().engine.session().edit;
    assert!(!e.edit_mode.is_grid(), "left Grid mode");
    assert_eq!(e.edit_mode, soundcraft_model::EditMode::Slip);
}

#[test]
fn the_adapt_key_toggles_the_adaptive_grid() {
    let mut h = harness(1600.0, ToolbarStyle::Standard);
    let on = |h: &Harness<SoundApp>| h.state().engine.session().edit.flag("grid.adaptive");
    assert!(!on(&h));
    h.get_by_label_contains("Adapt the grid").click();
    h.run_steps(2);
    assert!(on(&h));
    h.get_by_label_contains("Adapt the grid").click();
    h.run_steps(2);
    assert!(!on(&h));
}

#[test]
fn undo_and_redo_keys_run_the_edit_commands() {
    let mut h = harness(1600.0, ToolbarStyle::Standard);
    let tracks = |h: &Harness<SoundApp>| h.state().engine.session().tracks.len();
    let before = tracks(&h);
    h.state_mut().run("track.new", json!({})).unwrap();
    h.run_steps(2);
    assert_eq!(tracks(&h), before + 1);
    h.get_by_label_contains("Undo New").click();
    h.run_steps(2);
    assert_eq!(tracks(&h), before);
    h.get_by_label_contains("Redo New").click();
    h.run_steps(2);
    assert_eq!(tracks(&h), before + 1);
}

#[test]
fn panel_keys_toggle_their_panels() {
    let mut h = harness(1800.0, ToolbarStyle::Standard);
    let before = h.state().ui.show_memory_locations;
    let key = h.get_all_by_label("Memory Locations").find(|n| n.rect().min.y < 90.0).expect("a Memory Locations key in the toolbar");
    key.click();
    h.run_steps(2);
    assert_ne!(h.state().ui.show_memory_locations, before);
}

#[test]
fn a_narrow_window_still_draws_and_keeps_the_essential_keys() {
    let mut h = harness(700.0, ToolbarStyle::Standard);
    h.run_steps(3);
    h.get_by_label_contains("Pencil");
    h.get_by_label_contains("Snap to grid");
    h.get_by_label_contains("Zoom out");
}

fn track_id(h: &Harness<SoundApp>, name: &str) -> u64 {
    h.state().engine.session().track_by_name(name).unwrap().id.0
}

fn all_ids(h: &Harness<SoundApp>) -> Vec<u64> {
    h.state().engine.session().tracks.iter().map(|t| t.id.0).collect()
}

fn modes(h: &Harness<SoundApp>) -> Vec<(String, soundcraft_model::AutomationMode)> {
    h.state().engine.session().tracks.iter().map(|t| (t.name.clone(), t.mixer.automation_mode)).collect()
}

#[test]
fn the_m_key_clears_mutes_and_mutes_the_selection_when_none_are_muted() {
    let mut h = harness(1800.0, ToolbarStyle::Standard);
    let (kick, bass) = (track_id(&h, "Kick"), track_id(&h, "Bass"));
    h.state_mut().engine.execute("mix.mute", &json!({"tracks": [kick, bass], "value": true})).unwrap();
    h.run_steps(2);
    h.get_by_label_contains("Mute: clear all mutes").click();
    h.run_steps(2);
    assert!(h.state().engine.session().tracks.iter().all(|t| !t.mixer.mute), "all mutes cleared");

    h.state_mut().engine.execute("edit.select", &json!({"tracks": [kick]})).unwrap();
    h.run_steps(2);
    h.get_by_label_contains("Mute: clear all mutes").click();
    h.run_steps(2);
    let s = h.state().engine.session();
    assert!(s.track_by_name("Kick").unwrap().mixer.mute);
    assert!(!s.track_by_name("Bass").unwrap().mixer.mute, "only the selection");
}

#[test]
fn the_s_key_clears_solos() {
    let mut h = harness(1800.0, ToolbarStyle::Standard);
    let kick = track_id(&h, "Kick");
    h.state_mut().engine.execute("mix.solo", &json!({"tracks": [kick], "value": true})).unwrap();
    h.run_steps(2);
    h.get_by_label_contains("Solo: clear all solos").click();
    h.run_steps(2);
    assert!(h.state().engine.session().tracks.iter().all(|t| !t.mixer.solo));
}

#[test]
fn r_and_w_set_the_mode_on_the_selection_and_modifier_click_sets_all() {
    use soundcraft_model::AutomationMode::{Off, Read, Write};
    let mut h = harness(1800.0, ToolbarStyle::Standard);
    let kick = track_id(&h, "Kick");
    let ids = all_ids(&h);
    h.state_mut().engine.execute("mix.automation_mode", &json!({"tracks": ids, "mode": "off"})).unwrap();
    h.state_mut().engine.execute("edit.select", &json!({"tracks": [kick]})).unwrap();
    h.run_steps(2);
    h.get_by_label_contains("Read automation").click();
    h.run_steps(2);
    for (name, mode) in modes(&h) {
        assert_eq!(mode, if name == "Kick" { Read } else { Off }, "{name}");
    }
    h.get_by_label_contains("Write automation").click_modifiers(egui::Modifiers::COMMAND);
    h.run_steps(2);
    assert!(modes(&h).iter().all(|(_, m)| *m == Write), "modifier-click applies to every track");
}

#[test]
fn the_automation_combo_shows_the_selection_and_applies_a_mode() {
    use soundcraft_model::AutomationMode::{Off, Touch};
    let mut h = harness(1800.0, ToolbarStyle::Standard);
    let kick = track_id(&h, "Kick");
    let ids = all_ids(&h);
    h.state_mut().engine.execute("mix.automation_mode", &json!({"tracks": ids, "mode": "off"})).unwrap();
    h.state_mut().engine.execute("edit.select", &json!({"tracks": [kick]})).unwrap();
    h.run_steps(2);
    h.get_by_label_contains("Automation mode of the selected tracks").click();
    h.run_steps(3);
    h.get_by_label("Touch").click();
    h.run_steps(2);
    for (name, mode) in modes(&h) {
        assert_eq!(mode, if name == "Kick" { Touch } else { Off }, "{name}");
    }
}
