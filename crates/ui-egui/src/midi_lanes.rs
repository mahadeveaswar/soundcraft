//! Stacked controller lanes under the piano roll. Each lane has a label with a type dropdown at
//! its left (right-click removes it), a splitter above it for its height, and draws either the
//! note velocities or a controller. A `+` at the bottom left adds the next unused lane.
//! `ui.midi_lane_add/remove/height` do the same from the control channel.

use crate::SoundApp;
use crate::midi_editor::MidiTool;
use crate::theme::{Tokens, regular};
use egui::{Align2, Color32, Id, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde::{Deserialize, Serialize};
use serde_json::json;
use soundcraft_model::ClipId;

/// Default height of a lane in points.
pub const LANE_H_DEFAULT: f32 = 64.0;
/// Smallest lane height.
pub const LANE_H_MIN: f32 = 40.0;
/// Tallest lane height a command may set (the editor shrinks it further when the panel is small).
pub const LANE_H_MAX: f32 = 300.0;
/// The piano roll keeps at least this much of the editor height when lanes grow.
pub const ROLL_MIN_H: f32 = 120.0;
/// Height of the splitter above each lane.
pub const LANE_HANDLE_H: f32 = 6.0;
/// Gap between the last lane and the bottom edge of the editor.
pub const LANE_GAP: f32 = 2.0;
/// Background of a lane.
/// Width of the label column (same as the keyboard gutter).
pub const LABEL_W: f32 = 48.0;

/// What a lane edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lane {
    #[default]
    Velocity,
    Modulation,
    Expression,
    Sustain,
    PitchBend,
}

impl Lane {
    pub const ALL: [Lane; 5] = [Lane::Velocity, Lane::Modulation, Lane::Expression, Lane::Sustain, Lane::PitchBend];

    pub fn label(self) -> &'static str {
        match self {
            Lane::Velocity => "Velocity",
            Lane::Modulation => "Modulation (CC1)",
            Lane::Expression => "Expression (CC11)",
            Lane::Sustain => "Sustain (CC64)",
            Lane::PitchBend => "Pitch Bend",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Lane::Velocity => "Vel",
            Lane::Modulation => "CC1",
            Lane::Expression => "CC11",
            Lane::Sustain => "CC64",
            Lane::PitchBend => "Bend",
        }
    }

    /// The id used by `ui.midi_lane_add {lane}`.
    pub fn id(self) -> &'static str {
        match self {
            Lane::Velocity => "velocity",
            Lane::Modulation => "modulation",
            Lane::Expression => "expression",
            Lane::Sustain => "sustain",
            Lane::PitchBend => "pitch_bend",
        }
    }

    pub fn parse(text: &str) -> Option<Lane> {
        let t = text.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Self::ALL.into_iter().find(|l| l.id() == t)
    }

    /// `(lane, cc)` for the `midi.ctrl_*` commands, and the value range.
    pub fn command(self) -> (&'static str, i64, i32, i32) {
        match self {
            Lane::Velocity => ("cc", 0, 0, 127),
            Lane::Modulation => ("cc", 1, 0, 127),
            Lane::Expression => ("cc", 11, 0, 127),
            Lane::Sustain => ("cc", 64, 0, 127),
            Lane::PitchBend => ("pitch_bend", 0, -8192, 8191),
        }
    }

    /// The track automation parameter that drives the same controller (none for velocity).
    pub fn auto_param(self) -> Option<soundcraft_model::AutoParam> {
        use soundcraft_model::AutoParam;
        match self {
            Lane::Velocity => None,
            Lane::Modulation => Some(AutoParam::MidiCc(1)),
            Lane::Expression => Some(AutoParam::MidiCc(11)),
            Lane::Sustain => Some(AutoParam::MidiCc(64)),
            Lane::PitchBend => Some(AutoParam::PitchBend),
        }
    }

    pub fn kind(self) -> soundcraft_midi::CtrlKind {
        match self {
            Lane::PitchBend => soundcraft_midi::CtrlKind::PitchBend,
            other => soundcraft_midi::CtrlKind::Cc(other.command().1 as u8),
        }
    }
}

