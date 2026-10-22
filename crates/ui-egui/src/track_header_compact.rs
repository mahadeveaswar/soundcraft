//! The compact track header of the Edit window: a coloured strip, track number, M and S toggles,
//! the name, small round record, monitor, read, write and edit buttons, the instrument key button
//! and a volume slider with a dB readout. The header style is a preference (`ui.header_style`):
//! Compact is the default, Classic keeps the original header.
//!
//! The geometry is one pure function ([`layout`]) so that every height and width can be tested for
//! overlaps; secondary controls disappear when the header is too short or too narrow.

use crate::SoundApp;
use crate::edit_window::{HeaderKey, header_toggle};
use crate::theme::{Tokens, bold, rgb};
use crate::widgets::shade;
use egui::{Align2, Color32, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2};
use serde_json::{Value, json};
use soundcraft_model::{AutomationMode, Track, TrackId, TrackKind};

/// How track headers look in the Edit window (`ui.header_style {style}`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HeaderStyle {
    /// The original header: record, input, solo, mute, view selector, voice and automation mode.
    Classic,
    /// A clean row layout with a colour strip, M and S, name, small buttons and a volume slider.
    #[default]
    Compact,
}

impl HeaderStyle {
    pub const ALL: [Self; 2] = [Self::Compact, Self::Classic];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Compact => "Compact",
        }
    }

    /// The id used in `ui.header_style {"style": …}` and in the saved UI prefs.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Compact => "compact",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id().eq_ignore_ascii_case(s.trim()))
    }
}

/// `ui.header_style`: report or set the header style (no undo, not part of the session).
pub(crate) fn command(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    if let Some(name) = p.get("style").and_then(Value::as_str) {
        let Some(style) = HeaderStyle::parse(name) else {
            return Err(format!("unknown header style `{name}` (classic or compact)"));
        };
        app.ui.header_style = style;
    }
    Ok(json!({"style": app.ui.header_style.id()}))
}

/// View > Track Header: the two styles with a check mark on the current one.
pub(crate) fn view_menu(app: &mut SoundApp, ui: &mut Ui) {
    ui.menu_button("Track Header", |ui| {
        for style in HeaderStyle::ALL {
            let label = if app.ui.header_style == style { format!("\u{2714} {}", style.label()) } else { style.label().to_string() };
            if ui.button(label).clicked() {
                let _ = app.run("ui.header_style", json!({"style": style.id()}));
                ui.close();
            }
        }
        ui.separator();
        if ui.button("Reset Width").clicked() {
            let _ = app.run("ui.header_width", json!({"reset": true}));
            ui.close();
        }
    });
}

/// What the layout depends on besides the size.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeaderFlags {
    pub folder: bool,
    pub instrument: bool,
    /// The track records and monitors (has a playlist).
    pub records: bool,
    /// The track has solo and mute (every kind but the master).
    pub mixes: bool,
    /// Folders the track sits in: shifts the strip right.
    pub depth: usize,
}

/// The rectangles of a header's controls, relative to its top-left corner. `None`: hidden.
#[derive(Clone, Copy, Debug)]
pub struct HeaderLayout {
    pub strip: Rect,
    pub meter: Rect,
    pub fold: Option<Rect>,
    pub number: Option<Rect>,
    pub mute: Option<Rect>,
    pub solo: Option<Rect>,
    pub name: Option<Rect>,
    pub rec: Option<Rect>,
    pub monitor: Option<Rect>,
    pub read: Option<Rect>,
    pub write: Option<Rect>,
    pub edit: Option<Rect>,
    pub keys: Option<Rect>,
    pub volume: Option<Rect>,
}

impl Default for HeaderLayout {
    fn default() -> Self {
        Self {
            strip: Rect::ZERO,
            meter: Rect::ZERO,
            fold: None,
            number: None,
            mute: None,
            solo: None,
            name: None,
            rec: None,
            monitor: None,
            read: None,
            write: None,
            edit: None,
            keys: None,
            volume: None,
        }
    }
}

impl HeaderLayout {
    /// Every visible control rectangle (not the strip or the meter).
    pub fn controls(&self) -> Vec<(&'static str, Rect)> {
        [
            ("fold", self.fold),
            ("number", self.number),
            ("mute", self.mute),
            ("solo", self.solo),
            ("name", self.name),
            ("rec", self.rec),
            ("monitor", self.monitor),
            ("read", self.read),
            ("write", self.write),
            ("edit", self.edit),
            ("keys", self.keys),
            ("volume", self.volume),
        ]
        .into_iter()
        .filter_map(|(n, r)| r.map(|r| (n, r)))
        .collect()
    }
}

