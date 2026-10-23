//! Panel discoverability: the panel list with check marks, Show All Panels, a close and menu
//! button on every dock tab strip, and hint strips that bring back a hidden Tracks or MIDI Editor
//! pane. All changes go through the docking commands (`ui.dock`); nothing here touches the session.

use crate::SoundApp;
use crate::docking::Panel;
use crate::theme::{Tokens, regular};
use egui::{Align2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

/// The panels a user can hide and show, in menu order.
pub const LIST: [Panel; 11] = [
    Panel::Tracks,
    Panel::Inspector,
    Panel::Clips,
    Panel::Midi,
    Panel::Memory,
    Panel::History,
    Panel::Master,
    Panel::Visibility,
    Panel::Meter,
    Panel::Loudness,
    Panel::Mixer,
];

const HINT_THICKNESS: f32 = 20.0;
const BUTTON: f32 = 18.0;

/// Whether a panel is currently shown (the flag the dock keeps in sync with its layout).
pub fn is_visible(app: &SoundApp, panel: Panel) -> bool {
    match panel {
        Panel::Tracks => app.ui.show_tracks_list,
        Panel::Clips => app.ui.show_clip_list,
        Panel::Midi => app.ui.show_midi_editor,
        Panel::Memory => app.ui.show_memory_locations,
        Panel::History => app.ui.show_undo_history,
        Panel::Master => app.ui.show_master_panel,
        Panel::Visibility => app.ui.show_visibility_pane,
        Panel::Meter => app.ui.show_meter_panel,
        Panel::Loudness => app.ui.show_loudness_panel,
        Panel::Inspector => app.ui.show_inspector,
        Panel::Mixer => app.ui.show_mixer_panel,
        Panel::Canvas => true,
    }
}

/// The panels that exist in a window (see [`Panel::edit_only`]).
pub fn panels_for(edit: bool) -> Vec<Panel> {
    LIST.iter().copied().filter(|p| edit || !p.edit_only()).collect()
}

/// `(panel, label, visible)` for the panel list of the current window.
pub fn entries(app: &SoundApp) -> Vec<(Panel, &'static str, bool)> {
    let edit = app.ui.window == crate::MainWindow::Edit;
    panels_for(edit).into_iter().map(|p| (p, p.label(), is_visible(app, p))).collect()
}

fn dock(app: &mut SoundApp, params: Value) -> Result<Value, String> {
    crate::docking::command(app, false, &params)
}

/// Show or hide a panel through the docking `open` / `close` operations.
pub fn set_visible(app: &mut SoundApp, panel: Panel, visible: bool) -> Result<Value, String> {
    dock(app, json!({"operation": if visible { "open" } else { "close" }, "panel": panel.id()}))
}

/// `Window > Show All Panels`: open every hidden panel in its remembered or default place.
pub fn show_all(app: &mut SoundApp) -> Result<Value, String> {
    let mut opened = Vec::new();
    for (panel, _, visible) in entries(app) {
        if !visible {
            set_visible(app, panel, true)?;
            opened.push(panel.id());
        }
    }
    Ok(json!({"opened": opened}))
}

/// The panel list with check marks. Inside the dock the change is queued (`deferred`) because the
/// dock owns its layout while it draws; elsewhere it runs at once.
pub fn list_menu(app: &mut SoundApp, ui: &mut Ui, deferred: bool) {
    for (panel, label, visible) in entries(app) {
        let text = if visible { format!("\u{2714} {label}") } else { label.to_string() };
        if ui.button(text).clicked() {
            request(app, deferred, json!({"operation": if visible { "close" } else { "open" }, "panel": panel.id()}));
            ui.close();
        }
    }
    ui.separator();
    if ui.button("Show All Panels").clicked() {
        request(app, deferred, json!({"operation": "showAll"}));
        ui.close();
    }
}

fn request(app: &mut SoundApp, deferred: bool, params: Value) {
    if deferred {
        app.midi.dock_request = Some(params);
    } else if params.get("operation").and_then(Value::as_str) == Some("showAll") {
        let _ = show_all(app);
    } else {
        let _ = dock(app, params);
    }
}

fn float_rect(panel: Panel) -> [f32; 4] {
    match panel {
        Panel::Memory => [160.0, 120.0, 480.0, 280.0],
        Panel::History => [680.0, 120.0, 260.0, 300.0],
        Panel::Tracks | Panel::Clips | Panel::Inspector => [160.0, 120.0, 260.0, 420.0],
        _ => [160.0, 120.0, 360.0, 360.0],
    }
}

/// Width the dock reserves at the right of a tab strip for the panel buttons (0: none).
pub fn header_width(active: Panel) -> f32 {
    match active {
        // The MIDI editor draws its own controls inside its header.
        Panel::Midi => 0.0,
        Panel::Canvas => BUTTON + 8.0,
        _ => 2.0 * BUTTON + 3.0 + 8.0,
    }
}

/// Close (x) and menu buttons for the active panel of a tab group.
pub fn header(app: &mut SoundApp, ui: &mut Ui, panel: Panel, floating: bool) {
    let t = Tokens::current();
    let r = ui.max_rect();
    let mut x = r.max.x - 4.0 - BUTTON;
    let kinds: &[u8] = if panel == Panel::Canvas { &[1] } else { &[0, 1] };
    for kind in kinds {
        let rect = Rect::from_min_size(pos2(x, r.center().y - BUTTON / 2.0), vec2(BUTTON, BUTTON));
        x -= BUTTON + 3.0;
        let (label, tip) =
            if *kind == 0 { (format!("Close {}", panel.label()), "Hide this panel") } else { (format!("{} menu", panel.label()), "Panel menu") };
        let resp = ui.interact(rect, ui.id().with(("panel_btn", panel.id(), *kind)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
        let resp = resp.on_hover_text(tip);
        if resp.hovered() {
            ui.painter().rect_filled(rect, 3.0, t.row_selected);
        }
        let c = rect.center();
        if *kind == 0 {
            let stroke = Stroke::new(1.3, t.header_text);
            ui.painter().line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], stroke);
            ui.painter().line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], stroke);
            if resp.clicked() {
                app.midi.dock_request = Some(json!({"operation": "close", "panel": panel.id()}));
            }
        } else {
            for dx in [-4.0, 0.0, 4.0] {
                ui.painter().circle_filled(c + vec2(dx, 0.0), 1.4, t.header_text);
            }
            egui::Popup::menu(&resp).show(|menu| {
                if panel != Panel::Canvas {
                    if menu.button("Hide").clicked() {
                        app.midi.dock_request = Some(json!({"operation": "close", "panel": panel.id()}));
                        menu.close();
                    }
                    if menu.add_enabled(!floating, egui::Button::new("Float")).clicked() {
                        app.midi.dock_request = Some(json!({"operation": "float", "panel": panel.id(), "rect": float_rect(panel)}));
                        menu.close();
                    }
                    if panel == Panel::Mixer && menu.button("Dock beside Session").clicked() {
                        app.midi.dock_request = Some(json!({"operation": "move", "panel": "mixer", "target": "canvas", "zone": "right"}));
                        menu.close();
                    }
                    if menu.button("Dock to default").clicked() {
                        app.midi.dock_request =
                            Some(json!({"operation": "move", "panel": panel.id(), "target": "canvas", "zone": panel.default_zone_name()}));
                        menu.close();
                    }
                    menu.separator();
                }
                list_menu(app, menu, true);
            });
        }
    }
}

