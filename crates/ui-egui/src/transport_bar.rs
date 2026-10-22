//! The bottom transport bar: one compact row with the transport buttons, position, tempo and
//! locators. `UiState.transport_position` chooses between this bar and the cluster in the top
//! toolbar (`ui.transport_position`).

use crate::theme::{Tokens, bold, mono, regular};
use crate::widgets::{icon_button, key, key_fill, text_toggle};
use crate::{SoundApp, icons};
use egui::{Align2, CornerRadius, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::{Value, json};
use soundcraft_time::TimeFormat;

pub const HEIGHT: f32 = 32.0;

/// Where the transport controls live.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportPosition {
    /// The cluster in the top toolbar.
    Top,
    /// A bar along the bottom edge of the Edit and Mix windows.
    #[default]
    Bottom,
}

impl TransportPosition {
    pub const ALL: [Self; 2] = [Self::Bottom, Self::Top];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Top => "Top toolbar",
            Self::Bottom => "Bottom bar",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id().eq_ignore_ascii_case(s.trim()))
    }
}

/// `ui.transport_position`: report or set where the transport lives (no undo, not in the session).
pub(crate) fn command(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    if let Some(name) = p.get("position").and_then(Value::as_str) {
        let Some(pos) = TransportPosition::parse(name) else {
            return Err(format!("unknown transport position `{name}` (top or bottom)"));
        };
        app.ui.transport_position = pos;
    }
    Ok(json!({"position": app.ui.transport_position.id()}))
}

/// View > Transport: the two places with a check mark on the current one.
pub(crate) fn view_menu(app: &mut SoundApp, ui: &mut Ui) {
    ui.menu_button("Transport", |ui| {
        for pos in TransportPosition::ALL {
            let label = if app.ui.transport_position == pos { format!("\u{2714} {}", pos.label()) } else { pos.label().to_string() };
            if ui.button(label).clicked() {
                let _ = app.run("ui.transport_position", json!({"position": pos.id()}));
                ui.close();
            }
        }
    });
}

/// The bar's items, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Buttons,
    Position,
    Tempo,
    Locators,
    Meter,
    Click,
    CountOff,
    Skip,
}

const DISPLAY: [Item; 8] = [Item::Buttons, Item::Position, Item::Tempo, Item::Locators, Item::Meter, Item::Click, Item::CountOff, Item::Skip];
/// Highest priority first: what stays when the bar gets narrow.
const PRIORITY: [Item; 8] = [Item::Buttons, Item::Position, Item::Tempo, Item::Locators, Item::Meter, Item::Click, Item::CountOff, Item::Skip];
const GAP: f32 = 10.0;
const PAD: f32 = 8.0;
pub const OVERFLOW_W: f32 = 28.0;

pub const fn width(item: Item) -> f32 {
    match item {
        Item::Buttons => 5.0 * 30.0 + 4.0 * 3.0,
        Item::Position => 176.0,
        Item::Tempo => 136.0,
        Item::Locators => 224.0,
        Item::Meter => 52.0,
        Item::Click => 28.0,
        Item::CountOff => 28.0,
        Item::Skip => 3.0 * 28.0 + 2.0 * 3.0,
    }
}

fn need(keep: &[Item]) -> f32 {
    keep.iter().map(|i| width(*i)).sum::<f32>() + GAP * keep.len().saturating_sub(1) as f32 + 2.0 * PAD
}

