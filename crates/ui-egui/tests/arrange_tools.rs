//! The arrange edit tools: Split, Glue, Erase and Mute by click (Erase and Mute also by drag), one undo
//! step each, refused on locked clips; the Tools submenu of the right-click menu, the palette opened with
//! Ctrl (Cmd on macOS) plus right-click, the toolbar buttons with their key hints, and the tool saved
//! with the session.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use serde_json::json;
use soundcraft_engine::Engine;
use soundcraft_model::{Clip, ClipId, Source, SourceId, Tool, TrackId};
use soundcraft_ui_egui::{Services, SoundApp, edit_window};

const SEC: i64 = 48_000;

fn frame(ctx: &egui::Context, app: &mut SoundApp, mut events: Vec<Event>, modifiers: Modifiers) {
    events.insert(0, Event::ModifiersChanged(modifiers));
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))), events, ..Default::default() };
    ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    })
    .textures_delta
    .clear();
}

fn button(pos: Pos2, pressed: bool, which: PointerButton, modifiers: Modifiers) -> Event {
    Event::PointerButton { pos, button: which, pressed, modifiers }
}

fn session(mode: &str, tool: &str, lanes: &[&[(i64, i64)]]) -> (SoundApp, egui::Context, Vec<TrackId>, Vec<ClipId>) {
    let mut e = Engine::default();
    let made = e.execute("track.new", &json!({"count": lanes.len()})).unwrap()["tracks"].clone();
    let tracks: Vec<TrackId> = (0..lanes.len()).map(|i| TrackId(made[i].as_u64().unwrap())).collect();
    let s = e.session_mut();
    s.sources.push(Source {
        id: SourceId(7),
        name: "src".into(),
        path: "src.wav".into(),
        channels: 1,
        frames: 20 * SEC as u64,
        sample_rate: 48_000,
        format: soundcraft_audio_io::FileFormat::Wav,
        time_reference: 0,
        unsaved: false,
    });
    let mut ids = Vec::new();
    for (t, spans) in tracks.iter().zip(lanes) {
        for &(start, len) in *spans {
            let id = s.new_clip_id();
            s.track_mut(*t).and_then(|t| t.playlist_mut()).unwrap().clips.push(Clip::audio(id, "c", SourceId(7), start, start, len));
            ids.push(id);
        }
    }
    e.execute("edit.mode", &json!({"mode": mode})).unwrap();
    e.execute("edit.tool", &json!({"tool": tool})).unwrap();
    e.execute("view.zoom_set", &json!({"samples_per_px": 400.0})).unwrap();
    let mut app = SoundApp::new(e, None, Services::default());
    let ctx = egui::Context::default();
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    frame(&ctx, &mut app, vec![], Modifiers::NONE);
    (app, ctx, tracks, ids)
}

fn point(app: &SoundApp, t: TrackId, at: i64) -> Pos2 {
    let s = app.engine.session();
    let [x0, y0, x1, y1] = app.edit_layout.timeline;
    let row = app.edit_layout.rows.iter().find(|(id, _)| *id == t.0).unwrap().1;
    let x = edit_window::x_of(s, Rect::from_min_max(pos2(x0, y0), pos2(x1, y1)), at);
    pos2(x, row[1] + s.track(t).unwrap().height.points() * 0.75)
}

fn click_with(ctx: &egui::Context, app: &mut SoundApp, at: Pos2, which: PointerButton, m: Modifiers) {
    frame(ctx, app, vec![Event::PointerMoved(at)], m);
    frame(ctx, app, vec![button(at, true, which, m)], m);
    frame(ctx, app, vec![button(at, false, which, m)], m);
    frame(ctx, app, vec![], m);
}

fn click(ctx: &egui::Context, app: &mut SoundApp, at: Pos2) {
    click_with(ctx, app, at, PointerButton::Primary, Modifiers::NONE);
}

fn drag_between(ctx: &egui::Context, app: &mut SoundApp, from: Pos2, to: Pos2) {
    let n = Modifiers::NONE;
    frame(ctx, app, vec![Event::PointerMoved(from)], n);
    frame(ctx, app, vec![button(from, true, PointerButton::Primary, n)], n);
    let started = from + vec2(7.0_f32.copysign(to.x - from.x), 0.0);
    for k in 0..=12 {
        frame(ctx, app, vec![Event::PointerMoved(started + (to - started) * (k as f32 / 12.0))], n);
    }
    frame(ctx, app, vec![button(to, false, PointerButton::Primary, n)], n);
    frame(ctx, app, vec![], n);
}

fn clip_starts(app: &SoundApp, t: TrackId) -> Vec<i64> {
    app.engine.session().track(t).unwrap().clips().iter().map(|c| c.start).collect()
}

