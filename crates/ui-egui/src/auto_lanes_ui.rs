//! Automation lanes under a track row in the Edit window: a header (read and write buttons, hide,
//! lock, a searchable parameter menu and a value readout), a body that draws the parameter's
//! current value and its breakpoint curve, breakpoint editing, and "follow touched parameter".
//!
//! Which lanes a track shows is session state (`Track.shown_lanes`, commands in
//! `soundcraft_engine::cmd::auto_lanes`). Row height = track height + the lane heights.

use crate::edit_window::{auto_value, breakpoint_at, draw_automation, grid_lines, lane_range, sample_at, snap};
use crate::theme::{Tokens, bold, mono, regular, rgb};
use crate::widgets::shade;
use crate::{SoundApp, icons};
use egui::{Align2, Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_model::{AutoParam, AutomationMode, LANE_HEIGHT_DEFAULT, ShownLane, Track, TrackId, TrackKind};
use soundcraft_time::Samples;
use std::collections::HashMap;

const STRIP_W: f32 = 10.0;
const HANDLE_H: f32 = 5.0;

/// Sum of the heights of the lanes a track shows.
pub fn total_height(t: &Track) -> f32 {
    t.shown_lanes.iter().map(|l| ShownLane::clamp_height(l.height)).sum()
}

/// Where one lane sits: its header (left of the timeline) and its body (the timeline part).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaneGeom {
    pub header: Rect,
    pub body: Rect,
}

/// The lanes of `t` stacked at the bottom of `row` (the track's full row, lanes included).
pub fn lane_geometry(row: Rect, timeline_left: f32, t: &Track) -> Vec<LaneGeom> {
    let mut y = row.max.y - total_height(t);
    t.shown_lanes
        .iter()
        .map(|l| {
            let h = ShownLane::clamp_height(l.height);
            let g = LaneGeom {
                header: Rect::from_min_max(pos2(row.min.x, y), pos2(timeline_left, y + h)),
                body: Rect::from_min_max(pos2(timeline_left, y), pos2(row.max.x, y + h)),
            };
            y += h;
            g
        })
        .collect()
}

/// The controls of a lane header.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderLayout {
    pub read: Rect,
    pub write: Rect,
    pub hide: Rect,
    pub lock: Rect,
    pub param: Rect,
    pub value: Rect,
    pub follow: Rect,
}

pub fn header_layout(head: Rect) -> HeaderLayout {
    let rh = ((head.height() - 10.0) / 2.0).clamp(12.0, 18.0);
    let (y1, bw) = (head.min.y + 4.0, 20.0);
    let y2 = y1 + rh + 3.0;
    let ax = head.min.x + STRIP_W + 4.0;
    let cell = |x: f32, y: f32| Rect::from_min_size(pos2(x, y), vec2(bw, rh));
    let bx = ax + 2.0 * bw + 7.0;
    let right = (head.max.x - 4.0).max(bx + 2.0 * bw);
    let follow = cell(right - bw, y2);
    HeaderLayout {
        read: cell(ax, y1),
        write: cell(ax + bw + 2.0, y1),
        hide: cell(ax, y2),
        lock: cell(ax + bw + 2.0, y2),
        param: Rect::from_min_max(pos2(bx, y1), pos2(right, y1 + rh)),
        value: Rect::from_min_max(pos2(bx, y2), pos2(follow.min.x - 2.0, y2 + rh)),
        follow,
    }
}

/// The parameter's value when it is not automated (what the fader or knob shows). MIDI controllers
/// have no mixer value: they read their lane at `at` (the playhead), else the controller default.
pub fn static_value(t: &Track, p: &AutoParam, at: Samples) -> f32 {
    let send = |i: u8| t.mixer.sends.get(usize::from(i)).and_then(Option::as_ref);
    match p {
        AutoParam::Volume => t.mixer.volume_db,
        AutoParam::Pan(i) => t.mixer.pan.get(usize::from(*i)).copied().unwrap_or(0.0),
        AutoParam::Mute => f32::from(u8::from(t.mixer.mute)),
        AutoParam::SendLevel(i) => send(*i).map_or(-144.0, |s| s.level_db),
        AutoParam::SendPan(i) => send(*i).map_or(0.0, |s| s.pan),
        AutoParam::SendMute(i) => send(*i).map_or(0.0, |s| f32::from(u8::from(s.mute))),
        AutoParam::Plugin { slot, param } => {
            let ins = t.mixer.inserts.get(usize::from(*slot)).and_then(Option::as_ref);
            ins.and_then(|i| i.params.get(param).copied().or_else(|| soundcraft_dsp::plugin_info(&i.plugin)?.param(param).map(|p| p.default)))
                .unwrap_or(0.0)
        }
        AutoParam::MidiCc(_) | AutoParam::PitchBend | AutoParam::ChannelPressure => {
            let default = p.range().2;
            t.lane(p).map_or(default, |l| l.value_at(at, default))
        }
    }
}

/// Whether a toggle parameter currently reads as on.
pub fn toggle_on(p: &AutoParam, v: f32) -> bool {
    if p.is_midi() { v >= 64.0 } else { v > 0.5 }
}

/// The value a toggle click writes: on is the top of a controller's range (127), 1.0 otherwise.
pub fn toggle_value(p: &AutoParam, turn_on: bool) -> f32 {
    match (turn_on, p.is_midi()) {
        (false, _) => 0.0,
        (true, true) => p.range().1,
        (true, false) => 1.0,
    }
}

fn plugin_param(t: &Track, p: &AutoParam) -> Option<&'static soundcraft_dsp::ParamInfo> {
    let AutoParam::Plugin { slot, param } = p else { return None };
    let ins = t.mixer.inserts.get(usize::from(*slot))?.as_ref()?;
    soundcraft_dsp::plugin_info(&ins.plugin)?.param(param)
}

