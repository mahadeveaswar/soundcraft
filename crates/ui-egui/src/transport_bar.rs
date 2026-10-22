//! The transport bar: one compact row with the transport keys centred, readouts on both sides, a
//! gear menu and float/close controls. `UiState.transport_position` chooses between the cluster in
//! the top toolbar, a bar along the bottom edge, and a floating bar (`ui.transport_position`).
//!
//! Layout is a pure function in `transport_layout`; the left and right clusters draw in
//! `transport_left` and `transport_right`.

use crate::theme::Tokens;
use crate::transport_layout::{self as tl, Item, Placed};
use crate::widgets::{key, key_fill};
use crate::{SoundApp, icons};
use egui::{Color32, CornerRadius, Painter, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};

pub const HEIGHT: f32 = 32.0;
/// Height of the controls inside the bar.
pub const CONTROL_H: f32 = 26.0;
/// Narrowest a floating bar may be.
pub const FLOAT_MIN_W: f32 = 240.0;
/// Where a floating bar starts: x, y, width, height.
pub const DEFAULT_FLOAT_RECT: [f32; 4] = [200.0, 600.0, 900.0, 36.0];

/// Where the transport controls live.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportPosition {
    /// The cluster in the top toolbar.
    Top,
    /// A bar along the bottom edge of the Edit and Mix windows.
    #[default]
    Bottom,
    /// The same bar in a small window that can be moved and made wider or narrower.
    Floating,
}

impl TransportPosition {
    pub const ALL: [Self; 3] = [Self::Bottom, Self::Floating, Self::Top];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Top => "Top toolbar",
            Self::Bottom => "Bottom bar",
            Self::Floating => "Floating bar",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
            Self::Floating => "floating",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id().eq_ignore_ascii_case(s.trim()))
    }
}

/// `ui.transport_position`: report or set where the transport lives (no undo, not in the session).
/// `rect` ([x, y, width, height]) moves and sizes the floating bar.
pub(crate) fn command(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    if let Some(rect) = p.get("rect") {
        let values = rect.as_array().filter(|a| a.len() == 4).ok_or("rect must be [x, y, width, height]")?;
        let mut out = [0.0f32; 4];
        for (o, v) in out.iter_mut().zip(values) {
            *o = v.as_f64().filter(|n| n.is_finite() && n.abs() <= 100_000.0).ok_or("rect values must be finite numbers")? as f32;
        }
        app.ui.transport_float_rect = out;
        app.ui.transport_float_apply = true;
    }
    if let Some(name) = p.get("position").and_then(Value::as_str) {
        let Some(pos) = TransportPosition::parse(name) else {
            return Err(format!("unknown transport position `{name}` (top, bottom or floating)"));
        };
        app.ui.transport_position = pos;
        // Docking or floating brings a closed bar back.
        if pos != TransportPosition::Top {
            app.ui.transport_visible = true;
        }
    }
    let mut out = json!({"position": app.ui.transport_position.id()});
    if app.ui.transport_position == TransportPosition::Floating {
        out["rect"] = json!(app.ui.transport_float_rect);
    }
    Ok(out)
}

/// `ui.transport_visible {value?}`: show or hide the bar (toggles without `value`).
pub(crate) fn visible_command(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let on = p.get("value").and_then(Value::as_bool).unwrap_or(!app.ui.transport_visible);
    app.ui.transport_visible = on;
    Ok(json!({"visible": on}))
}

/// `ui.transport_item {item, visible?} | {reset: true}`: show or hide one bar item. Transport keys
/// (stop, play, record), the gear and close cannot be hidden.
pub(crate) fn item_command(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    if p.get("reset").and_then(Value::as_bool) == Some(true) {
        app.ui.transport_hidden_items.clear();
    } else if let Some(id) = p.get("item").and_then(Value::as_str) {
        let item = Item::parse(id).ok_or_else(|| format!("unknown bar item `{id}`"))?;
        if !item.hideable() {
            return Err(format!("`{id}` cannot be hidden"));
        }
        let hide = !p.get("visible").and_then(Value::as_bool).unwrap_or(app.ui.transport_hidden_items.iter().any(|h| h == id));
        app.ui.transport_hidden_items.retain(|h| h != id);
        if hide {
            app.ui.transport_hidden_items.push(id.to_string());
        }
    }
    Ok(json!({"hidden": app.ui.transport_hidden_items}))
}

