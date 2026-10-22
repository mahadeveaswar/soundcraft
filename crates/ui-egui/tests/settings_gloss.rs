//! Settings > Appearance: the glossy clips switch and the corner radius sliders.

use egui::{Event, Modifiers, PointerButton};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_ui_egui::settings_cmds::SettingsTab;
use soundcraft_ui_egui::theme::Tokens;
use soundcraft_ui_egui::{Services, SoundApp};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The palette is process-wide and the tests run on parallel threads.
static PALETTE: Mutex<()> = Mutex::new(());

fn own_palette() -> MutexGuard<'static, ()> {
    PALETTE.lock().unwrap_or_else(PoisonError::into_inner)
}

fn harness() -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
    app.extra.show_settings = true;
    app.extra.settings_tab = SettingsTab::Appearance;
    let mut h = Harness::builder().with_size(egui::vec2(1400.0, 1000.0)).with_pixels_per_point(1.0).build_ui_state(
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

/// Press and release the primary button on the "On" switch (the checkbox widget acts on drag starts).
fn toggle(h: &mut Harness<'_, SoundApp>) {
    h.get_by_label("On").scroll_to_me();
    h.run_steps(3);
    let at = h.get_by_label("On").rect().center();
    h.event(Event::PointerMoved(at));
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
        if pressed {
            h.event(Event::PointerMoved(at + egui::vec2(-14.0, 0.0)));
            h.run_steps(2);
        }
    }
}

fn get(h: &mut Harness<'_, SoundApp>) -> serde_json::Value {
    h.state_mut().run("ui.theme_get", json!({})).unwrap()["params"].clone()
}

#[test]
fn the_page_has_the_gloss_switch_and_both_radius_sliders() {
    let _g = own_palette();
    let h = harness();
    assert!(h.query_by_label("Glossy clips").is_some());
    assert!(h.query_by_label("Clip corner radius").is_some());
    assert!(h.query_by_label("Header corner radius").is_some());
    assert!(h.query_by_label("On").is_some());
}

#[test]
fn the_switch_turns_gloss_off_and_back_to_the_palette_default() {
    let _g = own_palette();
    let mut h = harness();
    assert_eq!(get(&mut h)["clip_gloss"], 1.0);
    toggle(&mut h);
    h.run_steps(2);
    assert_eq!(get(&mut h)["clip_gloss"], 0.0);
    assert_eq!(Tokens::current().clip_gloss, 0.0);
    toggle(&mut h);
    h.run_steps(2);
    assert_eq!(get(&mut h)["clip_gloss"], 1.0);
    assert_eq!(Tokens::current().clip_gloss, 1.0);
    h.state_mut().run("ui.theme", json!({"mode": "dark"})).unwrap();
}

#[test]
fn radius_params_validate_apply_round_trip_and_reset() {
    let _g = own_palette();
    let mut h = harness();
    assert_eq!((get(&mut h)["clip_corner"].clone(), get(&mut h)["header_corner"].clone()), (json!(4), json!(3)));
    let st = h.state_mut();
    assert!(st.run("ui.theme_set_param", json!({"key": "clip_corner", "value": 9})).is_err());
    assert!(st.run("ui.theme_set_param", json!({"key": "clip_gloss", "value": -0.1})).is_err());
    st.run("ui.theme_set_param", json!({"key": "clip_corner", "value": 6})).unwrap();
    st.run("ui.theme_set_param", json!({"key": "header_corner", "value": 0})).unwrap();
    h.run_steps(3);
    assert_eq!((Tokens::current().clip_corner, Tokens::current().header_corner), (6, 0));
    // The document round-trips through export and import.
    let doc = h.state().ui.custom_theme.clone().unwrap();
    let text = soundcraft_ui_egui::theme_doc::to_json(&doc).unwrap();
    let back = soundcraft_ui_egui::theme_doc::from_json(&text).unwrap();
    assert_eq!(back.params, doc.params);
    assert_eq!(back.params.clip_corner, Some(6.0));
    let st = h.state_mut();
    st.run("ui.theme_reset", json!({"key": "clip_corner"})).unwrap();
    st.run("ui.theme_reset", json!({"key": "header_corner"})).unwrap();
    h.run_steps(3);
    assert_eq!((Tokens::current().clip_corner, Tokens::current().header_corner), (4, 3));
    h.state_mut().run("ui.theme", json!({"mode": "dark"})).unwrap();
}