/// An on/off parameter: shown as a power toggle instead of a value field.
pub fn is_toggle(t: &Track, p: &AutoParam) -> bool {
    p.is_toggle() || plugin_param(t, p).is_some_and(|i| i.unit == soundcraft_dsp::Unit::Toggle)
}

/// The readout text with units.
pub fn value_text(t: &Track, p: &AutoParam, v: f32) -> String {
    match p {
        AutoParam::Volume | AutoParam::SendLevel(_) if v <= -143.9 => "-inf dB".into(),
        AutoParam::Volume | AutoParam::SendLevel(_) => format!("{v:.1} dB"),
        AutoParam::Pan(_) | AutoParam::SendPan(_) => format!("{v:+.2}"),
        AutoParam::Mute | AutoParam::SendMute(_) => if v > 0.5 { "on" } else { "off" }.into(),
        AutoParam::MidiCc(_) if p.stepped() => if v >= 64.0 { "on" } else { "off" }.into(),
        AutoParam::MidiCc(_) | AutoParam::ChannelPressure => format!("{v:.0}"),
        AutoParam::PitchBend => format!("{v:+.0}"),
        AutoParam::Plugin { .. } => plugin_param(t, p).map_or_else(|| format!("{v:.2}"), |i| i.format(v)),
    }
}

/// Step of the readout's arrows.
fn step_of(t: &Track, p: &AutoParam, range: (f32, f32)) -> f32 {
    match p {
        AutoParam::Volume | AutoParam::SendLevel(_) => 0.5,
        AutoParam::Pan(_) | AutoParam::SendPan(_) => 0.05,
        AutoParam::PitchBend => 128.0,
        AutoParam::MidiCc(_) | AutoParam::ChannelPressure => 1.0,
        AutoParam::Plugin { .. }
            if plugin_param(t, p).is_some_and(|i| matches!(i.unit, soundcraft_dsp::Unit::Choice | soundcraft_dsp::Unit::Toggle)) =>
        {
            1.0
        }
        _ => ((range.1 - range.0) / 100.0).max(1e-4),
    }
}

/// The command that sets a parameter's fixed value.
fn set_command(id: TrackId, p: &AutoParam, v: f32, at: Samples) -> (&'static str, serde_json::Value) {
    match p {
        AutoParam::Volume => ("mix.volume", json!({"track": id.0, "db": v})),
        AutoParam::Pan(i) => ("mix.pan", json!({"track": id.0, "pan": v, "index": i})),
        AutoParam::Mute => ("mix.mute", json!({"tracks": [id.0], "value": v > 0.5})),
        AutoParam::SendLevel(i) => ("mix.send_level", json!({"track": id.0, "slot": i, "db": v})),
        AutoParam::SendPan(i) => ("mix.send_level", json!({"track": id.0, "slot": i, "pan": v})),
        AutoParam::SendMute(i) => ("mix.send_level", json!({"track": id.0, "slot": i, "mute": v > 0.5})),
        AutoParam::Plugin { slot, param } => ("mix.insert_param", json!({"track": id.0, "slot": slot, "param": param, "value": v})),
        AutoParam::MidiCc(_) | AutoParam::PitchBend | AutoParam::ChannelPressure => {
            ("automation.midi_value", json!({"track": id.0, "param": p.id(), "value": v, "at": at}))
        }
    }
}

// ---- the parameter list ------------------------------------------------------------------------

/// Every parameter a lane can show on this track, in no particular order: the fader, pans, mute,
/// existing sends, and the parameters of the inserts.
pub fn track_params(t: &Track) -> Vec<(AutoParam, String)> {
    let mut v = vec![(AutoParam::Volume, "Volume".to_string())];
    for i in 0..t.mixer.pan.len().min(2) {
        v.push((AutoParam::Pan(i as u8), if i == 0 { "Pan".to_string() } else { "Pan 2".to_string() }));
    }
    v.push((AutoParam::Mute, "Mute".to_string()));
    v.extend(midi_entries(t));
    for (i, s) in t.mixer.sends.iter().enumerate().take(10) {
        if s.is_some() {
            let l = soundcraft_model::slot_letter(i as u8);
            v.push((AutoParam::SendLevel(i as u8), format!("Send {l} Level")));
            v.push((AutoParam::SendPan(i as u8), format!("Send {l} Pan")));
            v.push((AutoParam::SendMute(i as u8), format!("Send {l} Mute")));
        }
    }
    v
}

/// The controllers a MIDI-capable track offers in the "MIDI Controllers" section (instrument and
/// MIDI tracks only): modulation, expression, sustain, pitch bend, channel pressure.
pub fn midi_entries(t: &Track) -> Vec<(AutoParam, String)> {
    if !matches!(t.kind, TrackKind::Instrument | TrackKind::Midi) {
        return Vec::new();
    }
    [AutoParam::MidiCc(1), AutoParam::MidiCc(11), AutoParam::MidiCc(64), AutoParam::PitchBend, AutoParam::ChannelPressure]
        .into_iter()
        .map(|p| {
            let l = p.label();
            (p, l)
        })
        .collect()
}

/// The "All Controllers" submenu: CC0 to CC127 with generic names (empty on tracks without MIDI).
pub fn all_controllers(t: &Track) -> Vec<(AutoParam, String)> {
    if !matches!(t.kind, TrackKind::Instrument | TrackKind::Midi) {
        return Vec::new();
    }
    (0..=127u8)
        .map(|n| {
            let p = AutoParam::MidiCc(n);
            let l = p.label();
            (p, l)
        })
        .collect()
}

