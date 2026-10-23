//! The Mixer dock panel: opened through `ui.dock` and `window.mixer_panel`, Edit window only,
//! sharing the lower zone with the MIDI editor as exclusive Editor / Mixer tabs.
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::{MainWindow, Services, SoundApp, docking::Panel, menus, panel_menu};

fn harness() -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(5);
    h
}

fn active_lower(h: &Harness<'_, SoundApp>) -> Option<Panel> {
    soundcraft_ui_egui::docking::lower_tab_active(&h.state().ui.docking)
}

#[test]
fn ui_dock_opens_and_closes_the_mixer() {
    let mut h = harness();
    assert!(!h.state().ui.docking.layout.contains(&Panel::Mixer) && !h.state().ui.show_mixer_panel);
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "mixer"})).unwrap();
    h.run_steps(3);
    assert!(h.state().ui.docking.layout.contains(&Panel::Mixer) && h.state().ui.show_mixer_panel);
    assert!(panel_menu::is_visible(h.state(), Panel::Mixer));
    h.state_mut().run("ui.dock", json!({"operation": "close", "panel": "mixer"})).unwrap();
    h.run_steps(3);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Mixer) && !h.state().ui.show_mixer_panel);
    h.state_mut().run("window.mixer_panel", json!({})).unwrap();
    h.run_steps(3);
    assert!(h.state().ui.docking.layout.contains(&Panel::Mixer));
    assert!(menus::item_state(h.state(), "", Some("window.mixer_panel")).1);
}

#[test]
fn the_mix_window_refuses_only_the_timeline_panels() {
    let mut h = harness();
    h.state_mut().ui.window = MainWindow::Mix;
    h.run_steps(3);
    let err = h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap_err();
    assert!(err.contains("MIDI"), "{err}");
    assert!(h.state_mut().run("ui.dock", json!({"operation": "move", "panel": "tracks", "target": "clips"})).is_err());
    assert!(!panel_menu::panels_for(false).contains(&Panel::Midi));
    assert!(panel_menu::panels_for(false).contains(&Panel::Mixer));
}

#[test]
fn editor_and_mixer_are_exclusive_tabs_of_the_lower_zone() {
    let mut h = harness();
    assert_eq!(active_lower(&h), None, "the Mixer starts closed");
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap();
    h.run_steps(3);
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "mixer"})).unwrap();
    h.run_steps(3);
    assert_eq!(active_lower(&h), Some(Panel::Mixer), "opening the Mixer shows it in place of the Editor");
    assert!(h.state().ui.show_midi_editor && h.state().ui.show_mixer_panel);
    // The EDITOR tab switches back without closing the Mixer.
    h.get_by_label("EDITOR").click();
    h.run_steps(3);
    assert_eq!(active_lower(&h), Some(Panel::Midi));
    assert!(h.state().ui.show_mixer_panel && h.state().ui.show_midi_editor, "switching closes nothing");
    h.get_by_label("MIXER").click();
    h.run_steps(3);
    assert_eq!(active_lower(&h), Some(Panel::Mixer));
    // Opening the Editor again (through the dock) takes the tab back.
    h.state_mut().run("ui.dock", json!({"operation": "activate", "panel": "midi"})).unwrap();
    assert_eq!(active_lower(&h), Some(Panel::Midi));
}

#[test]
fn a_hidden_track_has_no_strip_in_the_docked_mixer() {
    let mut h = harness();
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "mixer"})).unwrap();
    h.run_steps(4);
    let name = h
        .state()
        .engine
        .session()
        .tracks
        .iter()
        .find(|t| !t.hidden && t.kind != soundcraft_model::TrackKind::Master)
        .map(|t| t.name.clone())
        .unwrap();
    assert!(h.query_all_by_label(&format!("Strip {name}")).count() >= 1);
    h.state_mut().run("track.hide", json!({"tracks": [name.clone()], "hidden": true})).unwrap();
    h.run_steps(4);
    assert_eq!(h.query_all_by_label(&format!("Strip {name}")).count(), 0);
}
