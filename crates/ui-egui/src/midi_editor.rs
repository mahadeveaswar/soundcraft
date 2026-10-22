//! The MIDI Editor: a piano roll with a velocity lane, docked under the Edit window.

use crate::SoundApp;
use crate::theme::{Tokens, bold, regular};
use egui::{Align2, Color32, Id, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use serde_json::json;
use soundcraft_model::{ClipContent, ClipId, Tool};
use soundcraft_time::TICKS_PER_QUARTER;

const KEY_W: f32 = 48.0;
const DEFAULT_ROW_H: f32 = 9.0;
const ROW_H_RANGE: (f32, f32) = (6.0, 32.0);
const ROW_H_STEP: f32 = 2.0;
const ZOOM_STEP: f32 = 1.25;

/// Horizontal zoom step: scale pixels per tick by `factor`, within `[min, max]`.
pub fn zoom_ppt(ppt: f32, factor: f32, min: f32, max: f32) -> f32 {
    (ppt * factor).clamp(min, max.max(min))
}

/// Vertical zoom step: change the row height by `delta` pixels, within the allowed range.
pub fn step_row_h(row_h: f32, delta: f32) -> f32 {
    (row_h + delta).clamp(ROW_H_RANGE.0, ROW_H_RANGE.1)
}

pub use crate::midi_lanes::{LANE_H_DEFAULT, Lane, LaneCfg, clamp_lane_h};

/// Parts of the editor header, in the order they are given up as the panel narrows (last first).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HeaderItem {
    Title,
    Tools,
    Glyphs,
    LoopFollow,
    Audition,
    Zoom,
    Ops,
    Inspector,
    Snap,
    Clip,
    Overflow,
    Label,
}

const TITLE_W: f32 = 90.0;
const TOOLS_W: f32 = 232.0;
const GLYPHS_W: f32 = 67.0;
const LOOP_W: f32 = 106.0;
const AUDITION_W: f32 = 70.0;
const ZOOM_W: f32 = 160.0;
const OPS_W: f32 = 296.0;
const SNAP_W: f32 = 120.0;
const CLIP_W: f32 = 140.0;
const OVERFLOW_W: f32 = 26.0;
const INSPECTOR_BTN_W: f32 = 74.0;
const LABEL_W: f32 = 190.0;
const HEADER_GAP: f32 = 6.0;
/// Width at which every header group is visible. The floating panel opens at this width when the
/// window is wide enough, else at the window width less a margin (see [`float_rect`]).
pub const FLOAT_W: f32 = 1460.0;

/// Where the panel floats by default: `[x, y, width, height]`, kept inside a window of `screen`.
pub fn float_rect(screen: Rect) -> [f32; 4] {
    let margin = 16.0;
    let w = FLOAT_W.min((screen.width() - 2.0 * margin).max(360.0));
    let x = (screen.width() - w - margin).clamp(margin, 160.0);
    let h = 380.0_f32.min((screen.height() - 140.0).max(200.0));
    [x, 120.0_f32.min((screen.height() - h - margin).max(0.0)), w, h]
}

/// Where each visible header item goes (left to right, never overlapping) for a header rect.
#[derive(Debug, Clone, Default)]
pub struct HeaderLayout {
    pub items: Vec<(HeaderItem, Rect)>,
}

impl HeaderLayout {
    pub fn rect(&self, item: HeaderItem) -> Option<Rect> {
        self.items.iter().find(|(i, _)| *i == item).map(|(_, r)| *r)
    }

    pub fn visible(&self, item: HeaderItem) -> bool {
        self.rect(item).is_some()
    }
}

/// Lay the header out for its width. Priority (kept longest first): title, tool palette, the panel
/// glyphs, then Loop/Follow, Fit and zoom, then the clip operations; the clip label goes first.
/// Groups that do not fit are listed in the overflow menu (the `Overflow` item appears instead).
pub fn header_layout(header: Rect, has_clip: bool) -> HeaderLayout {
    let (y0, y1) = (header.min.y + 1.0, header.max.y - 1.0);
    let at = |x0: f32, x1: f32| Rect::from_min_max(pos2(x0, y0), pos2(x1.max(x0), y1));
    let mut items = Vec::new();
    let glyph_left = (header.max.x - GLYPHS_W).max(header.min.x);
    let title_end = (header.min.x + 8.0 + TITLE_W).min(glyph_left);
    items.push((HeaderItem::Title, at(header.min.x + 8.0, title_end)));
    items.push((HeaderItem::Glyphs, at(glyph_left, header.max.x)));
    if has_clip {
        let tools_start = title_end + HEADER_GAP;
        // The overflow button keeps its own rect: the palette is what gets truncated.
        let tools_limit = glyph_left - HEADER_GAP - OVERFLOW_W - HEADER_GAP;
        let tools_end = (tools_start + TOOLS_W).min(tools_limit).max(tools_start);
        items.push((HeaderItem::Tools, at(tools_start, tools_end)));
        let free = (glyph_left - HEADER_GAP - tools_end - HEADER_GAP).max(0.0);
        // Kept longest first (priority), and where each sits left to right (display order).
        let groups = [
            (HeaderItem::Inspector, INSPECTOR_BTN_W),
            (HeaderItem::Snap, SNAP_W),
            (HeaderItem::LoopFollow, LOOP_W),
            (HeaderItem::Audition, AUDITION_W),
            (HeaderItem::Zoom, ZOOM_W),
            (HeaderItem::Clip, CLIP_W),
            (HeaderItem::Ops, OPS_W),
        ];
        let display = [
            HeaderItem::Inspector,
            HeaderItem::Snap,
            HeaderItem::Clip,
            HeaderItem::Ops,
            HeaderItem::Zoom,
            HeaderItem::Audition,
            HeaderItem::LoopFollow,
        ];
        let need = |k: usize| -> f32 {
            let sum: f32 = groups.iter().take(k).map(|g| g.1 + HEADER_GAP).sum();
            if k < groups.len() { sum + OVERFLOW_W + HEADER_GAP } else { sum }
        };
        let shown = (0..=groups.len()).rev().find(|k| need(*k) <= free).unwrap_or(0);
        let kept: Vec<HeaderItem> = groups.iter().take(shown).map(|g| g.0).collect();
        // Right to left from the glyphs: overflow button, then the kept groups in reverse display order.
        let mut right = glyph_left - HEADER_GAP;
        if shown < groups.len() {
            items.push((HeaderItem::Overflow, at(right - OVERFLOW_W, right)));
            right -= OVERFLOW_W + HEADER_GAP;
        }
        for item in display.iter().rev().filter(|i| kept.contains(i)) {
            let w = groups.iter().find(|g| g.0 == *item).map_or(0.0, |g| g.1);
            items.push((*item, at(right - w, right)));
            right -= w + HEADER_GAP;
        }
        // The clip label only when everything else fits and there is room left for it.
        if shown == groups.len() && right - tools_end - HEADER_GAP >= LABEL_W {
            items.push((HeaderItem::Label, at(tools_end + HEADER_GAP, right)));
        }
    }
    HeaderLayout { items }
}

/// The header layout for this frame, remembered so tests can read it.
fn app_header_layout(ui: &Ui, header: Rect, has_clip: bool) -> HeaderLayout {
    let layout = header_layout(header, has_clip);
    ui.ctx().data_mut(|d| d.insert_temp(Id::new("soundcraft-midi-header"), layout.items.clone()));
    layout
}

/// A child `Ui` limited to one header rect (clipped so nothing paints over a neighbour).
fn header_child(ui: &mut Ui, r: Rect) -> Ui {
    let mut c = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::left_to_right(egui::Align::Center)));
    c.set_clip_rect(r.intersect(ui.clip_rect()));
    c.spacing_mut().item_spacing.x = 4.0;
    c
}

/// The button that opens or closes the inspector strip.
fn inspector_toggle(app: &mut SoundApp, ui: &mut Ui) {
    if crate::midi_editor_toolbar::toggle_label(ui, app.midi.inspector_open, "Inspector")
        .on_hover_text("Show or hide the inspector strip (quantize, transpose, length, velocity, scale, chords)")
        .clicked()
    {
        app.midi.inspector_open = !app.midi.inspector_open;
    }
}