fn clip_count(app: &SoundApp) -> usize {
    app.engine.session().tracks.iter().map(|t| t.clips().len()).sum()
}

#[test]
fn split_click_cuts_the_clicked_clip_where_it_was_clicked_and_is_one_undo_step() {
    let (mut app, ctx, tracks, _) = session("slip", "split", &[&[(0, 4 * SEC)]]);
    let t = tracks[0];
    let at = point(&app, t, SEC * 3 / 2);
    click(&ctx, &mut app, at);
    let starts = clip_starts(&app, t);
    assert_eq!(starts.len(), 2, "{starts:?}");
    assert!((starts[1] - SEC * 3 / 2).abs() < 1600, "unsnapped cut lands at the pointer: {}", starts[1]);
    assert!(app.engine.session().edit.selected_clips.is_empty(), "the tool does not select");
    assert!(app.engine.undo());
    assert_eq!(clip_starts(&app, t), vec![0], "one undo restores the clip");
}

#[test]
fn split_click_snaps_to_the_grid_in_grid_mode_and_ctrl_click_does_not() {
    let (mut app, ctx, tracks, _) = session("grid", "split", &[&[(0, 8 * SEC)]]);
    let t = tracks[0];
    let s = app.engine.session();
    let step = s.edit.grid.step_samples(0, s.sample_rate, &s.tempo, s.frame_rate);
    let at = point(&app, t, step * 2 + step / 3);
    click(&ctx, &mut app, at);
    let starts = clip_starts(&app, t);
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[1] % step, 0, "snapped to the grid: {starts:?}");
    app.engine.undo();
    let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    click_with(&ctx, &mut app, at, PointerButton::Primary, ctrl);
    let starts = clip_starts(&app, t);
    assert_eq!(starts.len(), 2);
    assert_ne!(starts[1] % step, 0, "Ctrl swaps the snap off: {starts:?}");
}

#[test]
fn shift_split_also_cuts_the_selected_clips_under_that_time_on_other_tracks() {
    let (mut app, ctx, tracks, ids) = session("slip", "split", &[&[(0, 4 * SEC)], &[(0, 4 * SEC)], &[(0, 4 * SEC)]]);
    app.run("edit.select", json!({"clips": [ids[1].0]})).unwrap();
    let at = point(&app, tracks[0], 2 * SEC);
    click_with(&ctx, &mut app, at, PointerButton::Primary, Modifiers::SHIFT);
    assert_eq!(clip_starts(&app, tracks[0]).len(), 2);
    assert_eq!(clip_starts(&app, tracks[1]).len(), 2, "the selected clip is cut too");
    assert_eq!(clip_starts(&app, tracks[2]).len(), 1, "an unselected clip is not");
    assert!(app.engine.undo());
    assert_eq!(clip_count(&app), 3);
}

#[test]
fn glue_click_joins_the_clip_to_the_one_that_touches_it_and_says_when_there_is_none() {
    let (mut app, ctx, tracks, _) = session("slip", "glue", &[&[(0, SEC), (SEC, SEC), (5 * SEC, SEC)]]);
    let t = tracks[0];
    let at = point(&app, t, SEC / 2);
    click(&ctx, &mut app, at);
    assert_eq!(clip_starts(&app, t), vec![0, 5 * SEC], "the first two are one clip");
    assert!(app.engine.undo());
    assert_eq!(clip_starts(&app, t).len(), 3, "one undo step");
    let lone = point(&app, t, 5 * SEC + SEC / 2);
    click(&ctx, &mut app, lone);
    assert_eq!(clip_starts(&app, t).len(), 3, "nothing follows the last clip");
    assert!(app.ui.status.contains("Nothing to glue"), "{}", app.ui.status);
}

#[test]
fn erase_removes_a_clicked_clip_and_a_drag_sweeps_several_in_one_undo_step() {
    let (mut app, ctx, tracks, _) = session("slip", "erase", &[&[(0, SEC), (2 * SEC, SEC), (4 * SEC, SEC), (6 * SEC, SEC)]]);
    let t = tracks[0];
    let at = point(&app, t, SEC / 2);
    click(&ctx, &mut app, at);
    assert_eq!(clip_starts(&app, t), vec![2 * SEC, 4 * SEC, 6 * SEC]);
    assert!(app.engine.undo());
    assert_eq!(clip_count(&app), 4);
    let (a, b) = (point(&app, t, SEC / 2), point(&app, t, 4 * SEC + SEC / 2));
    drag_between(&ctx, &mut app, a, b);
    assert_eq!(clip_starts(&app, t), vec![6 * SEC], "the three swept clips are gone");
    assert!(app.engine.undo());
    assert_eq!(clip_count(&app), 4, "the whole sweep is one undo step");
}