/// True when the track's automation lane for this controller owns it during playback (the track
/// reads automation and the lane has points), so the clip's own events are not heard.
pub fn automated_by_track(track: &soundcraft_model::Track, lane: Lane) -> bool {
    track.mixer.automation_mode.reads() && lane.auto_param().and_then(|p| track.lane(&p).map(|l| !l.points.is_empty())).unwrap_or(false)
}

/// One lane of the stack.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LaneCfg {
    pub lane: Lane,
    #[serde(default = "default_height")]
    pub height: f32,
}

fn default_height() -> f32 {
    LANE_H_DEFAULT
}

impl LaneCfg {
    pub fn new(lane: Lane) -> Self {
        Self { lane, height: LANE_H_DEFAULT }
    }
}

/// The stack of a fresh editor: just the velocity lane.
pub fn default_lanes() -> Vec<LaneCfg> {
    vec![LaneCfg::new(Lane::Velocity)]
}

/// Lanes from stored data; anything unreadable gives the default stack, heights are sanitised.
pub fn lanes_from_json(v: &serde_json::Value) -> Vec<LaneCfg> {
    let Ok(mut lanes) = serde_json::from_value::<Vec<LaneCfg>>(v.clone()) else { return default_lanes() };
    let mut seen = Vec::new();
    lanes.retain(|l| {
        let fresh = !seen.contains(&l.lane);
        seen.push(l.lane);
        fresh
    });
    for l in &mut lanes {
        l.height = if l.height.is_finite() { l.height.clamp(LANE_H_MIN, LANE_H_MAX) } else { LANE_H_DEFAULT };
    }
    lanes
}

/// The first lane type that is not in the stack yet.
pub fn next_unused(lanes: &[LaneCfg]) -> Option<Lane> {
    Lane::ALL.into_iter().find(|l| !lanes.iter().any(|c| c.lane == *l))
}

/// Add `lane` (or the next unused one). Returns false when it is already there or all are used.
pub fn add(lanes: &mut Vec<LaneCfg>, lane: Option<Lane>) -> bool {
    let Some(lane) = lane.or_else(|| next_unused(lanes)) else { return false };
    if lanes.iter().any(|c| c.lane == lane) {
        return false;
    }
    lanes.push(LaneCfg::new(lane));
    true
}

/// Remove the lane at `index`. Returns false when there is none.
pub fn remove(lanes: &mut Vec<LaneCfg>, index: usize) -> bool {
    if index < lanes.len() {
        lanes.remove(index);
        true
    } else {
        false
    }
}

/// Clamp a lane height: at least 40 pt, at most `avail_h` (the editor height below the header and
/// the ruler, minus the other lanes) minus 120 pt for the roll. A small panel shrinks the lane
/// first, never below 40.
pub fn clamp_lane_h(h: f32, avail_h: f32) -> f32 {
    let max = (avail_h - ROLL_MIN_H - LANE_HANDLE_H - LANE_GAP).max(LANE_H_MIN);
    if h.is_finite() { h.clamp(LANE_H_MIN, max) } else { LANE_H_DEFAULT.clamp(LANE_H_MIN, max) }
}

/// Where one lane and its splitter sit.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LaneRect {
    pub handle: Rect,
    pub area: Rect,
}

/// The stack laid out upwards from `bottom`: the lane rects (same order as `lanes`) and the
/// bottom edge of the piano roll. `avail_h` is the editor height below the ruler.
pub fn layout(lanes: &[LaneCfg], left: f32, right: f32, bottom: f32, avail_h: f32) -> (Vec<LaneRect>, f32) {
    let mut budget = avail_h - ROLL_MIN_H - LANE_GAP;
    let mut heights = Vec::with_capacity(lanes.len());
    for (i, l) in lanes.iter().enumerate() {
        let after = (lanes.len() - i - 1) as f32;
        let max = (budget - LANE_HANDLE_H - after * (LANE_H_MIN + LANE_HANDLE_H)).max(LANE_H_MIN);
        let h = if l.height.is_finite() { l.height.clamp(LANE_H_MIN, max) } else { LANE_H_DEFAULT.clamp(LANE_H_MIN, max) };
        budget -= h + LANE_HANDLE_H;
        heights.push(h);
    }
    let mut y = bottom - LANE_GAP;
    let mut rects = vec![LaneRect { handle: Rect::NOTHING, area: Rect::NOTHING }; lanes.len()];
    for (i, h) in heights.iter().enumerate().rev() {
        let area = Rect::from_min_max(pos2(left, y - h), pos2(right, y));
        let handle = Rect::from_min_max(pos2(left - LABEL_W, y - h - LANE_HANDLE_H), pos2(right, y - h));
        if let Some(slot) = rects.get_mut(i) {
            *slot = LaneRect { handle, area };
        }
        y -= h + LANE_HANDLE_H;
    }
    (rects, y)
}

