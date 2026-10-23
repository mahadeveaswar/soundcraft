//! Settings > Keys: choose the key set, assign a key to a command, see a conflict and resolve it, reset.

use egui::{Event, Key, Modifiers, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp};

fn page() -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("window.settings", json!({"tab": "keys"})).unwrap();
    app.extra.key_search = "clip.glue".into();
    let mut h = Harness::builder().with_size(vec2(1200.0, 900.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    steps(&mut h, 4);
    h
}

fn steps(h: &mut Harness<'_, SoundApp>, n: usize) {
    for _ in 0..n {
        let mut raw = std::mem::take(h.input_mut());
        h.state_mut().raw_input_hook(&mut raw);
        *h.input_mut() = raw;
        h.step();
    }
}

fn press(h: &mut Harness<'_, SoundApp>, key: Key, modifiers: Modifiers) {
    h.input_mut().events.push(Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers });
    steps(h, 3);
}

#[test]
fn the_keys_page_lists_commands_and_switches_the_key_set() {
    let mut h = page();
    assert!(h.query_by_label("Key set").is_some());
    h.get_by_label("Assign");
    h.state_mut().run("ui.prefs", json!({"keymap_preset": "alternative"})).unwrap();
    steps(&mut h, 3);
    assert!(h.query_by_label_contains("Alt+G").is_some(), "the Alternative key for Glue is listed");
}

#[test]
fn assigning_a_free_chord_adds_it_and_reset_restores_the_key_set() {
    let mut h = page();
    h.get_by_label("Assign").click();
    steps(&mut h, 2);
    assert!(h.state().extra.key_capture.is_some(), "waiting for a key");
    assert!(h.query_by_label_contains("Press the new key").is_some());
    press(&mut h, Key::G, Modifiers { command: true, ctrl: true, alt: true, shift: true, mac_cmd: false });
    assert!(h.state().extra.key_capture.is_none());
    assert_eq!(h.state().ui.keymap.bindings("clip.glue"), vec!["Cmd+Alt+Shift+G".to_string()]);
    h.get_by_label("Reset").click();
    steps(&mut h, 2);
    assert!(h.state().ui.keymap.overrides.is_empty());
}

#[test]
fn a_taken_chord_shows_who_has_it_and_reassign_takes_it() {
    let mut h = page();
    h.state_mut().run("ui.prefs", json!({"keymap_preset": "alternative"})).unwrap();
    steps(&mut h, 2);
    h.get_by_label("Assign").click();
    steps(&mut h, 2);
    press(&mut h, Key::M, Modifiers::NONE);
    assert!(h.query_by_label_contains("already used by").is_some(), "the conflict is named");
    assert!(h.state().ui.keymap.overrides.is_empty(), "nothing changed yet");
    h.get_by_label("Reassign").click();
    steps(&mut h, 2);
    assert!(h.state().ui.keymap.bindings("clip.glue").contains(&"M".to_string()));
    assert!(h.state().ui.keymap.bindings("mix.mute").is_empty());
    h.get_by_label("Reset all").click();
    steps(&mut h, 2);
    assert_eq!(h.state().ui.keymap.bindings("mix.mute"), vec!["M".to_string()]);
}

#[test]
fn escape_cancels_the_capture() {
    let mut h = page();
    h.get_by_label("Assign").click();
    steps(&mut h, 2);
    press(&mut h, Key::Escape, Modifiers::NONE);
    assert!(h.state().extra.key_capture.is_none());
    assert!(h.state().ui.keymap.overrides.is_empty());
}
