//! The Standard Edit toolbar: one 34 pt row of compact groups. Positions come from the pure
//! `toolbar_layout::layout`; this file only draws the groups and dispatches their commands.

use crate::theme::{Tokens, bold};
use crate::toolbar_layout::{self as tl, Group, KEY, KEY_GAP};
use crate::widgets::{icon_button, key, key_fill};
use crate::{SoundApp, icons};
use egui::{Align, Align2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_engine::edit::{SHUFFLE_LOCK, SNAP_TO_GRID};
use soundcraft_model::{EditMode, Tool};

/// The groups this toolbar draws.
const PRESENT: [Group; 8] = [Group::Layout, Group::EditModes, Group::Tools, Group::Actions, Group::Snap, Group::Zoom, Group::Panels, Group::Meter];

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    ui.painter().rect_filled(ui.max_rect(), 0.0, t.toolbar_bg);
    remember_mode(app, ui);
    let w = ui.max_rect().width().max(tl::MIN_FIT);
    egui::ScrollArea::horizontal()
        .id_salt("edit_toolbar_scroll_standard")
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded)
        .show(ui, |ui| {
            let (row, _) = ui.allocate_exact_size(vec2(w, tl::ROW_H), Sense::hover());
            draw(app, ui, row);
        });
}

/// The last non-grid edit mode, so the snap key can leave Grid mode for it.
fn remember_mode(app: &SoundApp, ui: &Ui) {
    let mode = app.engine.session().edit.edit_mode;
    if !mode.is_grid() {
        ui.data_mut(|d| d.insert_temp(egui::Id::new("toolbar_previous_edit_mode"), mode));
    }
}

fn draw(app: &mut SoundApp, ui: &mut Ui, row: Rect) {
    let t = Tokens::current();
    let placed = tl::layout(row.min, row.width(), &PRESENT);
    let mut left_end: Option<f32> = None;
    for (g, r) in &placed.groups {
        if *g == Group::Meter {
            output_meter(app, ui, r.shrink(tl::GROUP_PAD));
            continue;
        }
        if let Some(x) = left_end.filter(|x| *x < r.min.x && r.min.x - *x <= tl::DIVIDER + 0.5) {
            let mid = (x + r.min.x) / 2.0;
            ui.painter().line_segment([pos2(mid, r.min.y + 4.0), pos2(mid, r.max.y - 4.0)], Stroke::new(1.0, t.border));
        }
        left_end = Some(r.max.x);
        ui.painter().rect(*r, egui::CornerRadius::same(4), t.toolbar_group, Stroke::new(1.0, t.group_border), StrokeKind::Inside);
        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(r.shrink(tl::GROUP_PAD)).layout(egui::Layout::left_to_right(Align::Center)));
        child.spacing_mut().item_spacing = vec2(KEY_GAP, 0.0);
        match g {
            Group::Layout => layout_group(app, &mut child),
            Group::EditModes => edit_modes(app, &mut child),
            Group::Tools => tools(app, &mut child),
            Group::Actions => actions(app, &mut child),
            Group::Snap => snap_group(app, &mut child),
            Group::Zoom => zoom(app, &mut child),
            Group::Panels => panel_toggles(app, &mut child),
            Group::State | Group::AutoCombo | Group::Meter => {}
        }
    }
    gear(app, ui, placed.gear, placed.panels_in_gear);
}

/// A key-styled drop-down button showing `text`.
fn combo_key(ui: &mut Ui, w: f32, text: &str, tip: &str) -> Response {
    let t = Tokens::current();
    let (r, resp) = ui.allocate_exact_size(vec2(w, KEY), Sense::click());
    key(ui.painter(), r, &resp, key_fill(&resp));
    let clip = ui.painter().with_clip_rect(r.shrink2(vec2(2.0, 0.0)));
    clip.text(pos2(r.min.x + 7.0, r.center().y), Align2::LEFT_CENTER, text, bold(10.5), t.text);
    icons::draw(ui.painter(), Rect::from_center_size(pos2(r.max.x - 8.0, r.center().y), vec2(9.0, 9.0)), "triangle_down", t.text_dim);
    resp.on_hover_text(tip)
}