/// Which items fit in `avail` points and where they go (x ranges from the bar's left edge), and
/// whether some were dropped into the overflow menu. Items never overlap.
pub fn layout(avail: f32) -> (Vec<(Item, f32, f32)>, bool) {
    let mut keep: Vec<Item> = Vec::new();
    for item in PRIORITY {
        keep.push(item);
        if need(&keep) > avail {
            keep.pop();
            break;
        }
    }
    let dropped = keep.len() < PRIORITY.len();
    if dropped {
        while !keep.is_empty() && need(&keep) + GAP + OVERFLOW_W > avail {
            keep.pop();
        }
    }
    let mut x = PAD;
    let mut out = Vec::new();
    for item in DISPLAY {
        if keep.contains(&item) {
            out.push((item, x, x + width(item)));
            x += width(item) + GAP;
        }
    }
    (out, dropped)
}

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.toolbar_bg);
    ui.painter().line_segment([full.left_top(), full.right_top()], Stroke::new(1.0, t.border));
    let (items, dropped) = layout(full.width());
    let mid = full.center().y;
    for (item, x0, x1) in &items {
        let r = Rect::from_min_max(pos2(full.min.x + x0, mid - 13.0), pos2(full.min.x + x1, mid + 13.0));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)));
        c.spacing_mut().item_spacing = vec2(3.0, 0.0);
        match item {
            Item::Buttons => buttons(app, &mut c, r),
            Item::Position => position(app, &mut c, r),
            Item::Tempo => tempo(app, &mut c, r),
            Item::Locators => locators(app, &mut c, r),
            Item::Meter => meter(app, &mut c, r),
            Item::Click => {
                let on = app.engine.session().edit.click;
                if icon_button(&mut c, vec2(28.0, 26.0), "click", on, "Click (metronome)").clicked() {
                    let _ = app.run("options.click", json!({}));
                }
            }
            Item::CountOff => {
                let on = app.engine.session().edit.countoff;
                if text_toggle(&mut c, vec2(28.0, 26.0), "CO", on, t.mode_on, "Count off before recording").clicked() {
                    let _ = app.run("options.countoff", json!({}));
                }
            }
            Item::Skip => {
                for (icon, cmd, tip) in [
                    ("rewind", "transport.rewind", "Rewind"),
                    ("ffwd", "transport.fast_forward", "Fast forward"),
                    ("end", "transport.go_to_end", "Go to end"),
                ] {
                    if icon_button(&mut c, vec2(28.0, 26.0), icon, false, tip).clicked() {
                        let _ = app.run(cmd, json!({}));
                    }
                }
            }
        }
    }
    if dropped {
        let r = Rect::from_min_size(pos2(full.max.x - PAD - OVERFLOW_W, mid - 13.0), vec2(OVERFLOW_W, 26.0));
        let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r));
        let resp = icon_button(&mut c, r.size(), "triangle_down", false, "More transport controls");
        egui::Popup::menu(&resp).show(|ui| overflow_menu(app, ui, &items));
    }
}

fn current_tempo(app: &SoundApp) -> f64 {
    let s = app.engine.session();
    let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
    s.tempo.tempo_at_tick(tick)
}

fn meter_text(app: &SoundApp) -> String {
    let s = app.engine.session();
    let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
    let m = s.tempo.meter_at_tick(tick);
    format!("{}/{}", m.numerator, m.denominator)
}

fn overflow_menu(app: &mut SoundApp, ui: &mut Ui, shown: &[(Item, f32, f32)]) {
    let hidden = |i: Item| !shown.iter().any(|s| s.0 == i);
    if hidden(Item::Tempo) {
        ui.label(format!("Tempo {:.2}", current_tempo(app)));
    }
    if hidden(Item::Locators) {
        let s = app.engine.session();
        let (sel, f) = (s.edit.selection, s.edit.main_counter);
        ui.label(format!("Left {}   Right {}", s.format_position(sel.start, f), s.format_position(sel.end, f)));
    }
    if hidden(Item::Meter) {
        ui.label(format!("Meter {}", meter_text(app)));
    }
    if hidden(Item::Click) {
        let mut on = app.engine.session().edit.click;
        if ui.checkbox(&mut on, "Click").clicked() {
            let _ = app.run("options.click", json!({}));
        }
    }
    if hidden(Item::CountOff) {
        let mut on = app.engine.session().edit.countoff;
        if ui.checkbox(&mut on, "Count off").clicked() {
            let _ = app.run("options.countoff", json!({}));
        }
    }
    if hidden(Item::Skip) {
        for (label, cmd) in [("Rewind", "transport.rewind"), ("Fast forward", "transport.fast_forward"), ("Go to end", "transport.go_to_end")] {
            if ui.button(label).clicked() {
                let _ = app.run(cmd, json!({}));
            }
        }
    }
}

