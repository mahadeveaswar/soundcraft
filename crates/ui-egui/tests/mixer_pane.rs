//! The bottom Mixer pane body (`mixer_pane::show`), hosted without docking: strips for visible
//! tracks plus Master, hidden tracks absent, click selects, compact narrows, preferences.
use egui::{Event, Modifiers, PointerButton, Pos2, vec2};
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::toolbar_layout::ToolbarStyle;
use soundcraft_ui_egui::{Services, SoundApp, UiState, menus, mixer_pane};

fn harness() -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut fonts_ready = false;
    let mut h = Harness::builder().with_size(vec2(1600.0, 800.0)).build_ui_state(
        move |ui, app: &mut SoundApp| {
            if !std::mem::replace(&mut fonts_ready, true) {
                // The strip's bold family exists from the next frame on.
                soundcraft_ui_egui::fonts::install(ui.ctx());
                return;
            }
            app.logic(&ui.ctx().clone());
            mixer_pane::show(app, ui);
        },
        app,
    );
    h.run_steps(4);
    h
}

fn names(app: &SoundApp, hidden: bool) -> Vec<String> {
    app.engine
        .session()
        .tracks
        .iter()
        .filter(|t| t.hidden == hidden && t.kind != soundcraft_model::TrackKind::Video)
        .map(|t| t.name.clone())
        .collect()
}

fn click_at(h: &mut Harness<SoundApp>, at: Pos2) {
    h.hover_at(at);
    for pressed in [true, false] {
        h.event(Event::PointerButton { pos: at, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE });
    }
    h.run_steps(3);
}

#[test]
fn lists_visible_tracks_and_master_and_skips_hidden_ones() {
    let mut h = harness();
    // Strips wider than the viewport are culled, so judge by a few leading tracks and Master.
    let first = names(h.state(), false);
    assert!(h.query_all_by_label(&format!("Strip {}", first[0])).count() == 1);
    let master = h.state().engine.session().tracks.iter().find(|t| t.kind == soundcraft_model::TrackKind::Master).map(|t| t.name.clone()).unwrap();
    let m = h.get_by_label(&format!("Strip {master}")).rect();
    assert!(m.min.x > 1400.0, "Master is pinned at the right: {m:?}");
    let gone = first[0].clone();
    h.state_mut().run("track.hide", json!({"tracks": [gone.clone()], "hidden": true})).unwrap();
    h.run_steps(3);
    assert!(names(h.state(), true).contains(&gone));
    assert_eq!(h.query_all_by_label(&format!("Strip {gone}")).count(), 0, "hidden track is absent");
}

#[test]
fn clicking_a_strip_selects_the_track() {
    let mut h = harness();
    let name = names(h.state(), false)[1].clone();
    assert_ne!(
        h.state().engine.session().edit.selected_tracks,
        h.state().engine.session().track_by_name(&name).map(|t| vec![t.id]).unwrap_or_default()
    );
    let r = h.get_by_label(&format!("Strip {name}")).rect();
    click_at(&mut h, Pos2::new(r.center().x, r.min.y + 8.0));
    let id = h.state().engine.session().track_by_name(&name).unwrap().id;
    assert_eq!(h.state().engine.session().edit.selected_tracks, vec![id]);
}

#[test]
fn compact_narrows_the_strips_and_persists() {
    let mut h = harness();
    let name = names(h.state(), false)[0].clone();
    let wide = h.get_by_label(&format!("Strip {name}")).rect().width();
    h.state_mut().ui.mixer_compact = true;
    h.run_steps(3);
    let narrow = h.get_by_label(&format!("Strip {name}")).rect().width();
    assert!(narrow < wide, "{narrow} < {wide}");
    let value = serde_json::to_value(&h.state().ui).unwrap();
    let back: UiState = serde_json::from_value(value).unwrap();
    assert!(back.mixer_compact && !back.show_mixer_panel);
}

#[test]
fn old_preferences_without_the_mixer_fields_still_load() {
    let mut value = serde_json::to_value(UiState::default()).unwrap();
    let object = value.as_object_mut().unwrap();
    object.remove("show_mixer_panel");
    object.remove("mixer_compact");
    let restored: UiState = serde_json::from_value(value).unwrap();
    assert!(!restored.show_mixer_panel && !restored.mixer_compact);
}

#[test]
fn the_window_command_toggles_with_a_check_mark_and_has_a_toolbar_key() {
    assert!(menus::UI_COMMANDS.iter().any(|c| c.0 == "window.mixer_panel"));
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.toolbar_style = ToolbarStyle::Standard;
    let mut h = Harness::builder().with_size(vec2(1800.0, 1000.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(4);
    assert!(!menus::item_state(h.state(), "", Some("window.mixer_panel")).1);
    h.get_all_by_label("Mixer").find(|n| n.rect().min.y < 90.0).expect("a Mixer key in the toolbar").click();
    h.run_steps(3);
    assert!(h.state().ui.show_mixer_panel);
    assert!(menus::item_state(h.state(), "", Some("window.mixer_panel")).1);
}
