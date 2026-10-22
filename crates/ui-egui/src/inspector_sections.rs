//! The Inspector's collapsible sections: which ones a track kind gets, the collapse toggle, and
//! the body of each (routing, instrument, inserts, sends, channel, notes). Every control runs the
//! same command as the Mix strip; collapse state is UI-only and saved in the preferences.

use crate::SoundApp;
use crate::mix_window::{self, input_name, route_name, route_popup};
use crate::theme::{Tokens, bold, regular};
use crate::widgets::{db_text, fader_slot, key, key_fill, pan_knob, pan_text, selector_box, text_toggle};
use egui::{Align2, Id, Rect, Sense, Shape, Stroke, Ui, WidgetInfo, WidgetType, pos2, vec2};
use serde_json::json;
use soundcraft_model::{INSERT_SLOTS, SEND_SLOTS, Track, TrackId, TrackKind};

pub(crate) const PAD: f32 = 8.0;
const SLOT_H: f32 = 18.0;
const FADER_H: f32 = 140.0;
const BTN: f32 = 18.0;

/// One collapsible block of the Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Routing,
    Instrument,
    Inserts,
    Sends,
    Channel,
    Notes,
}

impl Section {
    /// The key saved in the collapsed set.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Section::Routing => "routing",
            Section::Instrument => "instrument",
            Section::Inserts => "inserts",
            Section::Sends => "sends",
            Section::Channel => "channel",
            Section::Notes => "notes",
        }
    }
    pub(crate) fn title(self) -> &'static str {
        match self {
            Section::Routing => "Routing",
            Section::Instrument => "Instrument",
            Section::Inserts => "Inserts",
            Section::Sends => "Sends",
            Section::Channel => "Channel",
            Section::Notes => "Notes",
        }
    }
}

/// The sections a track kind gets, top to bottom.
pub(crate) fn inspector_sections(kind: TrackKind) -> &'static [Section] {
    use Section::{Channel, Inserts, Instrument, Notes, Routing, Sends};
    match kind {
        TrackKind::Audio | TrackKind::Aux => &[Routing, Inserts, Sends, Channel, Notes],
        TrackKind::Instrument => &[Routing, Instrument, Inserts, Sends, Channel, Notes],
        TrackKind::Midi => &[Routing, Notes],
        TrackKind::Master => &[Routing, Inserts, Channel],
        TrackKind::Vca => &[Channel, Notes],
        TrackKind::Folder | TrackKind::Video => &[Notes],
    }
}

/// Whether a kind shows an input selector / an output selector in Routing.
pub(crate) fn routing_rows(kind: TrackKind) -> (bool, bool) {
    match kind {
        TrackKind::Audio | TrackKind::Instrument | TrackKind::Aux => (true, true),
        TrackKind::Midi => (true, false),
        TrackKind::Master => (false, true),
        _ => (false, false),
    }
}

/// Pan knobs and the trim/phase row only on tracks that have them (as on the Mix strip).
pub(crate) fn channel_rows(kind: TrackKind) -> (bool, bool) {
    let pan = matches!(kind, TrackKind::Audio | TrackKind::Instrument | TrackKind::Aux);
    let trim = matches!(kind, TrackKind::Audio | TrackKind::Instrument);
    (pan, trim)
}

pub(crate) fn is_collapsed(app: &SoundApp, s: Section) -> bool {
    app.ui.inspector_collapsed.contains(s.id())
}

