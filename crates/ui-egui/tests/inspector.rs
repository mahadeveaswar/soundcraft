//! The Inspector dock panel: closed by default, opened through `ui.dock` and `window.inspector`,
//! Edit window only, old saved preferences still load, and it shows the selected track's slots.
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::{MainWindow, Services, SoundApp, UiState, docking::Panel, menus, panel_menu};

fn harness() -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1500.0, 1000.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(5);
    h
}

fn count(h: &Harness<'_, SoundApp>, label: &str) -> usize {
    h.query_all_by_label(label).count()
}

#[test]
fn closed_by_default_opens_left_and_closes_through_ui_dock() {
    let mut h = harness();
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector));
    assert!(!h.state().ui.show_inspector);
    assert!(!panel_menu::is_visible(h.state(), Panel::Inspector));
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    let ui = &h.state().ui;
    assert!(ui.docking.layout.contains(&Panel::Inspector) && ui.show_inspector);
    assert!(ui.docking.validate().is_ok());
    h.state_mut().run("ui.dock", json!({"operation": "close", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector) && !h.state().ui.show_inspector);
    // The panel remembers where it was: reopening restores a valid layout.
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    assert!(h.state().ui.docking.validate().is_ok());
    // Reset brings the default layout back, with the Inspector closed.
    h.state_mut().run("ui.dock.reset", json!({})).unwrap();
    h.run_steps(4);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector));
}

#[test]
fn the_window_command_and_menu_state_follow_the_panel() {
    let mut h = harness();
    assert!(!menus::item_state(h.state(), "", Some("window.inspector")).1);
    h.state_mut().run("window.inspector", json!({})).unwrap();
    h.run_steps(4);
    assert!(h.state().ui.docking.layout.contains(&Panel::Inspector));
    assert!(menus::item_state(h.state(), "", Some("window.inspector")).1);
    h.state_mut().run("window.inspector", json!({})).unwrap();
    h.run_steps(4);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector));
}

#[test]
fn the_mix_window_rejects_the_inspector() {
    let mut h = harness();
    h.state_mut().ui.window = MainWindow::Mix;
    h.run_steps(3);
    assert!(h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).is_err());
    assert!(!panel_menu::panels_for(false).contains(&Panel::Inspector));
}

#[test]
fn preferences_saved_before_the_inspector_still_load() {
    let mut value = serde_json::to_value(UiState::default()).unwrap();
    let object = value.as_object_mut().unwrap();
    assert!(object.remove("show_inspector").is_some(), "the flag is saved");
    let old: UiState = serde_json::from_value(value).unwrap();
    assert!(!old.show_inspector);
    assert!(old.docking.validate().is_ok());
    // A saved layout that predates the panel has no "inspector" anywhere and stays valid.
    let text = serde_json::to_string(&old.docking).unwrap();
    assert!(!text.contains("inspector"));
}

#[test]
fn it_shows_the_selected_tracks_inserts_and_sends() {
    let mut h = harness();
    h.state_mut().run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    h.run_steps(3);
    let (inserts, sends) = (count(&h, "Insert A"), count(&h, "Send A"));
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    assert_eq!(count(&h, "Insert A"), inserts + 1, "the Inspector adds one Insert A slot");
    assert_eq!(count(&h, "Send A"), sends + 1, "the Inspector adds one Send A slot");
    let all_slots = count(&h, "Insert J");
    // With nothing selected it shows no slots.
    h.state_mut().run("edit.select", json!({"tracks": []})).unwrap();
    h.run_steps(4);
    assert_eq!(count(&h, "Insert A"), inserts);
    assert_eq!(count(&h, "Insert J"), all_slots - 1, "all ten insert slots are gone with the selection");
}