const STRIP: f32 = 4.0;
const GUIDE_STEP: f32 = 5.0;
const METER_W: f32 = 8.0;
const GAP: f32 = 2.0;
const NAME_MIN: f32 = 22.0;
const BUTTON: f32 = 15.0;
const BUTTON_GAP: f32 = 3.0;
/// Shortest header that shows the second row (small buttons) and the third row (volume slider).
const ROW2_MIN_H: f32 = 40.0;
const ROW3_MIN_H: f32 = 58.0;
/// Narrowest header that shows the volume slider.
const SLIDER_MIN_W: f32 = 60.0;

/// The geometry of a compact header of `size`.
pub fn layout(size: Vec2, f: &HeaderFlags) -> HeaderLayout {
    let (w, h) = (size.x, size.y);
    let at = |x: f32, y: f32, rw: f32, rh: f32| Rect::from_min_size(pos2(x, y), vec2(rw, rh));
    let strip_x = f.depth.min(6) as f32 * GUIDE_STEP;
    let mut out = HeaderLayout {
        strip: at(strip_x, 0.0, STRIP, h),
        meter: Rect::from_min_max(pos2(w - METER_W, 1.0), pos2(w, (h - 1.5).max(1.0))),
        ..HeaderLayout::default()
    };
    let x0 = strip_x + STRIP + 4.0;
    let x1 = w - METER_W - 3.0;
    let avail = x1 - x0;
    if avail <= 0.0 || h < 12.0 {
        return out;
    }
    // Row 1: fold triangle, number, M, S, name.
    let bh = if h >= 24.0 { 16.0 } else { (h - 4.0).clamp(8.0, 16.0) };
    let y1 = ((h - bh) * 0.5).clamp(0.0, 3.0);
    let fold_w = if f.folder { bh - 2.0 } else { 0.0 };
    let number_w = 14.0;
    let ms_w = if f.mixes { bh * 2.0 + GAP } else { 0.0 };
    let width_of = |fold: bool, number: bool| {
        let mut parts = 0.0;
        let mut count = 0;
        for (on, pw) in [(fold && f.folder, fold_w), (number, number_w), (f.mixes, ms_w)] {
            if on {
                parts += pw;
                count += 1;
            }
        }
        parts + GAP * count as f32
    };
    let mut keep = (true, true);
    if avail - width_of(keep.0, keep.1) < NAME_MIN {
        keep.1 = false;
    }
    if avail - width_of(keep.0, keep.1) < NAME_MIN {
        keep.0 = false;
    }
    let show_ms = f.mixes && avail >= ms_w;
    let mut x = x0;
    if keep.0 && f.folder {
        out.fold = Some(at(x, y1, fold_w, bh));
        x += fold_w + GAP;
    }
    if keep.1 {
        out.number = Some(at(x, y1, number_w, bh));
        x += number_w + GAP;
    }
    if show_ms {
        out.mute = Some(at(x, y1, bh, bh));
        out.solo = Some(at(x + bh + GAP, y1, bh, bh));
        x += ms_w + GAP;
    }
    if x1 - x >= NAME_MIN {
        out.name = Some(at(x, y1, x1 - x, bh));
    }
    // Row 2: record, monitor, read, write, edit and the instrument key button; the least
    // important go first when the width runs out.
    let y2 = y1 + bh + 3.0;
    if h >= ROW2_MIN_H {
        // (on-screen order, priority: lower stays longer)
        let mut items: Vec<(usize, u8)> = Vec::new();
        if f.records {
            items.push((0, 1));
            items.push((1, 3));
        }
        items.push((2, 4));
        items.push((3, 5));
        items.push((4, 6));
        if f.instrument {
            items.push((5, 2));
        }
        let fits = |n: usize| n as f32 * BUTTON + n.saturating_sub(1) as f32 * BUTTON_GAP <= avail;
        while !items.is_empty() && !fits(items.len()) {
            if let Some(worst) = items.iter().enumerate().max_by_key(|(_, (_, p))| *p).map(|(i, _)| i) {
                items.remove(worst);
            }
        }
        items.sort_by_key(|(slot, _)| *slot);
        for (n, (slot, _)) in items.iter().enumerate() {
            let r = Some(at(x0 + n as f32 * (BUTTON + BUTTON_GAP), y2, BUTTON, BUTTON));
            match slot {
                0 => out.rec = r,
                1 => out.monitor = r,
                2 => out.read = r,
                3 => out.write = r,
                4 => out.edit = r,
                _ => out.keys = r,
            }
        }
    }
    // Row 3: the volume slider.
    let y3 = y2 + BUTTON + 3.0;
    if h >= ROW3_MIN_H && avail >= SLIDER_MIN_W {
        out.volume = Some(at(x0, y3, avail, 14.0));
    }
    out
}