/// The section's title row; a click toggles it. Returns whether the body shows.
pub(crate) fn header(app: &mut SoundApp, ui: &mut Ui, s: Section) -> bool {
    let t = Tokens::current();
    let open = !is_collapsed(app, s);
    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 20.0), Sense::click());
    let label = format!("Inspector section {}", s.title());
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, open, &label));
    if resp.hovered() {
        ui.painter().rect_filled(r, 0.0, t.button);
    }
    let c = pos2(r.min.x + PAD + 4.0, r.center().y);
    let pts = if open {
        vec![c + vec2(-4.0, -2.5), c + vec2(4.0, -2.5), c + vec2(0.0, 3.0)]
    } else {
        vec![c + vec2(-2.5, -4.0), c + vec2(3.0, 0.0), c + vec2(-2.5, 4.0)]
    };
    ui.painter().add(Shape::convex_polygon(pts, t.section_label, Stroke::NONE));
    ui.painter().text(pos2(r.min.x + PAD + 14.0, r.center().y), Align2::LEFT_CENTER, s.title().to_uppercase(), regular(9.5), t.section_label);
    if resp.clicked() {
        if open {
            app.ui.inspector_collapsed.insert(s.id().to_string());
        } else {
            app.ui.inspector_collapsed.remove(s.id());
        }
    }
    open
}

/// The body of a section for `track`.
pub(crate) fn body(app: &mut SoundApp, ui: &mut Ui, s: Section, track: &Track) {
    match s {
        Section::Routing => routing(app, ui, track),
        Section::Instrument => instrument(app, ui, track),
        Section::Inserts => inserts(app, ui, track),
        Section::Sends => sends(app, ui, track),
        Section::Channel => channel(app, ui, track),
        Section::Notes => notes(app, ui, track),
    }
    ui.add_space(4.0);
}

/// A small square button inside a row; `label` is its accessible name.
fn mini(ui: &mut Ui, r: Rect, text: &str, on: bool, enabled: bool, tip: &str, label: &str) -> egui::Response {
    let t = Tokens::current();
    let resp = ui.interact(r, ui.id().with(label), if enabled { Sense::click() } else { Sense::hover() });
    let fill = if on { t.accent } else { key_fill(&resp) };
    key(ui.painter(), r, &resp, fill);
    let col = if !enabled {
        t.text_dim
    } else if on {
        t.accent_text
    } else {
        t.text
    };
    ui.painter().text(r.center(), Align2::CENTER_CENTER, text, bold(10.0), col);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    resp.on_hover_text(tip)
}

/// `n` buttons of `BTN` width at the right end of `row`, last first; returns their rects left to right.
fn right_buttons(row: Rect, n: usize) -> (Rect, Vec<Rect>) {
    let w = n as f32 * (BTN + 2.0);
    let cells = (0..n).map(|i| Rect::from_min_size(pos2(row.max.x - w + i as f32 * (BTN + 2.0), row.min.y), vec2(BTN, 16.0))).collect();
    (Rect::from_min_max(row.min, pos2(row.max.x - w - 2.0, row.max.y)), cells)
}

fn row_rect(ui: &mut Ui) -> Rect {
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), SLOT_H), Sense::hover());
    Rect::from_min_max(pos2(r.min.x + PAD, r.min.y + 1.0), pos2(r.max.x - PAD, r.min.y + 17.0))
}

fn routing(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let (input, output) = routing_rows(track.kind);
    for (is_input, shown, name, caption, label) in [
        (true, input, input_name(app, track), "In", "Inspector: Input"),
        (false, output, route_name(app, &track.mixer.output), "Out", "Inspector: Output"),
    ] {
        if !shown {
            continue;
        }
        let row = row_rect(ui);
        ui.painter().text(pos2(row.min.x, row.center().y), Align2::LEFT_CENTER, caption, regular(10.0), t.text_dim);
        let box_r = Rect::from_min_max(pos2(row.min.x + 26.0, row.min.y), row.max);
        let mut c = ui.new_child(egui::UiBuilder::new().id_salt((label, track.id.0)).max_rect(box_r));
        let resp = selector_box(&mut c, box_r.width(), box_r.height(), &name, t.text);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, label));
        route_popup(app, &resp, track.id, is_input);
    }
}

