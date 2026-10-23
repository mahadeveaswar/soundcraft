//! The Alternative key-command preset: single letters, number-row tools, Delete, Insert, and the rules that
//! keep the Default preset and text fields undisturbed.

use egui::os::OperatingSystem;
use egui::{Key, Modifiers};
use serde_json::json;
use soundcraft_model::Tool;
use soundcraft_ui_egui::{Services, SoundApp, shortcuts};

const PC_CTRL: Modifiers = Modifiers { ctrl: true, command: true, alt: false, shift: false, mac_cmd: false };

fn app(preset: &str) -> SoundApp {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("ui.prefs", json!({"keymap_preset": preset})).unwrap();
    app
}

fn press(app: &mut SoundApp, key: Key, modifiers: Modifiers) {
    let ctx = egui::Context::default();
    ctx.set_os(OperatingSystem::Windows);
    let events = [true, false].map(|pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }).to_vec();
    ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| shortcuts::handle(app, ui.ctx())).textures_delta.clear();
}

fn kick(app: &SoundApp) -> soundcraft_model::Track {
    app.engine.session().track_by_name("Kick").unwrap().clone()
}

#[test]
fn alt_x_splits_the_selected_clip_at_the_playhead() {
    let mut app = app("alternative");
    let clip = kick(&app).clips()[0].clone();
    let before = kick(&app).clips().len();
    app.run("edit.select", json!({"clips": [clip.id.0]})).unwrap();
    app.run("transport.locate", json!({"at": clip.start + clip.length / 2})).unwrap();
    press(&mut app, Key::X, Modifiers::ALT);
    assert_eq!(kick(&app).clips().len(), before + 1);
}

#[test]
fn ctrl_k_opens_the_repeat_dialog() {
    let mut app = app("alternative");
    let clip = kick(&app).clips()[0].id;
    app.run("edit.select", json!({"clips": [clip.0]})).unwrap();
    press(&mut app, Key::K, PC_CTRL);
    assert!(app.dialogs.open.is_some(), "Cmd+K asks how many repeats");
}

#[test]
fn m_mutes_the_selected_track_and_shift_m_mutes_the_clip() {
    let mut app = app("alternative");
    app.run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
    press(&mut app, Key::M, Modifiers::NONE);
    assert!(app.engine.session().track_by_name("Kick").unwrap().mixer.mute);
    press(&mut app, Key::M, Modifiers::NONE);
    assert!(!app.engine.session().track_by_name("Kick").unwrap().mixer.mute, "M toggles");
    let clip = kick(&app).clips()[0].id;
    app.run("edit.select", json!({"clips": [clip.0]})).unwrap();
    press(&mut app, Key::M, Modifiers::SHIFT);
    assert!(kick(&app).clips()[0].muted);
    press(&mut app, Key::U, Modifiers::SHIFT);
    assert!(!kick(&app).clips()[0].muted);
}

#[test]
fn slash_toggles_loop_and_number_row_chooses_tools() {
    let mut app = app("alternative");
    let on = app.engine.session().edit.loop_playback;
    press(&mut app, Key::Slash, Modifiers::NONE);
    assert_ne!(app.engine.session().edit.loop_playback, on);
    for (key, tool) in
        [(Key::Num6, Tool::Zoom), (Key::Num1, Tool::Grabber), (Key::Num2, Tool::Selector), (Key::Num8, Tool::Pencil), (Key::Num9, Tool::Scrubber)]
    {
        press(&mut app, key, Modifiers::NONE);
        assert_eq!(app.engine.session().edit.tool, tool, "{key:?}");
    }
}

#[test]
fn ctrl_y_redoes_and_delete_removes_the_selected_clip() {
    let mut app = app("alternative");
    let clip = kick(&app).clips()[0].id;
    let before = kick(&app).clips().len();
    app.run("edit.select", json!({"clips": [clip.0]})).unwrap();
    press(&mut app, Key::Delete, Modifiers::NONE);
    assert_eq!(kick(&app).clips().len(), before - 1);
    press(&mut app, Key::Z, PC_CTRL);
    assert_eq!(kick(&app).clips().len(), before);
    press(&mut app, Key::Y, PC_CTRL);
    assert_eq!(kick(&app).clips().len(), before - 1, "Ctrl+Y redoes");
}

#[test]
fn the_default_preset_keeps_plain_letters_out_of_it() {
    let mut app = app("default");
    app.run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
    press(&mut app, Key::M, Modifiers::NONE);
    assert!(!app.engine.session().track_by_name("Kick").unwrap().mixer.mute);
    let tool = app.engine.session().edit.tool;
    press(&mut app, Key::Num6, Modifiers::NONE);
    assert_eq!(app.engine.session().edit.tool, tool, "6 is not the zoom tool in the Default preset");
    // Shift+M is the Default key for mute.
    press(&mut app, Key::M, Modifiers::SHIFT);
    assert!(app.engine.session().track_by_name("Kick").unwrap().mixer.mute);
}

#[test]
fn typing_in_a_text_field_fires_no_letter_commands() {
    let mut app = app("alternative");
    app.run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
    app.run("window.settings", json!({"tab": "keys"})).unwrap();
    let ctx = egui::Context::default();
    let frame = |app: &mut SoundApp, events: Vec<egui::Event>| {
        ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
            app.logic(ui.ctx());
            app.ui(ui);
        })
        .textures_delta
        .clear();
    };
    for _ in 0..3 {
        frame(&mut app, vec![]);
    }
    ctx.memory_mut(|m| m.request_focus(egui::Id::new("settings_keys_search")));
    frame(&mut app, vec![]);
    frame(&mut app, vec![]);
    assert!(ctx.egui_wants_keyboard_input(), "the search box has the keyboard");
    let key = |k| [true, false].map(|pressed| egui::Event::Key { key: k, physical_key: None, pressed, repeat: false, modifiers: Modifiers::NONE });
    frame(&mut app, key(Key::M).into_iter().chain([egui::Event::Text("m".into())]).collect());
    assert!(!app.engine.session().track_by_name("Kick").unwrap().mixer.mute, "M typed in a field is text");
    assert_eq!(app.extra.key_search, "m");
}

#[test]
fn a_user_binding_works_in_the_default_preset_and_frees_the_old_key() {
    let mut app = app("default");
    app.run("ui.prefs", json!({"keymap_overrides": {"clip.mute": ["Alt+Shift+M"]}})).unwrap();
    let clip = kick(&app).clips()[0].id;
    app.run("edit.select", json!({"clips": [clip.0]})).unwrap();
    press(&mut app, Key::M, Modifiers::ALT | Modifiers::SHIFT);
    assert!(kick(&app).clips()[0].muted);
    // Rebinding a command that had a Default key stops the old key.
    app.run("ui.prefs", json!({"keymap_overrides": {"edit.mute_clips": ["Cmd+Alt+Shift+9"]}})).unwrap();
    app.run("clip.unmute", json!({})).unwrap();
    press(&mut app, Key::M, PC_CTRL);
    assert!(!kick(&app).clips()[0].muted, "Cmd+M no longer mutes clips");
}