/// Total height taken by every lane except `skip` (lane plus splitter).
fn others_h(rects: &[LaneRect], skip: usize) -> f32 {
    rects.iter().enumerate().filter(|(i, _)| *i != skip).map(|(_, r)| r.area.height() + LANE_HANDLE_H).sum()
}

/// The splitter above a lane: drag to resize it, double-click to reset. UI state only. The height
/// follows the pointer position (not deltas of a rect that moves under it).
fn lane_handle(ui: &mut Ui, app: &mut SoundApp, rects: &[LaneRect], idx: usize, avail_h: f32, cid: ClipId) {
    let Some(r) = rects.get(idx).copied() else { return };
    let id = ui.id().with(("lane_handle", cid.0, idx));
    let resp = ui.interact(r.handle, id, Sense::click_and_drag());
    if idx == 0 {
        ui.ctx().data_mut(|d| d.insert_temp(Id::new("soundcraft-midi-lane-handle"), r.handle));
    }
    if resp.hovered() || resp.dragged() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
    }
    let grab_id = id.with("grab");
    let avail_here = avail_h - others_h(rects, idx);
    if resp.double_clicked() {
        if let Some(l) = app.midi.lanes.get_mut(idx) {
            l.height = LANE_H_DEFAULT;
        }
        ui.ctx().data_mut(|d| d.remove::<f32>(grab_id));
    } else if resp.drag_started() {
        // The press point, not the current one: egui starts a drag after a small movement.
        let grab = ui.input(|i| i.pointer.press_origin()).map_or(0.0, |p| p.y - r.handle.min.y);
        ui.ctx().data_mut(|d| d.insert_temp(grab_id, grab));
    } else if resp.dragged() {
        let grab = ui.ctx().data(|d| d.get_temp::<f32>(grab_id));
        if let (Some(grab), Some(p)) = (grab, resp.interact_pointer_pos())
            && let Some(l) = app.midi.lanes.get_mut(idx)
        {
            l.height = clamp_lane_h(r.area.max.y - LANE_HANDLE_H - (p.y - grab), avail_here);
        }
    }
    let t = Tokens::current();
    let active = resp.hovered() || resp.dragged();
    let col = if active { t.header_text } else { t.text_dim.gamma_multiply(0.6) };
    let c = r.handle.center();
    let c = pos2(c.x.max(r.area.min.x + 20.0), c.y);
    ui.painter().line_segment([pos2(c.x - 14.0, c.y), pos2(c.x + 14.0, c.y)], Stroke::new(if active { 2.0 } else { 1.0 }, col));
    resp.on_hover_text("Drag to resize the lane, double-click to reset");
}

/// The label column of one lane: a dropdown to change its type; the lane itself has the Clear and Remove menu.
fn lane_label(ui: &mut Ui, app: &mut SoundApp, rects: &[LaneRect], idx: usize, cid: ClipId) {
    let Some(r) = rects.get(idx) else { return };
    let Some(cur) = app.midi.lanes.get(idx).map(|l| l.lane) else { return };
    let sel_rect = Rect::from_min_size(pos2(r.area.min.x - LABEL_W + 2.0, r.area.min.y + 2.0), vec2(LABEL_W - 4.0, 18.0));
    let mut lane_ui = ui.new_child(egui::UiBuilder::new().max_rect(sel_rect));
    let mut chosen = None;
    let resp = egui::ComboBox::from_id_salt(("midi_lane", cid.0, idx)).selected_text(cur.short()).width(LABEL_W - 14.0).show_ui(&mut lane_ui, |ui| {
        for l in Lane::ALL {
            let used = app.midi.lanes.iter().any(|c| c.lane == l);
            if (!used || l == cur) && ui.selectable_label(l == cur, l.label()).clicked() {
                chosen = Some(l);
            }
        }
    });
    if let Some(l) = chosen
        && let Some(c) = app.midi.lanes.get_mut(idx)
    {
        c.lane = l;
    }
    resp.response.on_hover_text("Lane type. Right-click the lane for Clear and Remove");
}