/// `Plugin - Param` entries for the track's inserts, ordered by slot.
pub fn plugin_entries(t: &Track) -> Vec<(AutoParam, String)> {
    let mut out = Vec::new();
    let names: Vec<Option<String>> = t
        .mixer
        .inserts
        .iter()
        .map(|i| i.as_ref().map(|i| soundcraft_dsp::plugin_info(&i.plugin).map_or_else(|| i.plugin.clone(), |p| p.name.to_string())))
        .collect();
    for (slot, ins) in t.mixer.inserts.iter().enumerate() {
        let Some(ins) = ins else { continue };
        let Some(name) = names.get(slot).cloned().flatten() else { continue };
        let dup = names.iter().filter(|n| n.as_deref() == Some(name.as_str())).count() > 1;
        let name = if dup { format!("{name} {}", soundcraft_model::slot_letter(slot as u8)) } else { name };
        let ids: Vec<(String, String)> = match soundcraft_dsp::plugin_info(&ins.plugin) {
            Some(info) => info.params.iter().map(|p| (p.id.to_string(), p.name.to_string())).collect(),
            None => ins.params.keys().map(|k| (k.clone(), k.clone())).collect(),
        };
        for (id, pname) in ids {
            out.push((AutoParam::Plugin { slot: slot as u8, param: id }, format!("{name} - {pname}")));
        }
    }
    out
}

/// Plugin entries with the most recently touched first (`recent` is newest first).
pub fn order_by_recent(entries: Vec<(AutoParam, String)>, recent: &[AutoParam]) -> Vec<(AutoParam, String)> {
    let rank = |p: &AutoParam| recent.iter().position(|r| r == p).unwrap_or(usize::MAX);
    let mut entries = entries;
    entries.sort_by_key(|(p, _)| rank(p));
    entries
}

/// Entries whose label contains every word of `query` (case-insensitive).
pub fn filter_entries<'a>(entries: &'a [(AutoParam, String)], query: &str) -> Vec<&'a (AutoParam, String)> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    entries.iter().filter(|(_, l)| words.iter().all(|w| l.to_lowercase().contains(w.as_str()))).collect()
}

/// A readable name for a lane's parameter.
pub fn param_label(t: &Track, p: &AutoParam) -> String {
    track_params(t).into_iter().chain(plugin_entries(t)).find(|(q, _)| q == p).map_or_else(|| p.label(), |(_, l)| l)
}

// ---- recency and follow --------------------------------------------------------------------------

type Snapshot = HashMap<(u64, AutoParam), f32>;

fn recent_id() -> egui::Id {
    egui::Id::new("auto_lane_recent")
}

/// Parameters the user moved last, newest first, on `track`.
pub fn recent_params(ctx: &egui::Context, track: TrackId) -> Vec<AutoParam> {
    let v: Vec<(u64, AutoParam)> = ctx.data(|d| d.get_temp(recent_id()).unwrap_or_default());
    v.into_iter().filter(|(t, _)| *t == track.0).map(|(_, p)| p).collect()
}

fn all_values(t: &Track) -> Vec<(AutoParam, f32)> {
    track_params(t)
        .into_iter()
        .chain(plugin_entries(t))
        .map(|(p, _)| {
            // Position 0 keeps MIDI lanes steady during playback (follow only reacts to edits).
            let v = static_value(t, &p, 0);
            (p, v)
        })
        .collect()
}

/// The parameter of a track whose value changed since `before`, comparing `now`.
pub fn changed_param(before: &Snapshot, track: u64, now: &[(AutoParam, f32)]) -> Option<AutoParam> {
    now.iter().find(|(p, v)| before.get(&(track, p.clone())).is_some_and(|old| (old - v).abs() > 1e-6)).map(|(p, _)| p.clone())
}

/// Once per frame: notes which parameters moved (the order for the parameter menu) and switches a
/// lane that follows to the parameter being moved. Values change from the app's own controls
/// (faders, pan, send levels, plugin windows) and from hosted plugin editors (their edits reach the
/// session each frame); automation playback does not change them.
pub fn follow_update(app: &mut SoundApp, ctx: &egui::Context) {
    let s = app.engine.session_arc();
    if s.tracks.iter().all(|t| t.shown_lanes.is_empty()) {
        return;
    }
    let before: Snapshot = ctx.data_mut(|d| d.get_temp(egui::Id::new("auto_lane_snapshot")).unwrap_or_default());
    let mut recent: Vec<(u64, AutoParam)> = ctx.data_mut(|d| d.get_temp(recent_id()).unwrap_or_default());
    let mut next: Snapshot = HashMap::new();
    let mut switches = Vec::new();
    for t in s.tracks.iter().filter(|t| !t.shown_lanes.is_empty()) {
        let now = all_values(t);
        if let Some(p) = changed_param(&before, t.id.0, &now) {
            recent.retain(|(id, q)| !(*id == t.id.0 && *q == p));
            recent.insert(0, (t.id.0, p.clone()));
            let shown = t.shown_lanes.iter().any(|l| l.param == p);
            if let Some(i) = t.shown_lanes.iter().position(|l| l.follow)
                && !shown
            {
                switches.push((t.id, i, p));
            }
        }
        for (p, v) in now {
            next.insert((t.id.0, p), v);
        }
    }
    recent.truncate(96);
    ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("auto_lane_snapshot"), next);
        d.insert_temp(recent_id(), recent);
    });
    for (t, i, p) in switches {
        let _ = app.run("automation.lane_set_param", json!({"track": t.0, "index": i, "param": p.id()}));
    }
}