/// Loop and Follow buttons.
fn loop_follow_group(app: &mut SoundApp, ui: &mut Ui, cid: ClipId) {
    let looping = app.engine.session().edit.loop_playback;
    if crate::midi_editor_toolbar::toggle_label(ui, looping, "Loop")
        .on_hover_text("Loop this clip: selects the clip and turns loop playback on. Press Space to play it.")
        .clicked()
    {
        if looping {
            let _ = app.run("options.loop_playback", json!({"value": false}));
        } else {
            let _ = app.run("edit.select", json!({"clips": [cid.0]}));
            let _ = app.run("options.loop_playback", json!({"value": true}));
        }
    }
    if crate::midi_editor_toolbar::toggle_label(ui, !app.midi.no_follow, "Follow").on_hover_text("Scroll the view with the playhead").clicked() {
        app.midi.no_follow = !app.midi.no_follow;
    }
}

/// The Audition toggle: play notes as they are clicked, drawn or moved.
fn audition_group(app: &mut SoundApp, ui: &mut Ui) {
    if crate::midi_editor_toolbar::toggle_label(ui, app.ui.audition, "Audition").on_hover_text("Play notes as you click, draw or move them").clicked()
    {
        app.ui.audition = !app.ui.audition;
        if !app.ui.audition {
            crate::midi_audition::stop(app);
        }
    }
}

/// Fit and the four zoom buttons.
fn zoom_group(app: &mut SoundApp, ui: &mut Ui) {
    if ui.small_button("Fit").on_hover_text("Zoom to show the whole clip").clicked() {
        app.midi.ppt = None;
        app.midi.view_start = 0.0;
    }
    if ui.small_button("H-").on_hover_text("Zoom out horizontally").clicked() {
        app.midi.zoom_h = Some(1.0 / ZOOM_STEP);
    }
    if ui.small_button("H+").on_hover_text("Zoom in horizontally").clicked() {
        app.midi.zoom_h = Some(ZOOM_STEP);
    }
    if ui.small_button("V-").on_hover_text("Shorter piano-roll rows").clicked() {
        app.midi.row_h = step_row_h(app.midi.row_h, -ROW_H_STEP);
    }
    if ui.small_button("V+").on_hover_text("Taller piano-roll rows").clicked() {
        app.midi.row_h = step_row_h(app.midi.row_h, ROW_H_STEP);
    }
}

/// Whole-clip operations; returns the command to run.
fn ops_group(app: &mut SoundApp, ui: &mut Ui, grid_ticks: i64) -> Option<(&'static str, serde_json::Value)> {
    let mut op = None;
    if ui.small_button("Quantize").on_hover_text("Quantize to the grid").clicked() {
        op = Some(("event.quantize", json!({"grid": grid_ticks, "strength": app.midi.quant_strength})));
    }
    crate::midi_editor_toolbar::strength_field(app, ui);
    if ui.small_button("Vel +10").clicked() {
        op = Some(("event.change_velocity", json!({"add": 10})));
    }
    if ui.small_button("Vel −10").clicked() {
        op = Some(("event.change_velocity", json!({"add": -10})));
    }
    if ui.small_button("Legato").clicked() {
        op = Some(("event.change_duration", json!({"legato": 0})));
    }
    op
}

#[derive(Debug, Clone)]
pub struct MidiEditorState {
    pub selected: Vec<usize>,
    pub drag: Option<NoteDrag>,
    pub scroll: f32,
    pub clip: Option<ClipId>,
    /// Rubber-band selection start (screen position).
    pub band: Option<egui::Pos2>,
    /// The controller lanes under the piano roll, top to bottom.
    pub lanes: Vec<LaneCfg>,
    /// The lane last used (target of the Line tool in the roll).
    pub active_lane: usize,
    /// Press point, lane and whether it started in the roll, of a Line tool drag in progress.
    pub line: Option<(egui::Pos2, Lane, bool)>,
    /// The left inspector strip is open.
    pub inspector_open: bool,
    /// Values of the inspector strip and the note-entry assists.
    pub insp: crate::midi_editor_inspector::InspectorState,
    /// Last point written while drawing in a controller lane: (tick, value).
    pub ctrl_last: Option<(i64, i32)>,
    /// Horizontal zoom in pixels per tick; `None` fits the whole clip to the panel.
    pub ppt: Option<f32>,
    /// First visible tick, relative to the clip start.
    pub view_start: f64,
    /// Stop scrolling the view with the playhead.
    pub no_follow: bool,
    /// The user scrolled vertically, so stop re-centring on the notes.
    pub v_manual: bool,
    /// The active editing tool.
    pub tool: MidiTool,
    /// A note being drawn with the pencil: (start tick, pitch).
    pub pencil: Option<(i64, i32)>,
    /// Where the playhead was placed in the editor and the selection it was placed in. Space plays
    /// from there while the selection is unchanged.
    pub play_from: Option<(i64, soundcraft_time::Range)>,
    /// Row height of the piano roll in pixels (vertical zoom).
    pub row_h: f32,
    /// Pending horizontal zoom factor from the header buttons, applied once the view is known.
    pub zoom_h: Option<f32>,
    /// The panel is collapsed to its header.
    pub minimized: bool,
    /// What notes snap to in this editor (default: follow the session grid).
    pub snap: crate::midi_editor_state::MidiSnap,
    /// Quantize strength 0..=100 used by the header Quantize button.
    pub quant_strength: u8,
    /// The last rejected entry in the info strip.
    pub info_error: Option<String>,
    /// The panel is currently a floating panel (set by the dock before each frame).
    pub floating: bool,
    /// A docking operation requested by the header controls; run by the dock after the frame.
    pub dock_request: Option<serde_json::Value>,
    /// The note being auditioned (see `midi_audition`).
    pub audition: crate::midi_audition::Audition,
    /// The frame in which the editor last drew, so a closed editor releases its audition note.
    pub shown_frame: Option<u64>,
}

impl Default for MidiEditorState {
    fn default() -> Self {
        Self {
            selected: Vec::new(),
            drag: None,
            scroll: 0.0,
            clip: None,
            band: None,
            lanes: crate::midi_lanes::default_lanes(),
            active_lane: 0,
            line: None,
            inspector_open: false,
            insp: crate::midi_editor_inspector::InspectorState::default(),
            ctrl_last: None,
            ppt: None,
            view_start: 0.0,
            no_follow: false,
            v_manual: false,
            tool: MidiTool::default(),
            pencil: None,
            play_from: None,
            row_h: DEFAULT_ROW_H,
            zoom_h: None,
            minimized: false,
            snap: crate::midi_editor_state::MidiSnap::default(),
            quant_strength: 100,
            info_error: None,
            floating: false,
            dock_request: None,
            audition: crate::midi_audition::Audition::default(),
            shown_frame: None,
        }
    }
}

/// Editing tools of the piano roll.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MidiTool {
    /// Click to select, drag to move or resize, rubber-band on empty space, double-click to add.
    #[default]
    Select,
    /// Click or drag to draw notes.
    Pencil,
    /// Click or drag across notes to delete them.
    Erase,
    /// Click a note to cut it in two at the grid line.
    Split,
    /// Click a note to join it with the next note of the same pitch.
    Glue,
    /// Click a note to mute or unmute it.
    Mute,
    /// Click to zoom in, Alt-click to zoom out.
    Zoom,
    /// Drag in a controller lane to draw a straight ramp.
    Line,
}

/// Move the playhead from the ruler: an explicit locate, remembered so Space plays from there.
fn place_playhead(app: &mut SoundApp, samples: i64) {
    let _ = app.run("transport.locate", json!({"at": samples}));
    mark_play_from(app, samples);
}

/// Remember where Space should start, without touching the session (a click in the roll must not
/// change the document).
fn mark_play_from(app: &mut SoundApp, samples: i64) {
    app.midi.play_from = Some((samples, app.engine.session().edit.selection));
}