fn check(label: &str, on: bool) -> String {
    if on { format!("\u{2714} {label}") } else { label.to_string() }
}

/// View > Transport: where the bar lives, with a check mark on the current choice, and whether it
/// is shown.
pub(crate) fn view_menu(app: &mut SoundApp, ui: &mut Ui) {
    ui.menu_button("Transport", |ui| {
        for pos in TransportPosition::ALL {
            if ui.button(check(pos.label(), app.ui.transport_position == pos)).clicked() {
                let _ = app.run("ui.transport_position", json!({"position": pos.id()}));
                ui.close();
            }
        }
        ui.separator();
        let usable = app.ui.transport_position != TransportPosition::Top;
        if ui.add_enabled(usable, egui::Button::new(check("Show transport bar", app.ui.transport_visible))).clicked() {
            let _ = app.run("ui.transport_visible", json!({}));
            ui.close();
        }
    });
}

/// The user's hidden items as layout items.
pub fn hidden_items(app: &SoundApp) -> Vec<Item> {
    app.ui.transport_hidden_items.iter().filter_map(|id| Item::parse(id)).collect()
}

/// The bar in the Edit or Mix window: a bottom panel when docked, a small window when floating.
/// `panel_id` keeps the two windows' panels apart.
pub(crate) fn show_docked(app: &mut SoundApp, ui: &mut Ui, panel_id: &'static str) {
    if !app.ui.transport_visible {
        return;
    }
    let t = Tokens::current();
    match app.ui.transport_position {
        TransportPosition::Bottom => {
            egui::Panel::bottom(panel_id).exact_size(HEIGHT).frame(egui::Frame::NONE.fill(t.toolbar_bg)).show(ui, |ui| show(app, ui));
        }
        TransportPosition::Floating if ui.ctx().viewport_id() == egui::ViewportId::ROOT => show_floating(app, ui.ctx()),
        _ => {}
    }
}

/// Clamp a floating bar's rectangle to the screen: width at least [`FLOAT_MIN_W`] and at most the
/// screen, left top corner kept inside.
pub fn clamp_float_rect(rect: [f32; 4], screen: Rect) -> [f32; 4] {
    let w = rect[2].clamp(FLOAT_MIN_W.min(screen.width()), screen.width().max(1.0));
    let h = HEIGHT + 4.0;
    let x = rect[0].clamp(screen.min.x, (screen.max.x - w).max(screen.min.x));
    let y = rect[1].clamp(screen.min.y, (screen.max.y - h).max(screen.min.y));
    [x, y, w, h]
}

fn show_floating(app: &mut SoundApp, ctx: &egui::Context) {
    let t = Tokens::current();
    let screen = ctx.content_rect();
    let clamped = clamp_float_rect(app.ui.transport_float_rect, screen);
    let apply = std::mem::take(&mut app.ui.transport_float_apply);
    if apply {
        app.ui.transport_float_gen = app.ui.transport_float_gen.wrapping_add(1);
    }
    let id = egui::Id::new(("soundcraft_transport_float", app.ui.transport_float_gen));
    let frame = egui::Frame::NONE
        .fill(t.toolbar_bg)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(egui::Margin::same(1));
    let response = egui::Window::new("Transport")
        .id(id)
        .title_bar(false)
        .frame(frame)
        .resizable([true, false])
        .min_width(FLOAT_MIN_W.min(screen.width()))
        .max_width(screen.width())
        .default_pos(pos2(clamped[0], clamped[1]))
        .default_width(clamped[2])
        .constrain_to(screen)
        .show(ctx, |ui| {
            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEIGHT), Sense::hover());
            let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r));
            show_in(app, &mut c, r);
        });
    if let Some(r) = response {
        let now = clamp_float_rect([r.response.rect.min.x, r.response.rect.min.y, r.response.rect.width(), r.response.rect.height()], screen);
        if now[..3] != app.ui.transport_float_rect[..3] || (now[3] - app.ui.transport_float_rect[3]).abs() > 0.5 {
            app.ui.transport_float_rect = now;
        }
    }
}

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let full = ui.max_rect();
    show_in(app, ui, full);
}