#[test]
fn mute_click_toggles_and_a_drag_applies_the_first_clips_new_state_to_every_swept_clip() {
    let (mut app, ctx, tracks, _) = session("slip", "mute", &[&[(0, SEC), (2 * SEC, SEC), (4 * SEC, SEC)]]);
    let t = tracks[0];
    let muted = |app: &SoundApp| app.engine.session().track(t).unwrap().clips().iter().map(|c| c.muted).collect::<Vec<_>>();
    let at = point(&app, t, SEC / 2);
    click(&ctx, &mut app, at);
    assert_eq!(muted(&app), vec![true, false, false]);
    let at = point(&app, t, SEC / 2);
    click(&ctx, &mut app, at);
    assert_eq!(muted(&app), vec![false, false, false], "a second click unmutes");
    app.run("clip.mute", json!({"clips": [app.engine.session().track(t).unwrap().clips()[1].id.0]})).unwrap();
    // The first clip is unmuted, so the sweep mutes it and the others (the muted one stays muted).
    let (a, b) = (point(&app, t, SEC / 2), point(&app, t, 4 * SEC + SEC / 2));
    drag_between(&ctx, &mut app, a, b);
    assert_eq!(muted(&app), vec![true, true, true]);
    assert!(app.engine.undo());
    assert_eq!(muted(&app), vec![false, true, false], "one undo step for the sweep");
}

#[test]
fn every_new_tool_refuses_a_locked_clip_and_changes_nothing() {
    for tool in ["split", "glue", "erase", "mute"] {
        let (mut app, ctx, tracks, ids) = session("slip", tool, &[&[(0, SEC), (SEC, SEC)]]);
        app.engine.session_mut().find_clip_mut(ids[0]).unwrap().edit_locked = true;
        let before = app.engine.session().clone();
        let at = point(&app, tracks[0], SEC / 2);
        click(&ctx, &mut app, at);
        assert!(app.ui.status.contains("locked"), "{tool}: {}", app.ui.status);
        assert_eq!(app.engine.session().tracks, before.tracks, "{tool}: nothing changed");
        // A drag that starts on the locked clip does nothing either.
        let to = point(&app, tracks[0], SEC + SEC / 2);
        drag_between(&ctx, &mut app, at, to);
        assert_eq!(app.engine.session().tracks, before.tracks, "{tool}: drag changed nothing");
    }
}

#[test]
fn the_tool_is_saved_with_the_session_and_older_sessions_still_load() {
    for tool in Tool::ALL {
        let mut e = Engine::default();
        e.execute("edit.tool", &json!({"tool": tool.id()})).unwrap();
        let text = serde_json::to_string(&e.session().edit).unwrap();
        let back: soundcraft_model::EditState = serde_json::from_str(&text).unwrap();
        assert_eq!(back.tool, tool);
    }
    let old = serde_json::to_string(&Tool::Pencil).unwrap();
    assert_eq!(serde_json::from_str::<Tool>(&old).unwrap(), Tool::Pencil);
}

// ---- menus, palette, toolbar (the demo session) --------------------------------------------------------

type H<'a> = Harness<'a, SoundApp>;

fn steps(h: &mut H<'_>, n: usize) {
    for _ in 0..n {
        let mut raw = std::mem::take(h.input_mut());
        h.state_mut().raw_input_hook(&mut raw);
        *h.input_mut() = raw;
        h.step();
    }
}

fn demo(preset: &str, style: &str) -> H<'static> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("ui.prefs", json!({"keymap_preset": preset})).unwrap();
    app.run("ui.toolbar_style", json!({"value": style})).unwrap();
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_pixels_per_point(2.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    steps(&mut h, 6);
    h
}

fn press(h: &mut H<'_>, at: Pos2, which: PointerButton, m: Modifiers) {
    let s = &mut h.state_mut().synthetic;
    s.push(Event::ModifiersChanged(m));
    s.push(Event::PointerMoved(at));
    s.push(Event::PointerButton { pos: at, button: which, pressed: true, modifiers: m });
    s.push(Event::PointerButton { pos: at, button: which, pressed: false, modifiers: m });
    steps(h, 10);
}

fn kick_lane(h: &H<'_>, clip: bool) -> Pos2 {
    let s = h.state().engine.session();
    let t = s.tracks.iter().find(|t| t.name == "Kick").unwrap();
    let [x0, y0, x1, y1] = h.state().edit_layout.timeline;
    let row = h.state().edit_layout.rows.iter().find(|(id, _)| *id == t.id.0).unwrap().1;
    let tl = Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
    let y = row[1] + t.height.points() * 0.5;
    if clip {
        let c = &t.clips()[0];
        pos2(edit_window::x_of(s, tl, c.start + c.length / 2), y)
    } else {
        let end = t.clips().iter().map(Clip::end).max().unwrap_or(0);
        pos2(edit_window::x_of(s, tl, end + 2 * SEC).min(x1 - 20.0), y)
    }
}