// ---- drawing ---------------------------------------------------------------------------------------

/// Draw and handle every lane of `track`; `row` is the track's full row, lanes included.
pub fn show(app: &mut SoundApp, ui: &mut Ui, track: &Track, row: Rect, tl: Rect, selected: bool) {
    if track.shown_lanes.is_empty() {
        return;
    }
    let (head_bg, _) = crate::theme::track_backgrounds(track.color, selected, app.ui.track_tint);
    for (index, geom) in lane_geometry(row, tl.min.x, track).into_iter().enumerate() {
        let Some(lane) = track.shown_lanes.get(index) else { continue };
        let visible = ui.clip_rect();
        if geom.header.max.y < visible.min.y || geom.header.min.y > visible.max.y {
            continue;
        }
        body(app, ui, track, lane, index, geom.body, tl);
        header(app, ui, track, lane, index, geom.header, head_bg);
        height_handle(app, ui, track, lane, index, geom);
    }
}

fn header(app: &mut SoundApp, ui: &mut Ui, track: &Track, lane: &ShownLane, index: usize, head: Rect, head_bg: Color32) {
    let t = Tokens::current();
    let p = ui.painter().with_clip_rect(head.intersect(ui.clip_rect()));
    p.rect_filled(head, 0.0, shade(head_bg, -0.07));
    p.rect_filled(Rect::from_min_size(head.min, vec2(STRIP_W, head.height())), 0.0, shade(rgb(track.color), -0.25));
    // A little breakpoint curve in the strip.
    let c = head.min + vec2(STRIP_W * 0.5, 10.0);
    p.add(egui::Shape::line(vec![c + vec2(-3.0, 3.0), c + vec2(0.0, -3.0), c + vec2(3.0, 2.0)], Stroke::new(1.2, t.text_dark)));
    p.line_segment([pos2(head.min.x, head.max.y - 0.5), pos2(head.max.x, head.max.y - 0.5)], Stroke::new(1.0, t.border));
    let bg = ui.interact(head, ui.id().with(("lane_head", track.id.0, index)), Sense::click());
    bg.context_menu(|ui| header_menu(app, ui, track, lane));
    let lay = header_layout(head);
    let id = track.id;
    let mode = track.mixer.automation_mode;
    for (r, label, m, col, key) in [(lay.read, "R", AutomationMode::Read, t.auto_read, 0u8), (lay.write, "W", AutomationMode::Write, t.auto_write, 1)]
    {
        let resp = ui.interact(r, ui.id().with(("lane_rw", id.0, index, key)), Sense::click());
        let lit = mode == m;
        let face = if lit {
            col
        } else if resp.hovered() {
            t.button_hi
        } else {
            t.button
        };
        p.rect_filled(r, 4.0, face);
        p.text(r.center(), Align2::CENTER_CENTER, label, bold(10.0), if lit { t.text_dark } else { t.text });
        if resp.on_hover_text(if key == 0 { "Automation read" } else { "Automation write" }).clicked() {
            let next = if lit { AutomationMode::Off } else { m };
            let _ = app.run("mix.automation_mode", json!({"track": id.0, "mode": next.label()}));
        }
    }
    // Hide and lock.
    let resp = ui.interact(lay.hide, ui.id().with(("lane_hide", id.0, index)), Sense::click());
    p.rect_filled(lay.hide, 4.0, if resp.hovered() { t.button_hi } else { t.button });
    let (a, b) = (lay.hide.shrink2(vec2(6.0, lay.hide.height() * 0.28)).min, lay.hide.shrink2(vec2(6.0, lay.hide.height() * 0.28)).max);
    p.line_segment([a, b], Stroke::new(1.4, t.text));
    p.line_segment([pos2(a.x, b.y), pos2(b.x, a.y)], Stroke::new(1.4, t.text));
    if resp.on_hover_text("Hide this lane (its automation stays)").clicked() {
        let _ = app.run("automation.lane_hide", json!({"track": id.0, "param": lane.param.id()}));
        return;
    }
    let resp = ui.interact(lay.lock, ui.id().with(("lane_lock", id.0, index)), Sense::click());
    p.rect_filled(
        lay.lock,
        4.0,
        if lane.locked {
            t.accent
        } else if resp.hovered() {
            t.button_hi
        } else {
            t.button
        },
    );
    icons::draw(&p, lay.lock.shrink2(vec2(3.0, 1.0)), "lock", if lane.locked { t.text_dark } else { t.text_dim });
    if resp.on_hover_text(if lane.locked { "Unlock the lane" } else { "Lock the lane against edits" }).clicked() {
        let _ = app.run("automation.lane_lock", json!({"track": id.0, "index": index}));
    }
    // Follow.
    let resp = ui.interact(lay.follow, ui.id().with(("lane_follow", id.0, index)), Sense::click());
    p.rect_filled(
        lay.follow,
        4.0,
        if lane.follow {
            t.accent
        } else if resp.hovered() {
            t.button_hi
        } else {
            t.button
        },
    );
    p.text(lay.follow.center(), Align2::CENTER_CENTER, "F", bold(10.0), if lane.follow { t.text_dark } else { t.text });
    if resp.on_hover_text("Follow: show the parameter you move").clicked() {
        let _ = app.run("automation.lane_follow", json!({"track": id.0, "index": index}));
    }
    param_menu(app, ui, track, lane, index, lay.param);
    value_box(app, ui, track, lane, index, lay.value);
}