/// Draw hint strips for hidden Tracks (left edge) and MIDI Editor (bottom edge; Edit window only)
/// inside `rect`, and return the rectangle left for the dock. Clicking a strip queues its panel.
pub fn hints(app: &mut SoundApp, ui: &mut Ui, rect: Rect, edit: bool) -> Rect {
    let mut inner = rect;
    for panel in [Panel::Tracks, Panel::Midi] {
        if is_visible(app, panel) || (panel == Panel::Midi && !edit) {
            ui.ctx().data_mut(|d| d.remove::<Rect>(egui::Id::new(("soundcraft-panel-hint", panel.id()))));
        }
    }
    if !is_visible(app, Panel::Tracks) {
        let strip = Rect::from_min_max(rect.min, pos2(rect.min.x + HINT_THICKNESS, rect.max.y));
        hint_strip(app, ui, strip, Panel::Tracks, "Tracks hidden - click to show", true);
        inner.min.x += HINT_THICKNESS;
    }
    if edit && !is_visible(app, Panel::Midi) {
        let strip = Rect::from_min_max(pos2(inner.min.x, rect.max.y - HINT_THICKNESS), rect.max);
        hint_strip(app, ui, strip, Panel::Midi, "MIDI editor hidden - click to show", false);
        inner.max.y -= HINT_THICKNESS;
    }
    inner
}

fn hint_strip(app: &mut SoundApp, ui: &mut Ui, strip: Rect, panel: Panel, text: &str, vertical: bool) {
    let t = Tokens::current();
    let resp = ui.interact(strip, ui.id().with(("panel_hint", panel.id())), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, text));
    let resp = resp.on_hover_text(text);
    ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new(("soundcraft-panel-hint", panel.id())), strip));
    let fill = if resp.hovered() { t.row_selected } else { t.panel_bg2 };
    ui.painter().rect_filled(strip, 0.0, fill);
    ui.painter().rect_stroke(strip, 0.0, Stroke::new(1.0, t.border), StrokeKind::Inside);
    let galley = ui.painter().layout_no_wrap(text.to_string(), regular(10.5), t.text_dim);
    if vertical {
        let (w, h) = (galley.size().x, galley.size().y);
        let pos = pos2(strip.center().x - h / 2.0, strip.center().y + w / 2.0);
        ui.painter().add(egui::epaint::TextShape::new(pos, galley, t.text_dim).with_angle(-std::f32::consts::FRAC_PI_2));
    } else {
        ui.painter().text(strip.center(), Align2::CENTER_CENTER, text, regular(10.5), t.text_dim);
    }
    if resp.clicked() {
        app.midi.dock_request = Some(json!({"operation": "open", "panel": panel.id()}));
    }
    resp.context_menu(|menu| list_menu(app, menu, true));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mix_has_no_clips_or_midi_entries() {
        let mix = panels_for(false);
        assert!(!mix.contains(&Panel::Clips) && !mix.contains(&Panel::Midi));
        assert_eq!(panels_for(true).len(), 11);
        assert!(mix.contains(&Panel::Master) && mix.contains(&Panel::Inspector) && mix.contains(&Panel::Mixer));
    }

    #[test]
    fn edit_only_drives_both_the_menu_and_the_mix_validation() {
        let mix = panels_for(false);
        for p in LIST.iter().copied() {
            assert_eq!(mix.contains(&p), !p.edit_only(), "{p:?}");
        }
        assert!(Panel::Clips.edit_only() && Panel::Midi.edit_only());
        assert!(!Panel::Inspector.edit_only() && !Panel::Mixer.edit_only() && !Panel::Master.edit_only());
        assert!(!Panel::Tracks.edit_only() && !Panel::Canvas.edit_only() && !Panel::History.edit_only());
    }

    #[test]
    fn header_widths_leave_the_midi_editor_to_its_own_controls() {
        assert_eq!(header_width(Panel::Midi), 0.0);
        assert!(header_width(Panel::Tracks) > header_width(Panel::Canvas));
    }
}
