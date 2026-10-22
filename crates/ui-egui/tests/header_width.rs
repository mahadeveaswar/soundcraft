//! The resizable track header column.

use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use serde_json::json;
use soundcraft_ui_egui::edit_window::HEADER_W;
use soundcraft_ui_egui::track_header_compact::{HeaderFlags, layout};
use soundcraft_ui_egui::{Services, SoundApp, UiState};

fn harness() -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(
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
fn command_round_trips_and_old_preferences_keep_the_default() {
    let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default());
    assert_eq!(app.ui.header_w, HEADER_W);
    assert_eq!(app.run("ui.header_width", json!({})).unwrap()["value"], HEADER_W);
    assert_eq!(app.run("ui.header_width", json!({"value": 300})).unwrap()["value"], 300.0);
    assert_eq!(app.run("ui.header_width", json!({"value": 5})).unwrap()["value"], 112.0, "clamped to the compact minimum");
    assert_eq!(app.run("ui.header_width", json!({"value": 9000})).unwrap()["value"], 640.0);
    assert!(app.run("ui.header_width", json!({"value": "wide"})).is_err());
    app.run("ui.header_width", json!({"value": 250})).unwrap();
    let back: UiState = serde_json::from_value(serde_json::to_value(&app.ui).unwrap()).unwrap();
    assert_eq!(back.header_w, 250.0);
    assert_eq!(app.run("ui.header_width", json!({"reset": true})).unwrap()["value"], HEADER_W);
    let old: UiState = serde_json::from_value(json!({"show_tracks_list": true})).unwrap();
    assert_eq!(old.header_w, HEADER_W);
}

#[test]
fn timeline_origin_follows_the_header_width() {
    let mut h = harness();
    let origin = |h: &Harness<SoundApp>| h.state().edit_layout.timeline[0];
    let start = origin(&h);
    h.state_mut().run("ui.header_width", json!({"value": 300})).unwrap();
    h.run_steps(3);
    assert!((origin(&h) - (start + 300.0 - HEADER_W)).abs() < 0.5, "{start} -> {}", origin(&h));
    // The row rectangles still start at the column's left edge and the timeline at its right edge.
    let row = h.state().edit_layout.rows[0].1;
    assert!(row[0] < origin(&h));
    h.state_mut().run("ui.header_width", json!({"reset": true})).unwrap();
    h.run_steps(3);
    assert!((origin(&h) - start).abs() < 0.5);
}

#[test]
fn window_shrink_reclamps_the_column() {
    let mut h = harness();
    h.state_mut().run("ui.header_width", json!({"value": 600})).unwrap();
    h.set_size(vec2(700.0, 900.0));
    h.run_steps(4);
    let base = h.state().edit_layout.header_base;
    assert!((112.0..=0.45 * 700.0 + 0.5).contains(&base), "{base}");
    assert_eq!(h.state().ui.header_w, 600.0, "the saved width is kept for a wider window");
}

#[test]
fn dragging_the_handle_resizes_by_pointer_position_and_double_click_resets() {
    let mut h = harness();
    let tl = h.state().edit_layout.timeline;
    let y = tl[1] + 120.0;
    let x = tl[0] - 0.5;
    let press = |h: &mut Harness<SoundApp>, pos, pressed| {
        h.input_mut().events.push(Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
        h.run_steps(2);
    };
    h.input_mut().events.push(Event::PointerMoved(pos2(x, y)));
    h.run_steps(2);
    press(&mut h, pos2(x, y), true);
    for dx in [20.0, 60.0, 100.0] {
        h.input_mut().events.push(Event::PointerMoved(pos2(x + dx, y)));
        h.run_steps(2);
    }
    press(&mut h, pos2(x + 100.0, y), false);
    assert!((h.state().ui.header_w - (HEADER_W + 100.0)).abs() < 1.5, "{}", h.state().ui.header_w);
    h.state_mut().run("ui.header_width", json!({"reset": true})).unwrap();
}

#[test]
fn compact_header_never_overlaps_at_any_column_width() {
    for w in (112..=640).step_by(4) {
        for h in [18.0, 24.0, 40.0, 58.0, 80.0, 120.0] {
            for flags in [
                HeaderFlags::default(),
                HeaderFlags { folder: true, instrument: true, records: true, mixes: true, depth: 2 },
                HeaderFlags { records: true, mixes: true, ..HeaderFlags::default() },
            ] {
                let lay = layout(vec2(w as f32, h), &flags);
                let all = lay.controls();
                for (i, (na, a)) in all.iter().enumerate() {
                    assert!(a.min.x >= -0.01 && a.max.x <= w as f32 + 0.01 && a.min.y >= -0.01 && a.max.y <= h + 0.01, "{na} outside at {w}x{h}");
                    for (nb, b) in &all[i + 1..] {
                        let o = a.intersect(*b);
                        assert!(o.width() <= 0.01 || o.height() <= 0.01, "{na} overlaps {nb} at {w}x{h}");
                    }
                }
            }
        }
    }
}
