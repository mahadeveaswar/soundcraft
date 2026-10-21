use egui::{Event, Id, PointerButton, Pos2, RawInput, Rect, pos2, vec2};
use serde_json::json;
use soundcraft_ui_egui::{Services, SoundApp, midi_editor};

fn frame(ctx: &egui::Context, app: &mut SoundApp, events: Vec<Event>) {
    let raw = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1600.0, 1000.0))), events, ..Default::default() };
    ctx.run_ui(raw, |ui| {
        app.logic(ui.ctx());
        app.ui(ui);
    })
    .textures_delta
    .clear();
}

fn panel(ctx: &egui::Context) -> Rect {
    ctx.data(|data| data.get_temp::<Rect>(Id::new("soundcraft-midi-body"))).unwrap()
}

fn button(pos: Pos2, pressed: bool) -> Event {
    Event::PointerButton { pos, button: PointerButton::Primary, pressed, modifiers: Default::default() }
}

fn check_resize(with_clip: bool) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    if with_clip {
        app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    }
    assert_eq!(midi_editor::target_clip(&app).is_some(), with_clip);
    let ctx = egui::Context::default();
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    let initial = panel(&ctx);
    assert!((initial.height() - 320.0).abs() < 1.0, "initial panel: {initial:?}");
    let notes = app.engine.session().clone();

    // Drag the shared boundary up and down, then allow several idle frames
    // to catch a panel that looks resized but snaps back after release.
    for height in [440.0, 280.0, 200.0] {
        let before = panel(&ctx);
        let start = pos2(before.center().x, before.top() - 26.0 - 2.0);
        let end = pos2(start.x, before.bottom() - height - 26.0 - 2.0);
        frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
        frame(&ctx, &mut app, vec![button(start, true)]);
        for step in 1..=6 {
            frame(&ctx, &mut app, vec![Event::PointerMoved(start.lerp(end, step as f32 / 6.0))]);
        }
        frame(&ctx, &mut app, vec![button(end, false)]);
        for _ in 0..4 {
            frame(&ctx, &mut app, vec![]);
        }
        let after = panel(&ctx);
        assert!((after.height() - height.max(260.0)).abs() < 1.0, "requested {height}, panel: {after:?}");
        assert!((after.bottom() - initial.bottom()).abs() < 1.0);
        assert!(app.edit_layout.timeline[3] <= after.top());
    }

    // Closing and reopening the dock keeps the chosen height.
    let resized = panel(&ctx);
    app.run("window.midi_editor", json!({})).unwrap();
    frame(&ctx, &mut app, vec![]);
    app.run("window.midi_editor", json!({})).unwrap();
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(panel(&ctx), resized);
    let saved = serde_json::to_string(&app.ui).unwrap();
    app.ui = serde_json::from_str(&saved).unwrap();
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(panel(&ctx), resized);
    assert_eq!(*app.engine.session(), notes);
}

#[test]
fn midi_panel_resizes_with_a_selected_clip() {
    check_resize(true);
}

#[test]
fn midi_panel_resizes_without_a_selected_clip() {
    check_resize(false);
}

#[test]
fn layout_reset_cancels_a_held_midi_divider_without_editing_the_session() {
    check_held_replacement("reset");
}

#[test]
fn arrange_cancels_a_held_midi_divider_without_editing_the_session() {
    check_held_replacement("arrange");
}

#[test]
fn legacy_visibility_sync_cancels_a_held_midi_divider_without_editing_the_session() {
    check_held_replacement("visibility");
}