/// Draw the bar in `full` (its width decides the layout).
pub(crate) fn show_in(app: &mut SoundApp, ui: &mut Ui, full: Rect) {
    let t = Tokens::current();
    ui.painter().rect_filled(full, 0.0, t.toolbar_bg);
    ui.painter().line_segment([full.left_top(), full.right_top()], Stroke::new(1.0, t.border));
    let lay = tl::layout(full.width(), &hidden_items(app));
    let mid = full.center().y;
    let rect_of = |p: &Placed| Rect::from_min_max(pos2(full.min.x + p.x0, mid - CONTROL_H / 2.0), pos2(full.min.x + p.x1, mid + CONTROL_H / 2.0));
    for x in &lay.dividers {
        ui.painter().vline(full.min.x + *x, (mid - 10.0)..=(mid + 10.0), Stroke::new(1.0, t.border));
    }
    if let Some((x0, x1)) = lay.outline {
        let r = Rect::from_min_max(pos2(full.min.x + x0, mid - CONTROL_H / 2.0 - 2.0), pos2(full.min.x + x1, mid + CONTROL_H / 2.0 + 2.0));
        ui.painter().rect_stroke(r, CornerRadius::same(5), Stroke::new(1.0, t.counter_border), StrokeKind::Inside);
    }
    for x in &lay.centre_dividers {
        ui.painter().vline(full.min.x + *x, (mid - 8.0)..=(mid + 8.0), Stroke::new(1.0, t.counter_rule));
    }
    for p in lay.all() {
        draw_item(app, ui, rect_of(p), p.item, &lay.dropped);
    }
}

fn draw_item(app: &mut SoundApp, ui: &mut Ui, r: Rect, item: Item, dropped: &[Item]) {
    match item {
        Item::Start | Item::End | Item::Rew | Item::FFwd | Item::Loop | Item::Stop | Item::Play | Item::Rec => keys(app, ui, r, item),
        Item::RecMode | Item::Quantize | Item::Lock | Item::Primary | Item::Secondary => crate::transport_left::draw(app, ui, r, item),
        _ => crate::transport_right::draw(app, ui, r, item, dropped),
    }
}

/// A key of the bar: a raised key, lit in the accent colour when `on`. `draw` paints the face
/// with the colour to use for it.
pub(crate) fn bar_key(ui: &mut Ui, r: Rect, name: &str, tip: &str, on: bool, draw: impl FnOnce(&Painter, Rect, Color32)) -> Response {
    let t = Tokens::current();
    let resp = ui.interact(r, ui.id().with(("transport_bar_key", name)), Sense::click());
    key(ui.painter(), r, &resp, if on { t.accent } else { key_fill(&resp) });
    draw(ui.painter(), r, if on { t.accent_text } else { t.text });
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    resp.on_hover_text(tip)
}

/// A key showing one icon.
pub(crate) fn icon_key(ui: &mut Ui, r: Rect, icon: &str, tip: &str, on: bool) -> Response {
    bar_key(ui, r, icon, tip, on, |p, r, col| icons::draw(p, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, col))
}

/// A readout field: rounded, dark, with the counter outline.
pub(crate) fn field(ui: &Ui, r: Rect) {
    let t = Tokens::current();
    ui.painter().rect(r, CornerRadius::same(4), t.counter_bg, Stroke::new(1.0, t.counter_border), StrokeKind::Inside);
}

