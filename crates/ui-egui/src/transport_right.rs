//! The right cluster of the transport bar: locators, click, tempo, meter, count-off, pre/post-roll,
//! the master level, and the gear, float and close controls.

use crate::theme::{Tokens, bold, mono, regular};
use crate::transport_bar::{DEFAULT_FLOAT_RECT, TransportPosition, check_label, field, icon_key};
use crate::transport_layout::Item;
use crate::widgets::{key, key_fill, level_pos, meter_pos};
use crate::{SoundApp, icons};
use egui::{Align2, Rect, Sense, Ui, pos2, vec2};
use serde_json::json;

pub(crate) fn draw(app: &mut SoundApp, ui: &mut Ui, r: Rect, item: Item, dropped: &[Item]) {
    match item {
        Item::LLoc => locator(app, ui, r, false),
        Item::RLoc => locator(app, ui, r, true),
        Item::Click => {
            let on = app.engine.session().edit.click;
            if icon_key(ui, r, "click", "Click (metronome)", on).clicked() {
                let _ = app.run("options.click", json!({}));
            }
        }
        Item::Tempo => tempo(app, ui, r),
        Item::Meter => meter(app, ui, r),
        Item::CountOff => {
            let on = app.engine.session().edit.countoff;
            if icon_key(ui, r, "count_off", "Count off before recording", on).clicked() {
                let _ = app.run("options.countoff", json!({}));
            }
        }
        Item::PrePost => {
            let on = app.engine.session().edit.pre_post_roll;
            if icon_key(ui, r, "pre_post", "Pre and post roll (amounts in the gear menu)", on).clicked() {
                let _ = app.run("options.pre_post_roll", json!({}));
            }
        }
        Item::MasterMeter => master_meter(app, ui, r),
        Item::Float => {
            let floating = app.ui.transport_position == TransportPosition::Floating;
            let tip = if floating { "Dock the bar at the bottom" } else { "Float the bar" };
            if icon_key(ui, r, if floating { "dock" } else { "float" }, tip, false).clicked() {
                let to = if floating { "bottom" } else { "floating" };
                let _ = app.run("ui.transport_position", json!({"position": to}));
            }
        }
        Item::Gear => {
            let resp = icon_key(ui, r, "gear", "Bar settings", false);
            egui::Popup::menu(&resp).show(|ui| gear_menu(app, ui, dropped));
        }
        _ => {
            if icon_key(ui, r, "close", "Close the bar (View > Transport brings it back)", false).clicked() {
                let _ = app.run("ui.transport_visible", json!({"value": false}));
            }
        }
    }
}

/// The locator range: the timeline selection when there is one, else the edit selection.
pub fn locator_range(app: &SoundApp) -> (i64, i64) {
    let e = &app.engine.session().edit;
    let r = if e.timeline_selection.end > e.timeline_selection.start { e.timeline_selection } else { e.selection };
    (r.start, r.end)
}

fn locator(app: &mut SoundApp, ui: &mut Ui, r: Rect, right: bool) {
    let t = Tokens::current();
    let (start, end) = locator_range(app);
    let at = if right { end } else { start };
    let fmt = app.engine.session().edit.main_counter;
    field(ui, r);
    let p = ui.painter().with_clip_rect(r.shrink(1.0));
    icons::draw(
        &p,
        Rect::from_center_size(pos2(r.min.x + 12.0, r.center().y), vec2(13.0, 13.0)),
        if right { "flag_r" } else { "flag_l" },
        t.counter_label,
    );
    let txt = app.engine.session().format_position(at, fmt);
    p.text(pos2(r.max.x - 6.0, r.center().y), Align2::RIGHT_CENTER, txt, mono(12.0), t.counter_text);
    let tip = if right { "Right locator (click to move the playhead here)" } else { "Left locator (click to move the playhead here)" };
    let resp = ui.interact(r, ui.id().with(("bar_locator", right)), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, tip));
    if resp.hovered() {
        ui.painter().rect_stroke(r, 4.0, egui::Stroke::new(1.0, t.accent), egui::StrokeKind::Inside);
    }
    if resp.on_hover_text(tip).clicked() {
        let _ = app.run("transport.locate", json!({"at": at}));
    }
}

fn tempo_now(app: &SoundApp) -> f64 {
    let s = app.engine.session();
    let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
    s.tempo.tempo_at_tick(tick)
}