fn check_held_replacement(operation: &str) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    app.run("edit.select", json!({"tracks":["Keys"]})).unwrap();
    app.ui.show_memory_locations = operation == "arrange";
    let session = app.engine.session().clone();
    let ctx = egui::Context::default();
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    let initial = panel(&ctx);
    let start = pos2(initial.center().x, initial.top() - 28.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(start)]);
    frame(&ctx, &mut app, vec![button(start, true)]);
    let first = start - vec2(0.0, 80.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(first)]);
    frame(&ctx, &mut app, vec![]);
    assert!((panel(&ctx).height() - 400.0).abs() < 1.0, "first drag must resize");
    let second = start - vec2(0.0, 120.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(second)]);
    frame(&ctx, &mut app, vec![]);
    assert!((panel(&ctx).height() - 440.0).abs() < 1.0, "a normal second held move must continue resizing");
    match operation {
        "reset" => {
            app.run("ui.dock.reset", json!({})).unwrap();
        }
        "arrange" => {
            app.run("window.arrange_cascade", json!({})).unwrap();
        }
        "visibility" => {
            app.run("window.midi_editor", json!({})).unwrap();
            frame(&ctx, &mut app, vec![]);
            app.run("window.midi_editor", json!({})).unwrap();
            frame(&ctx, &mut app, vec![]);
        }
        _ => panic!("unknown synthetic operation"),
    }
    let reset = serde_json::to_value(&app.ui.docking).unwrap();
    let after_reset = start - vec2(0.0, 160.0);
    frame(&ctx, &mut app, vec![Event::PointerMoved(after_reset)]);
    frame(&ctx, &mut app, vec![]);
    assert_eq!(serde_json::to_value(&app.ui.docking).unwrap(), reset, "old held divider must not resize the replacement layout");
    let expected = if operation == "reset" { 320.0 } else { 440.0 };
    assert!((panel(&ctx).height() - expected).abs() < 1.0);
    frame(&ctx, &mut app, vec![button(after_reset, false)]);
    for _ in 0..4 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(serde_json::to_value(&app.ui.docking).unwrap(), reset);
    assert_eq!(*app.engine.session(), session);
}

#[test]
fn a_selected_video_clip_is_not_a_midi_target() {
    use soundcraft_model::{ChannelFormat, Clip, ClipContent, SourceId, Track, TrackKind};
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    let midi = midi_editor::target_clip(&app).unwrap();
    let s = app.engine.session_mut();
    let track = s.add_track(TrackKind::Video, ChannelFormat::Mono, None);
    let video = s.new_clip_id();
    let source = SourceId(s.alloc());
    s.track_mut(track).and_then(Track::playlist_mut).unwrap().clips.push(Clip::video(video, "picture", source, 0, 0, 48_000));

    app.run("edit.select", json!({"clips": [video.0]})).unwrap();
    assert_eq!(midi_editor::target_clip(&app), None, "a video clip alone leaves the MIDI editors without a target");
    app.run("edit.select", json!({"clips": [video.0, midi.0]})).unwrap();
    assert_eq!(midi_editor::target_clip(&app), Some(midi), "the MIDI clip after a video clip is the target");
    let (_, clip) = app.engine.session().find_clip(midi).unwrap();
    assert!(matches!(clip.content, ClipContent::Midi { .. }));
}

#[test]
fn only_pencil_inserts_notes_in_the_midi_roll() {
    use soundcraft_model::{ClipContent, Tool};

    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
    let clip = created["clip"].as_u64().unwrap();
    app.run("edit.select", json!({"clips": [clip]})).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..6 {
        frame(&ctx, &mut app, vec![]);
    }
    let body = panel(&ctx);
    let point = pos2(body.left() + 200.0, body.top() + 60.0);
    for tool in Tool::ALL.into_iter().filter(|tool| *tool != Tool::Pencil).chain([Tool::Pencil]) {
        app.run("edit.tool", json!({"tool": tool.id()})).unwrap();
        // Idle frames first: egui counts a click soon after the last one as a double-click, which adds a note.
        for _ in 0..60 {
            frame(&ctx, &mut app, vec![]);
        }
        let before = app.engine.session().clone();
        frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
        frame(&ctx, &mut app, vec![button(point, true)]);
        frame(&ctx, &mut app, vec![button(point, false)]);
        let (_, edited) = app.engine.session().find_clip(soundcraft_model::ClipId(clip)).unwrap();
        let ClipContent::Midi { sequence } = &edited.content else { panic!("MIDI clip required") };
        if tool == Tool::Pencil {
            assert_eq!(sequence.notes.len(), 1);
            app.run("edit.undo", json!({})).unwrap();
        }
        assert!(*app.engine.session() == before, "{tool:?} click must not change the session");
    }
}

#[test]
fn double_clicking_the_empty_midi_roll_adds_one_note() {
    use soundcraft_model::ClipContent;

    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
    let clip = soundcraft_model::ClipId(created["clip"].as_u64().unwrap());
    app.run("edit.select", json!({"clips": [clip.0]})).unwrap();
    app.run("edit.tool", json!({"tool": "selector"})).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..66 {
        frame(&ctx, &mut app, vec![]);
    }
    let body = panel(&ctx);
    let point = pos2(body.left() + 200.0, body.top() + 60.0);
    let notes = |app: &SoundApp| match &app.engine.session().find_clip(clip).unwrap().1.content {
        ClipContent::Midi { sequence } => sequence.notes.len(),
        _ => panic!("MIDI clip required"),
    };
    // A single click places the play position and leaves the document alone.
    frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
    frame(&ctx, &mut app, vec![button(point, true)]);
    frame(&ctx, &mut app, vec![button(point, false)]);
    assert_eq!(notes(&app), 0, "one click adds nothing");
    // A second click right after it is a double-click.
    frame(&ctx, &mut app, vec![button(point, true)]);
    frame(&ctx, &mut app, vec![button(point, false)]);
    assert_eq!(notes(&app), 1, "a double-click adds a note");
}

