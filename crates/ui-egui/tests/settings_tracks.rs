//! Settings > Track colours: swatches run the palette command and only new tracks change.

use egui::accesskit::Role;
use egui::{Event, Modifiers};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_ui_egui::settings_cmds::SettingsTab;
use soundcraft_ui_egui::{Services, SoundApp};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The palette is process-wide and the tests run on parallel threads.
static PALETTE: Mutex<()> = Mutex::new(());

fn own_palette() -> MutexGuard<'static, ()> {
    PALETTE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn harness() -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.extra.show_settings = true;
    app.extra.settings_tab = SettingsTab::Tracks;
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

fn track_colors(h: &Harness<'_, SoundApp>) -> Vec<[u8; 3]> {
    h.state().engine.session().tracks.iter().map(|t| t.color).collect()
}

#[test]
fn the_page_shows_sixteen_swatches_with_hex_fields() {
    let _g = own_palette();
    let h = harness();
    // One text field in the toolbar, then one hex field per swatch.
    assert_eq!(h.get_all_by_role(Role::TextInput).count(), 17);
    assert!(h.query_by_label_contains("Tracks that already exist keep theirs").is_some());
}

#[test]
fn a_typed_hex_runs_the_command_and_leaves_existing_tracks_alone() {
    let _g = own_palette();
    let mut h = harness();
    let before = track_colors(&h);
    // The next track takes palette entry `non-master track count % 16`; edit that swatch.
    let slot = h.state().engine.session().next_track_color();
    assert_ne!(slot, [0x11, 0x22, 0x33]);
    let n = h.state().engine.session().tracks.iter().filter(|t| t.kind != soundcraft_model::TrackKind::Master).count() % 16;
    h.get_all_by_role(Role::TextInput).nth(1 + n).expect("swatch hex").click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.event(Event::Text("#112233".into()));
    h.run_steps(1);
    h.get_all_by_role(Role::TextInput).nth(1 + (n + 1) % 16).expect("neighbouring swatch hex").click();
    h.run_steps(3);
    let got = h.state_mut().run("ui.theme_get", json!({})).unwrap();
    assert_eq!(got["track_palette"][n], "#112233");
    assert_eq!(track_colors(&h), before, "existing tracks keep their colours");
    // A track made now takes that palette colour.
    h.state_mut().run("track.new", json!({"count": 1, "format": "stereo", "name": "Fresh"})).unwrap();
    let fresh = h.state().engine.session().tracks.iter().find(|t| t.name == "Fresh").map(|t| t.color);
    assert_eq!(fresh, Some([0x11, 0x22, 0x33]));
}

#[test]
fn junk_is_rejected_and_reset_palette_restores_the_built_in_set() {
    let _g = own_palette();
    let mut h = harness();
    h.get_all_by_role(Role::TextInput).nth(1).expect("first swatch hex").click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.event(Event::Text("nope".into()));
    h.run_steps(1);
    h.get_all_by_role(Role::TextInput).nth(2).expect("second swatch hex").click();
    h.run_steps(3);
    assert_eq!(h.query_all_by_label_contains("is not a colour").count(), 1);
    assert_eq!(h.state_mut().run("ui.theme_get", json!({})).unwrap()["track_palette"], json!([]));

    h.state_mut().run("ui.theme_set_track_color", json!({"index": 3, "color": "#ABCDEF"})).unwrap();
    h.run_steps(3);
    h.get_by_label("Reset palette").click();
    h.run_steps(3);
    assert_eq!(h.state_mut().run("ui.theme_get", json!({})).unwrap()["track_palette"], json!([]));
    assert!(h.state().engine.track_palette().is_none());
}