fn k() -> egui::Vec2 {
    vec2(KEY, KEY)
}

fn layout_group(app: &mut SoundApp, ui: &mut Ui) {
    let resp = combo_key(ui, 110.0, "Layout", "Window layouts");
    let names: Vec<String> = app.ui.configurations.iter().map(|c| c.0.clone()).collect();
    let mut apply = None;
    egui::Popup::menu(&resp).show(|ui| {
        for (label, cmd) in [
            ("Window Configurations...", "window.configurations"),
            ("New Configuration", "window.config_new"),
            ("Update Active Configuration", "window.config_update"),
        ] {
            if ui.button(label).clicked() {
                let _ = app.run(cmd, json!({"value": true}));
            }
        }
        if !names.is_empty() {
            ui.separator();
        }
        for (i, name) in names.iter().enumerate() {
            if ui.button(format!("{}  {name}", i + 1)).clicked() {
                apply = Some(i);
            }
        }
    });
    if let Some(i) = apply {
        crate::windows::apply_configuration(app, i);
    }
    ui.add_space(2.0);
    let undo_tip = crate::menus::live_label(app, Some("edit.undo")).unwrap_or_default();
    let redo_tip = crate::menus::live_label(app, Some("edit.redo")).unwrap_or_default();
    let (can_undo, can_redo) = (app.engine.can_undo(), app.engine.redo_label().is_some());
    ui.add_enabled_ui(can_undo, |ui| {
        if icon_button(ui, k(), "undo", false, &undo_tip).clicked() {
            let _ = app.run("edit.undo", json!({}));
        }
    });
    ui.add_enabled_ui(can_redo, |ui| {
        if icon_button(ui, k(), "redo", false, &redo_tip).clicked() {
            let _ = app.run("edit.redo", json!({}));
        }
    });
}

fn edit_modes(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let ed = &app.engine.session().edit;
    let (cur, snap, locked) = (ed.edit_mode, ed.flag(SNAP_TO_GRID), ed.flag(SHUFFLE_LOCK));
    let cmd = crate::shortcuts::shortcut_label("Cmd", ui.ctx().os() == egui::os::OperatingSystem::Mac);
    let modes = [
        (EditMode::Shuffle, "Shuf", "Shuffle", "F1"),
        (EditMode::Slip, "Slip", "Slip", "F2"),
        (EditMode::Spot, "Spot", "Spot", "F3"),
        (EditMode::Grid, "Grid", "Grid", "F4"),
    ];
    for (m, label, name, fkey) in modes {
        let (r, resp) = ui.allocate_exact_size(vec2(44.0, KEY), Sense::click());
        let on = cur == m || (m == EditMode::Grid && cur == EditMode::GridRelative);
        key(ui.painter(), r, &resp, if on { t.mode_on } else { t.mode_off });
        let fg = if on { t.mode_on_text } else { t.mode_off_text };
        if m == EditMode::Grid && snap {
            ui.painter().rect_stroke(r, 1.0, Stroke::new(1.5, t.mode_on), StrokeKind::Inside);
        }
        if m == EditMode::Shuffle && locked {
            icons::draw(ui.painter(), Rect::from_center_size(r.center(), vec2(14.0, 14.0)), "lock", fg);
        } else {
            let txt = if m == EditMode::Grid && cur == EditMode::GridRelative { "Rel" } else { label };
            ui.painter().text(r.center(), Align2::CENTER_CENTER, txt, bold(10.5), fg);
        }
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, on, name));
        if resp.clicked() {
            crate::toolbar::mode_click(app, m, ui.input(|i| i.modifiers));
        }
        resp.on_hover_text(match m {
            EditMode::Shuffle if locked => format!("Shuffle is locked ({cmd}-click to unlock)"),
            EditMode::Shuffle => format!("Shuffle mode ({fkey}); {cmd}-click locks it out"),
            EditMode::Grid => format!("Grid mode ({fkey}; again for Relative); Shift-click: Snap to Grid in the other modes"),
            _ => format!("{name} mode ({fkey}); Shift-click adds Snap to Grid"),
        });
    }
}

