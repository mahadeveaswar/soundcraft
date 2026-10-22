//! `ui.theme_*` commands: live colours, saved themes, import/export and persistence.

use egui::Color32;
use egui_kittest::Harness;
use serde_json::{Value, json};
use soundcraft_ui_egui::theme::{ThemeMode, Tokens};
use soundcraft_ui_egui::{Services, SoundApp, UiState};
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The palette is process-wide and the tests run on parallel threads.
static PALETTE: Mutex<()> = Mutex::new(());

fn own_palette() -> MutexGuard<'static, ()> {
    PALETTE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("soundcraft-theme-test-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn app_with(dir: Option<PathBuf>) -> SoundApp {
    SoundApp::new(soundcraft_engine::Engine::default(), None, Services { themes_dir: dir, ..Services::default() })
}

/// An app with a user folder, a shared folder and the prefs flag, as the desktop app sets them.
fn app_dirs(user: Option<PathBuf>, shared: Option<PathBuf>, prefs_enabled: bool) -> SoundApp {
    SoundApp::new(
        soundcraft_engine::Engine::default(),
        None,
        Services { themes_dir: user, shared_themes_dir: shared, prefs_enabled, ..Services::default() },
    )
}

fn names_and_scopes(list: &Value) -> Vec<(String, String)> {
    list["themes"].as_array().unwrap().iter().map(|t| (t["name"].as_str().unwrap().to_string(), t["scope"].as_str().unwrap().to_string())).collect()
}

fn has(list: &Value, name: &str, scope: &str) -> bool {
    names_and_scopes(list).iter().any(|(n, s)| n == name && s == scope)
}

fn harness(app: SoundApp) -> Harness<'static, SoundApp> {
    Harness::builder().with_size(egui::vec2(800.0, 500.0)).with_pixels_per_point(1.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    )
}

fn run(h: &mut Harness<'_, SoundApp>, id: &str, p: Value) -> Result<Value, String> {
    h.state_mut().run(id, p)
}

fn orange() -> Color32 {
    Color32::from_rgb(0xFF, 0x88, 0x00)
}

#[test]
fn set_color_is_reported_and_reaches_the_tokens_after_a_frame() {
    let _g = own_palette();
    let mut h = harness(app_with(None));
    h.run_steps(2);
    assert_eq!(Tokens::current().accent, Tokens::DARK.accent);
    let got = run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    assert_eq!(got["colors"]["accent"], "#FF8800");
    assert_eq!(got["dirty"], true);
    h.run_steps(2);
    assert_eq!(Tokens::current().accent, orange());
    assert_eq!(h.state().ui.theme, ThemeMode::Custom);
    let get = run(&mut h, "ui.theme_get", json!({})).unwrap();
    assert_eq!(get["colors"]["accent"], "#FF8800");
    assert_eq!(get["base"], "dark");
    assert!(get["name"].as_str().unwrap().starts_with("Custom (from Dark"));
    let err = run(&mut h, "ui.theme_set_color", json!({"key": "nope", "color": "#FF8800"})).unwrap_err();
    assert!(err.contains("accent") && err.contains("panel"), "{err}");
    assert!(run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "orange"})).is_err());
    // Back to a built-in: tokens return to Dark.
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    h.run_steps(2);
    assert_eq!(Tokens::current().accent, Tokens::DARK.accent);
    assert!(h.state().ui.custom_theme.is_none());
}

#[test]
fn params_and_reset() {
    let _g = own_palette();
    let mut h = harness(app_with(None));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_param", json!({"key": "clip_opacity", "value": 0.5})).unwrap();
    h.run_steps(2);
    assert_eq!(Tokens::current().clip_fill_alpha, 128);
    assert!(run(&mut h, "ui.theme_set_param", json!({"key": "clip_opacity", "value": 3})).is_err());
    assert!(run(&mut h, "ui.theme_set_param", json!({"key": "bogus", "value": 1})).is_err());
    run(&mut h, "ui.theme_set_color", json!({"key": "text", "color": "#112233"})).unwrap();
    h.run_steps(2);
    run(&mut h, "ui.theme_reset", json!({"key": "text"})).unwrap();
    h.run_steps(2);
    assert_eq!(Tokens::current().text, Tokens::DARK.text);
    run(&mut h, "ui.theme_reset", json!({})).unwrap();
    h.run_steps(2);
    assert_eq!(Tokens::current().clip_fill_alpha, Tokens::DARK.clip_fill_alpha);
    let get = run(&mut h, "ui.theme_get", json!({})).unwrap();
    assert_eq!(get["dirty"], false);
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
}