impl HeaderFlags {
    pub fn of(track: &Track, depth: usize) -> Self {
        Self {
            folder: track.is_folder(),
            instrument: track.kind == TrackKind::Instrument,
            records: track.kind.has_playlist(),
            mixes: track.kind != TrackKind::Master,
            depth,
        }
    }
}

/// Draws the compact header of `track` in `head` and handles its input.
pub(crate) fn show(app: &mut SoundApp, ui: &mut Ui, track: &Track, head: Rect, selected: bool, head_bg: Color32) {
    let t = Tokens::current();
    let id = track.id;
    crate::edit_window::header_resize_edge(app, ui, track, head);
    // Click on the background selects the track; the context menu is the classic one.
    let bg = ui.interact(head, ui.id().with(("hdr", id.0)), Sense::click());
    if bg.clicked() {
        crate::edit_window::click_track(app, id, ui.input(|i| i.modifiers), crate::edit_window::in_edit_window);
    }
    bg.context_menu(|ui| crate::edit_window::track_context_menu(app, ui, id));

    let depth = crate::edit_window::folder_depth(app.engine.session(), track);
    let flags = HeaderFlags::of(track, depth);
    let lay = layout(head.size(), &flags);
    let abs = |r: Rect| r.translate(head.min.to_vec2());
    let p = ui.painter().clone();
    // Cover the classic strip, then draw ours.
    crate::edit_window::header_background(&p, head, head_bg);
    p.hline(head.x_range(), head.min.y + 0.5, Stroke::new(1.0, t.edge_light));
    crate::edit_window::header_strip(&p, abs(lay.strip), track.color);
    crate::folder_ui::draw_child_guides(&p, head, app.engine.session(), track);

    let strip_col = rgb(track.color);
    if let Some(r) = lay.fold.map(abs) {
        let resp = ui.interact(r, ui.id().with(("cfold", id.0)), Sense::click());
        let plate = shade(strip_col, if resp.hovered() { 0.5 } else { 0.35 });
        crate::folder_ui::draw_fold_triangle(&p, r, track.folder_open, plate, shade(strip_col, -0.8));
        if resp.on_hover_text(if track.folder_open { "Close folder" } else { "Open folder" }).clicked() {
            let _ = app.run("track.folder_toggle", json!({"track": id.0}));
        }
    }
    if let Some(r) = lay.number.map(abs) {
        let n = app.engine.session().track_index(id).map_or(0, |i| i + 1);
        p.text(r.center(), Align2::CENTER_CENTER, n.to_string(), bold(10.5), t.text_dim);
    }
    let m = &track.mixer;
    let implied = !m.solo && app.engine.session().tracks.iter().any(|t| t.mixer.solo);
    for (rect, key, lit, implied, cmd) in
        [(lay.mute, HeaderKey::Mute, m.mute, implied, "mix.mute"), (lay.solo, HeaderKey::Solo, m.solo, false, "mix.solo")]
    {
        if let Some(r) = rect.map(abs)
            && header_toggle(ui, r, id, key, lit, implied).clicked()
        {
            let _ = app.run(cmd, json!({"track": id.0}));
        }
    }
    if let Some(name_r) = lay.name.map(abs) {
        name_field(app, ui, track, name_r, selected);
    }
    // Small buttons.
    let btn = |ui: &Ui, r: Rect, key: &str| ui.interact(r, ui.id().with((key, id.0)), Sense::click());
    if let Some(r) = lay.rec.map(abs) {
        let resp = btn(ui, r, "crec");
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), m.record_arm, "Record enable"));
        round_face(&p, r, m.record_arm, t.rec, head_bg, resp.hovered());
        let ink = if m.record_arm { Color32::WHITE } else { t.rec.gamma_multiply(0.8) };
        crate::icons::draw(&p, r.shrink(1.5), "track_record", ink);
        if resp.on_hover_text("Record enable").clicked() {
            let _ = app.run("mix.record_arm", json!({"track": id.0}));
        }
    }
    if let Some(r) = lay.monitor.map(abs) {
        let resp = btn(ui, r, "cmon");
        resp.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, ui.is_enabled(), m.input_monitor, "Input monitoring"));
        round_face(&p, r, m.input_monitor, t.input, head_bg, resp.hovered());
        crate::icons::draw(&p, r.shrink(2.0), "track_monitor", if m.input_monitor { t.text_dark } else { t.text_dim });
        if resp.on_hover_text("Input monitoring").clicked() {
            let _ = app.run("mix.input_monitor", json!({"track": id.0}));
        }
    }
    for (rect, label, mode, col, key) in
        [(lay.read, "R", AutomationMode::Read, t.auto_read, "cread"), (lay.write, "W", AutomationMode::Write, t.auto_write, "cwrite")]
    {
        let Some(r) = rect.map(abs) else { continue };
        let lit = m.automation_mode == mode;
        let resp = btn(ui, r, key);
        let face = lit_face(head_bg, lit, col, resp.hovered());
        p.rect_filled(r, 4.0, face);
        p.text(r.center(), Align2::CENTER_CENTER, label, bold(10.0), if lit { t.text_dark } else { t.text });
        let tip = if mode == AutomationMode::Read { "Automation read" } else { "Automation write" };
        if resp.on_hover_text(tip).clicked() {
            let next = if lit { AutomationMode::Off } else { mode };
            let _ = app.run("mix.automation_mode", json!({"track": id.0, "mode": next.label()}));
        }
    }
    if let Some(r) = lay.edit.map(abs) {
        let target = edit_target(track);
        let resp = btn(ui, r, "cedit");
        round_face(&p, r, false, t.accent, head_bg, resp.hovered() && target.is_some());
        crate::icons::draw(&p, r.shrink(2.0), "track_edit", if target.is_some() { t.text } else { t.text_dim });
        let tip = if target.is_some() { "Edit plugin" } else { "No plugin to edit" };
        if resp.on_hover_text(tip).clicked()
            && let Some(slot) = target
            && !app.ui.plugin_windows.contains(&(id, slot))
        {
            app.ui.plugin_windows.push((id, slot));
        }
    }
    if let Some(r) = lay.keys.map(abs) {
        let resp = btn(ui, r, "ckeys");
        let loaded = track.instrument.is_some();
        crate::widgets::raised_key(&p, r, t.slot_face, resp.hovered() && loaded);
        crate::icons::draw(&p, r.shrink(1.0), "keyboard", if loaded { t.text } else { t.text_dim });
        let tip = if loaded { "Open instrument" } else { "No instrument loaded" };
        if resp.on_hover_text(tip).clicked() && loaded {
            let slot = soundcraft_mix::INSTRUMENT_SLOT;
            if !app.ui.plugin_windows.contains(&(id, slot)) {
                app.ui.plugin_windows.push((id, slot));
            }
        }
    }
    if let Some(r) = lay.volume.map(abs) {
        volume_slider(app, ui, track, r, head_bg);
    }
    let md = app.meters.get(&id).copied().unwrap_or_default();
    crate::edit_window::header_meter(ui, abs(lay.meter), &md, track.channels() >= 2);
}

