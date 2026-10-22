//! Folder tracks: the fold triangle hides and shows children; the menu moves tracks in and out.

use egui::{Event, Modifiers, PointerButton, Pos2, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp};

fn click(h: &mut Harness<SoundApp>, at: Pos2, button: PointerButton) {
    h.hover_at(at);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
}

fn harness() -> Harness<'static, SoundApp> {
    let mut engine = soundcraft_engine::Engine::default();
    engine.execute("track.new", &json!({"name": "Voice"})).unwrap();
    engine.execute("track.new", &json!({"name": "Bass"})).unwrap();
    engine.execute("track.move_to_new_folder", &json!({"tracks": ["Bass"], "name": "Low"})).unwrap();
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

fn folder_row(h: &Harness<SoundApp>) -> [f32; 4] {
    let s = h.state().engine.session();
    let id = s.track_by_name("Low").unwrap().id.0;
    h.state().edit_layout.rows.iter().find(|r| r.0 == id).unwrap().1
}

#[test]
fn fold_triangle_toggles_the_folder_and_hides_children() {
    let mut h = harness();
    let rows = h.state().edit_layout.rows.len();
    assert!(h.state().engine.session().track_by_name("Low").unwrap().folder_open);
    let r = folder_row(&h);
    click(&mut h, pos2(r[0] + 7.0, r[1] + 7.0), PointerButton::Primary);
    assert!(!h.state().engine.session().track_by_name("Low").unwrap().folder_open);
    assert_eq!(h.state().edit_layout.rows.len(), rows - 1, "the child row is hidden while closed");
    let r = folder_row(&h);
    click(&mut h, pos2(r[0] + 7.0, r[1] + 7.0), PointerButton::Primary);
    assert_eq!(h.state().edit_layout.rows.len(), rows);
}

#[test]
fn menu_removes_a_child_from_its_folder() {
    let mut h = harness();
    let s = h.state().engine.session();
    let bass = s.track_by_name("Bass").unwrap().id.0;
    let row = h.state().edit_layout.rows.iter().find(|r| r.0 == bass).unwrap().1;
    click(&mut h, pos2(row[0] + 60.0, row[1] + 10.0), PointerButton::Secondary);
    h.get_by_label("Remove from Folder").click();
    h.run_steps(3);
    assert!(h.state().engine.session().track_by_name("Bass").unwrap().folder.is_none());
}