fn big_key(ui: &mut Ui, tip: &str, draw: impl FnOnce(&egui::Painter, Rect, &egui::Response)) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(30.0, 26.0), Sense::click());
    draw(ui.painter(), r, &resp);
    resp.on_hover_text(tip)
}

fn buttons(app: &mut SoundApp, ui: &mut Ui, _r: Rect) {
    let t = Tokens::current();
    let playing = app.is_playing();
    let recording = app.engine.transport.recording;
    let looped = app.engine.session().edit.loop_playback;
    if icon_button(ui, vec2(30.0, 26.0), "rtz", false, "Return to zero").clicked() {
        let _ = app.run("transport.rtz", json!({}));
    }
    if icon_button(ui, vec2(30.0, 26.0), "loop", looped, "Loop playback").clicked() {
        let _ = app.run("options.loop_playback", json!({}));
    }
    let stop = big_key(ui, "Stop", |p, r, resp| {
        key(p, r, resp, key_fill(resp));
        icons::draw(p, r.shrink(6.0), "stop", if playing || recording { t.text } else { t.accent });
    });
    if stop.clicked() {
        let _ = app.run("transport.stop", json!({}));
    }
    let play = big_key(ui, "Play", |p, r, resp| {
        key(p, r, resp, if playing { t.mode_on } else { key_fill(resp) });
        icons::draw(p, r.shrink(6.0), "play", if playing { t.mode_on_text } else { t.counter_text });
    });
    if play.clicked() {
        let _ = app.run("transport.play", json!({}));
    }
    let rec = big_key(ui, "Record", |p, r, resp| {
        key(p, r, resp, if recording { t.rec.gamma_multiply(0.45) } else { key_fill(resp) });
        icons::draw(p, r.shrink(6.0), "record", t.rec);
    });
    let rect = rec.rect;
    if rec.clicked() {
        let _ = app.run("transport.record", json!({}));
    }
    if let Some(why) = app.record_hint() {
        let below = Rect::from_min_max(pos2(rect.min.x, rect.min.y - 30.0), pos2(rect.max.x, rect.min.y));
        egui::Tooltip::always_open(ui.ctx().clone(), rec.layer_id, rec.id, below).show(|ui| ui.label(why));
    }
}

fn field(ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    ui.painter().rect(r, CornerRadius::same(4), t.counter_bg, Stroke::new(1.0, t.counter_border), StrokeKind::Inside);
}

fn position(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    field(ui, r);
    let (fmt, pos) = (app.engine.session().edit.main_counter, app.position());
    let txt = app.engine.session().format_position(pos, fmt);
    let p = ui.painter().with_clip_rect(r.shrink(1.0));
    p.text(pos2(r.max.x - 18.0, r.center().y), Align2::RIGHT_CENTER, txt, mono(16.0), t.counter_text);
    icons::draw(ui.painter(), Rect::from_center_size(pos2(r.max.x - 9.0, r.center().y), vec2(10.0, 10.0)), "triangle_down", t.text_dim);
    let resp = ui.interact(r, ui.id().with("bar_position"), Sense::click()).on_hover_text("Position (click to change the format)");
    egui::Popup::menu(&resp).show(|ui| {
        for f in TimeFormat::ALL {
            if ui.selectable_label(fmt == f, f.label()).clicked() {
                let _ = app.run("view.main_counter", json!({"format": f.id()}));
            }
        }
    });
}

