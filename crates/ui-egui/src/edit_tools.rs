//! The arrange edit tools as UI: the order and names the Tools submenu and the palette share, key
//! hints for the active key set, and the compact palette popup opened by Ctrl (Cmd on macOS) plus
//! right-click on a lane.

use crate::SoundApp;
use crate::keymap;
use crate::theme::Tokens;
use egui::text::{LayoutJob, TextFormat};
use egui::{Button, Context, Id, Pos2, Rect, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_model::Tool;

/// Every tool in menu order, with the name the menu and palette show.
pub const MENU: [(Tool, &str); 11] = [
    (Tool::Grabber, "Object Select"),
    (Tool::Selector, "Range Select"),
    (Tool::Split, "Split"),
    (Tool::Glue, "Glue"),
    (Tool::Erase, "Erase"),
    (Tool::Zoom, "Zoom"),
    (Tool::Mute, "Mute"),
    (Tool::Pencil, "Draw"),
    (Tool::Scrubber, "Scrub"),
    (Tool::Trim, "Trim"),
    (Tool::Smart, "Smart"),
];

/// The icon drawn for a tool.
pub fn icon(tool: Tool) -> &'static str {
    match tool {
        Tool::Zoom => "zoom",
        Tool::Trim => "trim",
        Tool::Selector => "selector",
        Tool::Grabber => "grabber",
        Tool::Scrubber => "scrubber",
        Tool::Pencil => "pencil",
        Tool::Smart => "smart",
        Tool::Split => "split",
        Tool::Glue => "glue",
        Tool::Erase => "erase",
        Tool::Mute => "mute",
    }
}

/// The name of a tool in the menu and the palette.
pub fn menu_label(tool: Tool) -> &'static str {
    MENU.iter().find(|(t, _)| *t == tool).map_or_else(|| tool.label(), |(_, l)| l)
}

/// The key-binding row of a tool (`edit.tool` with its fixed parameter).
pub fn action_key(tool: Tool) -> String {
    keymap::action_key("edit.tool", &format!(r#"{{"tool":"{}"}}"#, tool.id()))
}

/// The key that picks the tool under the active key set: its bound key, else its F-key, else nothing.
pub fn key_hint(app: &SoundApp, tool: Tool, mac: bool) -> String {
    let bound = crate::menus::shortcut_text(app, &action_key(tool), mac);
    if bound.is_empty() { tool.fkey().map(str::to_string).unwrap_or_default() } else { bound }
}

/// Tooltip of a tool button: its name and key.
pub fn tool_tip(app: &SoundApp, tool: Tool, mac: bool) -> String {
    let hint = key_hint(app, tool, mac);
    if hint.is_empty() { tool.label().to_string() } else { format!("{} ({hint})", tool.label()) }
}

fn pick(app: &mut SoundApp, tool: Tool) {
    let _ = app.run("edit.tool", json!({"tool": tool.id()}));
}

/// One line of the Tools submenu: check mark, icon, name and key.
fn tool_item(app: &mut SoundApp, ui: &mut Ui, tool: Tool, label: &str) {
    let active = app.engine.session().edit.tool == tool;
    let hint = key_hint(app, tool, ui.ctx().os().is_mac());
    let font = egui::TextStyle::Button.resolve(ui.style());
    let colour = ui.visuals().text_color();
    let mut job = LayoutJob::default();
    job.append(label, 34.0, TextFormat::simple(font, colour));
    let resp = ui.add(Button::new(job).shortcut_text(hint).selected(active));
    let r = resp.rect;
    if active {
        let (x, y) = (r.min.x + 8.0, r.center().y);
        let stroke = egui::Stroke::new(1.6, colour);
        ui.painter().line_segment([pos2(x - 3.0, y), pos2(x - 1.0, y + 3.0)], stroke);
        ui.painter().line_segment([pos2(x - 1.0, y + 3.0), pos2(x + 4.0, y - 3.5)], stroke);
    }
    crate::icons::draw(ui.painter(), Rect::from_min_size(pos2(r.min.x + 16.0, r.center().y - 8.0), vec2(16.0, 16.0)), icon(tool), colour);
    if resp.clicked() {
        pick(app, tool);
        ui.close();
    }
}

/// The Tools submenu of the arrange context menus and the Edit menu.
pub fn tools_submenu(app: &mut SoundApp, ui: &mut Ui) {
    ui.menu_button("Tools", |ui| {
        for (tool, label) in MENU {
            tool_item(app, ui, tool, label);
        }
    });
}

/// The tools as flat lines at the top of the arrange right-click menus: each reads "<name> Tool" so it
/// cannot be mistaken for a clip command of the same name (Split, Glue, Mute).
pub fn tools_flat(app: &mut SoundApp, ui: &mut Ui) {
    for (tool, label) in MENU {
        tool_item(app, ui, tool, &format!("{label} Tool"));
    }
}

fn palette_id() -> Id {
    Id::new("tool_palette_pos")
}

/// Open the palette with its corner at `pos`.
pub fn open_palette(ctx: &Context, pos: Pos2) {
    ctx.data_mut(|d| d.insert_temp(palette_id(), pos));
    ctx.request_repaint();
}

/// Whether the palette is showing.
pub fn palette_open(ctx: &Context) -> bool {
    ctx.data(|d| d.get_temp::<Pos2>(palette_id())).is_some()
}

fn close_palette(ctx: &Context) {
    ctx.data_mut(|d| d.remove::<Pos2>(palette_id()));
}

/// Draw the palette when open: a 4 by 3 grid of tool icons. Escape or a press outside closes it.
pub fn show_palette(app: &mut SoundApp, ctx: &Context) {
    let Some(pos) = ctx.data(|d| d.get_temp::<Pos2>(palette_id())) else { return };
    let cur = app.engine.session().edit.tool;
    let mac = ctx.os().is_mac();
    let mut close = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let area = egui::Area::new(Id::new("tool_palette")).order(egui::Order::Foreground).fixed_pos(pos).constrain(true).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.spacing_mut().item_spacing = vec2(3.0, 3.0);
            ui.label(egui::RichText::new("Tools").small().color(Tokens::current().text_dim));
            egui::Grid::new("tool_palette_grid").spacing(vec2(3.0, 3.0)).show(ui, |ui| {
                for (i, (tool, _)) in MENU.iter().enumerate() {
                    let hint = key_hint(app, *tool, mac);
                    let tip = if hint.is_empty() { menu_label(*tool).to_string() } else { format!("{} ({hint})", menu_label(*tool)) };
                    if crate::widgets::icon_button(ui, vec2(34.0, 28.0), icon(*tool), cur == *tool, &tip).clicked() {
                        pick(app, *tool);
                        close = true;
                    }
                    if i % 4 == 3 {
                        ui.end_row();
                    }
                }
            });
        });
    });
    let outside = ctx.input(|i| i.pointer.any_pressed()) && ctx.pointer_interact_pos().is_some_and(|p| !area.response.rect.contains(p));
    if close || outside {
        close_palette(ctx);
    }
}

/// A small glyph of the active tool next to the pointer.
pub fn pointer_glyph(painter: &egui::Painter, at: Pos2, tool: Tool) {
    let r = Rect::from_min_size(at + vec2(12.0, 12.0), vec2(12.0, 12.0));
    crate::icons::draw(painter, r, icon(tool), Tokens::current().text);
}
