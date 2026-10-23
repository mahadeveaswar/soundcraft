//! The Settings window: how it opens, the General page controls and the bottom bar.

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_ui_egui::menus;
use soundcraft_ui_egui::settings_cmds::SettingsTab;
use soundcraft_ui_egui::{Services, SoundApp};

fn app() -> SoundApp {
    SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default())
}

fn harness(app: SoundApp) -> Harness<'static, SoundApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 900.0)).with_pixels_per_point(1.0).build_ui_state(
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

#[test]
fn preferences_menu_path_opens_the_window_on_general() {
    let mut h = harness(app());
    assert!(!h.state().extra.show_settings);
    menus::invoke_menu(h.state_mut(), "setup.preferences", "Setup > Preferences...");
    h.run_steps(3);
    assert!(h.state().extra.show_settings);
    assert_eq!(h.state().extra.settings_tab, SettingsTab::General);
    assert!(h.query_by_label("Interface scale").is_some());
    // The engine command keeps returning the edit state for scripts.
    let r = h.state_mut().run("setup.preferences", json!({})).unwrap();
    assert!(r.get("edit").is_some());
}

#[test]
fn setup_appearance_opens_the_appearance_page() {
    let mut h = harness(app());
    let id = menus::command_for_path("Setup > Appearance", &menus::ui_aliases());
    assert_eq!(id.as_deref(), Some("window.ui_customization"));
    menus::invoke_menu(h.state_mut(), "window.ui_customization", "Setup > Appearance");
    h.run_steps(3);
    assert!(h.state().extra.show_settings);
    assert_eq!(h.state().extra.settings_tab, SettingsTab::Appearance);
    assert!(h.query_by_label("Interface scale").is_none(), "the General page is not shown");
}

#[test]
fn window_menu_entry_is_settings() {
    let id = menus::command_for_path("Window > Settings...", &menus::ui_aliases());
    assert_eq!(id.as_deref(), Some("window.settings"));
}

#[test]
fn choosing_a_menu_layout_runs_the_command() {
    let mut h = harness(app());
    h.state_mut().run("window.settings", json!({"tab": "general"})).unwrap();
    h.run_steps(3);
    assert_eq!(h.state().ui.menu_layout, soundcraft_ui_egui::menu_layout::MenuLayout::Standard);
    // The toolbar has its own menu layout combo first; then the page's: Theme, Menu layout, Toolbar,
    // Track header, Transport.
    h.get_all_by_role(Role::ComboBox).nth(2).expect("menu layout combo").click();
    h.run_steps(3);
    h.get_by_label("Classic").click();
    h.run_steps(3);
    assert_eq!(h.state().ui.menu_layout, soundcraft_ui_egui::menu_layout::MenuLayout::Classic);
}

#[test]
fn checkboxes_and_the_slider_write_the_preferences() {
    let mut h = harness(app());
    h.state_mut().run("window.settings", json!({})).unwrap();
    h.run_steps(3);
    let boxes: Vec<_> = h.get_all_by_role(Role::CheckBox).collect();
    assert_eq!(boxes.len(), 3, "reduce motion, tooltips, auto-update");
    boxes[0].click();
    h.run_steps(3);
    assert!(h.state().ui.reduce_motion && h.state().extra.reduce_motion);
    h.get_all_by_role(Role::CheckBox).nth(1).expect("tooltips").click();
    h.run_steps(3);
    assert!(!h.state().ui.tooltips);
    h.get_all_by_role(Role::CheckBox).nth(2).expect("auto-update").click();
    h.run_steps(3);
    assert!(h.state().extra.auto_update_config);
}

#[test]
fn link_buttons_open_their_windows() {
    let mut h = harness(app());
    h.state_mut().run("window.settings", json!({})).unwrap();
    h.run_steps(3);
    h.get_by_label("Playback Engine...").click();
    h.run_steps(3);
    assert!(h.state().ui.show_playback_engine);
    h.get_by_label("Keyboard Shortcuts...").click();
    h.run_steps(3);
    assert!(h.state().ui.show_shortcuts);
    h.get_by_label("I/O...").click();
    h.run_steps(3);
    assert!(h.state().ui.show_io_setup);
}

#[test]
fn sidebar_switches_pages_and_close_hides_the_window() {
    let mut h = harness(app());
    h.state_mut().run("window.settings", json!({})).unwrap();
    h.run_steps(3);
    h.get_by_label("Track colours").click();
    h.run_steps(3);
    assert_eq!(h.state().extra.settings_tab, SettingsTab::Tracks);
    h.get_by_label("Close").click();
    h.run_steps(3);
    assert!(!h.state().extra.show_settings);
}

#[test]
fn reset_page_restores_general_defaults_and_revert_waits_for_a_change() {
    let mut h = harness(app());
    h.state_mut().run("ui.prefs", json!({"ui_scale": 1.25, "reduce_motion": true, "tooltips": false})).unwrap();
    h.state_mut().run("ui.menu_layout", json!({"layout": "classic"})).unwrap();
    h.state_mut().run("window.settings", json!({})).unwrap();
    h.run_steps(3);
    assert_eq!(h.state_mut().run("ui.theme_get", json!({})).unwrap()["dirty"], false);
    h.get_by_label("Reset page").click();
    h.run_steps(3);
    let ui = &h.state().ui;
    assert_eq!((ui.ui_scale, ui.reduce_motion, ui.tooltips), (1.0, false, true));
    assert_eq!(ui.menu_layout, soundcraft_ui_egui::menu_layout::MenuLayout::Standard);
}

#[test]
fn revert_discards_an_unsaved_theme_change() {
    let mut h = harness(app());
    h.state_mut().run("ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.state_mut().run("window.settings", json!({})).unwrap();
    h.run_steps(3);
    assert_eq!(h.state_mut().run("ui.theme_get", json!({})).unwrap()["dirty"], true);
    h.get_by_label("Revert").click();
    h.run_steps(3);
    let got = h.state_mut().run("ui.theme_get", json!({})).unwrap();
    assert_eq!(got["dirty"], false);
    assert_eq!(got["overrides"], json!([]));
}

#[test]
fn the_window_keeps_its_size_and_the_bottom_bar_stays_inside() {
    let mut h = harness(app());
    h.state_mut().run("window.settings", json!({})).unwrap();
    h.run_steps(3);
    let first = h.get_by_label("Close").rect();
    h.run_steps(10);
    assert_eq!(h.get_by_label("Close").rect(), first, "the window must not grow frame by frame");
    assert!(first.max.y < 640.0, "{first:?}");
}

#[test]
fn the_sidebar_lists_the_keys_page_and_opens_it() {
    let mut h = harness(app());
    h.state_mut().run("window.settings", json!({})).unwrap();
    h.run_steps(3);
    for page in ["General", "Appearance", "Track colours", "Keys"] {
        assert!(h.query_all_by_label(page).next().is_some(), "the sidebar has `{page}`");
    }
    h.state_mut().run("window.settings", json!({"tab": "keys"})).unwrap();
    h.run_steps(3);
    assert_eq!(h.state().extra.settings_tab, SettingsTab::Keys);
    assert!(h.query_by_label("Key set").is_some());
}