fn tools(app: &mut SoundApp, ui: &mut Ui) {
    let cur = app.engine.session().edit.tool;
    for (tool, icon) in [
        (Tool::Smart, "smart"),
        (Tool::Selector, "selector"),
        (Tool::Grabber, "grabber"),
        (Tool::Trim, "trim"),
        (Tool::Pencil, "pencil"),
        (Tool::Scrubber, "scrubber"),
        (Tool::Zoom, "zoom"),
    ] {
        let tip = format!("{}{}", tool.label(), tool.fkey().map(|f| format!(" ({f})")).unwrap_or_default());
        let smart_member = cur == Tool::Smart && matches!(tool, Tool::Trim | Tool::Selector | Tool::Grabber);
        if icon_button(ui, k(), icon, cur == tool || smart_member, &tip).clicked() {
            let _ = app.run("edit.tool", json!({"tool": tool.id()}));
        }
    }
}

/// Whether `id` can run now (a greyed key otherwise).
fn can_run(app: &SoundApp, id: &str) -> bool {
    soundcraft_engine::find_command(id).is_some_and(|c| (c.enabled)(&app.engine).is_ok() && !app.engine.blocked_while_recording(id))
}

fn actions(app: &mut SoundApp, ui: &mut Ui) {
    let cmd = crate::shortcuts::shortcut_label("Cmd", ui.ctx().os() == egui::os::OperatingSystem::Mac);
    for (id, icon, tip) in [
        ("edit.separate", "split", format!("Split at the selection ({cmd}+E)")),
        ("edit.heal", "glue", format!("Glue separated clips ({cmd}+H)")),
        ("edit.clear", "delete", format!("Delete the selection ({cmd}+B)")),
    ] {
        let enabled = can_run(app, id);
        ui.add_enabled_ui(enabled, |ui| {
            if icon_button(ui, k(), icon, false, &tip).clicked() {
                let _ = app.run(id, json!({}));
            }
        });
    }
}

fn snap_group(app: &mut SoundApp, ui: &mut Ui) {
    let (mode, snap, grid, nudge, adaptive) = {
        let e = &app.engine.session().edit;
        (e.edit_mode, e.flag(SNAP_TO_GRID), e.grid, e.nudge, e.flag(soundcraft_engine::cmd::grid_adapt::GRID_ADAPTIVE))
    };
    if icon_button(ui, k(), "magnet", mode.is_grid() || snap, "Snap to grid (leaves Grid mode for the previous mode)").clicked() {
        if mode.is_grid() {
            let prev = ui.data(|d| d.get_temp::<EditMode>(egui::Id::new("toolbar_previous_edit_mode")));
            let id = match prev {
                Some(EditMode::Shuffle) => "shuffle",
                Some(EditMode::Spot) => "spot",
                _ => "slip",
            };
            let _ = app.run("edit.mode", json!({"mode": id, "snap_to_grid": false}));
        } else {
            let _ = app.run("edit.snap_to_grid", json!({}));
        }
    }
    let resp = combo_key(ui, 64.0, &grid.label(), "Grid value");
    crate::toolbar::value_menu(app, &resp, "view.grid");
    if icon_button(ui, k(), "adapt", adaptive, "Adapt the grid to the zoom level").clicked() {
        let _ = app.run("edit.grid_adaptive", json!({}));
    }
    let resp = combo_key(ui, 80.0, &format!("Nudge {}", nudge.label()), "Nudge value");
    crate::toolbar::value_menu(app, &resp, "view.nudge");
}

