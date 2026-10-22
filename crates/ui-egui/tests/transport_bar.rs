//! The bottom transport bar: the position setting, its buttons and narrow windows.

use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;
use soundcraft_ui_egui::transport_bar::TransportPosition;
use soundcraft_ui_egui::transport_layout::{self, Item};
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
    // The keys sit in the centre group of the bar; find them in the pure layout.
    let y = 800.0 - 22.0 - 16.0;
    let lay = transport_layout::layout(1600.0, &[]);
    let centre_of = |item: Item| lay.find(item).map(|p| ((p.x0 + p.x1) / 2.0, y)).unwrap();
    let play = centre_of(Item::Play);
    click(&mut h, play.0, play.1);
    assert!(h.state().is_playing(), "play button starts playback");
    let stop = centre_of(Item::Stop);
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

#[test]
fn visible_item_and_floating_commands_round_trip() {
    let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
    assert!(app.ui.transport_visible);
    assert_eq!(app.run("ui.transport_visible", json!({"value": false})), Ok(json!({"visible": false})));
    assert_eq!(app.run("ui.transport_visible", json!({})), Ok(json!({"visible": true})));
    let r = app.run("ui.transport_position", json!({"position": "floating", "rect": [100, 120, 700, 36]})).unwrap();
    assert_eq!(r["position"], "floating");
    assert_eq!(app.ui.transport_float_rect, [100.0, 120.0, 700.0, 36.0]);
    assert!(app.run("ui.transport_position", json!({"rect": [1, 2, 3]})).is_err());
    assert_eq!(app.run("ui.transport_item", json!({"item": "secondary", "visible": false})).unwrap()["hidden"], json!(["secondary"]));
    assert!(app.run("ui.transport_item", json!({"item": "play", "visible": false})).is_err());
    assert!(app.run("ui.transport_item", json!({"item": "nope"})).is_err());
    assert_eq!(app.run("ui.transport_item", json!({"reset": true})).unwrap()["hidden"], json!([]));
    // Saved and restored; old preferences default to a visible bottom bar with nothing hidden.
    let saved = serde_json::to_value(&app.ui).unwrap();
    let back: UiState = serde_json::from_value(saved).unwrap();
    assert_eq!(back.transport_position, TransportPosition::Floating);
    assert_eq!(back.transport_float_rect, [100.0, 120.0, 700.0, 36.0]);
    let old: UiState = serde_json::from_value(json!({"transport_position": "top"})).unwrap();
    assert!(old.transport_visible && old.transport_hidden_items.is_empty());
    assert_eq!(old.transport_float_rect, soundcraft_ui_egui::transport_bar::DEFAULT_FLOAT_RECT);
}

#[test]
fn floating_bar_draws_inside_the_screen_and_closing_hides_it() {
    let mut h = harness(1200.0);
    h.state_mut().run("ui.transport_position", json!({"position": "floating", "rect": [5000, 5000, 9000, 36]})).unwrap();
    h.run_steps(6);
    let r = h.state().ui.transport_float_rect;
    assert!(r[0] + r[2] <= 1200.5 && r[1] + r[3] <= 800.5, "{r:?}");
    // Play from the floating bar (find the key through the layout; the window sits at r).
    let lay = transport_layout::layout(r[2], &[]);
    let play = lay.find(Item::Play).unwrap();
    click(&mut h, r[0] + 1.0 + (play.x0 + play.x1) / 2.0, r[1] + 1.0 + 16.0);
    assert!(h.state().is_playing(), "floating play key works");
    h.state_mut().run("transport.stop", json!({})).unwrap();
    h.state_mut().run("ui.transport_visible", json!({"value": false})).unwrap();
    h.run_steps(4);
    assert!(!h.state().ui.transport_visible);
}

fn tempo_at_start(h: &Harness<SoundApp>) -> f64 {
    let s = h.state().engine.session();
    let tick = s.tempo.samples_to_ticks(h.state().position(), s.sample_rate);
    s.tempo.tempo_at_tick(tick)
}

#[test]
fn tempo_arrows_nudge_by_one_bpm() {
    let mut h = harness(1900.0);
    let before = tempo_at_start(&h);
    let y = 800.0 - 22.0 - 16.0;
    let lay = transport_layout::layout(1900.0, &[]);
    let tempo = lay.find(Item::Tempo).unwrap();
    // The bar is centred a few points above this nominal y; these offsets sit in each arrow.
    click(&mut h, tempo.x1 - 8.5, y - 9.0);
    assert!((tempo_at_start(&h) - (before + 1.0)).abs() < 1e-6, "{before} -> {}", tempo_at_start(&h));
    click(&mut h, tempo.x1 - 8.5, y + 3.0);
    assert!((tempo_at_start(&h) - before).abs() < 1e-6);
}

#[test]
fn locator_click_moves_the_playhead() {
    let mut h = harness(1900.0);
    h.state_mut().run("edit.select", json!({"start": 96000, "end": 192000})).ok();
    h.run_steps(3);
    let y = 800.0 - 22.0 - 16.0;
    let lay = transport_layout::layout(1900.0, &[]);
    let r = lay.find(Item::RLoc).unwrap();
    click(&mut h, (r.x0 + r.x1) / 2.0, y);
    assert_eq!(h.state().position(), 192000);
}