fn midi_harness(with_clip: bool) -> egui_kittest::Harness<'static, SoundApp> {
    midi_harness_sized(with_clip, 1400.0)
}

fn midi_harness_sized(with_clip: bool, width: f32) -> egui_kittest::Harness<'static, SoundApp> {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    if with_clip {
        let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
        app.run("edit.select", json!({"clips": [created["clip"].as_u64().unwrap()]})).unwrap();
    }
    let mut h = egui_kittest::Harness::builder().with_size(vec2(width, 900.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(6);
    h
}

#[test]
fn zoom_buttons_change_the_view_and_leave_the_session_alone() {
    use egui_kittest::kittest::Queryable;
    let mut h = midi_harness(true);
    let before = h.state().engine.session().clone();
    assert_eq!(h.state().midi.ppt, None);
    h.get_by_label("H+").click();
    h.run_steps(3);
    let zoomed = h.state().midi.ppt.expect("H+ resolves the fit zoom into a value");
    h.get_by_label("H+").click();
    h.run_steps(3);
    assert!(h.state().midi.ppt.unwrap() > zoomed, "H+ zooms in further");
    h.get_by_label("H-").click();
    h.run_steps(3);
    assert!((h.state().midi.ppt.unwrap() - zoomed).abs() < 1e-4, "H- undoes one H+");
    let row = h.state().midi.row_h;
    h.get_by_label("V+").click();
    h.run_steps(3);
    assert_eq!(h.state().midi.row_h, row + 2.0);
    h.get_by_label("V-").click();
    h.run_steps(3);
    assert_eq!(h.state().midi.row_h, row);
    assert!(*h.state().engine.session() == before, "zooming must not change the session");
}

#[test]
fn float_dock_close_open_keep_the_midi_panel_flag_in_sync() {
    use egui_kittest::kittest::Queryable;
    use soundcraft_ui_egui::docking::Panel;
    let mut h = midi_harness(false);
    let floating = |h: &egui_kittest::Harness<'_, SoundApp>| h.state().ui.docking.layout.floating.iter().any(|g| g.panels.contains(&Panel::Midi));
    assert!(!floating(&h));
    h.get_by_label("Float MIDI editor").click();
    h.run_steps(5);
    assert!(floating(&h), "float detaches the panel");
    assert!(h.state().ui.show_midi_editor);
    h.get_by_label("Dock MIDI editor").click();
    h.run_steps(5);
    assert!(!floating(&h), "dock puts it back");
    assert!(h.state().ui.show_midi_editor);
    h.get_by_label("Minimize MIDI editor").click();
    h.run_steps(3);
    assert!(h.state().midi.minimized);
    h.get_by_label("Restore MIDI editor").click();
    h.run_steps(3);
    assert!(!h.state().midi.minimized);
    h.get_by_label("Close MIDI editor").click();
    h.run_steps(5);
    assert!(!h.state().ui.show_midi_editor, "close hides the panel");
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap();
    h.run_steps(5);
    assert!(h.state().ui.show_midi_editor, "open shows it again");
}

#[test]
fn minimized_state_resets_when_the_editor_is_closed_and_reopened() {
    use egui_kittest::kittest::Queryable;
    let mut h = midi_harness(false);
    h.get_by_label("Minimize MIDI editor").click();
    h.run_steps(3);
    assert!(h.state().midi.minimized);
    h.get_by_label("Close MIDI editor").click();
    h.run_steps(5);
    assert!(!h.state().midi.minimized, "closing resets minimized");
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "midi"})).unwrap();
    h.run_steps(5);
    assert!(h.state().ui.show_midi_editor);
    assert!(!h.state().midi.minimized, "reopened expanded");
    // Closing by command (menu, control channel) resets it too.
    h.get_by_label("Minimize MIDI editor").click();
    h.run_steps(3);
    h.state_mut().run("ui.dock", json!({"operation": "close", "panel": "midi"})).unwrap();
    h.run_steps(5);
    assert!(!h.state().midi.minimized);
}