/// The `+` button at the bottom left of the editor: adds the next unused lane.
pub fn add_button(ui: &mut Ui, app: &mut SoundApp, r: Rect) {
    let t = Tokens::current();
    let can = next_unused(&app.midi.lanes).is_some();
    let resp = ui.interact(r, ui.id().with("midi_lane_add"), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, can, "Add lane"));
    if resp.hovered() && can {
        ui.painter().rect_filled(r, 3.0, t.row_selected);
    }
    let c = r.center();
    let s = Stroke::new(1.4, if can { t.header_text } else { t.text_dim.gamma_multiply(0.5) });
    ui.painter().line_segment([c + vec2(-4.0, 0.0), c + vec2(4.0, 0.0)], s);
    ui.painter().line_segment([c + vec2(0.0, -4.0), c + vec2(0.0, 4.0)], s);
    if resp.on_hover_text("Add the next controller lane").clicked() && can {
        add(&mut app.midi.lanes, None);
    }
}

/// Draw every lane (splitters, labels and contents). `velocity` draws the velocity lane into a rect.
#[allow(clippy::too_many_arguments)]
pub fn show_all(
    app: &mut SoundApp,
    ui: &mut Ui,
    cid: ClipId,
    sequence: &soundcraft_midi::Sequence,
    rects: &[LaneRect],
    avail_h: f32,
    x_of: &dyn Fn(i64) -> f32,
    tick_at: &dyn Fn(f32) -> i64,
    grid_ticks: i64,
    base: Color32,
    velocity: &mut dyn FnMut(&mut SoundApp, &mut Ui, Rect),
) {
    for idx in 0..rects.len() {
        lane_handle(ui, app, rects, idx, avail_h, cid);
        lane_label(ui, app, rects, idx, cid);
        let (Some(r), Some(cfg)) = (rects.get(idx), app.midi.lanes.get(idx).copied()) else { continue };
        if cfg.lane == Lane::Velocity {
            velocity(app, ui, r.area);
        } else {
            ctrl_lane(app, ui, cid, idx, cfg.lane, sequence, r.area, x_of, tick_at, grid_ticks, base);
        }
    }
}

/// Points of a straight ramp from `(t0, v0)` to `(t1, v1)`, one per `grid` ticks (the end point is
/// always included). The ends may be given in any order.
pub fn ramp_points(t0: i64, v0: i32, t1: i64, v1: i32, grid: i64) -> Vec<(i64, i32)> {
    let ((t0, v0), (t1, v1)) = if t0 <= t1 { ((t0, v0), (t1, v1)) } else { ((t1, v1), (t0, v0)) };
    let grid = grid.max(1);
    if t1 == t0 {
        return vec![(t0, v1)];
    }
    let n = ((t1 - t0) / grid).clamp(1, 2048);
    let mut pts: Vec<(i64, i32)> = (0..=n)
        .map(|i| {
            let f = i as f64 / n as f64;
            (t0 + ((t1 - t0) as f64 * f).round() as i64, v0 + (f64::from(v1 - v0) * f).round() as i32)
        })
        .collect();
    pts.dedup_by_key(|p| p.0);
    pts
}

/// The controller value for a pointer height inside a lane rect.
pub fn value_at(y: f32, r: Rect, lo: i32, hi: i32) -> i32 {
    let frac = ((r.max.y - 2.0 - y) / (r.height() - 4.0).max(1.0)).clamp(0.0, 1.0);
    lo + (frac * (hi - lo) as f32).round() as i32
}

