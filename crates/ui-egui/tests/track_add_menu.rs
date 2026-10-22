//! Right-click Add Track menus: the empty area under the tracks, and a track's own menu.

use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use soundcraft_ui_egui::{Services, SoundApp};

fn right_click(h: &mut Harness<SoundApp>, at: Pos2) {
    h.hover_at(at);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button: PointerButton::Secondary, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
}

fn harness() -> Harness<'static, SoundApp> {
    let mut engine = soundcraft_engine::Engine::default();
    engine.execute("track.new", &serde_json::json!({"name": "Voice"})).unwrap();
    engine.execute("track.new", &serde_json::json!({"name": "Bass"})).unwrap();
    let app = SoundApp::new(engine, None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1400.0, 900.0)).with_step_dt(1.0 / 60.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn names(h: &Harness<SoundApp>) -> Vec<String> {
    h.state().engine.session().tracks.iter().map(|t| t.name.clone()).collect()
}

#[test]
fn empty_area_menu_adds_an_audio_track() {
    let mut h = harness();
    let before = names(&h).len();
    let last = h.state().edit_layout.rows.last().unwrap().1;
    right_click(&mut h, pos2(last[0] + 60.0, last[3] + 120.0));
    assert!(h.query_by_label("Folder Track").is_some());
    h.get_by_label("Audio Track").click();
    h.run_steps(3);
    assert_eq!(names(&h).len(), before + 1);
}

#[test]
fn track_menu_adds_after_the_clicked_track() {
    let mut h = harness();
    let row = h.state().edit_layout.rows[0].1;
    right_click(&mut h, pos2(row[0] + 60.0, row[1] + 10.0));
    h.get_by_label("Add Track ⏵").click();
    h.run_steps(3);
    h.get_by_label("MIDI Track").click();
    h.run_steps(3);
    let n = names(&h);
    assert_eq!(n.len(), 3, "{n:?}");
    assert_eq!(n[0], "Voice");
    assert!(n[1].starts_with("MIDI"), "{n:?}");
    assert_eq!(n[2], "Bass");
}