fn tempo(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    field(ui, r);
    let bpm = current_tempo(app);
    let tap = Rect::from_min_size(pos2(r.min.x + 4.0, r.min.y + 3.0), vec2(30.0, r.height() - 6.0));
    let resp = ui.interact(tap, ui.id().with("bar_tap"), Sense::click());
    key(ui.painter(), tap, &resp, key_fill(&resp));
    ui.painter().text(tap.center(), Align2::CENTER_CENTER, "Tap", bold(10.0), t.text);
    if resp.on_hover_text("Tap the tempo").clicked()
        && let Some(v) = tap_tempo(ui.ctx())
    {
        let s = app.engine.session();
        let tick = s.tempo.samples_to_ticks(app.position(), s.sample_rate);
        let event = s.tempo.tempos().iter().rev().find(|e| e.tick <= tick).map_or(0, |e| e.tick);
        let at = s.tempo.tick_to_samples(event, s.sample_rate);
        let _ = app.run("event.tempo", json!({"bpm": v, "at": at}));
    }
    icons::draw(ui.painter(), Rect::from_center_size(pos2(tap.max.x + 12.0, r.center().y), vec2(13.0, 13.0)), "note", t.counter_text);
    crate::toolbar::tempo_value(app, ui, r.max.x - 8.0, r.center().y, bpm);
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

fn locators(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    let (sel, f) = (app.engine.session().edit.selection, app.engine.session().edit.main_counter);
    let half = r.width() / 2.0;
    for (i, (label, at)) in [("L", sel.start), ("R", sel.end)].into_iter().enumerate() {
        let c = Rect::from_min_size(pos2(r.min.x + half * i as f32, r.min.y), vec2(half - 3.0, r.height()));
        field(ui, c);
        let p = ui.painter().with_clip_rect(c.shrink(1.0));
        p.text(pos2(c.min.x + 8.0, c.center().y), Align2::LEFT_CENTER, label, regular(11.0), t.counter_label);
        let txt = app.engine.session().format_position(at, f);
        p.text(pos2(c.max.x - 6.0, c.center().y), Align2::RIGHT_CENTER, txt, mono(12.0), t.counter_text);
    }
}

fn meter(app: &mut SoundApp, ui: &mut Ui, r: Rect) {
    let t = Tokens::current();
    field(ui, r);
    ui.painter().text(r.center(), Align2::CENTER_CENTER, meter_text(app), mono(12.0), t.counter_text);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_never_overlaps_and_keeps_priority() {
        let mut last_count = 0;
        for w in (80..2400).step_by(7) {
            let (items, dropped) = layout(w as f32);
            let mut right = 0.0f32;
            for (_, x0, x1) in &items {
                assert!(*x0 >= right, "overlap at {w}");
                assert!(x1 > x0);
                right = *x1;
            }
            let limit = w as f32 - if dropped { OVERFLOW_W + PAD } else { 0.0 };
            assert!(right <= limit + 0.01 || items.is_empty(), "overflow past the edge at {w}: {right} > {limit}");
            assert!(items.len() >= last_count || w < 0, "narrower bars never show more");
            last_count = items.len();
            for (i, p) in PRIORITY.iter().enumerate() {
                if items.iter().any(|x| x.0 == *p) {
                    assert!(PRIORITY[..i].iter().all(|q| items.iter().any(|x| x.0 == *q)), "priority order broken at {w}");
                }
            }
            assert_eq!(dropped, items.len() < DISPLAY.len());
        }
        assert_eq!(layout(2400.0).0.len(), DISPLAY.len());
    }

    #[test]
    fn position_ids_round_trip() {
        for p in TransportPosition::ALL {
            assert_eq!(TransportPosition::parse(p.id()), Some(p));
        }
        assert_eq!(TransportPosition::default(), TransportPosition::Bottom);
        assert_eq!(TransportPosition::parse("middle"), None);
    }
}