/// The plugin slot the Edit button opens: the instrument, else the first insert in use.
fn edit_target(track: &Track) -> Option<usize> {
    if track.instrument.is_some() {
        return Some(soundcraft_mix::INSTRUMENT_SLOT);
    }
    track.mixer.inserts.iter().position(Option::is_some)
}

/// The name: click selects, double-click renames, drag reorders; the track context menu on right-click.
fn name_field(app: &mut SoundApp, ui: &mut Ui, track: &Track, r: Rect, selected: bool) {
    let t = Tokens::current();
    let id = track.id;
    if selected {
        ui.painter().rect_filled(r, 2.0, t.name_field_sel);
    }
    let color = if selected { t.text_dark } else { t.text.lerp_to_gamma(t.text_dim, 0.4) };
    ui.painter().with_clip_rect(r).text(pos2(r.min.x + 3.0, r.center().y), Align2::LEFT_CENTER, &track.name, bold(12.0), color);
    let resp = ui.interact(r, ui.id().with(("name", id.0)), Sense::click_and_drag());
    if resp.double_clicked() {
        app.dialogs.open_rename_track(id, &track.name);
    } else if resp.clicked() {
        crate::edit_window::click_track(app, id, ui.input(|i| i.modifiers), crate::edit_window::in_edit_window);
    }
    resp.context_menu(|ui| crate::edit_window::track_context_menu(app, ui, id));
    crate::edit_window::name_drag_reorder(app, ui, r, id, &resp);
}