/// Set the tempo of the event in force at the playhead.
pub(crate) fn set_tempo(app: &mut SoundApp, bpm: f64) {
    let s = app.engine.session();
    let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
    let event = s.tempo.tempos().iter().rev().find(|e| e.tick <= tick).map_or(0, |e| e.tick);
    let at = s.tempo.tick_to_samples(event, s.sample_rate);
    let _ = app.run("event.tempo", json!({"bpm": bpm.clamp(5.0, 1000.0), "at": at}));
}

fn meter_text(app: &SoundApp) -> String {
    let s = app.engine.session();
    let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
    let m = s.tempo.meter_at_tick(tick);
    format!("{}/{}", m.numerator, m.denominator)
}

/// How far one nudge click moves the tempo: a whole BPM, or a hundredth with Shift held.
pub fn nudge_step(shift: bool) -> f64 {
    if shift { 0.01 } else { 1.0 }
}

fn tempo(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    field(ui, r);
    let bpm = tempo_now(app);
    let tap = Rect::from_min_size(pos2(r.min.x + 4.0, r.min.y + 3.0), vec2(30.0, r.height() - 6.0));
    let resp = ui.interact(tap, ui.id().with("bar_tap"), Sense::click());
    key(ui.painter(), tap, &resp, key_fill(&resp));
    ui.painter().text(tap.center(), Align2::CENTER_CENTER, "Tap", bold(10.0), t.text);
    if resp.on_hover_text("Tap the tempo").clicked()
        && let Some(v) = tap_tempo(ui.ctx())
    {
        set_tempo(app, v);
    }
    icons::draw(ui.painter(), Rect::from_center_size(pos2(tap.max.x + 12.0, r.center().y), vec2(13.0, 13.0)), "note", t.counter_text);
    // Up and down arrows at the right edge nudge the tempo.
    let col = Rect::from_min_max(pos2(r.max.x - 15.0, r.min.y + 2.0), pos2(r.max.x - 2.0, r.max.y - 2.0));
    for (up, rect) in
        [(true, Rect::from_min_max(col.min, pos2(col.max.x, col.center().y))), (false, Rect::from_min_max(pos2(col.min.x, col.center().y), col.max))]
    {
        let resp = ui.interact(rect, ui.id().with(("bar_tempo_nudge", up)), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(rect, 2.0, t.button_hi);
        }
        icons::draw(
            ui.painter(),
            Rect::from_center_size(rect.center(), vec2(8.0, 8.0)),
            if up { "triangle_up" } else { "triangle_down" },
            t.text_dim,
        );
        if resp.on_hover_text(if up { "Tempo up (Shift: a hundredth)" } else { "Tempo down (Shift: a hundredth)" }).clicked() {
            let step = nudge_step(ui.input(|i| i.modifiers.shift));
            set_tempo(app, bpm + if up { step } else { -step });
        }
    }
    crate::toolbar::tempo_value(app, ui, r.max.x - 20.0, r.center().y, bpm);
}

/// Average tempo of the recent taps; taps more than two seconds apart start over.
fn tap_tempo(ctx: &egui::Context) -> Option<f64> {
    let now = ctx.input(|i| i.time);
    let id = egui::Id::new("transport_taps");
    let mut taps: Vec<f64> = ctx.data_mut(|d| d.get_temp(id).unwrap_or_default());
    if taps.last().is_some_and(|l| now - l > 2.0) {
        taps.clear();
    }
    taps.push(now);
    if taps.len() > 8 {
        taps.remove(0);
    }
    let out = (taps.len() >= 2).then(|| {
        let span = taps.last().copied().unwrap_or(now) - taps.first().copied().unwrap_or(now);
        (60.0 * (taps.len() - 1) as f64 / span.max(0.001)).clamp(20.0, 400.0)
    });
    ctx.data_mut(|d| d.insert_temp(id, taps));
    out
}

fn meter(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    field(ui, r);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, meter_text(app), mono(12.0), t.counter_text);
    let resp = ui.interact(r, ui.id().with("bar_meter"), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Meter"));
    if resp.on_hover_text("Meter (click to open the tempo and meter window)").clicked() {
        let _ = app.run("window.tempo_ops", json!({"value": true}));
    }
}