/// A controller lane. Drag to draw (values follow the pointer, one point per grid step),
/// Shift+drag to erase, right-click to clear the lane, Sustain draws on/off steps. With the Line
/// tool a drag draws one straight ramp.
#[allow(clippy::too_many_arguments)]
fn ctrl_lane(
    app: &mut SoundApp,
    ui: &mut Ui,
    cid: ClipId,
    idx: usize,
    lane: Lane,
    sequence: &soundcraft_midi::Sequence,
    r: Rect,
    x_of: &dyn Fn(i64) -> f32,
    tick_at: &dyn Fn(f32) -> i64,
    grid_ticks: i64,
    base: Color32,
) {
    let (name, cc, lo, hi) = lane.command();
    let painter = ui.painter().with_clip_rect(r);
    painter.rect_filled(r, 0.0, Tokens::current().lane_bg);
    let span = (hi - lo) as f32;
    let y_of = |v: i32| r.max.y - 2.0 - (r.height() - 4.0) * (v - lo) as f32 / span;
    let zero = if lane == Lane::PitchBend { 0 } else { lo };
    painter.line_segment([pos2(r.min.x, y_of(zero)), pos2(r.max.x, y_of(zero))], Stroke::new(1.0, Tokens::current().lane_zero));
    painter.text(pos2(r.max.x - 6.0, r.min.y + 3.0), Align2::RIGHT_TOP, lane.label(), regular(9.0), Tokens::current().lane_label);
    // Events as a stepped line: the value holds until the next event.
    let kind = lane.kind();
    let evs: Vec<_> = sequence.ctrls.iter().filter(|c| c.channel == 0 && c.kind == kind).collect();
    let col = Color32::from_rgb(base.r().saturating_add(40), base.g().saturating_add(40), base.b().saturating_add(40));
    // Clip data is dimmed and badged while the track's own automation lane drives this controller.
    let automated = app.engine.session().find_clip(cid).and_then(|(t, _)| app.engine.session().track(t)).is_some_and(|t| automated_by_track(t, lane));
    let col = if automated { col.gamma_multiply(0.4) } else { col };
    let mut prev = (r.min.x, y_of(zero));
    for c in &evs {
        let (x, y) = (x_of(c.tick), y_of(c.value));
        painter.line_segment([pos2(prev.0, prev.1), pos2(x, prev.1)], Stroke::new(1.5, col));
        painter.line_segment([pos2(x, prev.1), pos2(x, y)], Stroke::new(1.5, col));
        painter.circle_filled(pos2(x, y), 2.5, col);
        prev = (x, y);
    }
    painter.line_segment([pos2(prev.0, prev.1), pos2(r.max.x, prev.1)], Stroke::new(1.5, col));

    if automated {
        let tk = Tokens::current();
        let galley = painter.layout_no_wrap("Automated by track lane".to_string(), regular(9.0), tk.text);
        let pill = Rect::from_min_size(pos2(r.min.x + 6.0, r.min.y + 3.0), galley.size() + vec2(10.0, 4.0));
        painter.rect(pill, 3.0, tk.button, Stroke::new(1.0, tk.border), egui::StrokeKind::Inside);
        painter.galley(pill.min + vec2(5.0, 2.0), galley, tk.text);
    }
    let resp = ui.interact(r, ui.id().with(("ctrl", cid.0, idx)), Sense::click_and_drag());
    if resp.is_pointer_button_down_on() || resp.clicked() {
        app.midi.active_lane = idx;
    }
    let merge = format!("ctrl:{}:{name}:{cc}", cid.0);
    if app.midi.tool == MidiTool::Line {
        line_in_lane(app, ui, &resp, cid, lane, r, tick_at, grid_ticks, &painter);
        return;
    }
    resp.context_menu(|ui| crate::context_menu::controller_lane_menu(app, ui, cid, name, cc, idx));
    if (resp.dragged() || resp.clicked())
        && let Some(p) = resp.interact_pointer_pos()
    {
        let tick = (tick_at(p.x).max(0) / grid_ticks) * grid_ticks;
        if ui.input(|i| i.modifiers.shift) {
            let from = app.midi.ctrl_last.map_or(tick, |l| l.0).min(tick);
            let to = app.midi.ctrl_last.map_or(tick, |l| l.0).max(tick) + grid_ticks;
            let _ = app.engine.execute_merged(
                "midi.ctrl_clear",
                &json!({"clip": cid.0, "lane": name, "cc": cc, "start_ticks": from, "end_ticks": to}),
                &merge,
            );
            app.midi.ctrl_last = Some((tick, 0));
        } else {
            let mut value = value_at(p.y, r, lo, hi);
            if lane == Lane::Sustain {
                value = if value > (lo + hi) / 2 { 127 } else { 0 };
            }
            // One point per grid step between the last point and this one, so fast drags leave no gaps.
            let (from, from_v) = app.midi.ctrl_last.unwrap_or((tick, value));
            let (a, b) = (from.min(tick), from.max(tick));
            let n = ((b - a) / grid_ticks).clamp(0, 2048);
            let points: Vec<_> = (0..=n)
                .map(|i| {
                    let tk = a + i * grid_ticks;
                    let f = if b == a { 1.0 } else { (tk - from) as f32 / (tick - from) as f32 };
                    let v = if lane == Lane::Sustain { value } else { from_v + ((value - from_v) as f32 * f.clamp(0.0, 1.0)).round() as i32 };
                    json!({"tick": tk, "value": v})
                })
                .collect();
            let _ = app.engine.execute_merged("midi.ctrl_set", &json!({"clip": cid.0, "lane": name, "cc": cc, "points": points}), &merge);
            app.midi.ctrl_last = Some((tick, value));
        }
    }
    if resp.drag_stopped() || (!resp.dragged() && !resp.clicked()) {
        app.midi.ctrl_last = None;
    }
    resp.on_hover_text("Drag: draw · Shift+drag: erase · right-click: clear lane");
}