/// Where a position placed in the editor still applies: the selection is unchanged and, unless it
/// is just a point, the position is inside it.
pub fn placed_position(play_from: Option<(i64, soundcraft_time::Range)>, sel: soundcraft_time::Range) -> Option<i64> {
    play_from.filter(|(at, placed_in)| *placed_in == sel && (sel.is_empty() || sel.contains(*at))).map(|(at, _)| at)
}

use crate::midi_ruler::RULER_H;
/// Width of the right-edge column that holds the vertical zoom slider.
const VZOOM_W: f32 = 18.0;
/// Height of the bottom strip that holds the horizontal zoom slider.
const HZOOM_H: f32 = 16.0;

#[derive(Debug, Clone)]
pub struct NoteDrag {
    pub index: usize,
    pub resize: bool,
    pub start: egui::Pos2,
    pub dx_ticks: i64,
    pub dy_semi: i64,
}

/// The MIDI clip being edited: the first selected MIDI clip, else a MIDI clip under the insertion
/// point on a selected track.
pub fn target_clip(app: &SoundApp) -> Option<ClipId> {
    let s = app.engine.session();
    for id in &s.edit.selected_clips {
        if let Some((_, c)) = s.find_clip(*id)
            && matches!(c.content, ClipContent::Midi { .. })
        {
            return Some(*id);
        }
    }
    let at = s.edit.selection.start;
    s.tracks
        .iter()
        .filter(|t| s.edit.selected_tracks.contains(&t.id) && t.kind.is_midi())
        .flat_map(|t| t.clips().iter())
        .find(|c| c.range().contains(at) || c.range().overlaps(&s.edit.selection))
        .or_else(|| s.tracks.iter().filter(|t| s.edit.selected_tracks.contains(&t.id) && t.kind.is_midi()).flat_map(|t| t.clips().iter()).next())
        .map(|c| c.id)
}