fn header_menu(app: &mut SoundApp, ui: &mut Ui, track: &Track, lane: &ShownLane) {
    let id = track.id.0;
    if ui.button("Add Lane").clicked() {
        run_or_status(app, "automation.lane_show", json!({"track": id}));
        ui.close();
    }
    if ui.button("Hide Lane").clicked() {
        let _ = app.run("automation.lane_hide", json!({"track": id, "param": lane.param.id()}));
        ui.close();
    }
    let index = track.shown_lanes.iter().position(|l| l.param == lane.param).unwrap_or(0);
    if ui.button(if lane.locked { "Unlock Lane" } else { "Lock Lane" }).clicked() {
        let _ = app.run("automation.lane_lock", json!({"track": id, "index": index}));
        ui.close();
    }
    if ui.button(if lane.follow { "Stop Following" } else { "Follow Touched Parameter" }).clicked() {
        let _ = app.run("automation.lane_follow", json!({"track": id, "index": index}));
        ui.close();
    }
}

fn run_or_status(app: &mut SoundApp, cmd: &str, params: serde_json::Value) {
    if let Err(e) = app.run(cmd, params) {
        app.ui.status = e;
    }
}

fn param_menu(app: &mut SoundApp, ui: &mut Ui, track: &Track, lane: &ShownLane, index: usize, r: Rect) {
    let t = Tokens::current();
    let id = track.id.0;
    let resp = ui.interact(r, ui.id().with(("lane_param", id, index)), Sense::click());
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    p.rect(r, 3.0, if resp.hovered() { t.button_hi } else { t.button }, Stroke::new(1.0, t.border), StrokeKind::Inside);
    let label = param_label(track, &lane.param);
    p.with_clip_rect(Rect::from_min_max(r.min, pos2(r.max.x - 14.0, r.max.y))).text(
        pos2(r.min.x + 5.0, r.center().y),
        Align2::LEFT_CENTER,
        &label,
        regular(10.5),
        t.text,
    );
    icons::draw(&p, Rect::from_center_size(pos2(r.max.x - 8.0, r.center().y), vec2(8.0, 8.0)), "triangle_down", t.text_dim);
    let fresh = egui::Id::new(("lane_search_fresh", id, index));
    let query_id = egui::Id::new(("lane_search", id, index));
    if resp.clicked() {
        ui.ctx().data_mut(|d| {
            d.insert_temp(fresh, true);
            d.insert_temp(query_id, String::new());
        });
    }
    let resp = resp.on_hover_text(label);
    egui::Popup::menu(&resp).show(|ui| {
        ui.set_min_width(250.0);
        ui.horizontal(|ui| {
            if ui.button("+").on_hover_text("Add another lane").clicked() {
                run_or_status(app, "automation.lane_show", json!({"track": id}));
                ui.close();
            }
            if ui.button("\u{2212}").on_hover_text("Remove this lane").clicked() {
                let _ = app.run("automation.lane_hide", json!({"track": id, "param": lane.param.id()}));
                ui.close();
            }
            let mut q: String = ui.ctx().data(|d| d.get_temp(query_id).unwrap_or_default());
            let te = ui.add(egui::TextEdit::singleline(&mut q).hint_text("Search parameters").desired_width(170.0));
            if ui.ctx().data(|d| d.get_temp::<bool>(fresh).unwrap_or(false)) {
                te.request_focus();
                ui.ctx().data_mut(|d| d.insert_temp(fresh, false));
            }
            ui.ctx().data_mut(|d| d.insert_temp(query_id, q));
        });
        ui.separator();
        let q: String = ui.ctx().data(|d| d.get_temp(query_id).unwrap_or_default());
        let plugins = order_by_recent(plugin_entries(track), &recent_params(ui.ctx(), track.id));
        let (midi, builtin): (Vec<_>, Vec<_>) = track_params(track).into_iter().partition(|(p, _)| p.is_midi());
        let all_cc = all_controllers(track);
        let mut chosen = None;
        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            let plug = filter_entries(&plugins, &q);
            let built = filter_entries(&builtin, &q);
            let mids = filter_entries(&midi, &q);
            for (p, l) in &plug {
                if ui.selectable_label(*p == lane.param, l).clicked() {
                    chosen = Some(p.clone());
                }
            }
            if !plug.is_empty() && !built.is_empty() {
                ui.separator();
            }
            for (p, l) in &built {
                if ui.selectable_label(*p == lane.param, l).clicked() {
                    chosen = Some(p.clone());
                }
            }
            if !mids.is_empty() {
                if !plug.is_empty() || !built.is_empty() {
                    ui.separator();
                }
                ui.label(egui::RichText::new("MIDI Controllers").color(Tokens::current().text_dim).small());
                for (p, l) in &mids {
                    if ui.selectable_label(*p == lane.param, l).clicked() {
                        chosen = Some(p.clone());
                    }
                }
            }
            if !all_cc.is_empty() {
                if q.trim().is_empty() {
                    ui.menu_button("All Controllers", |ui| {
                        egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                            for (p, l) in &all_cc {
                                if ui.selectable_label(*p == lane.param, l).clicked() {
                                    chosen = Some(p.clone());
                                }
                            }
                        });
                    });
                } else {
                    // A search looks through every controller too (the five above are not repeated).
                    for (p, l) in filter_entries(&all_cc, &q).into_iter().filter(|(p, _)| !midi.iter().any(|(m, _)| m == p)) {
                        if ui.selectable_label(*p == lane.param, l).clicked() {
                            chosen = Some(p.clone());
                        }
                    }
                }
            }
            let nothing = plug.is_empty() && built.is_empty() && mids.is_empty() && (all_cc.is_empty() || filter_entries(&all_cc, &q).is_empty());
            if nothing {
                ui.label(egui::RichText::new("No matching parameter").color(Tokens::current().text_dim));
            }
        });
        if let Some(p) = chosen {
            run_or_status(app, "automation.lane_set_param", json!({"track": id, "index": index, "param": p.id()}));
            ui.close();
        }
    });
}