fn instrument(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let row = row_rect(ui);
    let (slot_r, b) = right_buttons(row, 2);
    mix_window::instrument_slot(app, ui, track, slot_r);
    let hosted = track.instrument.as_ref().is_some_and(|i| soundcraft_mix::is_third_party(&i.plugin));
    let win = (track.id, soundcraft_mix::INSTRUMENT_SLOT);
    if mini(ui, b[0], "E", false, hosted, "Open the instrument's editor", "Inspector: Open instrument editor").clicked()
        && !app.ui.plugin_windows.contains(&win)
    {
        app.ui.plugin_windows.push(win);
    }
    let chooser = mini(ui, b[1], "...", false, true, "Choose another instrument", "Inspector: Choose instrument");
    egui::Popup::menu(&chooser)
        .id(Id::new(("inspector-instrument-chooser", track.id.0)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            mix_window::instrument_menu(app, ui, track.id);
        });
}

const SHOW_ALL_KEY: &str = "inspector-show-all-slots";

fn show_all(ui: &Ui) -> bool {
    ui.data(|d| d.get_temp::<bool>(Id::new(SHOW_ALL_KEY))).unwrap_or(false)
}

fn inserts(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let id = track.id;
    let all = show_all(ui);
    // Add EQ / bypass EQs / show all.
    let row = row_rect(ui);
    let has_free = track.mixer.inserts.iter().any(Option::is_none);
    let third = (row.width() - 4.0) / 3.0;
    let cell = |i: f32| Rect::from_min_size(pos2(row.min.x + i * (third + 2.0), row.min.y), vec2(third, 16.0));
    let wide = |ui: &mut Ui, r: Rect, text: &str, on: bool, enabled: bool, tip: &str, label: &str| mini(ui, r, text, on, enabled, tip, label);
    if wide(ui, cell(0.0), "Add EQ", false, has_free, "Insert an equaliser in the first free slot", "Inspector: Add EQ").clicked() {
        let _ = app.run("mix.insert", json!({"track": id.0, "plugin": "eq_7band"}));
    }
    if wide(ui, cell(1.0), "EQ byp", false, true, "Bypass or restore the EQ inserts", "Inspector: Bypass EQ").clicked() {
        let _ = app.run("track.bypass_eq", json!({"tracks": [id.0]}));
    }
    if wide(ui, cell(2.0), "All", all, true, "Show all ten insert slots", "Inspector: Show all insert slots").clicked() {
        ui.data_mut(|d| d.insert_temp(Id::new(SHOW_ALL_KEY), !all));
    }
    let first_free = track.mixer.inserts.iter().position(Option::is_none);
    for slot in 0..INSERT_SLOTS {
        let filled = track.mixer.inserts.get(slot).is_some_and(Option::is_some);
        if !(all || filled || first_free == Some(slot)) {
            continue;
        }
        let row = row_rect(ui);
        if !filled {
            mix_window::insert_slot(app, ui, track, slot, row);
            continue;
        }
        let (slot_r, b) = right_buttons(row, 4);
        mix_window::insert_slot(app, ui, track, slot, slot_r);
        let letter = (b'A' + slot as u8) as char;
        let bypassed = track.mixer.inserts.get(slot).and_then(Option::as_ref).is_some_and(|i| i.bypass);
        if mini(ui, b[0], "B", bypassed, true, "Bypass", &format!("Inspector: Insert {letter} bypass")).clicked() {
            let _ = app.run("mix.insert_bypass", json!({"track": id.0, "slot": slot}));
        }
        if mini(ui, b[1], "^", false, slot > 0, "Move up", &format!("Inspector: Insert {letter} up")).clicked() {
            let _ = app.run("mix.insert_move", json!({"track": id.0, "from": slot, "to": slot - 1}));
        }
        if mini(ui, b[2], "v", false, slot + 1 < INSERT_SLOTS, "Move down", &format!("Inspector: Insert {letter} down")).clicked() {
            let _ = app.run("mix.insert_move", json!({"track": id.0, "from": slot, "to": slot + 1}));
        }
        if mini(ui, b[3], "x", false, true, "Remove", &format!("Inspector: Insert {letter} remove")).clicked() {
            let _ = app.run("mix.insert_remove", json!({"track": id.0, "slot": slot}));
        }
    }
    // Opening the editor stays on the slot itself (click), as on the Mix strip.
}