fn is_black(p: i32) -> bool {
    matches!(p.rem_euclid(12), 1 | 3 | 6 | 8 | 10)
}

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    // Painter-only contents do not advance the layout. Claim the resized
    // space, including the empty state, so the dock keeps its chosen height.
    ui.take_available_space();
    app.midi.shown_frame = Some(ui.ctx().cumulative_frame_nr());
    let t = Tokens::current();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.panel_bg);
    let header = Rect::from_min_size(full.min, vec2(full.width(), 22.0));
    ui.painter().rect_filled(header, 0.0, t.panel_bg2);
    ui.painter().text(pos2(header.min.x + 8.0, header.center().y), Align2::LEFT_CENTER, "MIDI EDITOR", bold(11.5), t.header_text);
    let has_clip = !app.midi.minimized && target_clip(app).is_some();
    let layout = app_header_layout(ui, header, has_clip);
    panel_controls(app, ui, layout.rect(HeaderItem::Glyphs).unwrap_or(header));
    if app.midi.minimized {
        return;
    }
    let Some(cid) = target_clip(app) else {
        let track = {
            let s = app.engine.session();
            s.tracks.iter().find(|t| t.kind.is_midi() && s.edit.selected_tracks.contains(&t.id)).map(|t| (t.id, t.name.clone()))
        };
        let msg = match &track {
            Some((_, name)) => format!("Click to add a MIDI clip to {name} at the insertion point."),
            None => "Select a MIDI clip (or a MIDI track) to edit its notes.".into(),
        };
        ui.painter().text(full.center(), Align2::CENTER_CENTER, msg, regular(13.0), t.text_dim);
        let body = Rect::from_min_max(pos2(full.min.x, header.max.y), full.max);
        if let Some((id, _)) = track
            && ui.interact(body, ui.id().with("new_clip"), Sense::click()).clicked()
        {
            let sel = app.engine.session().edit.selection;
            if let Ok(v) = app.run("midi.new_clip", json!({"track": id.0, "start": sel.start, "end": sel.end})) {
                let _ = app.run("edit.select", json!({"clips": [v["clip"]]}));
            }
        }
        return;
    };
    let Some((track, clip)) = app.engine.session().find_clip(cid).map(|(t, c)| (t, c.clone())) else { return };
    let ClipContent::Midi { sequence } = &clip.content else { return };
    let track_color = app.engine.session().track(track).map_or([120, 120, 200], |t| t.color);
    // Tool palette: eight tools drawn as icons, and the Glue action.
    let tools_rect = layout.rect(HeaderItem::Tools).unwrap_or(header);
    crate::midi_tools_ui::palette(app, ui, tools_rect, cid);
    if let Some(r) = layout.rect(HeaderItem::Label) {
        ui.painter().with_clip_rect(r).text(
            pos2(r.min.x, header.center().y),
            Align2::LEFT_CENTER,
            format!("{} · {} notes · {} selected", clip.name, sequence.notes.len(), app.midi.selected.len()),
            regular(11.0),
            t.text_dim,
        );
    }
    let session_grid = match app.engine.session().edit.grid {
        soundcraft_time::GridValue::Note { value, dotted, triplet } => {
            let mut g = value.ticks();
            if dotted {
                g = g * 3 / 2;
            }
            if triplet {
                g = g * 2 / 3;
            }
            g.max(1)
        }
        _ => TICKS_PER_QUARTER / 4,
    };
    let (snap_display, snap_ticks, bar_ticks) = {
        let ses = app.engine.session();
        let start_tick = ses.tempo.samples_to_ticks(clip.start, ses.sample_rate);
        let bar = ses.tempo.meter_at_tick(start_tick).ticks_per_bar();
        (app.midi.snap.display_ticks(session_grid, bar), app.midi.snap.ticks(session_grid, bar), bar)
    };
    let grid_ticks = snap_display;
    // Header toolbar: view controls and whole-clip operations; what does not fit goes in a menu.
    let old_row_h = app.midi.row_h;
    let mut op: Option<(&str, serde_json::Value)> = None;
    if let Some(r) = layout.rect(HeaderItem::Ops) {
        let mut c = header_child(ui, r);
        op = ops_group(app, &mut c, grid_ticks);
    }
    if let Some(r) = layout.rect(HeaderItem::Inspector) {
        let mut c = header_child(ui, r);
        inspector_toggle(app, &mut c);
    }
    if let Some(r) = layout.rect(HeaderItem::Snap) {
        let mut c = header_child(ui, r);
        crate::midi_editor_toolbar::snap_group(app, &mut c, session_grid);
    }
    if let Some(r) = layout.rect(HeaderItem::Clip) {
        let mut c = header_child(ui, r);
        crate::midi_editor_toolbar::clip_group(app, &mut c, cid, &clip.name);
    }
    if let Some(r) = layout.rect(HeaderItem::LoopFollow) {
        let mut c = header_child(ui, r);
        loop_follow_group(app, &mut c, cid);
    }
    if let Some(r) = layout.rect(HeaderItem::Audition) {
        let mut c = header_child(ui, r);
        audition_group(app, &mut c);
    }
    if let Some(r) = layout.rect(HeaderItem::Zoom) {
        let mut c = header_child(ui, r);
        zoom_group(app, &mut c);
    }
    if let Some(r) = layout.rect(HeaderItem::Overflow) {
        let resp = ui.interact(r, ui.id().with("midi_header_overflow"), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "More MIDI editor actions"));
        let resp = resp.on_hover_text("More actions");
        if resp.hovered() {
            ui.painter().rect_filled(r, 3.0, t.row_selected);
        }
        for dx in [-5.0, 0.0, 5.0] {
            ui.painter().circle_filled(r.center() + vec2(dx, 0.0), 1.5, t.header_text);
        }
        egui::Popup::menu(&resp).show(|menu| {
            if !layout.visible(HeaderItem::Inspector) {
                inspector_toggle(app, menu);
            }
            if !layout.visible(HeaderItem::Snap) {
                crate::midi_editor_toolbar::snap_group(app, menu, session_grid);
            }
            if !layout.visible(HeaderItem::Clip) {
                crate::midi_editor_toolbar::clip_group(app, menu, cid, &clip.name);
            }
            if !layout.visible(HeaderItem::LoopFollow) {
                loop_follow_group(app, menu, cid);
            }
            if !layout.visible(HeaderItem::Audition) {
                audition_group(app, menu);
            }
            if !layout.visible(HeaderItem::Zoom) {
                zoom_group(app, menu);
            }
            if !layout.visible(HeaderItem::Ops) {
                op = ops_group(app, menu, grid_ticks);
            }
        });
    }
    if let Some((id, p)) = op {
        crate::midi_editor_inspector::act(app, cid, id, p);
    }
    let s = app.engine.session_arc();
    let sr = s.sample_rate;
    let base_tick = s.tempo.samples_to_ticks(clip.start, sr);
    let len_ticks = (s.tempo.samples_to_ticks(clip.end(), sr) - base_tick).max(1);
    let body_left = if app.midi.inspector_open { full.min.x + crate::midi_editor_inspector::INSPECTOR_W } else { full.min.x };
    let info = Rect::from_min_max(pos2(body_left, header.max.y), pos2(full.max.x, header.max.y + crate::midi_info_bar::INFO_H));
    crate::midi_info_bar::show(app, ui, info, cid, &sequence.notes, base_tick, &s.tempo);
    let right = full.max.x - VZOOM_W;
    let bottom = full.max.y - HZOOM_H;
    let ruler = Rect::from_min_max(pos2(body_left + KEY_W, info.max.y), pos2(right - 4.0, info.max.y + RULER_H));
    let avail_h = bottom - ruler.max.y;
    let (lane_rects, roll_bottom) = crate::midi_lanes::layout(&app.midi.lanes, ruler.min.x, right - 4.0, bottom, avail_h);
    let roll = Rect::from_min_max(pos2(ruler.min.x, ruler.max.y), pos2(right - 4.0, roll_bottom.max(ruler.max.y)));
    let keys = Rect::from_min_max(pos2(body_left, roll.min.y), pos2(roll.min.x, roll.max.y));
    ui.ctx().data_mut(|d| d.insert_temp(Id::new("soundcraft-midi-roll"), roll));
    let row_h = app.midi.row_h;
    // Vertical range: centre on the notes the first time.
    let rows = (roll.height() / row_h).floor() as i32;
    if app.midi.clip != Some(cid) {
        crate::midi_audition::stop(app);
        app.midi.clip = Some(cid);
        app.midi.ppt = None;
        app.midi.view_start = 0.0;
        app.midi.v_manual = false;
    }
    // Keep the notes centred until the user scrolls vertically (the panel may still be resizing).
    if !app.midi.v_manual {
        let hi = sequence.notes.iter().map(|n| i32::from(n.pitch)).max().unwrap_or(72);
        let lo = sequence.notes.iter().map(|n| i32::from(n.pitch)).min().unwrap_or(48);
        app.midi.scroll = (127 - (hi + lo) / 2 - rows / 2) as f32 * row_h;
    }
    let over_view = ui.rect_contains_pointer(roll) || ui.rect_contains_pointer(ruler);
    if ui.rect_contains_pointer(roll) {
        // Wheel scrolls the keys; with Shift (or a sideways wheel) it scrolls time.
        let (dy, shift) = ui.input(|i| (i.smooth_scroll_delta.y, i.modifiers.shift));
        if !shift && dy != 0.0 {
            app.midi.scroll -= dy;
            app.midi.v_manual = true;
        }
    }
    if app.midi.v_manual && old_row_h != row_h && old_row_h > 0.0 {
        // Keep the middle of the view steady while zooming vertically.
        app.midi.scroll = (app.midi.scroll + roll.height() / 2.0) / old_row_h * row_h - roll.height() / 2.0;
    }
    app.midi.scroll = app.midi.scroll.clamp(0.0, (128.0 * row_h - roll.height()).max(0.0));
    let scroll = app.midi.scroll;
    // Horizontal view: pixels per tick (fit by default), first visible tick.
    let fit_ppt = roll.width() / len_ticks as f32;
    let (min_ppt, max_ppt) = ((fit_ppt * 0.25).min(0.01), 1.0_f32.max(fit_ppt));
    let mut ppt = app.midi.ppt.unwrap_or(fit_ppt).clamp(min_ppt, max_ppt);
    if let Some(factor) = app.midi.zoom_h.take() {
        ppt = zoom_ppt(ppt, factor, min_ppt, max_ppt);
        app.midi.ppt = Some(ppt);
    }
    let mut view_start = app.midi.view_start;
    if over_view {
        let (zoom, dx, dy, shift, hover) =
            ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta.x, i.smooth_scroll_delta.y, i.modifiers.shift, i.pointer.hover_pos()));
        if let Some(h) = hover
            && (zoom - 1.0).abs() > f32::EPSILON
        {
            // Zoom around the pointer: the tick under it stays put.
            let anchor = view_start + f64::from((h.x - roll.min.x) / ppt);
            ppt = (ppt * zoom).clamp(min_ppt, max_ppt);
            view_start = anchor - f64::from((h.x - roll.min.x) / ppt);
            app.midi.ppt = Some(ppt);
        }
        let sideways = if shift { dy } else { dx };
        if sideways != 0.0 {
            view_start -= f64::from(sideways / ppt);
            app.midi.ppt = Some(ppt);
        }
    }
    let visible_ticks = f64::from(roll.width() / ppt);
    // Follow the playhead: when it leaves the view, page the view so it sits near the left.
    // While stopped, show the position placed in this editor (where Space will start), not the selection start.
    let shown_at = if app.is_playing() {
        app.position()
    } else {
        let sel = s.edit.selection;
        placed_position(app.midi.play_from, sel).unwrap_or_else(|| app.position())
    };
    let playhead_tick = s.tempo.samples_to_ticks_f(shown_at, sr) - base_tick as f64;
    if app.is_playing()
        && !app.midi.no_follow
        && app.midi.ppt.is_some()
        && (playhead_tick < view_start || playhead_tick > view_start + visible_ticks * 0.9)
    {
        view_start = playhead_tick - visible_ticks * 0.1;
    }
    view_start = view_start.clamp(0.0, (len_ticks as f64 - visible_ticks).max(0.0));
    app.midi.view_start = view_start;
    let hz = Rect::from_min_max(pos2(right - 4.0 - 190.0, bottom), pos2(right - 4.0, full.max.y - 1.0));
    let vz = Rect::from_min_max(pos2(right, roll.min.y), pos2(full.max.x - 2.0, roll.max.y));
    crate::midi_zoom::show(app, ui, hz, vz, ppt, (min_ppt, max_ppt), roll.height(), ROW_H_RANGE, ZOOM_STEP);
    crate::midi_lanes::add_button(ui, app, Rect::from_min_size(pos2(body_left + 4.0, bottom + 1.0), vec2(22.0, HZOOM_H - 2.0)));
    let px_per_tick = ppt;
    let x_of = |tick: i64| roll.min.x + ((tick as f64 - view_start) as f32) * px_per_tick;
    let tick_at = |x: f32| -> i64 { (view_start + f64::from((x - roll.min.x) / px_per_tick)).floor() as i64 };
    let y_of = |pitch: i32| roll.min.y + (127 - pitch) as f32 * row_h - scroll;
    let pitch_at = |y: f32| 127 - ((y - roll.min.y + scroll) / row_h).floor() as i32;
    let painter = ui.painter().with_clip_rect(roll.union(keys));
    let roll_p = ui.painter().with_clip_rect(roll);
    // Background rows and keys; pitches sounding at the playhead are tinted.
    let active = crate::midi_keys::active_pitches(&sequence.notes, playhead_tick.round() as i64);
    let scale = if app.midi.insp.highlight_rows { crate::midi_editor_inspector::scale_of(&s, cid) } else { None };
    let top = pitch_at(roll.min.y);
    for r in 0..=rows + 1 {
        let pitch = top - r;
        let y = y_of(pitch);
        let row = Rect::from_min_size(pos2(roll.min.x, y), vec2(roll.width(), row_h));
        let lit = (0..=127).contains(&pitch) && active.get(pitch as usize).copied().unwrap_or(false);
        painter.rect_filled(row, 0.0, crate::midi_keys::row_fill(is_black(pitch), lit));
        if let Some(sc) = scale
            && (0..=127).contains(&pitch)
            && !sc.contains(pitch as u8)
        {
            painter.rect_filled(row, 0.0, crate::midi_keys::OUT_OF_SCALE);
        }
        crate::midi_keys::draw_key(&painter, keys.min.x, KEY_W, y, row_h, pitch, is_black(pitch), lit);
        if pitch.rem_euclid(12) == 0 && (0..=127).contains(&pitch) {
            painter.line_segment([pos2(roll.min.x, y + row_h), pos2(roll.max.x, y + row_h)], Stroke::new(1.0, Tokens::current().roll_octave));
        }
    }
    // Keyboard gutter: press a key to hear it; sliding along the keys moves the sounding pitch.
    let kresp = ui.interact(keys, ui.id().with(("keys", cid.0)), Sense::click_and_drag());
    if kresp.is_pointer_button_down_on()
        && let Some(p) = ui.input(|i| i.pointer.interact_pos())
    {
        let pitch = pitch_at(p.y.clamp(keys.min.y, keys.max.y - 0.01)).clamp(0, 127);
        if app.midi.audition.sounding.is_some() {
            crate::midi_audition::moved(app, pitch);
        } else {
            crate::midi_audition::start_default(app, ui.ctx(), track, pitch, false);
        }
    }
    if kresp.drag_stopped() || kresp.clicked() {
        crate::midi_audition::stop(app);
    }
    // Grid.
    // Grid lines for the visible span only; too dense a grid is thinned to every few steps.
    let step_px = grid_ticks as f32 * px_per_tick;
    let step = if step_px > 4.0 { grid_ticks } else { grid_ticks.saturating_mul((4.0 / step_px).ceil() as i64) };
    let mut tick = (view_start.floor() as i64 / step.max(1)) * step.max(1);
    while tick <= len_ticks && x_of(tick) <= roll.max.x {
        let x = x_of(tick);
        roll_p.line_segment([pos2(x, roll.min.y), pos2(x, roll.max.y)], Stroke::new(1.0, Tokens::current().roll_grid));
        tick += step.max(1);
    }
    // Ruler: clip band with its name, bar numbers, and the stronger bar lines through the roll.
    let band = Color32::from_rgb(track_color[0], track_color[1], track_color[2]);
    let rc = crate::midi_ruler::RulerClip { name: &clip.name, color: band, base_tick, len_ticks, view_start };
    crate::midi_ruler::draw(ui, ruler, roll, &s, &rc, &x_of);
    // Click or drag on the ruler to move the playhead.
    let rresp = ui.interact(ruler, ui.id().with(("ruler", cid.0)), Sense::click_and_drag());
    if (rresp.clicked() || rresp.dragged())
        && let Some(p) = rresp.interact_pointer_pos()
    {
        let abs = base_tick.saturating_add(tick_at(p.x).clamp(0, len_ticks));
        place_playhead(app, s.tempo.tick_to_samples(abs, sr));
    }
    // Notes.
    let base = egui::Color32::from_rgb(track_color[0], track_color[1], track_color[2]);
    let drag = app.midi.drag.clone();
    let single_colour = s.edit.flag("midi.single_colour");
    for (i, n) in sequence.notes.iter().enumerate() {
        let (mut st, mut ln, mut p) = (n.start, n.length, i32::from(n.pitch));
        if let Some(d) = &drag
            && (d.index == i || (!d.resize && app.midi.selected.contains(&i) && app.midi.selected.contains(&d.index)))
        {
            if d.resize {
                if d.index == i {
                    ln = (ln + d.dx_ticks).max(grid_ticks.min(ln).max(1));
                }
            } else {
                st = (st + d.dx_ticks).max(0);
                p = (p + d.dy_semi as i32).clamp(0, 127);
            }
        }
        let r = Rect::from_min_max(pos2(x_of(st), y_of(p) + 1.0), pos2(x_of(st + ln).max(x_of(st) + 3.0), y_of(p) + row_h - 1.0));
        let sel = app.midi.selected.contains(&i);
        let single = single_colour.then_some(base);
        crate::midi_note_paint::paint_note(&roll_p, r, p.clamp(0, 127) as u8, n.velocity, sel, single, sequence.is_muted(i), &t);
    }
    // Interaction.
    let resp = ui.interact(roll, ui.id().with(("roll", cid.0)), Sense::click_and_drag());
    let hit = |pos: egui::Pos2| -> Option<(usize, bool)> {
        sequence.notes.iter().enumerate().rev().find_map(|(i, n)| {
            let r = Rect::from_min_max(
                pos2(x_of(n.start), y_of(i32::from(n.pitch))),
                pos2(x_of(n.start + n.length).max(x_of(n.start) + 3.0), y_of(i32::from(n.pitch)) + row_h),
            );
            r.contains(pos).then_some((i, pos.x > r.max.x - 5.0))
        })
    };
    let snap = |tk: f32| -> i64 { ((tk / snap_ticks as f32).round() as i64) * snap_ticks };
    let tool = if app.midi.tool == MidiTool::Select && s.edit.tool == Tool::Pencil { MidiTool::Pencil } else { app.midi.tool };
    if resp.hovered() && matches!(tool, MidiTool::Pencil | MidiTool::Split) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
    }
    let grid_floor = |tk: i64| (tk.max(0) / snap_ticks) * snap_ticks;
    if resp.drag_started()
        && let Some(p) = ui.input(|i| i.pointer.press_origin()).or_else(|| resp.interact_pointer_pos())
    {
        // The press point, not the current one: egui starts a drag after a small movement, which
        // can already be off the row of a thin note.
        match (tool, hit(p)) {
            (MidiTool::Erase | MidiTool::Split | MidiTool::Glue | MidiTool::Mute | MidiTool::Zoom | MidiTool::Line, _) => {}
            (_, Some((i, edge))) => {
                if !app.midi.selected.contains(&i) {
                    app.midi.selected = vec![i];
                }
                app.midi.drag = Some(NoteDrag { index: i, resize: edge, start: p, dx_ticks: 0, dy_semi: 0 });
                if !edge && let Some(n) = sequence.notes.get(i) {
                    crate::midi_audition::start(app, ui.ctx(), track, i32::from(n.pitch), n.velocity, false);
                }
            }
            (MidiTool::Pencil, None) => {
                let pitch = pitch_at(p.y).clamp(0, 127);
                app.midi.pencil = Some((grid_floor(tick_at(p.x)), pitch));
                crate::midi_audition::start_default(app, ui.ctx(), track, pitch, false);
            }
            (MidiTool::Select, None) => app.midi.band = Some(p),
        }
    }
    // Pencil: a drag draws one note whose length follows the pointer.
    if let Some((start, pitch)) = app.midi.pencil {
        let end = ui.ctx().pointer_latest_pos().map_or(start, |p| (grid_floor(tick_at(p.x)) + grid_ticks).max(start + grid_ticks));
        let r = Rect::from_min_max(pos2(x_of(start), y_of(pitch) + 1.0), pos2(x_of(end), y_of(pitch) + row_h - 1.0));
        roll_p.rect(r, 2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 120), Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
        if resp.drag_stopped() {
            crate::midi_editor_inspector::place(app, cid, pitch, start, end - start);
            app.midi.pencil = None;
            crate::midi_audition::stop(app);
        }
    }
    // Erase: drag across notes to delete them as the pointer touches each one.
    if tool == MidiTool::Erase
        && resp.dragged()
        && let Some(p) = resp.interact_pointer_pos()
        && let Some((i, _)) = hit(p)
    {
        let _ = app.engine.execute_merged("midi.note_delete", &json!({"clip": cid.0, "indices": [i]}), &format!("erase:{}", cid.0));
        app.midi.selected.clear();
    }
    // Rubber-band selection on empty space.
    if let (Some(a), Some(b)) = (app.midi.band, ui.ctx().pointer_latest_pos()) {
        let band = Rect::from_two_pos(a, b);
        ui.painter().with_clip_rect(roll).rect(
            band,
            0.0,
            Color32::from_rgba_unmultiplied(120, 170, 255, 40),
            Stroke::new(1.0, Color32::from_rgb(120, 170, 255)),
            StrokeKind::Inside,
        );
        if resp.drag_stopped() {
            app.midi.selected = sequence
                .notes
                .iter()
                .enumerate()
                .filter(|(_, n)| {
                    let r = Rect::from_min_max(
                        pos2(x_of(n.start), y_of(i32::from(n.pitch))),
                        pos2(x_of(n.start + n.length), y_of(i32::from(n.pitch)) + row_h),
                    );
                    r.intersects(band)
                })
                .map(|(i, _)| i)
                .collect();
            app.midi.band = None;
        }
    }
    if resp.dragged()
        && let (Some(dd), Some(p)) = (app.midi.drag.clone(), resp.interact_pointer_pos())
    {
        let raw = -((p.y - dd.start.y) / row_h).round() as i64;
        let dy = match sequence.notes.get(dd.index) {
            Some(n) if !dd.resize => crate::midi_editor_inspector::snap_drag(app, cid, n.pitch, raw),
            _ => raw,
        };
        if let Some(d) = &mut app.midi.drag {
            d.dx_ticks = snap((p.x - d.start.x) / px_per_tick);
            d.dy_semi = dy;
        }
        if !dd.resize
            && let Some(n) = sequence.notes.get(dd.index)
        {
            let pitch = (i32::from(n.pitch) + dy as i32).clamp(0, 127);
            crate::midi_audition::moved(app, pitch);
        }
    }
    // Line tool dragged in the roll: a ramp in the active controller lane.
    if tool == MidiTool::Line {
        let lane = app.midi.lanes.get(app.midi.active_lane).map(|l| l.lane).filter(|l| *l != Lane::Velocity);
        if resp.drag_started()
            && let (Some(lane), Some(p)) = (lane, ui.input(|i| i.pointer.press_origin()).or_else(|| resp.interact_pointer_pos()))
        {
            app.midi.line = Some((p, lane, true));
        }
        if let Some((start, lane, true)) = app.midi.line
            && let Some(now) = ui.ctx().pointer_latest_pos()
        {
            roll_p.line_segment([start, pos2(now.x, now.y.clamp(roll.min.y, roll.max.y))], Stroke::new(2.0, Color32::WHITE));
            if resp.drag_stopped() {
                let (name, cc, lo, hi) = lane.command();
                let tk = |x: f32| (tick_at(x).max(0) / grid_ticks) * grid_ticks;
                let pts = crate::midi_lanes::ramp_points(
                    tk(start.x),
                    crate::midi_lanes::value_at(start.y, roll, lo, hi),
                    tk(now.x),
                    crate::midi_lanes::value_at(now.y, roll, lo, hi),
                    grid_ticks,
                );
                let points: Vec<_> = pts.iter().map(|(tick, value)| json!({"tick": tick, "value": value})).collect();
                let _ = app.run("midi.ctrl_set", json!({"clip": cid.0, "lane": name, "cc": cc, "points": points}));
                app.midi.line = None;
            }
        }
    }
    if resp.drag_stopped() {
        crate::midi_audition::stop(app);
    }
    if resp.drag_stopped()
        && let Some(d) = app.midi.drag.take()
    {
        if d.resize {
            if let Some(n) = sequence.notes.get(d.index) {
                let _ = app.run("midi.note_edit", json!({"clip": cid.0, "index": d.index, "length_ticks": (n.length + d.dx_ticks).max(1)}));
            }
        } else if d.dx_ticks != 0 || d.dy_semi != 0 {
            let _ = app.run("midi.notes_move", json!({"clip": cid.0, "indices": app.midi.selected, "ticks": d.dx_ticks, "semitones": d.dy_semi}));
            app.midi.selected.clear();
        }
    }
    if resp.clicked()
        && let Some(p) = resp.interact_pointer_pos()
    {
        match (tool, hit(p)) {
            (MidiTool::Erase, Some((i, _))) => {
                let _ = app.run("midi.note_delete", json!({"clip": cid.0, "indices": [i]}));
                app.midi.selected.clear();
            }
            (MidiTool::Erase, None) => {}
            (MidiTool::Split, Some((i, _))) => {
                let _ = app.run("midi.note_split", json!({"clip": cid.0, "index": i, "tick": snap(tick_at(p.x) as f32)}));
                app.midi.selected.clear();
            }
            (MidiTool::Split, None) => {}
            (MidiTool::Glue, Some((i, _))) => {
                if let Some(j) = crate::midi_tools_ui::next_same_pitch(&sequence.notes, i) {
                    let _ = app.run("midi.notes_glue", json!({"clip": cid.0, "indices": [i, j]}));
                    app.midi.selected.clear();
                }
            }
            (MidiTool::Mute, Some((i, _))) => {
                let targets = crate::midi_tools_ui::mute_targets(&app.midi.selected, i);
                let _ = app.run("midi.notes_mute", json!({"clip": cid.0, "indices": targets}));
            }
            (MidiTool::Zoom, _) => {
                let alt = ui.input(|i| i.modifiers.alt);
                let (np, nv) = crate::midi_tools_ui::zoom_click(ppt, view_start, p.x - roll.min.x, alt, ZOOM_STEP, min_ppt, max_ppt);
                app.midi.ppt = Some(np);
                app.midi.view_start = nv;
            }
            (MidiTool::Glue | MidiTool::Mute | MidiTool::Line, None) => {}
            (MidiTool::Line, Some(_)) => {}
            (_, Some((i, _))) => {
                if ui.input(|x| x.modifiers.shift) {
                    if !app.midi.selected.contains(&i) {
                        app.midi.selected.push(i);
                    }
                } else {
                    app.midi.selected = vec![i];
                }
                if let Some(n) = sequence.notes.get(i) {
                    crate::midi_audition::start(app, ui.ctx(), track, i32::from(n.pitch), n.velocity, true);
                }
            }
            (MidiTool::Pencil, None) => {
                let pitch = i32::from(crate::midi_editor_inspector::entry_pitch(app, cid, pitch_at(p.y)));
                crate::midi_editor_inspector::place(app, cid, pitch, grid_floor(tick_at(p.x)), grid_ticks);
                crate::midi_audition::start_default(app, ui.ctx(), track, pitch, true);
                app.midi.selected.clear();
            }
            (MidiTool::Select, None) => {
                // Click places the playhead (Space plays from there, no document change); double-click adds a note.
                app.midi.selected.clear();
                let abs = base_tick.saturating_add(grid_floor(tick_at(p.x)).min(len_ticks));
                mark_play_from(app, s.tempo.tick_to_samples(abs, sr));
                if resp.double_clicked() {
                    let pitch = i32::from(crate::midi_editor_inspector::entry_pitch(app, cid, pitch_at(p.y)));
                    let _ = app.run(
                        "midi.note_add",
                        json!({"clip": cid.0, "pitch": pitch, "start_ticks": grid_floor(tick_at(p.x)), "length_ticks": grid_ticks, "velocity": 100}),
                    );
                    crate::midi_audition::start_default(app, ui.ctx(), track, pitch, true);
                }
            }
        }
    }
    // Keyboard: Cmd+A select all; arrows transpose (Shift = octave) and move by the grid.
    if ui.rect_contains_pointer(roll) && !ui.ctx().egui_wants_keyboard_input() {
        let (all, up, down, left, right, shift) = ui.input(|i| {
            let arrows = !i.modifiers.alt && !i.modifiers.command && !i.modifiers.ctrl;
            (
                i.modifiers.command && i.key_pressed(egui::Key::A),
                arrows && i.key_pressed(egui::Key::ArrowUp),
                arrows && i.key_pressed(egui::Key::ArrowDown),
                arrows && !i.modifiers.shift && i.key_pressed(egui::Key::ArrowLeft),
                arrows && !i.modifiers.shift && i.key_pressed(egui::Key::ArrowRight),
                i.modifiers.shift,
            )
        });
        if all {
            app.midi.selected = (0..sequence.notes.len()).collect();
        }
        let semi = if up {
            1
        } else if down {
            -1
        } else {
            0
        } * if shift { 12 } else { 1 };
        let ticks = if right {
            grid_ticks
        } else if left {
            -grid_ticks
        } else {
            0
        };
        if (semi != 0 || ticks != 0) && !app.midi.selected.is_empty() {
            let _ = app.run("midi.notes_move", json!({"clip": cid.0, "indices": app.midi.selected, "ticks": ticks, "semitones": semi}));
        }
    }
    if !app.midi.selected.is_empty()
        && !ui.ctx().egui_wants_keyboard_input()
        && ui.input(|i| i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace))
    {
        let _ = app.run("midi.note_delete", json!({"clip": cid.0, "indices": app.midi.selected}));
        app.midi.selected.clear();
    }
    // Scrollbars over the roll's right and bottom edges. Drag (or click) a track to move the view.
    let bar_w = 9.0;
    let sb_track = ui.visuals().extreme_bg_color.gamma_multiply(0.6);
    let sb_thumb = Color32::from_rgb(110, 110, 120);
    let vtrack = Rect::from_min_max(pos2(roll.max.x - bar_w, roll.min.y), pos2(roll.max.x, roll.max.y - bar_w));
    let content_h = 128.0 * row_h;
    let v_range = (content_h - roll.height()).max(0.0);
    let vthumb_h = (vtrack.height() * (roll.height() / content_h)).clamp(16.0, vtrack.height());
    let vresp = ui.interact(vtrack, ui.id().with(("vscroll", cid.0)), Sense::click_and_drag());
    if (vresp.dragged() || vresp.clicked())
        && v_range > 0.0
        && let Some(p) = vresp.interact_pointer_pos()
    {
        let frac = ((p.y - vtrack.min.y - vthumb_h / 2.0) / (vtrack.height() - vthumb_h).max(1.0)).clamp(0.0, 1.0);
        app.midi.scroll = frac * v_range;
        app.midi.v_manual = true;
    }
    let vfrac = if v_range > 0.0 { (scroll / v_range).clamp(0.0, 1.0) } else { 0.0 };
    let vthumb_y = vtrack.min.y + vfrac * (vtrack.height() - vthumb_h);
    ui.painter().rect_filled(vtrack, 3.0, sb_track);
    ui.painter().rect_filled(Rect::from_min_size(pos2(vtrack.min.x + 1.0, vthumb_y), vec2(bar_w - 2.0, vthumb_h)), 3.0, sb_thumb);

    let htrack = Rect::from_min_max(pos2(roll.min.x, roll.max.y - bar_w), pos2(roll.max.x - bar_w, roll.max.y));
    let h_range = (len_ticks as f64 - visible_ticks).max(0.0);
    let hthumb_w = (htrack.width() * (visible_ticks / len_ticks as f64).min(1.0) as f32).clamp(16.0, htrack.width());
    let hresp = ui.interact(htrack, ui.id().with(("hscroll", cid.0)), Sense::click_and_drag());
    if (hresp.dragged() || hresp.clicked())
        && h_range > 0.0
        && let Some(p) = hresp.interact_pointer_pos()
    {
        let frac = f64::from(((p.x - htrack.min.x - hthumb_w / 2.0) / (htrack.width() - hthumb_w).max(1.0)).clamp(0.0, 1.0));
        app.midi.view_start = frac * h_range;
        app.midi.ppt = Some(ppt);
    }
    let hfrac = if h_range > 0.0 { (view_start / h_range).clamp(0.0, 1.0) as f32 } else { 0.0 };
    let hthumb_x = htrack.min.x + hfrac * (htrack.width() - hthumb_w);
    ui.painter().rect_filled(htrack, 3.0, sb_track);
    ui.painter().rect_filled(Rect::from_min_size(pos2(hthumb_x, htrack.min.y + 1.0), vec2(hthumb_w, bar_w - 2.0)), 3.0, sb_thumb);

    // Lanes: velocity and controllers, each with its label, splitter and contents.
    let lane_bottom = lane_rects.last().map_or(roll.max.y, |r| r.area.max.y);
    let mut vel_lane = |app: &mut SoundApp, ui: &mut Ui, r: Rect| velocity_lane(app, ui, cid, sequence, r, &x_of, base, single_colour, &t);
    crate::midi_lanes::show_all(app, ui, cid, sequence, &lane_rects, avail_h, &x_of, &tick_at, grid_ticks, base, &mut vel_lane);
    playhead(ui, x_of(playhead_tick.round() as i64), ruler.min.y, lane_bottom, roll.min.x, roll.max.x);
    if app.midi.inspector_open {
        let rect = Rect::from_min_max(pos2(full.min.x, header.max.y), pos2(body_left, full.max.y));
        crate::midi_editor_inspector::show(app, ui, rect, cid, sequence, playhead_tick.round() as i64, bar_ticks);
    }
}

