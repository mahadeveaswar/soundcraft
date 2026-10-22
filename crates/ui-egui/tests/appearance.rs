use egui::accesskit::Role;
use egui::{Event, Modifiers, PointerButton, Pos2, Vec2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::{Value, json};
use soundcraft_ui_egui::{Services, SoundApp};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The palette is process-wide and the tests run on parallel threads.
static PALETTE: Mutex<()> = Mutex::new(());

fn own_palette() -> MutexGuard<'static, ()> {
    PALETTE.lock().unwrap_or_else(PoisonError::into_inner)
}

const WINDOW_PX: Vec2 = vec2(1600.0, 1000.0);

fn frame(h: &mut Harness<'_, SoundApp>, px: Pos2, pressed: Option<bool>) {
    let zoom = h.ctx.zoom_factor();
    h.set_size(WINDOW_PX / zoom);
    let pos = (px.to_vec2() / zoom).to_pos2();
    h.input_mut().events.push(Event::PointerMoved(pos));
    if let Some(pressed) = pressed {
        h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.step();
}

#[test]
fn interface_scale_slider_follows_the_pointer() {
    let _g = own_palette();
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.extra.show_settings = true;
    let mut h = Harness::builder().with_size(WINDOW_PX).with_pixels_per_point(1.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    let rail = h.get_by_role(Role::Slider).rect();
    let start = pos2(rail.left() + rail.width() / 3.0, rail.center().y);
    let end = pos2(rail.right() + 40.0, rail.center().y);
    frame(&mut h, start, Some(true));
    let mut scales = vec![];
    for i in 1..=20 {
        frame(&mut h, start.lerp(end, i as f32 / 20.0), None);
        scales.push(h.state().extra.ui_scale);
    }
    assert!(scales.windows(2).all(|w| w[0] <= w[1]), "{scales:?}");
    frame(&mut h, end, Some(false));
    frame(&mut h, end, None);
    assert_eq!(h.state().extra.ui_scale, 1.5);
    assert_eq!(h.ctx.zoom_factor(), 1.5);
}

fn appearance_harness() -> Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
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

fn get(h: &mut Harness<'_, SoundApp>) -> Value {
    h.state_mut().run("ui.theme_get", json!({})).unwrap()
}

/// Types `text` into the first colour's hex field and leaves the field (which applies it).
fn type_into_first_hex(h: &mut Harness<'_, SoundApp>, text: &str) {
    // Text fields in the window: one in the toolbar, then the page's theme name, then one hex field
    // per colour in list order.
    h.get_all_by_role(Role::TextInput).nth(2).expect("first hex field").click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.event(Event::Text(text.into()));
    h.run_steps(1);
    h.get_all_by_role(Role::TextInput).nth(1).expect("name field").click();
    h.run_steps(3);
}

#[test]
fn hex_entry_applies_a_valid_colour_through_the_command() {
    let _g = own_palette();
    let mut h = appearance_harness();
    type_into_first_hex(&mut h, "#123456");
    let got = get(&mut h);
    assert_eq!(got["colors"]["window_bg"], "#123456");
    assert_eq!(got["dirty"], true);
}

#[test]
fn hex_entry_rejects_junk_and_says_so() {
    let _g = own_palette();
    let mut h = appearance_harness();
    let before = get(&mut h)["colors"]["window_bg"].clone();
    type_into_first_hex(&mut h, "zzzzzz");
    assert_eq!(get(&mut h)["colors"]["window_bg"], before);
    assert_eq!(h.query_all_by_label_contains("is not a colour").count(), 1);
}

#[test]
fn reset_all_goes_through_the_reset_command() {
    let _g = own_palette();
    let mut h = appearance_harness();
    h.state_mut().run("ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.state_mut().run("ui.theme_set_param", json!({"key": "grid_strength", "value": 1.5})).unwrap();
    h.run_steps(3);
    assert_eq!(get(&mut h)["overrides"], json!(["accent"]));
    h.get_by_label("Reset All").click();
    h.run_steps(3);
    let got = get(&mut h);
    assert_eq!(got["overrides"], json!([]));
    assert_eq!(got["params"]["grid_strength"], 1.0);
}

#[test]
fn per_colour_reset_clears_only_that_colour() {
    let _g = own_palette();
    let mut h = appearance_harness();
    h.state_mut().run("ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.state_mut().run("ui.theme_set_color", json!({"key": "playhead", "color": "#00FF00"})).unwrap();
    h.run_steps(3);
    // The page is taller since the shared-folder rows; bring the button into view first.
    h.get_by_label("Reset Accent").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("Reset Accent").click();
    h.run_steps(3);
    assert_eq!(get(&mut h)["overrides"], json!(["playhead"]));
}

#[test]
fn a_contrast_badge_appears_when_text_equals_the_panel_colour() {
    let _g = own_palette();
    let mut h = appearance_harness();
    assert!(h.query_by_label_contains("Low contrast").is_none());
    assert!(h.query_by_label_contains("Contrast checks passed").is_some());
    h.state_mut().run("ui.theme_set_color", json!({"key": "text", "color": "#202020"})).unwrap();
    h.state_mut().run("ui.theme_set_color", json!({"key": "panel", "color": "#202020"})).unwrap();
    h.run_steps(4);
    assert!(h.query_by_label_contains("Low contrast 1.0:1, needs 4.5:1: Text on Panel").is_some());
    assert!(h.query_by_label_contains("Contrast checks passed").is_none());
}

#[test]
fn theme_buttons_duplicate_and_the_picker_lists_the_copy() {
    let _g = own_palette();
    let mut h = appearance_harness();
    h.get_by_label("Duplicate").click();
    h.run_steps(3);
    let list = h.state_mut().run("ui.theme_list", json!({})).unwrap();
    let names: Vec<&str> = list["themes"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.iter().any(|n| n.ends_with(" copy")), "{names:?}");
}

#[test]
fn save_is_disabled_for_a_built_in_and_enabled_after_a_change() {
    let _g = own_palette();
    let mut h = appearance_harness();
    // A click on the disabled Save leaves the message empty; after a change it saves.
    h.get_by_label("Save").click();
    h.run_steps(2);
    assert!(h.state().ui.custom_theme.is_none());
    h.state_mut().run("ui.theme_set_color", json!({"key": "accent", "color": "#FF8800"})).unwrap();
    h.run_steps(3);
    h.get_by_label("Save").click();
    h.run_steps(3);
    assert_eq!(get(&mut h)["dirty"], false);
}