fn lit_face(head_bg: Color32, lit: bool, color: Color32, hovered: bool) -> Color32 {
    let base = if lit { color } else { shade(head_bg, -0.35) };
    if hovered { shade(base, 0.12) } else { base }
}

/// A small round button face: lit in `color`, else a dark well.
fn round_face(p: &egui::Painter, r: Rect, lit: bool, color: Color32, head_bg: Color32, hovered: bool) {
    let face = if lit { color } else { shade(head_bg, -0.35) };
    p.circle_filled(r.center(), r.width() * 0.5, if hovered { shade(face, 0.12) } else { face });
    p.circle_stroke(r.center(), r.width() * 0.5, Stroke::new(1.0, shade(head_bg, -0.55)));
}

/// Slider position (0..1) to the dB it stands for, and back; the mixer fader law.
pub(crate) fn slider_db(pos: f32) -> f32 {
    soundcraft_model::fader_pos_to_db(pos.clamp(0.0, 1.0)).clamp(-144.0, 12.0)
}

pub(crate) fn slider_pos(db: f32) -> f32 {
    soundcraft_model::fader_db_to_pos(db)
}

/// The readout text beside the slider.
pub(crate) fn readout(db: f32) -> String {
    if db <= -143.9 { "-inf dB".into() } else { format!("{db:.2} dB") }
}

const READOUT_W: f32 = 46.0;

/// A horizontal volume slider with a dB readout. Drag moves it (one undo step per drag), a
/// double-click returns to 0 dB. The readout needs room; a narrow header gets the slider alone.
fn volume_slider(app: &mut SoundApp, ui: &mut Ui, track: &Track, r: Rect, head_bg: Color32) {
    let t = Tokens::current();
    let id = track.id;
    let db = track.mixer.volume_db;
    let readout_w = if r.width() >= 100.0 { READOUT_W } else { 0.0 };
    let groove_r = Rect::from_min_max(r.min, pos2(r.max.x - readout_w, r.max.y));
    let (a, b) = (groove_r.min.x + 7.0, groove_r.max.x - 7.0);
    let resp = ui.interact(groove_r, ui.id().with(("cvol", id.0)), Sense::click_and_drag());
    let p = ui.painter();
    let y = groove_r.center().y;
    let pos = slider_pos(db);
    let knob_x = a + (b - a) * pos;
    p.rect_filled(Rect::from_min_max(pos2(a, y - 2.0), pos2(b, y + 2.0)), 2.0, shade(head_bg, -0.5));
    p.rect_filled(Rect::from_min_max(pos2(a, y - 2.0), pos2(knob_x, y + 2.0)), 2.0, t.text_dim);
    let unity = a + (b - a) * slider_pos(0.0);
    p.vline(unity, y - 4.0..=y + 4.0, Stroke::new(1.0, t.text_dim.gamma_multiply(0.7)));
    let hot = resp.hovered() || resp.dragged();
    p.circle_filled(pos2(knob_x, y), 5.5, if hot { Color32::WHITE } else { t.text });
    p.circle_stroke(pos2(knob_x, y), 5.5, Stroke::new(1.0, shade(head_bg, -0.6)));
    if readout_w > 0.0 {
        let rr = Rect::from_min_max(pos2(groove_r.max.x + 2.0, r.min.y), r.max);
        p.rect_filled(rr, 2.0, shade(head_bg, -0.35));
        p.with_clip_rect(rr).text(rr.center(), Align2::CENTER_CENTER, readout(db), crate::theme::regular(9.5), t.text);
    }
    if resp.double_clicked() {
        let _ = app.engine.execute_merged("mix.volume", &json!({"track": id.0, "db": 0.0}), &fader_key(id));
        app.engine.end_merge();
    } else if resp.dragged() && b > a {
        let fine = ui.input(|i| i.modifiers.command || i.modifiers.ctrl);
        let dx = resp.drag_delta().x / (b - a) * if fine { 0.1 } else { 1.0 };
        let new = slider_db(pos + dx);
        app.gesture = Some(crate::Gesture::Fader { track: id, start_db: db });
        let _ = app.engine.execute_merged("mix.volume", &json!({"track": id.0, "db": new}), &fader_key(id));
    }
    resp.on_hover_text("Volume (double-click for 0 dB)");
}