/// The velocity lane: one bar per note, drag a bar to change that note's velocity.
#[allow(clippy::too_many_arguments)]
fn velocity_lane(
    app: &mut SoundApp,
    ui: &mut Ui,
    cid: ClipId,
    sequence: &soundcraft_midi::Sequence,
    vel: Rect,
    x_of: &dyn Fn(i64) -> f32,
    base: Color32,
    single_colour: bool,
    t: &Tokens,
) {
    ui.painter().rect_filled(vel, 0.0, Tokens::current().lane_bg);
    let vresp = ui.interact(vel, ui.id().with(("vel", cid.0)), Sense::click_and_drag());
    if vresp.is_pointer_button_down_on() {
        app.midi.active_lane = app.midi.lanes.iter().position(|l| l.lane == Lane::Velocity).unwrap_or(0);
    }
    let vp = ui.painter().with_clip_rect(vel);
    for (i, n) in sequence.notes.iter().enumerate() {
        let x = x_of(n.start);
        let h = (vel.height() - 4.0) * f32::from(n.velocity) / 127.0;
        let mut col = if app.midi.selected.contains(&i) {
            Color32::WHITE
        } else if single_colour {
            base
        } else {
            crate::midi_note_paint::vel_color(n.velocity, t)
        };
        if sequence.is_muted(i) {
            col = col.gamma_multiply(0.4);
        }
        vp.line_segment([pos2(x + 1.0, vel.max.y - 2.0), pos2(x + 1.0, vel.max.y - 2.0 - h)], Stroke::new(3.0, col));
        vp.circle_filled(pos2(x + 1.0, vel.max.y - 2.0 - h), 2.5, col);
    }
    if (vresp.clicked() || vresp.dragged())
        && let Some(p) = vresp.interact_pointer_pos()
        && let Some((i, n)) = sequence.notes.iter().enumerate().min_by_key(|(_, n)| ((x_of(n.start) - p.x).abs() * 10.0) as i64)
        && (x_of(n.start) - p.x).abs() < 8.0
    {
        let v = (((vel.max.y - 2.0 - p.y) / (vel.height() - 4.0)) * 127.0).round().clamp(1.0, 127.0) as i64;
        let _ = app.engine.execute_merged("midi.note_edit", &json!({"clip": cid.0, "index": i, "velocity": v}), &format!("vel:{}", cid.0));
    }
}

