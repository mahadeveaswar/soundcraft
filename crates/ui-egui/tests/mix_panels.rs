//! Panels in the Mix window, Editor/Mixer side by side, the MIDI editor's remembered slot height
//! and the panel list on empty canvas.
use egui::vec2;
use egui_kittest::Harness;
use serde_json::json;
use soundcraft_ui_egui::{MainWindow, Services, SoundApp, UiState, docking::Panel};

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

#[test]
fn the_mix_window_starts_with_no_optional_panels() {
    let mix = soundcraft_ui_egui::docking::mix_default();
    mix.validate_mix().unwrap();
    for p in [Panel::Inspector, Panel::Master, Panel::Meter, Panel::Loudness, Panel::Visibility, Panel::Mixer, Panel::Clips, Panel::Midi] {
        assert!(!mix.layout.contains(&p), "{p:?}");
    }
}

#[test]
fn inspector_meters_loudness_mixer_and_master_open_in_the_mix_window() {
    let mut h = harness();
    h.state_mut().ui.window = MainWindow::Mix;
    h.run_steps(3);
    for p in ["inspector", "visibility", "meter", "loudness", "mixer", "master"] {
        h.state_mut().run("ui.dock", json!({"operation": "open", "panel": p})).unwrap_or_else(|e| panic!("{p}: {e}"));
        h.run_steps(3);
    }
    let w = &h.state().ui.mix_docking;
    w.validate_mix().unwrap();
    for p in [Panel::Inspector, Panel::Visibility, Panel::Meter, Panel::Loudness, Panel::Mixer, Panel::Master] {
        assert!(w.layout.contains(&p), "{p:?}");
    }
    assert!(h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "clips"})).is_err());
    h.state_mut().run("ui.dock", json!({"operation": "close", "panel": "inspector"})).unwrap();
    h.run_steps(3);
    assert!(!h.state().ui.mix_docking.layout.contains(&Panel::Inspector));
}

#[test]
fn the_mixer_docks_beside_the_session_and_keeps_its_place_when_reopened() {
    let mut h = harness();
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap();
    h.run_steps(3);
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "mixer"})).unwrap();
    h.run_steps(3);
    h.state_mut().run("ui.dock", json!({"operation": "move", "panel": "mixer", "target": "canvas", "zone": "right"})).unwrap();
    h.run_steps(3);
    let w = &h.state().ui.docking;
    w.validate().unwrap();
    assert!(w.layout.contains(&Panel::Mixer) && w.layout.contains(&Panel::Midi));
    assert_eq!(soundcraft_ui_egui::docking::lower_tab_active(w), None, "no longer tabbed with the editor");
    h.state_mut().run("ui.dock", json!({"operation": "close", "panel": "mixer"})).unwrap();
    h.run_steps(3);
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "mixer"})).unwrap();
    h.run_steps(3);
    assert_eq!(soundcraft_ui_egui::docking::lower_tab_active(&h.state().ui.docking), None, "reopened beside the session, not as a tab");
}

#[test]
fn minimizing_the_editor_shrinks_its_slot_and_restore_puts_the_height_back() {
    let mut h = harness();
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap();
    h.run_steps(3);
    let before = h.state().ui.docking.midi_slot().expect("a fixed slot");
    h.state_mut().midi.minimized = true;
    h.state_mut().midi.dock_request = Some(json!({"operation": "midiSlot", "collapse": true}));
    h.run_steps(4);
    let during = h.state().ui.docking.midi_slot().expect("still a fixed slot");
    assert_eq!(during.0, before.0);
    assert_ne!(during.1, before.1, "the slot got smaller");
    h.state_mut().midi.minimized = false;
    h.state_mut().midi.dock_request = Some(json!({"operation": "midiSlot", "collapse": false}));
    h.run_steps(4);
    assert_eq!(h.state().ui.docking.midi_slot(), Some(before.clone()), "the height is restored");
    // Closing while minimized restores the slot first.
    h.state_mut().midi.minimized = true;
    h.state_mut().midi.dock_request = Some(json!({"operation": "midiSlot", "collapse": true}));
    h.run_steps(4);
    h.state_mut().run("ui.dock", json!({"operation": "close", "panel": "midi"})).unwrap();
    h.run_steps(4);
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap();
    h.run_steps(4);
    assert!(!h.state().midi.minimized);
    assert!(h.state().ui.docking.midi_slot().is_some());
}

#[test]
fn piano_roll_settings_are_kept_in_the_ui_state_and_old_files_still_load() {
    let mut h = harness();
    h.state_mut().run("ui.midi_tool", json!({"tool": "erase"})).unwrap();
    h.state_mut().run("ui.midi_snap", json!({"snap": "1/16t"})).unwrap();
    h.state_mut().run("ui.midi_inspector", json!({"open": true})).unwrap();
    h.state_mut().run("ui.midi_lane_add", json!({"lane": "modulation"})).unwrap();
    h.state_mut().midi.quant_strength = 60;
    h.run_steps(3);
    let saved = serde_json::to_value(&h.state().ui).unwrap();
    let stored = &saved["midi_editor"];
    assert_eq!((stored["tool"].as_str(), stored["snap"].as_str(), stored["quant_strength"].as_u64()), (Some("erase"), Some("1/16t"), Some(60)));
    assert_eq!(stored["lanes"].as_array().map(Vec::len), Some(2));
    // A fresh app restores them from the saved state.
    let mut fresh = harness();
    fresh.state_mut().ui = serde_json::from_value::<UiState>(saved).unwrap();
    fresh.run_steps(3);
    let m = &fresh.state().midi;
    assert_eq!((m.tool.id(), m.snap.id(), m.quant_strength, m.inspector_open, m.lanes.len()), ("erase", "1/16t".to_string(), 60, true, 2));
    // Old files without the field (or with junk in it) load with defaults.
    let old: UiState = serde_json::from_value(json!({"show_transport": true})).unwrap();
    assert_eq!(old.midi_editor, Default::default());
    let junk: UiState = serde_json::from_value(json!({"midi_editor": {"tool": "nope", "snap": "x", "quant_strength": 250}})).unwrap_or_default();
    let mut h2 = harness();
    h2.state_mut().ui = junk;
    h2.run_steps(3);
    assert_eq!((h2.state().midi.tool.id(), h2.state().midi.snap.id()), ("select", "grid".to_string()));
    assert!(h2.state().midi.quant_strength <= 100);
}