fn zoom(app: &mut SoundApp, ui: &mut Ui) {
    if icon_button(ui, k(), "minus", false, "Zoom out").clicked() {
        let _ = app.run("view.zoom_out", json!({}));
    }
    if icon_button(ui, k(), "wave", false, "Waveform zoom in").clicked() {
        let _ = app.run("view.waveform_zoom", json!({"factor": 2.0}));
    }
    if icon_button(ui, k(), "plus", false, "Zoom in").clicked() {
        let _ = app.run("view.zoom_in", json!({}));
    }
    if icon_button(ui, k(), "zoom", false, "Fit the session in the window").clicked() {
        let w = ui.ctx().content_rect().width() - 420.0;
        let _ = app.run("view.zoom_fit", json!({"width_px": w}));
    }
}

/// (label, icon, command, shown) for the panel toggles.
fn panels(app: &SoundApp) -> [(&'static str, &'static str, &'static str, bool); 5] {
    let u = &app.ui;
    [
        ("Tracks", "tracks", "window.track_list", u.show_tracks_list),
        ("Clips", "clips", "window.clip_list", u.show_clip_list),
        ("MIDI Editor", "note", "window.midi_editor", u.show_midi_editor),
        ("Memory Locations", "flag_l", "window.memory_locations", u.show_memory_locations),
        ("Undo History", "clock", "window.undo_history", u.show_undo_history),
    ]
}

fn panel_toggles(app: &mut SoundApp, ui: &mut Ui) {
    let all = panels(app);
    for (i, (label, icon, cmd, on)) in all.into_iter().enumerate() {
        if i == 1 {
            // Between Tracks and Clips: the Inspector, which arrives later.
            ui.add_enabled_ui(false, |ui| {
                icon_button(ui, k(), "inspector", false, "Inspector (not available yet)");
            });
        }
        if icon_button(ui, k(), icon, on, label).clicked() {
            let _ = app.run(cmd, json!({}));
        }
    }
}

fn output_meter(app: &SoundApp, ui: &mut Ui, r: Rect) {
    let m = app.main_meter;
    let w = (r.width() - 2.0) / 2.0;
    for c in 0..2 {
        let rr = Rect::from_min_size(pos2(r.min.x + c as f32 * (w + 2.0), r.min.y + 1.0), vec2(w, r.height() - 2.0));
        crate::widgets::meter(ui, rr, m.level[c], m.hold[c], m.clip);
    }
}

fn gear(app: &mut SoundApp, ui: &mut Ui, r: Rect, panels_in_gear: bool) {
    let t = Tokens::current();
    let resp = ui.interact(r, ui.id().with("toolbar_gear"), Sense::click());
    key(ui.painter(), r, &resp, key_fill(&resp));
    icons::draw(ui.painter(), Rect::from_center_size(r.center(), vec2(18.0, 18.0)), "gear", t.text);
    let resp = resp.on_hover_text("Toolbar and menu options");
    egui::Popup::menu(&resp).show(|ui| {
        ui.label(egui::RichText::new("Toolbar style").small().color(t.text_dim));
        for s in tl::ToolbarStyle::ALL {
            if ui.selectable_label(app.ui.toolbar_style == s, s.label()).clicked() {
                let _ = app.run("ui.toolbar_style", json!({"value": s.id()}));
            }
        }
        ui.separator();
        ui.label(egui::RichText::new("Menu layout").small().color(t.text_dim));
        for m in crate::menu_layout::MenuLayout::ALL {
            if ui.selectable_label(app.ui.menu_layout == m, m.label()).clicked() {
                let _ = app.run("ui.menu_layout", json!({"layout": m.id()}));
            }
        }
        if panels_in_gear {
            ui.separator();
            ui.label(egui::RichText::new("Panels").small().color(t.text_dim));
            for (label, _, cmd, on) in panels(app) {
                if ui.selectable_label(on, label).clicked() {
                    let _ = app.run(cmd, json!({}));
                }
            }
        }
    });
}