/// The playhead: a vertical line from the ruler through the lanes, with a small marker on the ruler.
/// Float/Dock, Minimize and Close buttons for the panel, right-aligned in the header. The dock
/// operations run after the frame (the dock owns its layout while it draws).
fn panel_controls(app: &mut SoundApp, ui: &mut Ui, header: Rect) {
    let t = Tokens::current();
    let floating = app.midi.floating;
    let size = 18.0;
    let mut x = header.max.x - 4.0 - size;
    for kind in 0..3 {
        let rect = Rect::from_min_size(pos2(x, header.center().y - size / 2.0), vec2(size, size));
        x -= size + 3.0;
        let (label, tip) = match kind {
            0 => ("Close MIDI editor", "Close the MIDI editor panel"),
            1 if app.midi.minimized => ("Restore MIDI editor", "Show the whole panel again"),
            1 => ("Minimize MIDI editor", "Collapse the panel to its header"),
            _ if floating => ("Dock MIDI editor", "Put the panel back in the window"),
            _ => ("Float MIDI editor", "Detach the panel into a floating panel"),
        };
        let resp = ui.interact(rect, ui.id().with(("midi_panel_ctl", kind)), Sense::click());
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
        let resp = resp.on_hover_text(tip);
        if resp.hovered() {
            ui.painter().rect_filled(rect, 3.0, t.row_selected);
        }
        let stroke = Stroke::new(1.3, t.header_text);
        let c = rect.center();
        let p = ui.painter();
        match kind {
            0 => {
                p.line_segment([c + vec2(-4.0, -4.0), c + vec2(4.0, 4.0)], stroke);
                p.line_segment([c + vec2(-4.0, 4.0), c + vec2(4.0, -4.0)], stroke);
            }
            1 if app.midi.minimized => {
                p.rect_stroke(Rect::from_center_size(c, vec2(9.0, 8.0)), 1.0, stroke, StrokeKind::Inside);
            }
            1 => {
                p.line_segment([c + vec2(-4.0, 3.0), c + vec2(4.0, 3.0)], stroke);
            }
            _ if floating => {
                p.rect_stroke(Rect::from_center_size(c, vec2(10.0, 9.0)), 1.0, stroke, StrokeKind::Inside);
                p.rect_filled(Rect::from_min_max(c + vec2(-5.0, 1.0), c + vec2(5.0, 4.5)), 0.0, t.header_text);
            }
            _ => {
                p.rect_stroke(Rect::from_min_size(c + vec2(-5.0, -2.0), vec2(7.0, 7.0)), 1.0, stroke, StrokeKind::Inside);
                p.rect_stroke(Rect::from_min_size(c + vec2(-2.0, -5.0), vec2(7.0, 7.0)), 1.0, stroke, StrokeKind::Inside);
            }
        }
        if resp.clicked() {
            match kind {
                0 => {
                    crate::midi_audition::stop(app);
                    app.midi.minimized = false;
                    app.midi.dock_request = Some(json!({"operation": "close", "panel": "midi"}));
                }
                1 => app.midi.minimized = !app.midi.minimized,
                _ if floating => {
                    app.midi.dock_request = Some(json!({"operation": "move", "panel": "midi", "target": "canvas", "zone": "bottom"}));
                }
                _ => app.midi.dock_request = Some(json!({"operation": "float", "panel": "midi", "rect": float_rect(ui.ctx().content_rect())})),
            }
        }
    }
}