fn value_box(app: &mut SoundApp, ui: &mut Ui, track: &Track, lane: &ShownLane, index: usize, r: Rect) {
    let t = Tokens::current();
    let id = track.id;
    let p = ui.painter().with_clip_rect(r.intersect(ui.clip_rect()));
    // A drag keeps the position it started at, so a moving playhead does not scatter breakpoints.
    let drag_key = ui.id().with(("lane_val_at", id.0, index));
    let at = ui.data(|d| d.get_temp::<Samples>(drag_key)).unwrap_or_else(|| app.position());
    let v = static_value(track, &lane.param, at);
    if is_toggle(track, &lane.param) {
        // A power-style toggle for on/off parameters.
        let on = toggle_on(&lane.param, v);
        let resp = ui.interact(r, ui.id().with(("lane_val", id.0, index)), Sense::click());
        p.rect(r, 3.0, if on { t.accent } else { t.button }, Stroke::new(1.0, t.border), StrokeKind::Inside);
        let c = r.center();
        let rad = (r.height() * 0.28).max(3.0);
        let col = if on { t.text_dark } else { t.text_dim };
        p.circle_stroke(c, rad, Stroke::new(1.4, col));
        p.line_segment([c + vec2(0.0, -rad - 1.0), c + vec2(0.0, -1.0)], Stroke::new(1.6, col));
        if resp.on_hover_text(value_text(track, &lane.param, v)).clicked() {
            set_value(app, track, &lane.param, toggle_value(&lane.param, !on), at, false);
        }
        return;
    }
    let range = lane_range(track, &lane.param);
    let step = step_of(track, &lane.param, range);
    let arrows_w = 12.0;
    let field = Rect::from_min_max(r.min, pos2(r.max.x - arrows_w, r.max.y));
    p.rect(r, 3.0, t.counter_bg, Stroke::new(1.0, t.counter_border), StrokeKind::Inside);
    p.with_clip_rect(field).text(
        pos2(field.max.x - 3.0, field.center().y),
        Align2::RIGHT_CENTER,
        value_text(track, &lane.param, v),
        mono(9.0),
        t.counter_text,
    );
    let resp = ui.interact(field, ui.id().with(("lane_val", id.0, index)), Sense::click_and_drag());
    if resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(drag_key, at));
    }
    if resp.dragged() {
        let per_px = (range.1 - range.0) / 200.0 * if ui.input(|i| i.modifiers.shift) { 0.1 } else { 1.0 };
        let dy = -resp.drag_delta().y;
        if dy != 0.0 {
            set_value(app, track, &lane.param, (v + dy * per_px).clamp(range.0, range.1), at, true);
        }
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<Samples>(drag_key));
        app.engine.end_merge();
    }
    if resp.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    resp.on_hover_text("Drag up or down to change the value (Shift: fine)");
    // Stepper arrows.
    for (up, rr) in [
        (true, Rect::from_min_max(pos2(r.max.x - arrows_w, r.min.y), pos2(r.max.x, r.center().y))),
        (false, Rect::from_min_max(pos2(r.max.x - arrows_w, r.center().y), pos2(r.max.x, r.max.y))),
    ] {
        let resp = ui.interact(rr, ui.id().with(("lane_step", id.0, index, up)), Sense::click());
        let c = rr.center();
        let (a, b) = if up { (-2.5, 2.5) } else { (2.5, -2.5) };
        let col = if resp.hovered() { t.text } else { t.text_dim };
        p.add(egui::Shape::convex_polygon(vec![c + vec2(-3.5, b * 0.6), c + vec2(3.5, b * 0.6), c + vec2(0.0, a * 0.6)], col, Stroke::NONE));
        if resp.clicked() {
            let next = (v + if up { step } else { -step }).clamp(range.0, range.1);
            set_value(app, track, &lane.param, next, at, false);
        }
    }
}

fn set_value(app: &mut SoundApp, track: &Track, p: &AutoParam, v: f32, at: Samples, merged: bool) {
    let (cmd, params) = set_command(track.id, p, v, at);
    if merged {
        let key = format!("lane_value:{}:{}", track.id.0, p.id());
        let _ = app.engine.execute_merged(cmd, &params, &key);
    } else {
        let _ = app.run(cmd, params);
    }
}

fn height_handle(app: &mut SoundApp, ui: &mut Ui, track: &Track, lane: &ShownLane, index: usize, g: LaneGeom) {
    let edge = Rect::from_min_max(pos2(g.header.min.x, g.header.max.y - HANDLE_H), pos2(g.body.max.x, g.header.max.y));
    let resp = ui.interact(edge, ui.id().with(("lane_edge", track.id.0, index)), Sense::click_and_drag());
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    if resp.drag_started() {
        ui.data_mut(|d| d.insert_temp(resp.id, lane.height));
    }
    if resp.dragged()
        && let Some(p) = ui.ctx().pointer_latest_pos()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        let start = ui.data(|d| d.get_temp::<f32>(resp.id)).unwrap_or(lane.height);
        let want = ShownLane::clamp_height(start + p.y - origin.y);
        if (want - lane.height).abs() > 0.5 {
            let _ = app.run("automation.lane_height", json!({"track": track.id.0, "index": index, "height": want}));
        }
    }
    if resp.double_clicked() {
        let _ = app.run("automation.lane_height", json!({"track": track.id.0, "index": index, "height": LANE_HEIGHT_DEFAULT}));
    }
}