/// The Line tool inside a controller lane: press, drag and release draws one straight ramp
/// (a single `midi.ctrl_set`, one undo step).
#[allow(clippy::too_many_arguments)]
fn line_in_lane(
    app: &mut SoundApp,
    ui: &mut Ui,
    resp: &egui::Response,
    cid: ClipId,
    lane: Lane,
    r: Rect,
    tick_at: &dyn Fn(f32) -> i64,
    grid_ticks: i64,
    painter: &egui::Painter,
) {
    let (name, cc, lo, hi) = lane.command();
    let snap = |x: f32| (tick_at(x).max(0) / grid_ticks) * grid_ticks;
    if resp.drag_started()
        && let Some(p) = ui.input(|i| i.pointer.press_origin()).or_else(|| resp.interact_pointer_pos())
    {
        app.midi.line = Some((p, lane, false));
    }
    if let Some((start, from_lane, false)) = app.midi.line
        && from_lane == lane
        && let Some(now) = ui.ctx().pointer_latest_pos()
    {
        painter.line_segment([start, pos2(now.x, now.y.clamp(r.min.y, r.max.y))], Stroke::new(2.0, Color32::WHITE));
        if resp.drag_stopped() {
            let (t0, t1) = (snap(start.x), snap(now.x));
            let pts = ramp_points(t0, value_at(start.y, r, lo, hi), t1, value_at(now.y, r, lo, hi), grid_ticks);
            let points: Vec<_> = pts.iter().map(|(tick, value)| json!({"tick": tick, "value": value})).collect();
            let _ = app.run("midi.ctrl_set", json!({"clip": cid.0, "lane": name, "cc": cc, "points": points}));
            app.midi.line = None;
        }
    }
    resp.clone().on_hover_text("Line tool: drag to draw a straight ramp");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn badge_shows_only_when_a_lane_with_points_owns_the_controller() {
        use soundcraft_model::{AutoParam, AutomationMode, ChannelFormat, Track, TrackId, TrackKind};
        let mut t = Track::new(TrackId(1), "I", TrackKind::Instrument, ChannelFormat::Stereo, [1, 2, 3]);
        assert!(!automated_by_track(&t, Lane::Expression), "no lane");
        t.lane_mut(&AutoParam::MidiCc(11));
        assert!(!automated_by_track(&t, Lane::Expression), "empty lane does not own");
        t.lane_mut(&AutoParam::MidiCc(11)).set_point(0, 5.0);
        assert!(automated_by_track(&t, Lane::Expression));
        assert!(!automated_by_track(&t, Lane::Modulation), "other controllers are unaffected");
        assert!(!automated_by_track(&t, Lane::Velocity));
        t.mixer.automation_mode = AutomationMode::Off;
        assert!(!automated_by_track(&t, Lane::Expression), "mode Off: clip data plays");
    }

    #[test]
    fn default_stack_is_velocity_only_and_add_remove_work() {
        let mut lanes = default_lanes();
        assert_eq!(lanes, vec![LaneCfg::new(Lane::Velocity)]);
        assert_eq!(next_unused(&lanes), Some(Lane::Modulation));
        assert!(add(&mut lanes, None));
        assert!(!add(&mut lanes, Some(Lane::Modulation)), "no duplicates");
        assert_eq!(lanes.len(), 2);
        while add(&mut lanes, None) {}
        assert_eq!(lanes.len(), Lane::ALL.len());
        assert!(remove(&mut lanes, 0));
        assert!(!remove(&mut lanes, 99));
        assert_eq!(next_unused(&lanes), Some(Lane::Velocity));
    }

    #[test]
    fn old_or_bad_stored_lanes_load() {
        assert_eq!(lanes_from_json(&json!(null)), default_lanes());
        assert_eq!(lanes_from_json(&json!("x")), default_lanes());
        let l = lanes_from_json(&json!([{"lane": "modulation"}, {"lane": "modulation", "height": 90.0}, {"lane": "velocity", "height": 5000.0}]));
        assert_eq!(l.len(), 2, "duplicates dropped");
        assert_eq!(l[0].height, LANE_H_DEFAULT, "missing height defaults");
        assert_eq!(l[1], LaneCfg { lane: Lane::Velocity, height: LANE_H_MAX });
        let round: Vec<LaneCfg> = serde_json::from_value(serde_json::to_value(&l).unwrap()).unwrap();
        assert_eq!(round, l);
    }

    #[test]
    fn layout_stacks_upwards_and_keeps_the_roll_tall_enough() {
        let lanes = vec![LaneCfg { lane: Lane::Velocity, height: 100.0 }, LaneCfg { lane: Lane::Modulation, height: 100.0 }];
        let (rects, roll_bottom) = layout(&lanes, 50.0, 800.0, 600.0, 500.0);
        assert!(rects[0].area.max.y <= rects[1].handle.min.y + 0.01, "lane 0 sits above lane 1's splitter");
        assert_eq!(rects[1].area.max.y, 598.0);
        assert!(roll_bottom - (600.0 - 500.0) >= ROLL_MIN_H - 0.01, "roll keeps 120 pt: {roll_bottom}");
        // Huge lanes shrink, never below the minimum, and the roll keeps its share.
        let big = vec![LaneCfg { lane: Lane::Velocity, height: 300.0 }; 3];
        let (rects, roll_bottom) = layout(&big, 50.0, 800.0, 600.0, 400.0);
        assert!(rects.iter().all(|r| r.area.height() >= LANE_H_MIN - 0.01));
        assert!(roll_bottom >= 200.0);
        let (rects, roll_bottom) = layout(&[], 0.0, 100.0, 300.0, 200.0);
        assert!(rects.is_empty());
        assert_eq!(roll_bottom, 298.0);
    }

    #[test]
    fn single_lane_clamp_matches_the_old_rules() {
        assert_eq!(clamp_lane_h(64.0, 600.0), 64.0);
        assert_eq!(clamp_lane_h(10.0, 600.0), 40.0);
        assert_eq!(clamp_lane_h(900.0, 600.0), 472.0);
        assert_eq!(clamp_lane_h(f32::NAN, 600.0), 64.0);
    }

    #[test]
    fn ramp_points_are_straight_and_ordered() {
        let p = ramp_points(0, 0, 960, 96, 240);
        assert_eq!(p, vec![(0, 0), (240, 24), (480, 48), (720, 72), (960, 96)]);
        assert_eq!(ramp_points(960, 96, 0, 0, 240), p, "ends in either order");
        assert_eq!(ramp_points(500, 10, 500, 90, 240), vec![(500, 90)]);
        assert!(ramp_points(0, 0, i64::MAX / 2, 127, 1).len() <= 2049, "capped");
    }

    #[test]
    fn value_at_spans_the_lane() {
        let r = Rect::from_min_max(pos2(0.0, 0.0), pos2(10.0, 104.0));
        assert_eq!(value_at(2.0, r, 0, 127), 127);
        assert_eq!(value_at(102.0, r, 0, 127), 0);
        assert_eq!(value_at(-50.0, r, 0, 127), 127);
        assert_eq!(value_at(52.0, r, -8192, 8191), 0);
    }
}