fn playhead(ui: &Ui, x: f32, y0: f32, y1: f32, min_x: f32, max_x: f32) {
    if !(min_x..=max_x).contains(&x) {
        return;
    }
    let color = Color32::from_rgb(255, 205, 80);
    let p = ui.painter();
    p.line_segment([pos2(x, y0), pos2(x, y1)], Stroke::new(1.5, color));
    p.add(egui::Shape::convex_polygon(vec![pos2(x - 4.0, y0), pos2(x + 4.0, y0), pos2(x, y0 + 6.0)], color, Stroke::NONE));
}

#[cfg(test)]
mod tests {
    use super::{HeaderItem, clamp_lane_h, header_layout, placed_position, step_row_h, zoom_ppt};
    use egui::{Rect, pos2};
    use soundcraft_time::Range;

    #[test]
    fn zoom_steps_stay_inside_their_limits() {
        assert_eq!(zoom_ppt(0.1, 1.25, 0.01, 0.11), 0.11);
        assert_eq!(zoom_ppt(0.1, 0.8, 0.09, 1.0), 0.09);
        assert!((zoom_ppt(0.1, 1.25, 0.01, 1.0) - 0.125).abs() < 1e-6);
        assert_eq!(step_row_h(9.0, 2.0), 11.0);
        assert_eq!(step_row_h(31.0, 2.0), 32.0);
        assert_eq!(step_row_h(7.0, -2.0), 6.0);
        assert_eq!(step_row_h(6.0, -2.0), 6.0);
    }