#[test]
fn save_list_load_rename_duplicate_delete_round_trip() {
    let _g = own_palette();
    let dir = temp_dir("roundtrip");
    let mut h = harness(app_with(Some(dir.clone())));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    assert!(run(&mut h, "ui.theme_save", json!({"name": "dark"})).is_err(), "built-in names are protected");
    assert!(run(&mut h, "ui.theme_save", json!({"name": "  "})).is_err());
    let saved = run(&mut h, "ui.theme_save", json!({"name": "Sunset"})).unwrap();
    assert_eq!(saved["name"], "Sunset");
    assert_eq!(saved["dirty"], false);
    assert!(dir.join("sunset.json").is_file());
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    let names: Vec<&str> = list["themes"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap()).collect();
    for n in ["Dark", "Light", "Midnight", "Colour-safe", "Sunset"] {
        assert!(names.contains(&n), "{names:?}");
    }
    run(&mut h, "ui.theme", json!({"mode": "light"})).unwrap();
    assert_ne!(Tokens::current().accent, orange());
    run(&mut h, "ui.theme_load", json!({"name": "sunset"})).unwrap();
    h.run_steps(2);
    assert_eq!(Tokens::current().accent, orange());
    assert_eq!(h.state().ui.theme_name.as_deref(), Some("Sunset"));
    assert!(run(&mut h, "ui.theme_duplicate", json!({"name": "Sunset", "to": "Dark"})).is_err());
    run(&mut h, "ui.theme_duplicate", json!({"name": "Sunset", "to": "Dusk"})).unwrap();
    assert!(dir.join("dusk.json").is_file());
    run(&mut h, "ui.theme_rename", json!({"from": "Dusk", "to": "Twilight"})).unwrap();
    assert!(dir.join("twilight.json").is_file() && !dir.join("dusk.json").exists());
    assert!(run(&mut h, "ui.theme_rename", json!({"from": "Twilight", "to": "Sunset"})).is_err());
    assert!(run(&mut h, "ui.theme_rename", json!({"from": "Dark", "to": "X"})).is_err());
    assert!(run(&mut h, "ui.theme_delete", json!({"name": "Dark"})).is_err());
    run(&mut h, "ui.theme_delete", json!({"name": "Twilight"})).unwrap();
    assert!(!dir.join("twilight.json").exists());
    assert!(run(&mut h, "ui.theme_load", json!({"name": "Twilight"})).is_err());
    run(&mut h, "ui.theme_load", json!({"name": "Colour-safe"})).unwrap();
    assert_eq!(h.state().ui.theme, ThemeMode::Custom);
    run(&mut h, "ui.theme_load", json!({"name": "Midnight"})).unwrap();
    assert_eq!(h.state().ui.theme, ThemeMode::Midnight);
    assert!(h.state().ui.custom_theme.is_none());
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_themes_dir_keeps_saved_themes_in_memory_and_writes_nothing() {
    let _g = own_palette();
    let mut h = harness(app_with(None));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    run(&mut h, "ui.theme_save", json!({"name": "Mem"})).unwrap();
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(list["themes_dir"].is_null());
    assert!(list["themes"].as_array().unwrap().iter().any(|t| t["name"] == "Mem"));
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
}

#[test]
fn export_and_import_with_unknown_and_missing_keys() {
    let _g = own_palette();
    let dir = temp_dir("exchange");
    let mut h = harness(app_with(Some(dir.join("themes"))));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    run(&mut h, "ui.theme_save", json!({"name": "Out"})).unwrap();
    let path = dir.join("out.json");
    run(&mut h, "ui.theme_export", json!({"path": path.to_string_lossy()})).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(doc["format"], "soundcraft-theme");
    assert_eq!(doc["colors"]["accent"], "#FF8800");
    // Importing the same file again clashes with the saved "Out": a suffix is added.
    let got = run(&mut h, "ui.theme_import", json!({"path": path.to_string_lossy()})).unwrap();
    assert_eq!(got["name"], "Out 2");
    h.run_steps(2);
    assert_eq!(Tokens::current().accent, orange());
    // Unknown keys are ignored and reported; missing keys use the base; a future version loads.
    let odd = dir.join("odd.json");
    std::fs::write(
        &odd,
        r##"{"format":"soundcraft-theme","version":7,"name":"Odd","base":"light","colors":{"text":"#102030","sparkle":"#FFFFFF"},"extra":1}"##,
    )
    .unwrap();
    let got = run(&mut h, "ui.theme_import", json!({"path": odd.to_string_lossy()})).unwrap();
    assert_eq!(got["name"], "Odd");
    assert_eq!(got["ignored"], json!(["sparkle"]));
    h.run_steps(2);
    assert_eq!(Tokens::current().text, Color32::from_rgb(0x10, 0x20, 0x30));
    assert_eq!(Tokens::current().panel_bg, Tokens::LIGHT.panel_bg);
    let bad = dir.join("bad.json");
    std::fs::write(&bad, "{not json").unwrap();
    assert!(run(&mut h, "ui.theme_import", json!({"path": bad.to_string_lossy()})).is_err());
    std::fs::write(&bad, r#"{"format":"something-else"}"#).unwrap();
    assert!(run(&mut h, "ui.theme_import", json!({"path": bad.to_string_lossy()})).is_err());
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn track_palette_is_pushed_to_the_engine_and_cleared() {
    let _g = own_palette();
    let mut h = harness(app_with(None));
    h.run_steps(1);
    assert!(h.state().engine.track_palette().is_none());
    run(&mut h, "ui.theme_set_track_color", json!({"index": 2, "color": "#123456"})).unwrap();
    let pal = h.state().engine.track_palette().map(<[[u8; 3]]>::to_vec).unwrap();
    assert_eq!(pal.len(), 16);
    assert_eq!(pal[2], [0x12, 0x34, 0x56]);
    assert!(run(&mut h, "ui.theme_set_track_color", json!({"index": 16, "color": "#123456"})).is_err());
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    assert!(h.state().engine.track_palette().is_none(), "built-in themes clear the palette");
}

#[test]
fn custom_theme_survives_a_restart_through_ui_state() {
    let _g = own_palette();
    let mut h = harness(app_with(None));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    run(&mut h, "ui.theme_set_track_color", json!({"index": 0, "color": "#ABCDEF"})).unwrap();
    let saved = serde_json::to_string(&h.state().ui).unwrap();
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    h.run_steps(2);
    assert_ne!(Tokens::current().accent, orange());
    // A fresh process: new app, prefs loaded, first frame restores tokens and the engine palette.
    let mut app = app_with(None);
    app.ui = serde_json::from_str::<UiState>(&saved).unwrap();
    let mut h2 = harness(app);
    h2.run_steps(2);
    assert_eq!(Tokens::current().accent, orange());
    assert_eq!(h2.state().engine.track_palette().map(|p| p[0]), Some([0xAB, 0xCD, 0xEF]));
    run(&mut h2, "ui.theme", json!({"mode": "dark"})).unwrap();
}

#[test]
fn prefs_saved_before_themes_load_with_defaults() {
    let old: UiState = serde_json::from_value(json!({"theme": "midnight", "show_tracks_list": true})).unwrap();
    assert_eq!(old.theme, ThemeMode::Midnight);
    assert!(old.theme_name.is_none() && old.custom_theme.is_none());
    // A custom mode whose theme is gone falls back to Dark on the first frame.
    let _g = own_palette();
    let mut app = app_with(None);
    app.ui.theme = ThemeMode::Custom;
    app.ui.theme_name = Some("Vanished".into());
    let mut h = harness(app);
    h.run_steps(2);
    assert_eq!(h.state().ui.theme, ThemeMode::Dark);
    assert_eq!(Tokens::current().accent, Tokens::DARK.accent);
}

#[test]
fn saving_twice_gives_identical_bytes_and_a_slug_file_name() {
    let _g = own_palette();
    let dir = temp_dir("bytes");
    let mut h = harness(app_dirs(Some(dir.clone()), None, true));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    run(&mut h, "ui.theme_set_param", json!({"key": "clip_opacity", "value": 0.3})).unwrap();
    let saved = run(&mut h, "ui.theme_save", json!({"name": "My Night/Theme"})).unwrap();
    assert_eq!(saved["saved_to"], "user");
    let path = PathBuf::from(saved["path"].as_str().unwrap());
    assert_eq!(path.file_name().unwrap(), "my-night-theme.json");
    let first = std::fs::read(&path).unwrap();
    run(&mut h, "ui.theme_save", json!({})).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), first, "saving again changes nothing");
    assert!(first.ends_with(b"}\n") && !first.contains(&b'\r'));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "no temp file is left behind");
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shared_and_user_scopes_list_load_collide_and_clean_up_legacy_files() {
    let _g = own_palette();
    let root = temp_dir("scopes");
    let (user, shared) = (root.join("user"), root.join("shared"));
    let mut h = harness(app_dirs(Some(user.clone()), Some(shared.clone()), true));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    // The default scope is the shared folder when there is one.
    let s = run(&mut h, "ui.theme_save", json!({"name": "Alpha"})).unwrap();
    assert_eq!(s["saved_to"], "shared");
    assert_eq!(s["scope"], "shared");
    assert!(shared.join("alpha.json").is_file() && !user.exists());
    // The same name on this computer is a second, separate theme.
    let s = run(&mut h, "ui.theme_save", json!({"name": "alpha", "scope": "user"})).unwrap();
    assert_eq!(s["saved_to"], "user");
    assert!(user.join("alpha.json").is_file());
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(has(&list, "Alpha", "shared") && has(&list, "alpha", "user"), "{:?}", names_and_scopes(&list));
    assert_eq!(list["shared_dir"], shared.to_string_lossy().as_ref());
    assert_eq!(list["user_dir"], user.to_string_lossy().as_ref());
    assert_eq!(list["prefs_enabled"], true);
    assert_eq!(list["skipped"], json!([]));
    // Loading by name prefers the shared copy; a scope picks the other.
    run(&mut h, "ui.theme", json!({"mode": "light"})).unwrap();
    run(&mut h, "ui.theme_load", json!({"name": "ALPHA"})).unwrap();
    assert_eq!(run(&mut h, "ui.theme_get", json!({})).unwrap()["scope"], "shared");
    run(&mut h, "ui.theme_load", json!({"name": "alpha", "scope": "user"})).unwrap();
    assert_eq!(run(&mut h, "ui.theme_get", json!({})).unwrap()["scope"], "user");
    // Different names with the same slug get numbered files; the same name overwrites its own.
    run(&mut h, "ui.theme_save", json!({"name": "My Theme", "scope": "shared"})).unwrap();
    let second = run(&mut h, "ui.theme_save", json!({"name": "my-theme", "scope": "shared"})).unwrap();
    assert!(second["path"].as_str().unwrap().ends_with("my-theme-2.json"), "{second}");
    let again = run(&mut h, "ui.theme_save", json!({"name": "my-theme", "scope": "shared"})).unwrap();
    assert!(again["path"].as_str().unwrap().ends_with("my-theme-2.json"), "{again}");
    assert!(shared.join("my-theme.json").is_file());
    // A file that belongs to another theme, or is not a theme at all, is never overwritten.
    std::fs::write(shared.join("other.json"), r#"{"format":"soundcraft-theme","name":"Other"}"#).unwrap();
    std::fs::write(shared.join("other-2.json"), "taken").unwrap();
    run(&mut h, "ui.theme_save", json!({"name": "Other!", "scope": "shared"})).unwrap();
    assert!(shared.join("other-3.json").is_file());
    assert_eq!(std::fs::read_to_string(shared.join("other-2.json")).unwrap(), "taken");
    // A legacy `<Name>.json` is read, and replaced by the slug file on the next save.
    std::fs::write(
        shared.join("Legacy Name.json"),
        r##"{"format":"soundcraft-theme","version":1,"name":"Legacy Name","colors":{"accent":"#102030"}}"##,
    )
    .unwrap();
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(has(&list, "Legacy Name", "shared"));
    run(&mut h, "ui.theme_load", json!({"name": "Legacy Name"})).unwrap();
    run(&mut h, "ui.theme_save", json!({})).unwrap();
    assert!(shared.join("legacy-name.json").is_file() && !shared.join("Legacy Name.json").exists());
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert_eq!(names_and_scopes(&list).iter().filter(|(n, _)| n == "Legacy Name").count(), 1, "never duplicated");
    // Rename and delete stay inside their scope.
    run(&mut h, "ui.theme_rename", json!({"from": "alpha", "to": "Beta", "scope": "user"})).unwrap();
    assert!(user.join("beta.json").is_file() && !user.join("alpha.json").exists() && shared.join("alpha.json").is_file());
    run(&mut h, "ui.theme_delete", json!({"name": "Alpha", "scope": "shared"})).unwrap();
    assert!(!shared.join("alpha.json").exists() && user.join("beta.json").is_file());
    assert!(run(&mut h, "ui.theme_delete", json!({"name": "Alpha"})).is_err());
    assert!(run(&mut h, "ui.theme_save", json!({"name": "x", "scope": "everywhere"})).is_err());
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn move_copies_to_the_other_folder_and_removes_the_original() {
    let _g = own_palette();
    let root = temp_dir("move");
    let (user, shared) = (root.join("user"), root.join("shared"));
    let mut h = harness(app_dirs(Some(user.clone()), Some(shared.clone()), true));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    run(&mut h, "ui.theme_save", json!({"name": "Mover", "scope": "user"})).unwrap();
    let before = std::fs::read(user.join("mover.json")).unwrap();
    let moved = run(&mut h, "ui.theme_move", json!({"name": "Mover", "from": "user", "to": "shared"})).unwrap();
    assert_eq!(moved["saved_to"], "shared");
    assert!(!user.join("mover.json").exists());
    assert_eq!(std::fs::read(shared.join("mover.json")).unwrap(), before);
    assert_eq!(run(&mut h, "ui.theme_get", json!({})).unwrap()["scope"], "shared");
    assert!(run(&mut h, "ui.theme_move", json!({"name": "Mover", "from": "user", "to": "shared"})).is_err(), "not in the user folder any more");
    assert!(run(&mut h, "ui.theme_move", json!({"name": "Mover", "from": "shared", "to": "shared"})).is_err());
    assert!(run(&mut h, "ui.theme_move", json!({"name": "Slate", "from": "shipped", "to": "shared"})).is_err(), "shipped themes are read-only");
    // A name already in the target is not overwritten.
    run(&mut h, "ui.theme_save", json!({"name": "Mover", "scope": "user"})).unwrap();
    assert!(run(&mut h, "ui.theme_move", json!({"name": "Mover", "from": "user", "to": "shared"})).is_err());
    assert!(user.join("mover.json").is_file());
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn shipped_themes_are_listed_read_only_and_can_be_copied() {
    let _g = own_palette();
    let root = temp_dir("shipped");
    let shared = root.join("shared");
    let mut h = harness(app_dirs(Some(root.join("user")), Some(shared.clone()), true));
    h.run_steps(1);
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(has(&list, "Slate", "shipped"), "{:?}", names_and_scopes(&list));
    let slate = list["themes"].as_array().unwrap().iter().find(|t| t["name"] == "Slate").unwrap();
    assert_eq!(slate["read_only"], true);
    assert_eq!(slate["builtin"], false);
    assert!(run(&mut h, "ui.theme_rename", json!({"from": "Slate", "to": "Other"})).unwrap_err().contains("shipped"));
    assert!(run(&mut h, "ui.theme_delete", json!({"name": "Slate"})).unwrap_err().contains("shipped"));
    assert!(run(&mut h, "ui.theme_duplicate", json!({"name": "Slate", "to": "Dark"})).is_err());
    run(&mut h, "ui.theme_load", json!({"name": "slate"})).unwrap();
    h.run_steps(2);
    assert_eq!(h.state().ui.theme, ThemeMode::Custom);
    assert_eq!(run(&mut h, "ui.theme_get", json!({})).unwrap()["scope"], "shipped");
    assert_eq!(Tokens::current().accent, Color32::from_rgb(0x4F, 0x9D, 0xDE));
    let copy = run(&mut h, "ui.theme_duplicate", json!({"name": "Slate", "to": "Slate Copy"})).unwrap();
    assert_eq!(copy["saved_to"], "shared");
    assert!(shared.join("slate-copy.json").is_file());
    // A shared theme of the same name hides the shipped one; the shipped file itself is untouched.
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    run(&mut h, "ui.theme_save", json!({"name": "Slate", "scope": "shared"})).unwrap();
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(has(&list, "Slate", "shared") && !has(&list, "Slate", "shipped"), "{:?}", names_and_scopes(&list));
    run(&mut h, "ui.theme_delete", json!({"name": "Slate", "scope": "shared"})).unwrap();
    assert!(has(&run(&mut h, "ui.theme_list", json!({})).unwrap(), "Slate", "shipped"), "the shipped theme is back");
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn saving_with_no_folder_keeps_the_theme_in_memory_and_says_so() {
    let _g = own_palette();
    let mut h = harness(app_dirs(None, None, false));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    let saved = run(&mut h, "ui.theme_save", json!({"name": "Mem"})).unwrap();
    assert_eq!(saved["saved_to"], "memory");
    assert!(saved["path"].is_null());
    assert!(saved["warning"].as_str().unwrap().contains("NOT saved to disk"), "{saved}");
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(has(&list, "Mem", "memory"));
    assert_eq!(list["prefs_enabled"], false);
    assert!(list["shared_dir"].is_null() && list["user_dir"].is_null());
    assert!(run(&mut h, "ui.theme_save", json!({"name": "Mem2", "scope": "shared"})).unwrap_err().contains("no shared themes folder"));
    let dup = run(&mut h, "ui.theme_duplicate", json!({"name": "Mem", "to": "Mem copy"})).unwrap();
    assert_eq!(dup["saved_to"], "memory");
    assert!(dup["warning"].is_string());
    run(&mut h, "ui.theme_rename", json!({"from": "Mem copy", "to": "Mem renamed"})).unwrap();
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(has(&list, "Mem renamed", "memory") && !has(&list, "Mem copy", "memory"));
    run(&mut h, "ui.theme_delete", json!({"name": "Mem renamed"})).unwrap();
    assert!(!has(&run(&mut h, "ui.theme_list", json!({})).unwrap(), "Mem renamed", "memory"));
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
}

#[test]
fn a_shared_folder_that_cannot_be_written_is_an_error_and_nothing_is_redirected() {
    let _g = own_palette();
    let root = temp_dir("unwritable");
    std::fs::create_dir_all(&root).unwrap();
    // A file where the folder should be: creating the folder fails on every platform.
    let blocker = root.join("shared");
    std::fs::write(&blocker, "not a folder").unwrap();
    let user = root.join("user");
    let mut h = harness(app_dirs(Some(user.clone()), Some(blocker.clone()), true));
    h.run_steps(1);
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    let err = run(&mut h, "ui.theme_save", json!({"name": "Nope"})).unwrap_err();
    assert!(err.contains(&blocker.to_string_lossy().to_string()), "the error names the folder: {err}");
    assert!(!user.exists(), "nothing was silently saved elsewhere");
    assert_eq!(run(&mut h, "ui.theme_get", json!({})).unwrap()["dirty"], true, "the working copy stays unsaved");
    // The folder command refuses a file too.
    assert!(run(&mut h, "ui.theme_folder", json!({"path": blocker.to_string_lossy()})).is_err());
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn unreadable_theme_files_are_skipped_and_reported_in_the_list() {
    let _g = own_palette();
    let root = temp_dir("skipped");
    let shared = root.join("shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("broken.json"), "{oops").unwrap();
    std::fs::write(shared.join("foreign.json"), r#"{"format":"other"}"#).unwrap();
    let big = format!("{{\"format\":\"soundcraft-theme\",\"name\":\"Big\",\"pad\":\"{}\"}}", "x".repeat(300 * 1024));
    std::fs::write(shared.join("big.json"), big).unwrap();
    std::fs::write(shared.join("fine.json"), r#"{"format":"soundcraft-theme","version":42,"name":"Fine"}"#).unwrap();
    let mut h = harness(app_dirs(None, Some(shared.clone()), true));
    h.run_steps(1);
    let list = run(&mut h, "ui.theme_list", json!({})).unwrap();
    assert!(has(&list, "Fine", "shared"), "a newer version still loads");
    let skipped = list["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 3, "{skipped:?}");
    assert!(skipped.iter().all(|s| s["scope"] == "shared" && s["reason"].is_string()));
    assert!(!has(&list, "Big", "shared"));
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn theme_folder_reports_sets_turns_off_and_goes_back_to_detected() {
    let _g = own_palette();
    let root = temp_dir("folder");
    let (detected, chosen) = (root.join("detected"), root.join("chosen"));
    std::fs::create_dir_all(&chosen).unwrap();
    let mut h = harness(app_dirs(Some(root.join("user")), Some(detected.clone()), true));
    h.run_steps(1);
    let got = run(&mut h, "ui.theme_folder", json!({})).unwrap();
    assert_eq!((got["source"].as_str(), got["shared_dir"].as_str()), (Some("detected"), detected.to_str()));
    let got = run(&mut h, "ui.theme_folder", json!({"path": chosen.to_string_lossy()})).unwrap();
    assert_eq!((got["source"].as_str(), got["shared_dir"].as_str(), got["detected"].as_str()), (Some("pref"), chosen.to_str(), detected.to_str()));
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    let saved = run(&mut h, "ui.theme_save", json!({"name": "In Chosen"})).unwrap();
    assert!(saved["path"].as_str().unwrap().starts_with(chosen.to_str().unwrap()));
    let got = run(&mut h, "ui.theme_folder", json!({"path": ""})).unwrap();
    assert_eq!((got["source"].as_str(), got["shared_dir"].is_null()), (Some("off"), true));
    // Off: saves go to this computer.
    let saved = run(&mut h, "ui.theme_save", json!({"name": "On User"})).unwrap();
    assert_eq!(saved["saved_to"], "user");
    let got = run(&mut h, "ui.theme_folder", json!({"detect": true})).unwrap();
    assert_eq!((got["source"].as_str(), got["shared_dir"].as_str()), (Some("detected"), detected.to_str()));
    assert!(h.state().ui.shared_themes_folder.is_none());
    assert!(run(&mut h, "ui.theme_folder", json!({"path": 5})).is_err());
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn import_goes_to_the_named_scope_and_the_default_scope_preference_is_used() {
    let _g = own_palette();
    let root = temp_dir("importscope");
    let (user, shared) = (root.join("user"), root.join("shared"));
    let mut h = harness(app_dirs(Some(user.clone()), Some(shared.clone()), true));
    h.run_steps(1);
    let file = root.join("incoming.json");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(&file, r##"{"format":"soundcraft-theme","version":1,"name":"Incoming","colors":{"accent":"#102030"}}"##).unwrap();
    let got = run(&mut h, "ui.theme_import", json!({"path": file.to_string_lossy(), "scope": "user"})).unwrap();
    assert_eq!(got["saved_to"], "user");
    assert!(user.join("incoming.json").is_file());
    let got = run(&mut h, "ui.theme_import", json!({"path": file.to_string_lossy(), "scope": "shared"})).unwrap();
    assert_eq!((got["name"].as_str(), got["saved_to"].as_str()), (Some("Incoming 2"), Some("shared")));
    // The preference decides when nothing is named and the active theme is not saved anywhere.
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.state_mut().ui.theme_default_scope = Some("user".into());
    assert_eq!(run(&mut h, "ui.theme_save", json!({"name": "Pref User"})).unwrap()["saved_to"], "user");
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    run(&mut h, "ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.state_mut().ui.theme_default_scope = Some("shared".into());
    assert_eq!(run(&mut h, "ui.theme_save", json!({"name": "Pref Shared"})).unwrap()["saved_to"], "shared");
    run(&mut h, "ui.theme", json!({"mode": "dark"})).unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