pub(crate) fn check_label(label: &str, on: bool) -> String {
    check(label, on)
}

/// The centre keys.
fn keys(app: &mut SoundApp, ui: &mut Ui, r: Rect, item: Item) {
    let t = Tokens::current();
    let playing = app.is_playing();
    let recording = app.engine.transport.recording;
    match item {
        Item::Start | Item::End | Item::Rew | Item::FFwd => {
            let (icon, cmd, tip) = match item {
                Item::Start => ("rtz", "transport.rtz", "Go to start"),
                Item::End => ("end", "transport.go_to_end", "Go to end"),
                Item::Rew => ("rewind", "transport.rewind", "Rewind"),
                _ => ("ffwd", "transport.fast_forward", "Fast forward"),
            };
            if icon_key(ui, r, icon, tip, false).clicked() {
                let _ = app.run(cmd, json!({}));
            }
        }
        Item::Loop => {
            let looped = app.engine.session().edit.loop_playback;
            if icon_key(ui, r, "loop", "Loop playback", looped).clicked() {
                let _ = app.run("options.loop_playback", json!({}));
            }
        }
        Item::Stop => {
            // At rest the Stop key is filled with the accent colour, like a lit "stopped" state.
            let idle = !playing && !recording;
            let resp =
                bar_key(ui, r, "stop", "Stop", idle, |p, r, col| icons::draw(p, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), "stop", col));
            if resp.clicked() {
                let _ = app.run("transport.stop", json!({}));
            }
        }
        Item::Play => {
            let resp = ui.interact(r, ui.id().with("transport_bar_key_play"), Sense::click());
            key(ui.painter(), r, &resp, if playing { t.mode_on } else { key_fill(&resp) });
            icons::draw(
                ui.painter(),
                Rect::from_center_size(r.center(), vec2(16.0, 16.0)),
                "play",
                if playing { t.mode_on_text } else { t.counter_text },
            );
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Play"));
            if resp.on_hover_text("Play").clicked() {
                let _ = app.run("transport.play", json!({}));
            }
        }
        _ => {
            let resp = ui.interact(r, ui.id().with("transport_bar_key_record"), Sense::click());
            key(ui.painter(), r, &resp, if recording { t.rec.gamma_multiply(0.45) } else { key_fill(&resp) });
            icons::draw(ui.painter(), Rect::from_center_size(r.center(), vec2(16.0, 16.0)), "record", t.rec);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Record"));
            let rec = resp.on_hover_text("Record");
            if rec.clicked() {
                let _ = app.run("transport.record", json!({}));
            }
            if let Some(why) = app.record_hint() {
                let below = Rect::from_min_max(pos2(r.min.x, r.min.y - 30.0), pos2(r.max.x, r.min.y));
                egui::Tooltip::always_open(ui.ctx().clone(), rec.layer_id, rec.id, below).show(|ui| ui.label(why));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_ids_round_trip() {
        for p in TransportPosition::ALL {
            assert_eq!(TransportPosition::parse(p.id()), Some(p));
        }
        assert_eq!(TransportPosition::default(), TransportPosition::Bottom);
        assert_eq!(TransportPosition::parse("middle"), None);
    }

    #[test]
    fn floating_rect_is_clamped_to_the_screen() {
        let screen = Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0));
        let r = clamp_float_rect([900.0, 900.0, 5000.0, 10.0], screen);
        assert!(r[2] <= 1000.0 && r[0] + r[2] <= 1000.0 + 0.01, "{r:?}");
        assert!(r[1] + r[3] <= 700.0 + 0.01, "{r:?}");
        let r = clamp_float_rect([-50.0, -50.0, 10.0, 10.0], screen);
        assert!(r[0] >= 0.0 && r[1] >= 0.0 && r[2] >= FLOAT_MIN_W);
    }
}