    #[test]
    fn a_placed_position_applies_only_to_the_selection_it_was_placed_in() {
        let clip = Range::new(0, 1000);
        assert_eq!(placed_position(Some((400, clip)), clip), Some(400), "inside the unchanged selection");
        assert_eq!(placed_position(Some((1500, clip)), clip), None, "outside a range selection");
        assert_eq!(placed_position(Some((400, clip)), Range::new(0, 2000)), None, "the selection changed");
        let point = Range::point(300);
        assert_eq!(placed_position(Some((300, point)), point), Some(300), "a point selection keeps its position");
        assert_eq!(placed_position(None, clip), None);
    }

    #[test]
    fn lane_height_clamps_between_40_and_the_editor_height_minus_the_roll_handle_and_gap() {
        assert_eq!(clamp_lane_h(64.0, 600.0), 64.0);
        assert_eq!(clamp_lane_h(10.0, 600.0), 40.0);
        assert_eq!(clamp_lane_h(900.0, 600.0), 472.0, "the roll keeps 120 pt after the 6 pt handle and the 2 pt gap");
        assert_eq!(clamp_lane_h(100.0, 140.0), 40.0, "a small panel shrinks the lane first, to the minimum");
        assert_eq!(clamp_lane_h(f32::NAN, 600.0), 64.0);
    }

    fn laid_out(width: f32) -> super::HeaderLayout {
        header_layout(Rect::from_min_max(pos2(0.0, 0.0), pos2(width, 22.0)), true)
    }

    #[test]
    fn header_items_never_overlap_and_give_up_in_priority_order() {
        // Lowest priority first.
        let order = [
            HeaderItem::Label,
            HeaderItem::Ops,
            HeaderItem::Clip,
            HeaderItem::Zoom,
            HeaderItem::Audition,
            HeaderItem::LoopFollow,
            HeaderItem::Snap,
            HeaderItem::Inspector,
            HeaderItem::Glyphs,
            HeaderItem::Tools,
            HeaderItem::Title,
        ];
        let mut width = 420.0;
        let mut seen = [false; 11];
        while width <= 1800.0 {
            let l = laid_out(width);
            for (i, (a, ra)) in l.items.iter().enumerate() {
                assert!(ra.min.x >= 0.0 && ra.max.x <= width + 0.01, "{a:?} inside at {width}");
                for (b, rb) in &l.items[i + 1..] {
                    assert!(ra.max.x <= rb.min.x + 0.01 || rb.max.x <= ra.min.x + 0.01, "{a:?} overlaps {b:?} at {width}");
                }
            }
            // A lower-priority item is only visible when every higher-priority one is.
            for k in 0..order.len() {
                if l.visible(order[k]) {
                    assert!(order[k + 1..].iter().all(|h| l.visible(*h)), "{:?} visible without a higher-priority item at {width}", order[k]);
                    seen[k] = true;
                }
            }
            // Overflow appears exactly when a group is hidden.
            let hidden = [
                HeaderItem::Inspector,
                HeaderItem::Snap,
                HeaderItem::LoopFollow,
                HeaderItem::Audition,
                HeaderItem::Zoom,
                HeaderItem::Clip,
                HeaderItem::Ops,
            ]
            .iter()
            .any(|g| !l.visible(*g));
            assert_eq!(l.visible(HeaderItem::Overflow), hidden, "overflow at {width}");
            width += 10.0;
        }
        assert!(seen.iter().all(|v| *v), "every item shows up at some width");
        assert!(!laid_out(760.0).visible(HeaderItem::Label));
        assert!(laid_out(super::FLOAT_W).visible(HeaderItem::Ops), "the default floating width keeps all groups");
        assert!(laid_out(1700.0).visible(HeaderItem::Label), "the clip label fits once the Inspector button is accounted for");
        // Very narrow: the glyphs stay and nothing overlaps even with the palette squeezed.
        let tiny = laid_out(200.0);
        assert!(tiny.visible(HeaderItem::Glyphs));
    }
}
