use egui::{Event, Id, PointerButton, Pos2, RawInput, Rect, vec2};
use serde_json::json;
use soundcraft_ui_egui::docking::Panel;
use soundcraft_ui_egui::{Services, SoundApp, UiState, menus, panel_menu};

fn frame(ctx: &egui::Context, app: &mut SoundApp, events: Vec<Event>) {
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))), events, ..Default::default() };
    ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    })
    .textures_delta
    .clear();
}

fn settle(ctx: &egui::Context, app: &mut SoundApp) {
    for _ in 0..4 {
        frame(ctx, app, vec![]);
    }
}

fn new_app() -> (egui::Context, SoundApp) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("ui.dock.reset", json!({})).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    (ctx, app)
}

fn hint(ctx: &egui::Context, panel: &str) -> Option<Rect> {
    ctx.data(|d| d.get_temp::<Rect>(Id::new(("soundcraft-panel-hint", panel.to_string()))))
}

fn click(ctx: &egui::Context, app: &mut SoundApp, p: Pos2) {
    let b = |pressed| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Default::default() };
    frame(ctx, app, vec![Event::PointerMoved(p)]);
    frame(ctx, app, vec![b(true)]);
    frame(ctx, app, vec![b(false)]);
    settle(ctx, app);
}

#[test]
fn every_panel_hides_and_shows_again_through_the_dock_commands() {
    let (ctx, mut app) = new_app();
    for panel in panel_menu::panels_for(true) {
        let id = panel.id();
        app.run("ui.dock", json!({"operation": "open", "panel": id})).unwrap();
        settle(&ctx, &mut app);
        assert!(panel_menu::is_visible(&app, panel), "{id} shown");
        assert!(app.ui.docking.layout.contains(&panel), "{id} in the layout");
        app.run("ui.dock", json!({"operation": "close", "panel": id})).unwrap();
        settle(&ctx, &mut app);
        assert!(!panel_menu::is_visible(&app, panel), "{id} hidden");
        assert!(!app.ui.docking.layout.contains(&panel), "{id} out of the layout");
        app.run("ui.dock", json!({"operation": "open", "panel": id})).unwrap();
        settle(&ctx, &mut app);
        assert!(panel_menu::is_visible(&app, panel), "{id} back");
        assert!(app.ui.docking.layout.contains(&panel));
    }
}

#[test]
fn the_hint_strips_show_only_while_their_panel_is_hidden() {
    let (ctx, mut app) = new_app();
    app.run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap();
    settle(&ctx, &mut app);
    assert!(hint(&ctx, "tracks").is_none() && hint(&ctx, "midi").is_none());
    app.run("ui.dock", json!({"operation": "close", "panel": "tracks"})).unwrap();
    app.run("ui.dock", json!({"operation": "close", "panel": "midi"})).unwrap();
    settle(&ctx, &mut app);
    let (t, m) = (hint(&ctx, "tracks").expect("tracks hint"), hint(&ctx, "midi").expect("midi hint"));
    assert!(t.width() < 30.0 && t.height() > 200.0, "left strip {t:?}");
    assert!(m.height() < 30.0 && m.width() > 200.0, "bottom strip {m:?}");
    // Clicking the tracks strip restores the pane and the strip goes away.
    click(&ctx, &mut app, t.center());
    assert!(panel_menu::is_visible(&app, Panel::Tracks), "tracks restored by the hint");
    assert!(hint(&ctx, "tracks").is_none());
    click(&ctx, &mut app, m.center());
    assert!(panel_menu::is_visible(&app, Panel::Midi), "MIDI editor restored by the hint");
    assert!(hint(&ctx, "midi").is_none());
}

#[test]
fn menu_check_marks_match_the_panel_state() {
    let (ctx, mut app) = new_app();
    let ids = [
        (Panel::Tracks, "window.track_list"),
        (Panel::Clips, "window.clip_list"),
        (Panel::Midi, "window.midi_editor"),
        (Panel::Memory, "window.memory_locations"),
        (Panel::History, "window.undo_history"),
        (Panel::Master, "window.master_panel"),
    ];
    for open in [true, false, true] {
        for (panel, _) in ids {
            app.run("ui.dock", json!({"operation": if open { "open" } else { "close" }, "panel": panel.id()})).unwrap();
        }
        settle(&ctx, &mut app);
        for (panel, id) in ids {
            let (_, checked) = menus::item_state(&app, "", Some(id));
            assert_eq!(checked, open, "{id}");
            assert_eq!(panel_menu::is_visible(&app, panel), open);
        }
    }
    let labels: Vec<_> = panel_menu::entries(&app).into_iter().map(|e| e.1).collect();
    assert_eq!(labels, ["Tracks and Groups", "Clips", "MIDI Editor", "Memory Locations", "Undo History", "Master"]);
    // The Window menu lists the new entries.
    let tree = menus::tree();
    let window = tree.iter().find(|n| n.label == "Window").unwrap();
    for leaf in ["Show All Panels", "Tracks and Groups", "Reset Panel Layout", "MIDI Editor", "Memory Locations", "Undo History", "Clip List"] {
        assert!(window.children.iter().any(|c| c.label == leaf), "Window > {leaf}");
    }
}

#[test]
fn show_all_panels_opens_every_hidden_panel() {
    let (ctx, mut app) = new_app();
    for panel in panel_menu::panels_for(true) {
        if panel_menu::is_visible(&app, panel) {
            app.run("ui.dock", json!({"operation": "close", "panel": panel.id()})).unwrap();
        }
    }
    settle(&ctx, &mut app);
    assert!(panel_menu::entries(&app).iter().all(|e| !e.2));
    app.run("window.show_all_panels", json!({})).unwrap();
    settle(&ctx, &mut app);
    assert!(panel_menu::entries(&app).iter().all(|e| e.2), "{:?}", panel_menu::entries(&app));
    let session = app.engine.session().clone();
    app.run("window.show_all_panels", json!({})).unwrap();
    assert!(*app.engine.session() == session, "the session is untouched");
}

#[test]
fn old_saved_preferences_still_load() {
    let mut v = serde_json::to_value(UiState::default()).unwrap();
    v.as_object_mut().unwrap().remove("audition");
    let loaded: UiState = serde_json::from_value(v).unwrap();
    assert!(loaded.audition, "a missing audition flag defaults to on");
    let bare: UiState = serde_json::from_str("{}").unwrap();
    assert!(bare.show_tracks_list && bare.audition);
}
