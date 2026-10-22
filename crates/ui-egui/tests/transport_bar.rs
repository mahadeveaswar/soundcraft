//! The bottom transport bar: the position setting, its buttons and narrow windows.

use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;
use soundcraft_ui_egui::transport_bar::TransportPosition;
use soundcraft_ui_egui::{Services, SoundApp, UiState};

fn harness(width: f32) -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(width, 800.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    h
}

#[test]
fn position_command_round_trips_and_bottom_is_the_default() {
    let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
    assert_eq!(app.ui.transport_position, TransportPosition::Bottom);
    assert_eq!(app.run("ui.transport_position", json!({"position": "top"})), Ok(json!({"position": "top"})));
    assert_eq!(app.ui.transport_position, TransportPosition::Top);
    assert!(app.run("ui.transport_position", json!({"position": "left"})).is_err());
    let saved = serde_json::to_value(&app.ui).unwrap();
    assert_eq!(saved["transport_position"], "top");
    let back: UiState = serde_json::from_value(saved).unwrap();
    assert_eq!(back.transport_position, TransportPosition::Top);
    let old: UiState = serde_json::from_value(json!({"show_tracks_list": true})).unwrap();
    assert_eq!(old.transport_position, TransportPosition::Bottom, "old preferences default to the bottom bar");
    assert!(soundcraft_ui_egui::menus::UI_COMMANDS.iter().any(|c| c.0 == "ui.transport_position"));
}

fn click(h: &mut Harness<SoundApp>, x: f32, y: f32) {
    let pos = pos2(x, y);
    h.input_mut().events.push(Event::PointerMoved(pos));
    h.run_steps(2);
    for pressed in [true, false] {
        h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
    }
}

#[test]
fn bar_play_and_stop_buttons_drive_the_transport() {
    let mut h = harness(1600.0);
    // Bar keys: return to zero, loop, stop, play, record (30 pt keys, 3 pt apart, 8 pt in).
    let y = 800.0 - 22.0 - 16.0;
    let play = (8.0 + 3.0 * 33.0 + 15.0, y);
    click(&mut h, play.0, play.1);
    assert!(h.state().is_playing(), "play button starts playback");
    let stop = (8.0 + 2.0 * 33.0 + 15.0, y);
    click(&mut h, stop.0, stop.1);
    assert!(!h.state().is_playing(), "stop button stops it");
}

#[test]
fn bar_renders_at_every_width_in_both_windows() {
    for width in [420.0, 700.0, 1000.0, 1600.0] {
        let mut h = harness(width);
        h.run_steps(2);
        h.state_mut().ui.window = soundcraft_ui_egui::MainWindow::Mix;
        h.run_steps(3);
        h.state_mut().ui.transport_position = TransportPosition::Top;
        h.run_steps(3);
    }
}