fn sends(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let id = track.id;
    let first_free = track.mixer.sends.iter().position(Option::is_none);
    for slot in 0..SEND_SLOTS {
        let snd = track.mixer.sends.get(slot).and_then(Option::as_ref);
        if snd.is_none() && first_free != Some(slot) {
            continue;
        }
        let row = row_rect(ui);
        let Some(send) = snd else {
            mix_window::send_slot(app, ui, track, slot, row);
            continue;
        };
        let letter = (b'A' + slot as u8) as char;
        let (rest, b) = right_buttons(row, 1);
        let db_w = 64.0;
        let slot_r = Rect::from_min_max(rest.min, pos2(rest.max.x - db_w - 2.0, rest.max.y));
        let db_r = Rect::from_min_max(pos2(rest.max.x - db_w, rest.min.y), rest.max);
        mix_window::send_slot(app, ui, track, slot, slot_r);
        let mut db = send.level_db;
        let mut c = ui.new_child(egui::UiBuilder::new().id_salt(("inspector-send-db", id.0, slot)).max_rect(db_r));
        let resp = c.add_sized(db_r.size(), egui::DragValue::new(&mut db).range(-80.0..=12.0).speed(0.1).suffix(" dB").max_decimals(1));
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, format!("Inspector: Send {letter} level")));
        if resp.changed() {
            let _ = app.engine.execute_merged("mix.send_level", &json!({"track": id.0, "slot": slot, "db": db}), &format!("send:{}:{slot}", id.0));
        }
        if resp.drag_stopped() || resp.lost_focus() {
            app.engine.end_merge();
        }
        if mini(ui, b[0], "x", false, true, "Remove send", &format!("Inspector: Send {letter} remove")).clicked() {
            let _ = app.run("mix.send_remove", json!({"track": id.0, "slot": slot}));
        }
    }
}

fn channel(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let id = track.id;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), FADER_H + 24.0), Sense::hover());
    let fader_r = Rect::from_min_size(pos2(r.min.x + PAD, r.min.y + 4.0), vec2(((r.width() - 2.0 * PAD) * 0.4).clamp(34.0, 60.0), FADER_H));
    mix_window::strip_fader(app, ui, fader_r, id, track.mixer.volume_db);
    let slot = fader_slot(fader_r);
    let x = fader_r.max.x + 14.0;
    ui.painter().text(pos2(x, slot.min.y + 8.0), Align2::LEFT_CENTER, "VOLUME", regular(9.5), t.text_dim);
    ui.painter().text(pos2(x, slot.min.y + 26.0), Align2::LEFT_CENTER, db_text(track.mixer.volume_db), bold(13.0), t.counter_text);
    let md = app.meters.get(&id).copied().unwrap_or_default();
    let mr = Rect::from_min_max(pos2(x, slot.min.y + 40.0), pos2((x + 30.0).min(r.max.x - PAD), slot.max.y));
    if mr.width() > 4.0 && mr.height() > 4.0 {
        mix_window::stereo_meters(ui, mr, &md);
    }
    let (pan, trim) = channel_rows(track.kind);
    if pan {
        for i in 0..track.mixer.pan.len().min(2) {
            let row = row_rect(ui);
            ui.painter().text(
                pos2(row.min.x, row.center().y),
                Align2::LEFT_CENTER,
                if track.mixer.pan.len() > 1 { if i == 0 { "Pan L" } else { "Pan R" } } else { "Pan" },
                regular(10.0),
                t.text_dim,
            );
            let mut v = track.mixer.pan.get(i).copied().unwrap_or(0.0);
            let before = v;
            let mut c = ui.new_child(
                egui::UiBuilder::new()
                    .id_salt(("inspector-pan", id.0, i))
                    .max_rect(Rect::from_min_size(pos2(row.min.x + 44.0, row.min.y - 2.0), vec2(20.0, 20.0))),
            );
            let resp = pan_knob(&mut c, 20.0, &mut v, "Pan (drag; double-click centres)");
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, true, format!("Inspector: Pan {i}")));
            if (v - before).abs() > f32::EPSILON {
                let _ = app.engine.execute_merged("mix.pan", &json!({"track": id.0, "pan": v, "index": i}), &format!("pan:{}:{i}", id.0));
            }
            ui.painter().text(pos2(row.min.x + 72.0, row.center().y), Align2::LEFT_CENTER, pan_text(v), regular(10.0), t.counter_text);
        }
    }
    if trim {
        let row = row_rect(ui);
        ui.painter().text(pos2(row.min.x, row.center().y), Align2::LEFT_CENTER, "Trim", regular(10.0), t.text_dim);
        let mut db = track.mixer.trim_db;
        let dr = Rect::from_min_size(pos2(row.min.x + 44.0, row.min.y), vec2(72.0, 16.0));
        let mut c = ui.new_child(egui::UiBuilder::new().id_salt(("inspector-trim", id.0)).max_rect(dr));
        let resp = c.add_sized(dr.size(), egui::DragValue::new(&mut db).range(-24.0..=24.0).speed(0.1).suffix(" dB").max_decimals(1));
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Inspector: Trim"));
        if resp.changed() {
            let _ = app.engine.execute_merged("mix.trim", &json!({"tracks": [id.0], "db": db}), &format!("trim:{}", id.0));
        }
        if resp.drag_stopped() || resp.lost_focus() {
            app.engine.end_merge();
        }
        let pr = Rect::from_min_size(pos2(dr.max.x + 8.0, row.min.y), vec2(22.0, 16.0));
        let on = track.mixer.phase_invert;
        let mut c = ui.new_child(egui::UiBuilder::new().id_salt(("inspector-phase", id.0)).max_rect(pr));
        let resp = text_toggle(&mut c, pr.size(), "\u{d8}", on, t.accent, "Phase invert");
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, "Inspector: Phase invert"));
        if resp.clicked() {
            let _ = app.run("mix.phase_invert", json!({"tracks": [id.0]}));
        }
    }
}