/// The node whose whole label is `label` (menu lines carry their key text after the name).
fn labelled<'a>(h: &'a H<'_>, label: &'a str) -> Option<egui_kittest::Node<'a>> {
    h.query_all_by_label_contains(label).find(|n| n.accesskit_node().label().is_some_and(|l| l.trim() == label))
}

#[test]
fn the_right_click_menu_has_a_tools_submenu_with_a_check_on_the_active_tool() {
    for on_clip in [true, false] {
        let mut h = demo("alternative", "classic");
        h.state_mut().run("edit.tool", json!({"tool": "selector"})).unwrap();
        steps(&mut h, 3);
        let at = kick_lane(&h, on_clip);
        press(&mut h, at, PointerButton::Secondary, Modifiers::NONE);
        let tools = labelled(&h, "Tools ⏵").expect("the menu starts with a Tools submenu").rect();
        // Opening a submenu: hover it.
        h.state_mut().synthetic.push(Event::PointerMoved(tools.center()));
        steps(&mut h, 10);
        for name in ["Object Select 1", "Range Select 2", "Split 3", "Glue 4", "Erase 5", "Zoom 6", "Mute 7", "Draw 8", "Scrub 9", "Trim F6", "Smart"]
        {
            assert!(labelled(&h, name).is_some(), "the Tools submenu lists `{name}`");
        }
        let active = h.state().engine.session().edit.tool;
        let range = labelled(&h, "Range Select 2").unwrap();
        assert_eq!(active, Tool::Selector);
        assert_eq!(range.accesskit_node().toggled(), Some(egui::accesskit::Toggled::True), "the active tool is checked");
        assert_ne!(labelled(&h, "Erase 5").unwrap().accesskit_node().toggled(), Some(egui::accesskit::Toggled::True));
        // Choosing a tool changes the tool and nothing else.
        let before = h.state().engine.session().edit.selected_clips.clone();
        let split = labelled(&h, "Split 3").unwrap().rect().center();
        press(&mut h, split, PointerButton::Primary, Modifiers::NONE);
        assert_eq!(h.state().engine.session().edit.tool, Tool::Split);
        assert_eq!(h.state().engine.session().edit.selected_clips, before, "choosing a tool selects nothing");
    }
}

#[test]
fn ctrl_right_click_opens_the_tool_palette_instead_of_the_menu_and_leaves_the_selection_alone() {
    let mut h = demo("alternative", "classic");
    let before = h.state().engine.session().edit.selected_clips.clone();
    let ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
    let at = kick_lane(&h, true);
    press(&mut h, at, PointerButton::Secondary, ctrl);
    assert!(soundcraft_ui_egui::edit_tools::palette_open(&h.ctx), "the palette is open");
    assert!(labelled(&h, "Tools ⏵").is_none(), "and the normal menu is not");
    assert_eq!(h.state().engine.session().edit.selected_clips, before, "no selection change");
    let erase = labelled(&h, "Erase (5)").expect("palette buttons carry their key digit").rect().center();
    press(&mut h, erase, PointerButton::Primary, Modifiers::NONE);
    assert_eq!(h.state().engine.session().edit.tool, Tool::Erase);
    assert!(!soundcraft_ui_egui::edit_tools::palette_open(&h.ctx), "choosing a tool closes it");
    // Escape closes it too.
    press(&mut h, at, PointerButton::Secondary, ctrl);
    assert!(soundcraft_ui_egui::edit_tools::palette_open(&h.ctx));
    h.input_mut().events.push(Event::ModifiersChanged(Modifiers::NONE));
    h.input_mut().events.push(Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE });
    steps(&mut h, 3);
    assert!(!soundcraft_ui_egui::edit_tools::palette_open(&h.ctx), "Escape closes it");
}

#[test]
fn both_toolbars_have_the_four_new_tool_buttons_with_key_hints() {
    for style in ["classic", "standard"] {
        for (preset, hints) in [("alternative", ["Split (3)", "Glue (4)", "Erase (5)", "Mute (7)"]), ("default", ["Split", "Glue", "Erase", "Mute"])]
        {
            let mut h = demo(preset, style);
            for tip in hints {
                let node = h.query_all_by_label(tip).next();
                assert!(node.is_some(), "{style}/{preset}: a toolbar button `{tip}`");
            }
            let split = h.query_all_by_label(hints[0]).next().unwrap().rect().center();
            press(&mut h, split, PointerButton::Primary, Modifiers::NONE);
            assert_eq!(h.state().engine.session().edit.tool, Tool::Split, "{style}/{preset}");
        }
    }
}