fn header_items(h: &egui_kittest::Harness<'_, SoundApp>) -> Vec<(midi_editor::HeaderItem, Rect)> {
    h.ctx.data(|d| d.get_temp::<Vec<(midi_editor::HeaderItem, Rect)>>(Id::new("soundcraft-midi-header"))).unwrap_or_default()
}

#[test]
fn the_header_controls_never_overlap_at_any_panel_width() {
    use egui_kittest::kittest::Queryable;
    use midi_editor::HeaderItem;
    for width in [760.0, 1100.0, 1600.0] {
        let h = midi_harness_sized(true, width);
        let items = header_items(&h);
        assert!(items.len() >= 5, "header laid out at {width}: {items:?}");
        for (i, (a, ra)) in items.iter().enumerate() {
            assert!(ra.max.x <= width + 0.5, "{a:?} stays inside the panel at {width}");
            for (b, rb) in &items[i + 1..] {
                assert!(ra.max.x <= rb.min.x + 0.01 || rb.max.x <= ra.min.x + 0.01, "{a:?} overlaps {b:?} at {width}: {ra:?} {rb:?}");
            }
        }
        let has = |item| items.iter().any(|(i, _)| *i == item);
        assert!(has(HeaderItem::Title) && has(HeaderItem::Tools) && has(HeaderItem::Glyphs), "{width}");
        assert!(h.query_by_label("Close MIDI editor").is_some(), "panel glyphs stay visible at {width}");
        // Hidden groups are reachable through the overflow button, never both shown.
        assert_eq!(has(HeaderItem::Overflow), h.query_by_label("Quantize").is_none(), "Quantize is in the header or the menu at {width}");
        assert_eq!(has(HeaderItem::Overflow), h.query_by_label("More MIDI editor actions").is_some(), "{width}");
    }
    let wide = header_items(&midi_harness_sized(true, 1600.0));
    assert!(wide.iter().any(|(i, _)| *i == HeaderItem::Label), "the clip label shows when there is room");
    assert!(!wide.iter().any(|(i, _)| *i == HeaderItem::Overflow));
    let narrow = header_items(&midi_harness_sized(true, 760.0));
    assert!(!narrow.iter().any(|(i, _)| *i == HeaderItem::Label), "the label is hidden first");
}

fn lane_handle_rect(h: &egui_kittest::Harness<'_, SoundApp>) -> Rect {
    h.ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-lane-handle"))).unwrap()
}

fn drag_handle_by(h: &mut egui_kittest::Harness<'_, SoundApp>, dy: f32) {
    let start = lane_handle_rect(h).center();
    h.hover_at(start);
    h.run_steps(2);
    h.drag_at(start);
    h.run_steps(2);
    h.hover_at(start + vec2(0.0, dy));
    h.run_steps(3);
    h.drop_at(start + vec2(0.0, dy));
    h.run_steps(3);
}

#[test]
fn dragging_the_lane_handle_resizes_the_lane_and_leaves_the_session_alone() {
    let mut h = midi_harness(true);
    let before = h.state().engine.session().clone();
    assert_eq!(h.state().midi.lane_h, midi_editor::LANE_H_DEFAULT);
    drag_handle_by(&mut h, -60.0);
    let taller = h.state().midi.lane_h;
    assert!(taller > midi_editor::LANE_H_DEFAULT + 40.0, "dragging up grows the lane: {taller}");
    drag_handle_by(&mut h, 500.0);
    assert_eq!(h.state().midi.lane_h, 40.0, "dragging far down clamps to the minimum");
    assert!(*h.state().engine.session() == before, "resizing the lane must not change the session");
}

#[test]
fn double_clicking_the_lane_handle_resets_the_lane_height() {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
    app.run("edit.select", json!({"clips": [created["clip"].as_u64().unwrap()]})).unwrap();
    app.midi.lane_h = 150.0;
    let ctx = egui::Context::default();
    for _ in 0..66 {
        frame(&ctx, &mut app, vec![]);
    }
    let p = ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-lane-handle"))).unwrap().center();
    assert_eq!(app.midi.lane_h, 150.0, "a taller lane stays put while idle");
    frame(&ctx, &mut app, vec![Event::PointerMoved(p)]);
    frame(&ctx, &mut app, vec![button(p, true)]);
    frame(&ctx, &mut app, vec![button(p, false)]);
    assert_eq!(app.midi.lane_h, 150.0, "one click does not reset");
    frame(&ctx, &mut app, vec![button(p, true)]);
    frame(&ctx, &mut app, vec![button(p, false)]);
    assert_eq!(app.midi.lane_h, midi_editor::LANE_H_DEFAULT);
}