/// Compact horizontal master peak meter: two thin bars, left over right, plus a clip lamp.
fn master_meter(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    let m = crate::mix_window::master_display(app);
    let resp = ui.interact(r, ui.id().with("bar_master_meter"), Sense::hover());
    let lamp = Rect::from_min_size(pos2(r.max.x - 5.0, r.min.y + 6.0), vec2(5.0, r.height() - 12.0));
    ui.painter().rect_filled(lamp, 1.0, if m.clip { t.meter_red } else { t.meter_clip_off });
    let bars = Rect::from_min_max(pos2(r.min.x, r.min.y + 6.0), pos2(r.max.x - 8.0, r.max.y - 6.0));
    let h = (bars.height() - 2.0) / 2.0;
    let (amber, red) = (meter_pos(-12.0), meter_pos(-3.0));
    for ch in 0..2 {
        let row = Rect::from_min_size(pos2(bars.min.x, bars.min.y + ch as f32 * (h + 2.0)), vec2(bars.width(), h));
        ui.painter().rect_filled(row, 1.0, t.meter_bg);
        let lit = level_pos(m.level[ch]);
        for (lo, hi, color) in [(0.0, amber, t.meter_green), (amber, red, t.meter_yellow), (red, 1.0, t.meter_red)] {
            let top = lit.clamp(lo, hi);
            if top > lo {
                let seg = Rect::from_min_max(pos2(row.min.x + row.width() * lo, row.min.y), pos2(row.min.x + row.width() * top, row.max.y));
                ui.painter().rect_filled(seg, 0.0, color);
            }
        }
        let held = level_pos(m.hold[ch]);
        if held > 0.01 {
            let x = row.min.x + row.width() * held;
            ui.painter().vline(x.min(row.max.x - 0.5), row.y_range(), egui::Stroke::new(1.5, t.counter_text));
        }
    }
    let db = |v: f32| if v <= 1e-6 { f32::NEG_INFINITY } else { 20.0 * v.log10() };
    let peak = db(m.hold[0].max(m.hold[1]));
    resp.on_hover_text(if peak.is_finite() { format!("Master level (peak {peak:.1} dB)") } else { "Master level".into() });
}

fn seconds_of(app: &SoundApp, samples: i64) -> f64 {
    app.engine.session().sample_rate.seconds(samples)
}

fn amount_row(app: &mut SoundApp, ui: &mut Ui, label: &str, command: &str, samples: i64) {
    let mut secs = seconds_of(app, samples);
    ui.horizontal(|ui| {
        ui.label(label);
        if ui.add(egui::DragValue::new(&mut secs).speed(0.05).range(0.0..=30.0).fixed_decimals(2).suffix(" s")).changed() {
            let _ = app.run(command, json!({"length": {"seconds": secs}}));
        }
    });
}

/// The gear menu: items that did not fit, roll amounts, which items to show, where the bar lives.
fn gear_menu(app: &mut SoundApp, ui: &mut Ui, dropped: &[Item]) {
    let t = Tokens::current();
    if !dropped.is_empty() {
        ui.label(egui::RichText::new("Does not fit in this width").font(regular(10.5)).color(t.text_dim));
        for item in dropped {
            dropped_entry(app, ui, *item);
        }
        ui.separator();
    }
    let (pre, post) = {
        let e = &app.engine.session().edit;
        (e.pre_roll, e.post_roll)
    };
    amount_row(app, ui, "Pre-roll", "options.pre_roll", pre);
    amount_row(app, ui, "Post-roll", "options.post_roll", post);
    ui.separator();
    ui.menu_button("Show items", |ui| {
        for item in Item::ALL.into_iter().filter(|i| i.hideable()) {
            let hidden = app.ui.transport_hidden_items.iter().any(|h| h == item.id());
            if ui.button(check_label(item.label(), !hidden)).clicked() {
                let _ = app.run("ui.transport_item", json!({"item": item.id(), "visible": hidden}));
            }
        }
        ui.separator();
        if ui.button("Show all items").clicked() {
            let _ = app.run("ui.transport_item", json!({"reset": true}));
            ui.close();
        }
    });
    ui.menu_button("Position", |ui| {
        for pos in [TransportPosition::Bottom, TransportPosition::Floating] {
            if ui.button(check_label(pos.label(), app.ui.transport_position == pos)).clicked() {
                let rect = if pos == TransportPosition::Floating { json!(app.ui.transport_float_rect) } else { json!(null) };
                let params = if rect.is_null() { json!({"position": pos.id()}) } else { json!({"position": pos.id(), "rect": rect}) };
                let _ = app.run("ui.transport_position", params);
                ui.close();
            }
        }
        if ui.button("Reset floating position").clicked() {
            let _ = app.run("ui.transport_position", json!({"position": "floating", "rect": DEFAULT_FLOAT_RECT}));
            ui.close();
        }
    });
    if ui.button("Close bar").clicked() {
        let _ = app.run("ui.transport_visible", json!({"value": false}));
        ui.close();
    }
}