fn notes(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let mut c = track.comments.clone();
    let resp = ui.add_sized(
        vec2(ui.available_width() - 2.0 * PAD, 54.0),
        egui::TextEdit::multiline(&mut c).id(Id::new(("inspector-notes", track.id.0))).hint_text("notes").font(regular(10.0)),
    );
    if resp.changed() {
        let _ = app.engine.execute_merged("track.comments", &json!({"track": track.id.0, "comments": c}), &format!("comments:{}", track.id.0));
    }
    if resp.lost_focus() {
        app.engine.end_merge();
    }
}

/// The ids a colour pick or other bulk action applies to: the selection when it holds `id`, else just `id`.
pub(crate) fn acting_tracks(app: &SoundApp, id: TrackId) -> Vec<TrackId> {
    let sel = &app.engine.session().edit.selected_tracks;
    if sel.contains(&id) { sel.clone() } else { vec![id] }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_follow_the_track_kind() {
        use Section::*;
        assert_eq!(inspector_sections(TrackKind::Audio), &[Routing, Inserts, Sends, Channel, Notes]);
        assert!(inspector_sections(TrackKind::Instrument).contains(&Instrument));
        assert!(!inspector_sections(TrackKind::Audio).contains(&Instrument));
        assert_eq!(inspector_sections(TrackKind::Midi), &[Routing, Notes]);
        assert_eq!(inspector_sections(TrackKind::Folder), &[Notes]);
        assert_eq!(inspector_sections(TrackKind::Vca), &[Channel, Notes]);
        let master = inspector_sections(TrackKind::Master);
        assert!(master.contains(&Routing) && master.contains(&Channel) && !master.contains(&Notes) && !master.contains(&Sends));
        assert_eq!(routing_rows(TrackKind::Master), (false, true), "master hides its input");
        assert_eq!(routing_rows(TrackKind::Midi), (true, false));
        assert_eq!(channel_rows(TrackKind::Vca), (false, false));
    }

    #[test]
    fn every_section_has_a_unique_id() {
        use Section::*;
        let ids: Vec<_> = [Routing, Instrument, Inserts, Sends, Channel, Notes].iter().map(|s| s.id()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }
}
