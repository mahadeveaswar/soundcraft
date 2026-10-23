use egui::os::OperatingSystem;
use egui::{Key, Modifiers};
use serde_json::json;
use soundcraft_ui_egui::{MainWindow, Services, SoundApp, shortcuts};

fn press(ctx: &egui::Context, app: &mut SoundApp, key: Key, modifiers: Modifiers) {
    let events = [true, false].map(|pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }).to_vec();
    ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| shortcuts::handle(app, ui.ctx())).textures_delta.clear();
}

#[test]
fn ctrl_shift_arrows_nudge_the_selected_clip_gain() {
    let pc_ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    for (os, ctrl) in [(OperatingSystem::Mac, Modifiers::CTRL), (OperatingSystem::Windows, pc_ctrl), (OperatingSystem::Nix, pc_ctrl)] {
        let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
        let kick = app.engine.session().track_by_name("Kick").unwrap().clips()[0].id;
        app.engine.execute("edit.select", &json!({"clips": [kick.0]})).unwrap();
        let ctx = egui::Context::default();
        ctx.set_os(os);
        let ctrl_shift = Modifiers { shift: true, ..ctrl };
        press(&ctx, &mut app, Key::ArrowUp, ctrl_shift);
        press(&ctx, &mut app, Key::ArrowUp, ctrl_shift);
        press(&ctx, &mut app, Key::ArrowDown, ctrl_shift);
        assert_eq!(app.engine.session().find_clip(kick).unwrap().1.gain_db, 1.0, "{os:?}");
    }
}

fn type_in_app(ctx: &egui::Context, app: &mut SoundApp, keys: &[(Key, Modifiers)]) {
    let events = keys
        .iter()
        .flat_map(|&(key, modifiers)| [true, false].map(|pressed| egui::Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }))
        .collect();
    ctx.run_ui(egui::RawInput { events, ..Default::default() }, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    })
    .textures_delta
    .clear();
}

#[test]
fn tab_leaves_the_keys_to_the_shortcuts() {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let ctx = egui::Context::default();
    for _ in 0..3 {
        type_in_app(&ctx, &mut app, &[]);
    }
    for tab in [Modifiers::NONE, Modifiers::SHIFT] {
        type_in_app(&ctx, &mut app, &[(Key::Tab, tab)]);
        assert_eq!(ctx.memory(|m| m.focused()), None, "{tab:?}");
        let zoom = app.engine.session().edit.zoom.samples_per_px;
        type_in_app(&ctx, &mut app, &[(Key::T, Modifiers::NONE)]);
        assert_eq!(app.engine.session().edit.zoom.samples_per_px, zoom / 2.0, "{tab:?}");
        type_in_app(&ctx, &mut app, &[(Key::R, Modifiers::NONE)]);
        assert_eq!(app.engine.session().edit.zoom.samples_per_px, zoom, "{tab:?}");
    }
}

#[test]
fn cmd_equals_toggles_mix_and_edit_without_zooming() {
    let mac_cmd = Modifiers { mac_cmd: true, command: true, ..Modifiers::NONE };
    let pc_ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    for (os, cmd) in [(OperatingSystem::Mac, mac_cmd), (OperatingSystem::Windows, pc_ctrl), (OperatingSystem::Nix, pc_ctrl)] {
        let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
        let ctx = egui::Context::default();
        ctx.set_os(os);
        ctx.set_zoom_factor(1.25);
        press(&ctx, &mut app, Key::Equals, cmd);
        assert_eq!(app.ui.window, MainWindow::Mix, "{os:?}");
        press(&ctx, &mut app, Key::Equals, cmd);
        press(&ctx, &mut app, Key::Num0, cmd);
        press(&ctx, &mut app, Key::Minus, cmd);
        ctx.run_ui(egui::RawInput::default(), |_| {}).textures_delta.clear();
        assert_eq!(app.ui.window, MainWindow::Edit, "{os:?}");
        assert_eq!(ctx.zoom_factor(), 1.25, "{os:?}");
    }
}

/// Every parseable default shortcut belongs to one command, so none is shadowed and unreachable.
/// The Space trio (play, stop, toggle) and the Mix/Edit trio (Cmd+=) are each handled by one fixed rule,
/// so they are the only allowed repeats.
#[test]
fn default_shortcuts_are_unique_per_chord() {
    use std::collections::BTreeMap;
    let mut seen: BTreeMap<String, Vec<&str>> = BTreeMap::new();
    let engine = soundcraft_engine::command_specs().iter().map(|c| (c.id, c.shortcut));
    let ui = soundcraft_ui_egui::menus::UI_COMMANDS.iter().map(|c| (c.0, c.3));
    for (id, shortcut) in engine.chain(ui) {
        let Some((m, key)) = shortcut.and_then(shortcuts::parse) else { continue };
        let chord = format!("{}{}{}{}{key:?}", u8::from(m.command), u8::from(m.ctrl), u8::from(m.alt), u8::from(m.shift));
        seen.entry(chord).or_default().push(id);
    }
    let allowed = ["transport.play", "transport.stop", "transport.toggle", "window.mix", "window.edit", "window.toggle_mix_edit"];
    let clashes: Vec<_> = seen.values().filter(|ids| ids.len() > 1 && !ids.iter().all(|i| allowed.contains(i))).collect();
    assert!(clashes.is_empty(), "shortcuts shared by several commands: {clashes:?}");
}
