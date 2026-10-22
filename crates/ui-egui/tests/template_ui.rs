use egui::accesskit::Role;
use egui::{Key, vec2};
use egui_kittest::{
    Harness,
    kittest::{NodeT, Queryable},
};
use soundcraft_ui_egui::{Services, SoundApp, menus};
use std::path::PathBuf;

fn tmp(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("sc-tpl-ui-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn app_with_dir(dir: &std::path::Path) -> SoundApp {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.templates.dir = Some(dir.to_string_lossy().to_string());
    app
}

fn harness(app: SoundApp) -> Harness<'static, SoundApp> {
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(3);
    h
}

/// Types into the dialog's text field, which takes the focus when it opens.
fn type_focused(h: &mut Harness<'_, SoundApp>, text: &str) {
    h.run_steps(2);
    let node = h.query_all_by_role(Role::TextInput).find(|n| n.accesskit_node().is_focused());
    assert!(node.is_some(), "no focused text field");
    if let Some(n) = node {
        n.type_text(text);
    }
}

#[test]
fn menu_items_open_their_dialogs_and_escape_closes() {
    let mut app = app_with_dir(&tmp("open"));
    for (id, path, name) in [
        ("session.save_template", "File > Save As Template...", "template_save"),
        ("template.new_session", "File > New from Template...", "template_new"),
        ("template.instrument_rack", "File > Instrument Rack...", "template_rack"),
    ] {
        menus::invoke_menu(&mut app, id, path);
        assert_eq!(app.templates.open.as_ref().map(|d| d.name()), Some(name), "{path}");
        app.templates.open = None;
    }
    menus::invoke_menu(&mut app, "template.instrument_rack", "File > Instrument Rack...");
    let mut h = harness(app);
    h.get_by_role_and_label(Role::Button, "Cancel");
    h.key_press(Key::Escape);
    h.run_steps(2);
    assert!(h.state().templates.open.is_none());
}

#[test]
fn save_as_template_writes_a_file_and_new_from_template_starts_untitled() {
    let dir = tmp("save");
    let mut app = app_with_dir(&dir);
    let tracks = app.engine.session().tracks.len();
    assert!(tracks > 0);
    menus::invoke_menu(&mut app, "session.save_template", "File > Save As Template...");
    let mut h = harness(app);
    type_focused(&mut h, "Band Setup");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert!(h.state().templates.open.is_none(), "status: {}", h.state().ui.status);
    assert!(dir.join("Band Setup.scraft").is_file());

    // Saving the same name again asks before replacing.
    menus::invoke_menu(h.state_mut(), "session.save_template", "File > Save As Template...");
    h.run_steps(2);
    type_focused(&mut h, "Band Setup");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert!(h.state().templates.open.is_some(), "an existing name keeps the dialog open");
    h.get_by_role_and_label(Role::Button, "Replace");
    h.key_press(Key::Escape);
    h.run_steps(2);

    // The list shows it, and Create starts an untitled session without clips.
    h.state_mut().engine.mark_clean();
    menus::invoke_menu(h.state_mut(), "template.new_session", "File > New from Template...");
    h.run_steps(3);
    h.get_by_role_and_label(Role::Button, "Band Setup");
    h.get_by_role_and_label(Role::Button, "Create").click();
    h.run_steps(3);
    let e = &h.state().engine;
    assert_eq!(e.session().name, "Untitled");
    assert_eq!(e.session().tracks.len(), tracks);
    assert!(e.path.is_none());
    assert!(e.session().tracks.iter().all(|t| t.playlists.iter().all(|p| p.clips.is_empty())));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn instrument_rack_dialog_adds_named_tracks_in_one_undo_step() {
    let mut app = app_with_dir(&tmp("rack"));
    let before = app.engine.session().tracks.len();
    menus::invoke_menu(&mut app, "template.instrument_rack", "File > Instrument Rack...");
    let mut h = harness(app);
    type_focused(&mut h, "Piano, Strings");
    h.run_steps(2);
    h.key_press(Key::Enter);
    h.run_steps(3);
    let s = h.state().engine.session();
    assert_eq!(s.tracks.len(), before + 10);
    let has = |n: &str| s.tracks.iter().any(|t| t.name == n && t.kind == soundcraft_model::TrackKind::Instrument);
    assert!(has("Piano") && has("Strings") && has("Instrument 3") && has("Instrument 10"));
    h.state_mut().run("edit.undo", serde_json::json!({})).unwrap();
    assert_eq!(h.state().engine.session().tracks.len(), before, "one undo removes the rack");
}

#[test]
fn a_command_error_shows_in_the_dialog_and_the_status_line() {
    let mut app = app_with_dir(&tmp("err"));
    menus::invoke_menu(&mut app, "session.save_template", "File > Save As Template...");
    if let Some(soundcraft_ui_egui::template_ui::TemplateDialog::Save { name, .. }) = &mut app.templates.open {
        *name = "???".into();
    }
    let mut h = harness(app);
    h.key_press(Key::Enter);
    h.run_steps(3);
    assert!(h.state().templates.open.is_some());
    assert!(h.state().ui.status.contains("name"), "status: {}", h.state().ui.status);
}