fn fader_key(id: TrackId) -> String {
    format!("fader:{}", id.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: Rect, b: Rect) -> bool {
        a.intersect(b).width() > 0.01 && a.intersect(b).height() > 0.01
    }

    #[test]
    fn style_ids_round_trip_and_compact_is_the_default() {
        assert_eq!(HeaderStyle::default(), HeaderStyle::Compact);
        for s in HeaderStyle::ALL {
            assert_eq!(HeaderStyle::parse(s.id()), Some(s));
            let v = serde_json::to_value(s).unwrap_or_default();
            assert_eq!(serde_json::from_value::<HeaderStyle>(v).ok(), Some(s));
        }
        assert_eq!(HeaderStyle::parse(" Classic "), Some(HeaderStyle::Classic));
        assert_eq!(HeaderStyle::parse("tall"), None);
    }

    #[test]
    fn controls_never_overlap_and_stay_inside_the_header() {
        for kind in 0..4 {
            for depth in [0usize, 1, 3, 6] {
                let flags = HeaderFlags { folder: kind == 1, instrument: kind == 2, records: kind != 1 && kind != 3, mixes: kind != 3, depth };
                for w in [60.0f32, 90.0, 110.0, 136.0, 160.0, 200.0, 320.0, 480.0] {
                    for h in [12.0f32, 16.0, 20.0, 24.0, 30.0, 39.0, 40.0, 46.0, 57.0, 58.0, 70.0, 88.0, 132.0, 220.0, 440.0] {
                        let lay = layout(vec2(w, h), &flags);
                        let bounds = Rect::from_min_size(pos2(0.0, 0.0), vec2(w, h));
                        let controls = lay.controls();
                        for (n, r) in &controls {
                            assert!(r.width() > 0.0 && r.height() > 0.0, "{n} empty at {w}x{h}");
                            assert!(bounds.contains_rect(*r), "{n} {r:?} outside {w}x{h} kind {kind} depth {depth}");
                            assert!(!overlaps(*r, lay.meter), "{n} overlaps the meter at {w}x{h}");
                            assert!(!overlaps(*r, lay.strip), "{n} overlaps the strip at {w}x{h} depth {depth}");
                        }
                        for (i, (na, a)) in controls.iter().enumerate() {
                            for (nb, b) in controls.iter().skip(i + 1) {
                                assert!(!overlaps(*a, *b), "{na} and {nb} overlap at {w}x{h} kind {kind} depth {depth}");
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn secondary_controls_appear_with_height() {
        let flags = HeaderFlags { folder: false, instrument: true, records: true, mixes: true, depth: 0 };
        let micro = layout(vec2(136.0, 16.0), &flags);
        assert!(micro.mute.is_some() && micro.solo.is_some() && micro.name.is_some());
        assert!(micro.rec.is_none() && micro.volume.is_none());
        let small = layout(vec2(136.0, 46.0), &flags);
        assert!(small.rec.is_some() && small.keys.is_some() && small.edit.is_some() && small.volume.is_none());
        let medium = layout(vec2(136.0, 88.0), &flags);
        assert!(medium.volume.is_some() && medium.rec.is_some() && medium.read.is_some() && medium.write.is_some());
        assert!(medium.monitor.is_some());
    }

    #[test]
    fn narrow_headers_keep_the_instrument_key_button_longest() {
        let flags = HeaderFlags { folder: false, instrument: true, records: true, mixes: true, depth: 0 };
        let lay = layout(vec2(60.0, 88.0), &flags);
        assert!(lay.keys.is_some(), "the keyboard button is kept");
        assert!(lay.edit.is_none(), "the edit button goes first");
    }

    #[test]
    fn volume_slider_mapping_round_trips() {
        for db in [-60.0f32, -20.0, -6.0, 0.0, 6.0, 12.0] {
            assert!((slider_db(slider_pos(db)) - db).abs() < 0.05, "{db}");
        }
        assert_eq!(slider_db(2.0), 12.0);
        assert!(slider_db(-1.0) <= -143.0);
        assert_eq!(readout(0.0), "0.00 dB");
        assert_eq!(readout(-150.0), "-inf dB");
    }
}
