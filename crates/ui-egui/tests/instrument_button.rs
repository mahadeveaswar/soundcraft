//! The instrument button: a hosted instrument opens its own editor (the panel only when there is
//! none), a built-in instrument opens its panel.

use soundcraft_ui_egui::{Services, SoundApp};

fn app() -> (SoundApp, soundcraft_model::TrackId) {
    let engine = soundcraft_engine::demo::demo_engine();
    let keys = engine.session().tracks.iter().find(|t| t.name == "Keys").unwrap().id;
    (SoundApp::new(engine, None, Services::default()), keys)
}

#[test]
fn a_built_in_instrument_opens_its_panel() {
    let (mut app, keys) = app();
    app.open_instrument(keys);
    assert_eq!(app.ui.plugin_windows, vec![(keys, soundcraft_mix::INSTRUMENT_SLOT)]);
    app.open_instrument(keys);
    assert_eq!(app.ui.plugin_windows.len(), 1, "no duplicate window");
}

#[test]
fn a_hosted_instrument_without_an_editor_falls_back_to_the_panel() {
    let (mut app, keys) = app();
    let hosted = format!("{}test", soundcraft_clap_host::ID_PREFIX);
    if let Some(i) = app.engine.session_mut().track_mut(keys).and_then(|t| t.instrument.as_mut()) {
        i.plugin = hosted;
    }
    assert!(app.player.is_none() || !app.player.as_ref().unwrap().has_editor(keys, soundcraft_mix::INSTRUMENT_SLOT));
    app.open_instrument(keys);
    assert_eq!(app.ui.plugin_windows, vec![(keys, soundcraft_mix::INSTRUMENT_SLOT)]);
}

#[test]
fn the_panel_no_longer_carries_the_editor_hint() {
    let src = include_str!("../src/panels.rs");
    assert!(!src.contains("Load and edit sounds"));
}