/// One item that does not fit the bar, as a menu entry.
fn dropped_entry(app: &mut SoundApp, ui: &mut Ui, item: Item) {
    let (fmt, e) = {
        let s = app.engine.session();
        (s.edit.main_counter, s.edit.clone())
    };
    let run = |app: &mut SoundApp, cmd: &str| {
        let _ = app.run(cmd, json!({}));
    };
    match item {
        Item::RecMode => {
            ui.menu_button("Record mode", |ui| crate::transport_left::rec_mode_menu(app, ui));
        }
        Item::Quantize => {
            ui.menu_button("Input quantize", |ui| crate::transport_left::quantize_menu(app, ui));
        }
        Item::Lock => {
            let on = e.flag("options.edit_tool_mode_keyboard_lock");
            if ui.button(check_label("Keyboard lock", on)).clicked() {
                run(app, "options.keyboard_lock");
            }
        }
        Item::Primary | Item::Secondary => {
            let secondary = item == Item::Secondary;
            let f = if secondary { crate::transport_left::secondary_format(app) } else { fmt };
            let text = app.engine.session().format_position(app.position(), f);
            ui.menu_button(format!("{} {text}", item.label()), |ui| crate::transport_left::format_menu(app, ui, secondary));
        }
        Item::Start => {
            if ui.button("Go to start").clicked() {
                run(app, "transport.rtz");
            }
        }
        Item::End => {
            if ui.button("Go to end").clicked() {
                run(app, "transport.go_to_end");
            }
        }
        Item::Rew => {
            if ui.button("Rewind").clicked() {
                run(app, "transport.rewind");
            }
        }
        Item::FFwd => {
            if ui.button("Fast forward").clicked() {
                run(app, "transport.fast_forward");
            }
        }
        Item::Loop => {
            if ui.button(check_label("Loop playback", e.loop_playback)).clicked() {
                run(app, "options.loop_playback");
            }
        }
        Item::LLoc | Item::RLoc => {
            let (s, en) = locator_range(app);
            let at = if item == Item::RLoc { en } else { s };
            let text = app.engine.session().format_position(at, fmt);
            if ui.button(format!("{} {text}", item.label())).clicked() {
                let _ = app.run("transport.locate", json!({"at": at}));
            }
        }
        Item::Click => {
            if ui.button(check_label("Click", e.click)).clicked() {
                run(app, "options.click");
            }
        }
        Item::CountOff => {
            if ui.button(check_label("Count off", e.countoff)).clicked() {
                run(app, "options.countoff");
            }
        }
        Item::PrePost => {
            if ui.button(check_label("Pre and post roll", e.pre_post_roll)).clicked() {
                run(app, "options.pre_post_roll");
            }
        }
        Item::Tempo => {
            let bpm = tempo_now(app);
            ui.horizontal(|ui| {
                ui.label(format!("Tempo {bpm:.2}"));
                if ui.small_button("-1").clicked() {
                    set_tempo(app, bpm - 1.0);
                }
                if ui.small_button("+1").clicked() {
                    set_tempo(app, bpm + 1.0);
                }
            });
        }
        Item::Meter => {
            if ui.button(format!("Meter {}", meter_text(app))).clicked() {
                let _ = app.run("window.tempo_ops", json!({"value": true}));
            }
        }
        Item::MasterMeter => {
            let m = crate::mix_window::master_display(app);
            let peak = m.hold[0].max(m.hold[1]);
            let db = if peak <= 1e-6 { "-inf".to_string() } else { format!("{:.1}", 20.0 * peak.log10()) };
            ui.label(format!("Master level {db} dB"));
        }
        Item::Float => {
            let float = ui.button("Float the bar").clicked();
            if float {
                let _ = app.run("ui.transport_position", json!({"position": "floating"}));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nudge_is_a_whole_bpm_or_a_hundredth_with_shift() {
        assert_eq!(nudge_step(false), 1.0);
        assert_eq!(nudge_step(true), 0.01);
    }
}
