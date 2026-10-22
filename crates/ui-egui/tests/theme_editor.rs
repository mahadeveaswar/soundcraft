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
    assert!(dir.join("Sunset.json").is_file());
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
    assert!(dir.join("Dusk.json").is_file());
    run(&mut h, "ui.theme_rename", json!({"from": "Dusk", "to": "Twilight"})).unwrap();
    assert!(dir.join("Twilight.json").is_file() && !dir.join("Dusk.json").exists());
    assert!(run(&mut h, "ui.theme_rename", json!({"from": "Twilight", "to": "Sunset"})).is_err());
    assert!(run(&mut h, "ui.theme_rename", json!({"from": "Dark", "to": "X"})).is_err());
    assert!(run(&mut h, "ui.theme_delete", json!({"name": "Dark"})).is_err());
    run(&mut h, "ui.theme_delete", json!({"name": "Twilight"})).unwrap();
    assert!(!dir.join("Twilight.json").exists());
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