fn lane_app(lane_h: f32) -> (egui::Context, SoundApp) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
    app.run("edit.select", json!({"clips": [created["clip"].as_u64().unwrap()]})).unwrap();
    app.midi.lane_h = lane_h;
    let ctx = egui::Context::default();
    for _ in 0..66 {
        frame(&ctx, &mut app, vec![]);
    }
    (ctx, app)
}

/// Press at the handle centre, move by `dy` in several steps, release.
fn real_drag(ctx: &egui::Context, app: &mut SoundApp, dy: f32) {
    let p = ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-lane-handle"))).unwrap().center();
    frame(ctx, app, vec![Event::PointerMoved(p)]);
    frame(ctx, app, vec![button(p, true)]);
    let steps = 10;
    for i in 1..=steps {
        frame(ctx, app, vec![Event::PointerMoved(p + vec2(0.0, dy * i as f32 / steps as f32))]);
    }
    frame(ctx, app, vec![button(p + vec2(0.0, dy), false)]);
    frame(ctx, app, vec![]);
}

#[test]
fn a_real_pointer_drag_up_80_points_grows_the_lane_by_about_80() {
    let (ctx, mut app) = lane_app(midi_editor::LANE_H_DEFAULT);
    let before = app.engine.session().clone();
    real_drag(&ctx, &mut app, -80.0);
    let grown = app.midi.lane_h - midi_editor::LANE_H_DEFAULT;
    assert!((grown - 80.0).abs() <= 4.0, "lane grew by {grown}");
    assert!(*app.engine.session() == before, "resizing must not change the session");
    real_drag(&ctx, &mut app, 400.0);
    assert_eq!(app.midi.lane_h, 40.0, "dragging down past the minimum clamps");
}

#[test]
fn a_double_click_on_the_handle_resets_without_moving_the_lane() {
    let (ctx, mut app) = lane_app(180.0);
    let p = ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-lane-handle"))).unwrap().center();
    frame(&ctx, &mut app, vec![Event::PointerMoved(p)]);
    for _ in 0..2 {
        frame(&ctx, &mut app, vec![button(p, true)]);
        frame(&ctx, &mut app, vec![button(p, false)]);
    }
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
    }
    assert_eq!(app.midi.lane_h, midi_editor::LANE_H_DEFAULT);
}

/// Audition requests the UI queued since the last call (the next frame's logic would drop them).
fn drain_auditions(app: &mut SoundApp) -> Vec<(u8, bool)> {
    std::mem::take(&mut app.engine.transport_requests)
        .into_iter()
        .filter_map(|r| match r {
            soundcraft_engine::TransportRequest::Audition { pitch, on, .. } => Some((pitch, on)),
            _ => None,
        })
        .collect()
}

/// A MIDI clip with one note at the start, the roll centred on it, and the point on that note.
fn note_app() -> (egui::Context, SoundApp, Pos2) {
    let mut app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    app.ui.show_midi_editor = true;
    let created = app.run("midi.new_clip", json!({"track": "Keys", "start": 0, "length": 96000})).unwrap();
    let clip = created["clip"].as_u64().unwrap();
    app.run("midi.note_add", json!({"clip": clip, "pitch": 64, "start_ticks": 0, "length_ticks": 480, "velocity": 90})).unwrap();
    app.run("edit.select", json!({"clips": [clip]})).unwrap();
    app.run("edit.tool", json!({"tool": "selector"})).unwrap();
    let ctx = egui::Context::default();
    for _ in 0..66 {
        frame(&ctx, &mut app, vec![]);
    }
    let roll = ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-roll"))).unwrap();
    let rows = (roll.height() / app.midi.row_h).floor() as i32;
    let point = pos2(roll.min.x + 10.0, roll.min.y + (rows / 2) as f32 * app.midi.row_h + app.midi.row_h / 2.0);
    drain_auditions(&mut app);
    (ctx, app, point)
}

#[test]
fn clicking_a_note_sends_exactly_one_audition_on_and_later_one_off() {
    let (ctx, mut app, point) = note_app();
    let mut log = Vec::new();
    frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
    frame(&ctx, &mut app, vec![button(point, true)]);
    log.extend(drain_auditions(&mut app));
    frame(&ctx, &mut app, vec![button(point, false)]);
    log.extend(drain_auditions(&mut app));
    assert_eq!(log, [(64, true)], "the note sounds on the click");
    for _ in 0..30 {
        frame(&ctx, &mut app, vec![]);
        log.extend(drain_auditions(&mut app));
    }
    assert_eq!(log, [(64, true), (64, false)], "released after the click time, once");
    assert_eq!(app.midi.audition.sounding, None);
}