fn body(app: &mut SoundApp, ui: &mut Ui, track: &Track, lane: &ShownLane, index: usize, rect: Rect, tl: Rect) {
    let t = Tokens::current();
    let s = app.engine.session_arc();
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    painter.rect_filled(rect, 0.0, t.lane_bg);
    grid_lines(&painter, &s, tl, rect);
    draw_automation(&painter, &s, track, &lane.param, tl, rect);
    painter.line_segment([pos2(rect.min.x, rect.max.y - 0.5), pos2(rect.max.x, rect.max.y - 0.5)], Stroke::new(1.0, t.border));
    if lane.locked {
        painter.rect_filled(rect, 0.0, t.window_bg.gamma_multiply(0.35));
        icons::draw(&painter, Rect::from_min_size(pos2(rect.max.x - 22.0, rect.min.y + 4.0), vec2(14.0, 14.0)), "lock", t.text_dim);
    }
    let resp = ui.interact(rect, ui.id().with(("lane_body", track.id.0, index)), Sense::click_and_drag());
    let Some(pos) = resp.interact_pointer_pos().or_else(|| resp.hover_pos()) else { return };
    let range = lane_range(track, &lane.param);
    let pid = lane.param.id();
    let tid = track.id.0;
    let hit = |at: Pos2| breakpoint_at(&s, tl, rect, track, &lane.param, at);
    if resp.hovered() {
        ui.ctx().set_cursor_icon(if lane.locked {
            egui::CursorIcon::NotAllowed
        } else if hit(pos).is_some() {
            egui::CursorIcon::Grab
        } else {
            egui::CursorIcon::Crosshair
        });
    }
    if lane.locked {
        return;
    }
    let value_at = |y: f32| {
        let v = auto_value(rect, range, y);
        if is_toggle(track, &lane.param) { if v > (range.0 + range.1) * 0.5 { range.1 } else { range.0 } } else { v }
    };
    if resp.drag_started()
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
        && let Some(at) = hit(origin)
    {
        ui.data_mut(|d| d.insert_temp(resp.id, at));
    }
    if resp.dragged()
        && let Some(from) = ui.data(|d| d.get_temp::<i64>(resp.id))
        && let Some(points) = track.lane(&lane.param).map(|l| l.points.as_slice())
        && let Some(i) = points.iter().position(|pt| pt.at == from)
    {
        let after = i.checked_sub(1).and_then(|j| points.get(j)).map_or(0, |pt| pt.at.saturating_add(1));
        let before = points.get(i + 1).map_or(i64::MAX, |pt| pt.at.saturating_sub(1));
        let to = snap(&s, sample_at(&s, tl, pos.x).max(0)).clamp(after, before.max(after));
        let value = value_at(pos.y);
        if to != from || points.get(i).is_some_and(|pt| (pt.value - value).abs() > 1e-6) {
            let key = format!("lane_breakpoint:{tid}:{pid}");
            if to != from {
                let clear = json!({"track": tid, "param": pid, "start": from, "end": from.saturating_add(1)});
                let _ = app.engine.execute_merged("automation.clear", &clear, &key);
            }
            let _ = app.engine.execute_merged("automation.set_point", &json!({"track": tid, "param": pid, "at": to, "value": value}), &key);
            ui.data_mut(|d| d.insert_temp(resp.id, to));
        }
    }
    if resp.drag_stopped() {
        ui.data_mut(|d| d.remove::<i64>(resp.id));
        app.engine.end_merge();
    }
    if resp.clicked() && hit(pos).is_none() {
        let at = snap(&s, sample_at(&s, tl, pos.x).max(0));
        let _ = app.run("automation.set_point", json!({"track": tid, "param": pid, "at": at, "value": value_at(pos.y)}));
    }
    if resp.secondary_clicked()
        && let Some(at) = hit(pos)
    {
        let _ = app.run("automation.clear", json!({"track": tid, "param": pid, "start": at, "end": at.saturating_add(1)}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::{ChannelFormat, TrackKind};

    fn track() -> Track {
        Track::new(TrackId(1), "T", TrackKind::Audio, ChannelFormat::Mono, [200, 100, 50])
    }

    #[test]
    fn rows_stack_lanes_under_the_track() {
        let mut t = track();
        t.shown_lanes = vec![ShownLane::new(AutoParam::Volume), ShownLane { height: 100.0, ..ShownLane::new(AutoParam::Pan(0)) }];
        let row = Rect::from_min_max(pos2(0.0, 100.0), pos2(800.0, 100.0 + 88.0 + total_height(&t)));
        let g = lane_geometry(row, 300.0, &t);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].header.min.y, 188.0);
        assert_eq!(g[1].header.min.y, g[0].header.max.y);
        assert_eq!(g[1].body.max.y, row.max.y);
        assert_eq!(g[0].body.min.x, 300.0);
    }

    #[test]
    fn header_controls_never_overlap_or_leave_the_header() {
        for h in [36.0, 48.0, 72.0, 200.0] {
            for w in [136.0, 180.0, 400.0] {
                let head = Rect::from_min_size(pos2(10.0, 20.0), vec2(w, h));
                let l = header_layout(head);
                let all = [l.read, l.write, l.hide, l.lock, l.param, l.value, l.follow];
                for (i, a) in all.iter().enumerate() {
                    assert!(head.contains_rect(*a), "control {i} outside at {w}x{h}");
                    for b in &all[i + 1..] {
                        assert!(!a.intersects(*b), "overlap at {w}x{h}");
                    }
                }
            }
        }
    }

    #[test]
    fn filter_matches_all_words_in_any_case() {
        let e = vec![
            (AutoParam::Plugin { slot: 0, param: "ratio".into() }, "Compressor - Ratio".to_string()),
            (AutoParam::Plugin { slot: 0, param: "knee".into() }, "Compressor - Knee".to_string()),
            (AutoParam::Volume, "Volume".to_string()),
        ];
        assert_eq!(filter_entries(&e, "").len(), 3);
        assert_eq!(filter_entries(&e, "comp RAT").len(), 1);
        assert_eq!(filter_entries(&e, "vol").len(), 1);
        assert!(filter_entries(&e, "zzz").is_empty());
    }

    #[test]
    fn recent_parameters_come_first() {
        let a = AutoParam::Plugin { slot: 0, param: "a".into() };
        let b = AutoParam::Plugin { slot: 0, param: "b".into() };
        let c = AutoParam::Plugin { slot: 0, param: "c".into() };
        let e = vec![(a.clone(), "A".to_string()), (b.clone(), "B".to_string()), (c.clone(), "C".to_string())];
        let o = order_by_recent(e, &[c.clone(), a.clone()]);
        assert_eq!(o.iter().map(|x| x.0.clone()).collect::<Vec<_>>(), vec![c, a, b]);
    }

    #[test]
    fn changed_param_finds_the_moved_one_and_ignores_new_tracks() {
        let t = track();
        let now = all_values(&t);
        let mut before: Snapshot = HashMap::new();
        assert_eq!(changed_param(&before, 1, &now), None, "no earlier snapshot means no change");
        for (p, v) in &now {
            before.insert((1, p.clone()), *v);
        }
        assert_eq!(changed_param(&before, 1, &now), None);
        before.insert((1, AutoParam::Pan(0)), 0.5);
        assert_eq!(changed_param(&before, 1, &now), Some(AutoParam::Pan(0)));
    }

    fn instrument() -> Track {
        Track::new(TrackId(2), "I", TrackKind::Instrument, ChannelFormat::Stereo, [1, 2, 3])
    }

    #[test]
    fn midi_controllers_only_on_instrument_and_midi_tracks() {
        assert!(track_params(&track()).iter().all(|(p, _)| !p.is_midi()), "audio track has no MIDI entries");
        assert!(all_controllers(&track()).is_empty());
        let ids: Vec<String> = track_params(&instrument()).into_iter().filter(|(p, _)| p.is_midi()).map(|(p, _)| p.id()).collect();
        assert_eq!(ids, ["cc1", "cc11", "cc64", "pitch_bend", "pressure"]);
        assert_eq!(all_controllers(&instrument()).len(), 128);
        let mut midi_track = instrument();
        midi_track.kind = TrackKind::Midi;
        assert_eq!(midi_entries(&midi_track).len(), 5);
        assert!(all_values(&instrument()).iter().any(|(p, _)| *p == AutoParam::MidiCc(64)), "follow sees MIDI params");
    }

    #[test]
    fn controller_search_matches_names_and_numbers() {
        let e = track_params(&instrument());
        assert_eq!(filter_entries(&e, "sus").len(), 1);
        assert_eq!(filter_entries(&e, "11").len(), 1);
        assert_eq!(filter_entries(&all_controllers(&instrument()), "sus").iter().map(|(p, _)| p.id()).collect::<Vec<_>>(), ["cc64"]);
    }

    #[test]
    fn static_value_reads_the_lane_for_midi_controllers() {
        let mut t = instrument();
        let cc = AutoParam::MidiCc(11);
        assert_eq!(static_value(&t, &cc, 500), 127.0, "no lane: the controller default");
        t.lane_mut(&cc).set_point(0, 0.0);
        t.lane_mut(&cc).set_point(1000, 100.0);
        assert!((static_value(&t, &cc, 500) - 50.0).abs() < 1e-3, "reads the lane at the position");
        assert_eq!(static_value(&t, &cc, 5000), 100.0);
        let bend = AutoParam::PitchBend;
        t.lane_mut(&bend).set_point(0, -4000.0);
        assert_eq!(static_value(&t, &bend, 0), -4000.0);
    }

    #[test]
    fn midi_toggles_write_zero_or_one_twenty_seven() {
        let sus = AutoParam::MidiCc(64);
        assert!(is_toggle(&instrument(), &sus));
        assert_eq!(toggle_value(&sus, true), 127.0);
        assert_eq!(toggle_value(&sus, false), 0.0);
        assert_eq!(toggle_value(&AutoParam::Mute, true), 1.0);
        let mut t = instrument();
        assert!(!toggle_on(&sus, static_value(&t, &sus, 0)));
        t.lane_mut(&sus).set_point(0, 127.0);
        assert!(toggle_on(&sus, static_value(&t, &sus, 10)));
        t.lane_mut(&sus).set_point(0, 63.0);
        assert!(!toggle_on(&sus, 63.0), "below 64 is off, as the mixer reads it");
        let (cmd, p) = set_command(t.id, &sus, 127.0, 4800);
        assert_eq!((cmd, p["at"].as_i64(), p["value"].as_f64()), ("automation.midi_value", Some(4800), Some(127.0)));
        assert_eq!(step_of(&t, &AutoParam::MidiCc(1), (0.0, 127.0)), 1.0);
    }

    #[test]
    fn value_text_has_units() {
        let t = track();
        assert_eq!(value_text(&t, &AutoParam::Volume, -3.0), "-3.0 dB");
        assert_eq!(value_text(&t, &AutoParam::Volume, -144.0), "-inf dB");
        assert_eq!(value_text(&t, &AutoParam::Pan(0), 0.25), "+0.25");
        assert_eq!(value_text(&t, &AutoParam::Mute, 1.0), "on");
        assert!(is_toggle(&t, &AutoParam::Mute));
        assert!(!is_toggle(&t, &AutoParam::Volume));
    }
}
