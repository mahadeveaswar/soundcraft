//! Settings > Appearance: theme scopes, the shared folder row and the "not saved" warnings.

use egui::accesskit::Role;
use egui::vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::{Value, json};
use soundcraft_ui_egui::{Services, SoundApp};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The palette is process-wide and the tests run on parallel threads.
static PALETTE: Mutex<()> = Mutex::new(());

fn own_palette() -> MutexGuard<'static, ()> {
    PALETTE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("soundcraft-theme-ui-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn page(services: Services) -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, services);
    app.extra.show_settings = true;
    app.extra.settings_tab = soundcraft_ui_egui::settings_cmds::SettingsTab::Appearance;
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).with_pixels_per_point(1.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn run(h: &mut Harness<'_, SoundApp>, id: &str, p: Value) -> Result<Value, String> {
    h.state_mut().run(id, p)
}

fn disabled(h: &Harness<'_, SoundApp>, label: &str) -> bool {
    h.get_by_role_and_label(Role::Button, label).accesskit_node().is_disabled()
}

#[test]
fn the_page_warns_when_preferences_are_off() {
    let _g = own_palette();
    let h = page(Services::default());
    assert_eq!(h.query_all_by_label_contains("Preferences are off for this run").count(), 1);
    let h = page(Services { prefs_enabled: true, ..Services::default() });
    assert_eq!(h.query_all_by_label_contains("Preferences are off for this run").count(), 0);
}

#[test]
fn the_shared_folder_row_shows_the_folder_or_off() {
    let _g = own_palette();
    let dir = temp_dir("row");
    let mut h = page(Services { prefs_enabled: true, shared_themes_dir: Some(dir.clone()), ..Services::default() });
    assert!(h.query_by_label_contains("Themes saved here can be committed to git").is_some());
    assert!(h.query_by_label_contains(dir.to_str().unwrap()).is_some(), "the folder path is shown");
    assert!(disabled(&h, "Use detected"), "nothing to go back to while following the detected folder");
    assert!(!disabled(&h, "Off"));
    run(&mut h, "ui.theme_folder", json!({"path": ""})).unwrap();
    h.run_steps(5);
    assert!(disabled(&h, "Off"));
    assert!(!disabled(&h, "Use detected"));
    h.get_by_role_and_label(Role::Button, "Use detected").click();
    h.run_steps(5);
    assert_eq!(run(&mut h, "ui.theme_folder", json!({})).unwrap()["source"], "detected");
    assert!(h.query_by_label_contains(dir.to_str().unwrap()).is_some());
    h.get_by_role_and_label(Role::Button, "Off").click();
    h.run_steps(5);
    assert_eq!(run(&mut h, "ui.theme_folder", json!({})).unwrap()["source"], "off");
}

#[test]
fn save_as_writes_to_the_shared_folder_and_move_is_offered_for_user_themes() {
    let _g = own_palette();
    let root = temp_dir("saveas");
    let (user, shared) = (root.join("user"), root.join("shared"));
    let mut h =
        page(Services { prefs_enabled: true, themes_dir: Some(user.clone()), shared_themes_dir: Some(shared.clone()), ..Services::default() });
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.run_steps(3);
    assert!(disabled(&h, "Move to shared folder"), "an unsaved theme cannot be moved");
    h.get_all_by_role(Role::TextInput).nth(1).expect("name field").click();
    h.run_steps(2);
    h.event(egui::Event::Text("Panel Test".into()));
    h.run_steps(2);
    h.get_by_label("Save As").click();
    h.run_steps(4);
    assert!(shared.join("panel-test.json").is_file(), "the default Save to is the shared folder");
    assert!(!user.exists());
    assert_eq!(run(&mut h, "ui.theme_get", json!({})).unwrap()["scope"], "shared");
    // Save to This computer, then Move to shared folder.
    h.state_mut().ui.theme_default_scope = Some("user".into());
    run(&mut h, "ui.theme_set_color", json!({"key": "text", "color": "#EEEEEE"})).unwrap();
    run(&mut h, "ui.theme_save", json!({"name": "Local One", "scope": "user"})).unwrap();
    h.run_steps(4);
    assert!(user.join("local-one.json").is_file());
    assert!(!disabled(&h, "Move to shared folder"));
    h.get_by_label("Move to shared folder").click();
    h.run_steps(4);
    assert!(shared.join("local-one.json").is_file() && !user.join("local-one.json").exists());
    assert!(disabled(&h, "Move to shared folder"), "it is shared now");
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_shipped_theme_cannot_be_saved_renamed_or_deleted_from_the_page() {
    let _g = own_palette();
    let mut h = page(Services { prefs_enabled: true, ..Services::default() });
    run(&mut h, "ui.theme_load", json!({"name": "Slate"})).unwrap();
    h.run_steps(4);
    run(&mut h, "ui.theme_set_color", json!({"key": "text", "color": "#EEEEEE"})).unwrap();
    h.run_steps(4);
    assert!(disabled(&h, "Save"), "edits to a shipped theme go to Save As");
    assert!(!disabled(&h, "Save As"));
    assert!(disabled(&h, "Rename"));
    assert!(disabled(&h, "Delete"));
    assert!(!disabled(&h, "Duplicate"));
    assert!(h.query_by_label_contains("Shipped").is_some(), "the badge says where the theme lives");
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
}

#[test]
fn saving_without_a_folder_shows_the_warning_and_keeps_the_name() {
    let _g = own_palette();
    let mut h = page(Services::default());
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.run_steps(3);
    h.get_all_by_role(Role::TextInput).nth(1).expect("name field").click();
    h.run_steps(2);
    h.event(egui::Event::Text("Kept In Memory".into()));
    h.run_steps(2);
    h.get_by_label("Save As").click();
    h.run_steps(4);
    assert!(h.query_by_label_contains("NOT saved to disk").is_some(), "the warning is shown");
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    let mem = list["themes"].as_array().unwrap().iter().find(|t| t["name"] == "Kept In Memory").expect("kept");
    assert_eq!(mem["scope"], "memory");
    assert!(h.query_by_label_contains("Not saved").is_some(), "the badge says it is not on disk");
    let name_box = h.get_all_by_role(Role::TextInput).nth(1).expect("name field");
    assert_eq!(name_box.value().as_deref(), Some("Kept In Memory"), "the Name box keeps its text");
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
}