#[test]
fn nothing_is_audited_when_the_toggle_is_off() {
    let (ctx, mut app, point) = note_app();
    app.ui.audition = false;
    frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
    frame(&ctx, &mut app, vec![button(point, true)]);
    frame(&ctx, &mut app, vec![button(point, false)]);
    for _ in 0..30 {
        frame(&ctx, &mut app, vec![]);
    }
    assert!(drain_auditions(&mut app).is_empty());
}

#[test]
fn dragging_a_note_up_moves_the_sounding_pitch_and_releases_on_drop() {
    let (ctx, mut app, point) = note_app();
    let mut log = Vec::new();
    let row = app.midi.row_h;
    frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
    frame(&ctx, &mut app, vec![button(point, true)]);
    for i in 1..=8 {
        frame(&ctx, &mut app, vec![Event::PointerMoved(point - vec2(0.0, row * 2.0 * i as f32 / 8.0))]);
        log.extend(drain_auditions(&mut app));
    }
    assert!(!app.midi.audition.sounding.is_none(), "still sounding mid-drag");
    frame(&ctx, &mut app, vec![button(point - vec2(0.0, row * 2.0), false)]);
    log.extend(drain_auditions(&mut app));
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
        log.extend(drain_auditions(&mut app));
    }
    let on = |p| (p, true);
    let off = |p| (p, false);
    assert_eq!(log, [on(64), off(64), on(65), off(65), on(66), off(66)], "{log:?}");
    assert_eq!(app.midi.audition.sounding, None);
}

#[test]
fn closing_the_editor_releases_a_held_note() {
    let (ctx, mut app, point) = note_app();
    let mut log = Vec::new();
    frame(&ctx, &mut app, vec![Event::PointerMoved(point)]);
    frame(&ctx, &mut app, vec![button(point, true)]);
    frame(&ctx, &mut app, vec![Event::PointerMoved(point + vec2(0.0, -30.0))]);
    log.extend(drain_auditions(&mut app));
    app.run("ui.dock", json!({"operation": "close", "panel": "midi"})).unwrap();
    for _ in 0..3 {
        frame(&ctx, &mut app, vec![]);
        log.extend(drain_auditions(&mut app));
    }
    let ons = log.iter().filter(|e| e.1).count();
    let offs = log.iter().filter(|e| !e.1).count();
    assert!(ons >= 1 && ons == offs, "{log:?}");
    assert_eq!(app.midi.audition.sounding, None);
}

#[test]
fn pencil_clicks_and_gutter_presses_sound_and_release() {
    let (ctx, mut app, point) = note_app();
    // Gutter: press a key left of the roll, then release.
    let roll = ctx.data(|d| d.get_temp::<Rect>(Id::new("soundcraft-midi-roll"))).unwrap();
    let key = pos2(roll.min.x - 20.0, point.y);
    let mut log = Vec::new();
    frame(&ctx, &mut app, vec![Event::PointerMoved(key)]);
    frame(&ctx, &mut app, vec![button(key, true)]);
    log.extend(drain_auditions(&mut app));
    assert_eq!(log, [(64, true)], "a key press sounds its pitch");
    frame(&ctx, &mut app, vec![button(key, false)]);
    log.extend(drain_auditions(&mut app));
    assert_eq!(log, [(64, true), (64, false)]);
    // Pencil: a click on empty roll adds a note and sounds it briefly.
    app.run("edit.tool", json!({"tool": "pencil"})).unwrap();
    for _ in 0..60 {
        frame(&ctx, &mut app, vec![]);
    }
    drain_auditions(&mut app);
    let empty = point + vec2(0.0, 40.0);
    let mut log = Vec::new();
    frame(&ctx, &mut app, vec![Event::PointerMoved(empty)]);
    frame(&ctx, &mut app, vec![button(empty, true)]);
    frame(&ctx, &mut app, vec![button(empty, false)]);
    log.extend(drain_auditions(&mut app));
    for _ in 0..30 {
        frame(&ctx, &mut app, vec![]);
        log.extend(drain_auditions(&mut app));
    }
    assert_eq!(log.len(), 2, "{log:?}");
    assert!(log[0].1 && !log[1].1 && log[0].0 == log[1].0);
}
