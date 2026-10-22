//! The left cluster of the transport bar: record mode, input quantize, keyboard lock and the two
//! position readouts.

use crate::theme::{Tokens, mono};
use crate::transport_bar::{bar_key, check_label, field, icon_key};
use crate::transport_layout::Item;
use crate::{SoundApp, icons};
use egui::{Align2, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_time::{TICKS_PER_QUARTER, TimeFormat};

/// Session flag behind the keyboard lock key.
const LOCK_FLAG: &str = "options.edit_tool_mode_keyboard_lock";
const QUANTIZE_FLAG: &str = "midi.input_quantize";
const QUANTIZE_GRID: &str = "midi.input_quantize.grid_ticks";

/// Input quantize grids: label, value for `event.input_quantize`, length in ticks.
pub const GRIDS: [(&str, &str, i64); 7] = [
    ("1/4", "1/4", TICKS_PER_QUARTER),
    ("1/8", "1/8", TICKS_PER_QUARTER / 2),
    ("1/8 triplet", "1/8t", TICKS_PER_QUARTER / 3),
    ("1/16", "1/16", TICKS_PER_QUARTER / 4),
    ("1/16 triplet", "1/16t", TICKS_PER_QUARTER / 6),
    ("1/32", "1/32", TICKS_PER_QUARTER / 8),
    ("1/64", "1/64", TICKS_PER_QUARTER / 16),
];

pub(crate) fn draw(app: &mut SoundApp, ui: &mut Ui, r: Rect, item: Item) {
    match item {
        Item::RecMode => rec_mode(app, ui, r),
        Item::Quantize => quantize(app, ui, r),
        Item::Lock => lock(app, ui, r),
        Item::Primary => position(app, ui, r, false),
        _ => position(app, ui, r, true),
    }
}

fn caret(p: &egui::Painter, r: Rect, col: egui::Color32) {
    icons::draw(p, Rect::from_center_size(pos2(r.max.x - 8.0, r.center().y), vec2(9.0, 9.0)), "triangle_down", col);
}

struct RecModes {
    loop_record: bool,
    quick_punch: bool,
    destructive: bool,
    track_punch: bool,
}

fn rec_modes(app: &SoundApp) -> RecModes {
    let e = &app.engine.session().edit;
    RecModes {
        loop_record: e.loop_record,
        quick_punch: e.quickpunch,
        destructive: e.record_mode == "destructive",
        track_punch: e.record_mode == "trackpunch",
    }
}

/// The four record-mode switches with check marks (also used by the gear menu).
pub(crate) fn rec_mode_menu(app: &mut SoundApp, ui: &mut Ui) {
    let m = rec_modes(app);
    for (label, cmd, on) in [
        ("Loop record", "options.loop_record", m.loop_record),
        ("Quick punch", "options.quickpunch", m.quick_punch),
        ("Destructive record", "options.destructive_record", m.destructive),
        ("Track punch", "options.trackpunch", m.track_punch),
    ] {
        if ui.button(check_label(label, on)).clicked() {
            let _ = app.run(cmd, json!({}));
        }
    }
}

fn rec_mode(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let m = rec_modes(app);
    let any = m.loop_record || m.quick_punch || m.destructive || m.track_punch;
    let resp = bar_key(ui, r, "rec_mode", "Record mode", any, |p, r, col| {
        icons::draw(p, Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(15.0, 15.0)), "rec_mode", col);
        caret(p, r, col);
    });
    egui::Popup::menu(&resp).show(|ui| rec_mode_menu(app, ui));
}

pub(crate) fn quantize_state(app: &SoundApp) -> (bool, i64) {
    let e = &app.engine.session().edit;
    (e.flag(QUANTIZE_FLAG), e.value(QUANTIZE_GRID, (TICKS_PER_QUARTER / 4) as f64) as i64)
}

/// The grid list of input quantize (also used by the gear menu).
pub(crate) fn quantize_menu(app: &mut SoundApp, ui: &mut Ui) {
    let (on, ticks) = quantize_state(app);
    if ui.button(check_label("Input quantize", on)).clicked() {
        let _ = app.run("event.input_quantize", json!({"enabled": !on}));
    }
    ui.separator();
    for (label, value, t) in GRIDS {
        if ui.button(check_label(label, ticks == t)).clicked() {
            let _ = app.run("event.input_quantize", json!({"grid": value}));
        }
    }
}

fn quantize(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let (on, _) = quantize_state(app);
    // The left part toggles; the arrow on the right opens the grid list.
    let caret_r = Rect::from_min_max(pos2(r.max.x - 14.0, r.min.y), r.max);
    let resp = bar_key(ui, r, "quantize", "Input quantize (click: on or off, arrow: grid)", on, |p, r, col| {
        icons::draw(p, Rect::from_center_size(pos2(r.min.x + 11.0, r.center().y), vec2(14.0, 14.0)), "quantize", col);
        caret(p, r, col);
    });
    let caret_resp = ui.interact(caret_r, ui.id().with("quantize_grid_arrow"), Sense::click()).on_hover_text("Input quantize grid");
    if resp.clicked() && !caret_resp.clicked() {
        let _ = app.run("event.input_quantize", json!({"enabled": !on}));
    }
    egui::Popup::menu(&caret_resp).show(|ui| quantize_menu(app, ui));
}

fn lock(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let on = app.engine.session().edit.flag(LOCK_FLAG);
    if icon_key(ui, r, "lock", "Keyboard lock for edit and tool modes", on).clicked() {
        let _ = app.run("options.keyboard_lock", json!({}));
    }
}

/// The format glyph: notes for bars and beats, a clock for the time formats.
fn format_icon(f: TimeFormat) -> &'static str {
    if f == TimeFormat::BarsBeats { "note" } else { "clock" }
}

/// The format used by the secondary readout.
pub fn secondary_format(app: &SoundApp) -> TimeFormat {
    app.ui.transport_secondary_format
}

fn position(app: &mut SoundApp, ui: &mut Ui, r: Rect, secondary: bool) {
    let t = Tokens::current();
    field(ui, r);
    let main = app.engine.session().edit.main_counter;
    let fmt = if secondary { secondary_format(app) } else { main };
    let pos = app.position();
    let txt = app.engine.session().format_position(pos, fmt);
    let p = ui.painter().with_clip_rect(r.shrink(1.0));
    icons::draw(&p, Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(13.0, 13.0)), format_icon(fmt), t.counter_label);
    p.text(pos2(r.max.x - 20.0, r.center().y), Align2::RIGHT_CENTER, txt, mono(15.0), t.counter_text);
    icons::draw(&p, Rect::from_center_size(pos2(r.max.x - 9.0, r.center().y), vec2(10.0, 10.0)), "triangle_down", t.text_dim);
    let name = if secondary { "bar_secondary" } else { "bar_primary" };
    let tip = if secondary { "Secondary position (click to change the format)" } else { "Position (click to change the format)" };
    let resp = ui.interact(r, ui.id().with(name), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    let resp = resp.on_hover_text(tip);
    egui::Popup::menu(&resp).show(|ui| format_menu(app, ui, secondary));
}

/// The list of position formats for the primary or the secondary readout.
pub(crate) fn format_menu(app: &mut SoundApp, ui: &mut Ui, secondary: bool) {
    let current = if secondary { secondary_format(app) } else { app.engine.session().edit.main_counter };
    for f in TimeFormat::ALL {
        if ui.selectable_label(current == f, f.label()).clicked() {
            if secondary {
                app.ui.transport_secondary_format = f;
            } else {
                let _ = app.run("view.main_counter", json!({"format": f.id()}));
            }
        }
    }
}
