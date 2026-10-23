//! The Inspector's collapsible sections: which ones a track kind gets, the collapse toggle, and
//! the body of each (routing, instrument, inserts, sends, channel, notes). Every control runs the
//! same command as the Mix strip; collapse state is UI-only and saved in the preferences.

use crate::SoundApp;
use crate::icons;
use crate::mix_window::{self, input_name, plugin_info, route_name, route_popup};
use crate::theme::{Tokens, bold, regular};
use crate::widgets::{db_text, fader_slot, pan_knob, pan_text, raised_key, shade, text_toggle};
use egui::{
    Align, Align2, Color32, Id, Layout, Margin, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, UiBuilder, WidgetInfo, WidgetType, pos2, vec2,
};
use serde_json::json;
use soundcraft_model::{INSERT_SLOTS, SEND_SLOTS, Track, TrackId, TrackKind};

/// Space left and right of the header and cards.
pub(crate) const PAD: f32 = 8.0;
/// Height of a slot row.
const ROW_H: f32 = 22.0;
const ROW_GAP: f32 = 4.0;
const FADER_H: f32 = 196.0;
/// Height of the equaliser curve card body.
const EQ_H: f32 = 72.0;
const RADIUS: f32 = 7.0;
const HEAD_H: f32 = 24.0;

/// One collapsible block of the Inspector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Section {
    Routing,
    Instrument,
    Midi,
    Inserts,
    Eq,
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
            Section::Midi => "midi",
            Section::Inserts => "inserts",
            Section::Eq => "eq",
            Section::Sends => "sends",
            Section::Channel => "channel",
            Section::Notes => "notes",
        }
    }
    pub(crate) fn title(self) -> &'static str {
        match self {
            Section::Routing => "Routing",
            Section::Instrument => "Instrument",
            Section::Midi => "MIDI",
            Section::Inserts => "Inserts",
            Section::Eq => "Equaliser",
            Section::Sends => "Sends",
            Section::Channel => "Channel",
            Section::Notes => "Notes",
        }
    }
}

/// The sections a track kind gets, top to bottom.
pub(crate) fn inspector_sections(kind: TrackKind) -> &'static [Section] {
    use Section::{Channel, Eq, Inserts, Instrument, Midi, Notes, Routing, Sends};
    match kind {
        TrackKind::Audio | TrackKind::Aux => &[Routing, Inserts, Eq, Sends, Channel, Notes],
        TrackKind::Instrument => &[Routing, Instrument, Midi, Inserts, Eq, Sends, Channel, Notes],
        TrackKind::Midi => &[Routing, Midi, Notes],
        TrackKind::Master => &[Routing, Inserts, Eq, Channel],
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

/// Black or white text, whichever reads on `fill`.
pub(crate) fn contrast_text(fill: Color32, t: &Tokens) -> Color32 {
    let lum = 0.299 * f32::from(fill.r()) + 0.587 * f32::from(fill.g()) + 0.114 * f32::from(fill.b());
    if lum > 140.0 { t.text_dark } else { t.text }
}

/// A small icon button in a card header or row; `icon` is an `icons::draw` name, "dots" is three dots.
fn icon_button(ui: &mut Ui, r: Rect, icon: &str, enabled: bool, tip: &str, label: &str) -> Response {
    let t = Tokens::current();
    let resp = ui.interact(r, ui.id().with(label), if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    let col = if !enabled {
        t.text_dim.gamma_multiply(0.5)
    } else if resp.hovered() {
        t.text
    } else {
        t.text_dim
    };
    if resp.hovered() && enabled {
        ui.painter().rect_filled(r, 5.0, t.button_hi);
    }
    if icon == "dots" {
        for dx in [-4.0, 0.0, 4.0] {
            ui.painter().circle_filled(r.center() + vec2(dx, 0.0), 1.3, col);
        }
    } else {
        icons::draw(ui.painter(), Rect::from_center_size(r.center(), vec2(10.0, 10.0)), icon, col);
    }
    resp.on_hover_text(tip)
}

/// A status LED: lit in `color` when on, a dark well when off.
fn led(ui: &Ui, c: egui::Pos2, on: bool, color: Color32, hovered: bool) {
    let t = Tokens::current();
    let p = ui.painter();
    if on {
        p.circle_filled(c, 5.5, color.gamma_multiply(0.25));
        p.circle_filled(c, 3.6, color);
        p.circle_filled(c + vec2(-0.8, -0.9), 1.2, Color32::from_white_alpha(120));
    } else {
        p.circle_filled(c, 3.6, t.slot_empty);
        p.circle_stroke(c, 3.6, Stroke::new(1.0, if hovered { t.text_dim } else { t.border }));
    }
}

/// The Inspector's cards for `track`: header bar (click to collapse) and, when open, the body.
pub(crate) fn card(app: &mut SoundApp, ui: &mut Ui, s: Section, track: &Track) {
    let t = Tokens::current();
    let open = !is_collapsed(app, s);
    egui::Frame::new()
        .outer_margin(Margin { left: (PAD - 2.0) as i8, right: (PAD - 2.0) as i8, top: 0, bottom: 6 })
        .fill(t.strip_section)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(RADIUS)
        .inner_margin(Margin::ZERO)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing = vec2(0.0, 0.0);
            let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), HEAD_H), Sense::click());
            let label = format!("Inspector section {}", s.title());
            resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, open, &label));
            let c = pos2(r.min.x + 12.0, r.center().y);
            let pts = if open {
                vec![c + vec2(-3.5, -2.0), c + vec2(3.5, -2.0), c + vec2(0.0, 2.5)]
            } else {
                vec![c + vec2(-2.0, -3.5), c + vec2(2.5, 0.0), c + vec2(-2.0, 3.5)]
            };
            let col = if resp.hovered() { t.text } else { t.section_label };
            ui.painter().add(Shape::convex_polygon(pts, col, Stroke::NONE));
            let title = ui.painter().layout_no_wrap(s.title().to_uppercase(), bold(9.5), col);
            let tx = r.min.x + 22.0;
            ui.painter().galley(pos2(tx, r.center().y - title.size().y / 2.0), title.clone(), col);
            if let Some(n) = count(s, track) {
                let g = ui.painter().layout_no_wrap(n.to_string(), bold(9.0), t.text_dim);
                let pill = Rect::from_min_size(pos2(tx + title.size().x + 6.0, r.center().y - 7.0), vec2(g.size().x + 10.0, 14.0));
                ui.painter().rect_filled(pill, 7.0, t.slot_bg);
                ui.painter().galley(pill.center() - g.size() / 2.0, g, t.text_dim);
            }
            if resp.clicked() {
                if open {
                    app.ui.inspector_collapsed.insert(s.id().to_string());
                } else {
                    app.ui.inspector_collapsed.remove(s.id());
                }
            }
            if s == Section::Inserts {
                insert_header_menus(app, ui, r, track);
            }
            if !open {
                return;
            }
            ui.painter().hline(r.min.x + 1.0..=r.max.x - 1.0, r.max.y, Stroke::new(1.0, t.border));
            egui::Frame::new().inner_margin(Margin { left: 8, right: 8, top: 8, bottom: 8 }).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing = vec2(4.0, ROW_GAP);
                body(app, ui, s, track);
            });
        });
}

/// The badge number of a section: filled inserts or sends.
fn count(s: Section, track: &Track) -> Option<usize> {
    match s {
        Section::Inserts => Some(track.mixer.inserts.iter().flatten().count()),
        Section::Sends => Some(track.mixer.sends.iter().flatten().count()),
        _ => None,
    }
}

/// The body of a section for `track`.
pub(crate) fn body(app: &mut SoundApp, ui: &mut Ui, s: Section, track: &Track) {
    match s {
        Section::Routing => routing(app, ui, track),
        Section::Instrument => instrument(app, ui, track),
        Section::Midi => midi(app, ui, track),
        Section::Inserts => inserts(app, ui, track),
        Section::Eq => eq_card(app, ui, track),
        Section::Sends => sends(app, ui, track),
        Section::Channel => channel(app, ui, track),
        Section::Notes => notes(app, ui, track),
    }
}

/// A full-width row of `h` points.
fn row(ui: &mut Ui, h: f32) -> Rect {
    ui.allocate_exact_size(vec2(ui.available_width(), h), Sense::hover()).0
}

/// True while the pointer is over `r` (also when another widget of the row is under it).
fn row_hot(ui: &Ui, r: Rect) -> bool {
    ui.rect_contains_pointer(r) || ui.input(|i| i.pointer.hover_pos()).is_some_and(|p| r.contains(p))
}

/// The rounded well behind a slot row.
fn well(ui: &Ui, r: Rect, hovered: bool, empty: bool) {
    let t = Tokens::current();
    let fill = if empty { t.slot_empty } else { t.slot_bg };
    let fill = if hovered { fill.lerp_to_gamma(t.button_hi, 0.35) } else { fill };
    ui.painter().rect(r, 5.0, fill, Stroke::new(1.0, if empty { t.border.gamma_multiply(0.8) } else { t.border }), StrokeKind::Inside);
}

/// Text clipped to `r`, vertically centred and left aligned.
fn clipped_text(ui: &Ui, r: Rect, text: &str, font: egui::FontId, color: Color32) {
    let g = ui.painter().layout_no_wrap(text.to_string(), font, color);
    ui.painter().with_clip_rect(r).galley(pos2(r.min.x, r.center().y - g.size().y / 2.0), g, color);
}

fn routing(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let (input, output) = routing_rows(track.kind);
    for (is_input, shown, name, caption, label) in [
        (true, input, input_name(app, track), "IN", "Inspector: Input"),
        (false, output, route_name(app, &track.mixer.output), "OUT", "Inspector: Output"),
    ] {
        if !shown {
            continue;
        }
        let r = row(ui, ROW_H);
        let chip = Rect::from_min_size(r.min, vec2(32.0, ROW_H));
        ui.painter().rect_filled(chip, 5.0, t.slot_bg);
        ui.painter().text(chip.center(), Align2::CENTER_CENTER, caption, bold(9.0), t.text_dim);
        let box_r = Rect::from_min_max(pos2(chip.max.x + 4.0, r.min.y), r.max);
        let resp = ui.interact(box_r, Id::new((label, track.id.0)), Sense::click());
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, true, label));
        well(ui, box_r, resp.hovered(), false);
        clipped_text(ui, Rect::from_min_max(box_r.min + vec2(8.0, 0.0), pos2(box_r.max.x - 18.0, box_r.max.y)), &name, regular(11.0), t.text);
        icons::draw(ui.painter(), Rect::from_center_size(pos2(box_r.max.x - 10.0, box_r.center().y), vec2(8.0, 8.0)), "triangle_down", t.text_dim);
        route_popup(app, &resp, track.id, is_input);
    }
}

fn instrument(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let (id, slot) = (track.id, soundcraft_mix::INSTRUMENT_SLOT);
    let r = row(ui, 26.0);
    let b = 24.0;
    let slot_r = Rect::from_min_max(r.min, pos2(r.max.x - 2.0 * (b + 3.0), r.max.y));
    let edit_r = Rect::from_min_size(pos2(r.max.x - 2.0 * b - 3.0, r.min.y + 1.0), vec2(b, b));
    let choose_r = Rect::from_min_size(pos2(r.max.x - b, r.min.y + 1.0), vec2(b, b));
    let ins = track.instrument.as_ref();
    let info = ins.and_then(|i| plugin_info(&i.plugin));
    let resp = ui.interact(slot_r, ui.id().with(("instrument", id.0)), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Instrument"));
    well(ui, slot_r, resp.hovered(), ins.is_none());
    let led_r = Rect::from_center_size(pos2(slot_r.min.x + 12.0, slot_r.center().y), vec2(18.0, ROW_H));
    let led_resp = ui.interact(led_r, ui.id().with(("instrument-led", id.0)), if ins.is_some() { Sense::click() } else { Sense::hover() });
    led_resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, ins.is_some(), ins.is_some_and(|i| !i.bypass), "Inspector: Instrument bypass"));
    let live = ins.is_some_and(|i| !i.bypass);
    led(ui, led_r.center(), live, t.meter_green, led_resp.hovered() || resp.hovered());
    if led_resp.clicked() {
        let _ = app.run("mix.instrument_bypass", json!({"track": id.0}));
    }
    let _ = led_resp.on_hover_text(if ins.is_none() {
        "No instrument"
    } else if live {
        "Active: click to bypass the instrument"
    } else {
        "Bypassed: click to enable"
    });
    let text_r = Rect::from_min_max(pos2(slot_r.min.x + 24.0, slot_r.min.y), pos2(slot_r.max.x - 6.0, slot_r.max.y));
    match info {
        Some(i) => clipped_text(ui, text_r, i.short_name, bold(11.0), if live { t.text } else { t.text_dim }),
        None => clipped_text(ui, text_r, "Select instrument...", regular(11.0), t.text_dim),
    }
    let hosted = ins.is_some_and(|i| soundcraft_mix::is_third_party(&i.plugin));
    let win = (id, slot);
    if resp.clicked() && !led_r.contains(resp.interact_pointer_pos().unwrap_or_default()) && hosted && !app.ui.plugin_windows.contains(&win) {
        app.ui.plugin_windows.push(win);
    }
    let menu_resp = if hosted { resp.clone().on_hover_text("Click: open instrument · right-click: change") } else { resp.clone() };
    let popup = if hosted { egui::Popup::context_menu(&menu_resp) } else { egui::Popup::menu(&menu_resp) };
    popup.close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| mix_window::instrument_menu(app, ui, id));
    crate::sampler_window::drop_target(app, ui.ctx(), slot_r, id);
    if icon_button(ui, edit_r, "float", hosted, "Open the instrument's editor", "Inspector: Open instrument editor").clicked()
        && !app.ui.plugin_windows.contains(&win)
    {
        app.ui.plugin_windows.push(win);
    }
    let chooser = icon_button(ui, choose_r, "triangle_down", true, "Choose another instrument", "Inspector: Choose instrument");
    egui::Popup::menu(&chooser)
        .id(Id::new(("inspector-instrument-chooser", id.0)))
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| mix_window::instrument_menu(app, ui, id));
}

const SHOW_ALL_KEY: &str = "inspector-show-all-slots";

fn show_all(ui: &Ui) -> bool {
    ui.data(|d| d.get_temp::<bool>(Id::new(SHOW_ALL_KEY))).unwrap_or(false)
}

/// The "+" and "..." buttons at the right of the Inserts header.
fn insert_header_menus(app: &mut SoundApp, ui: &mut Ui, head: Rect, track: &Track) {
    let id = track.id;
    let b = 18.0;
    let dots = Rect::from_min_size(pos2(head.max.x - b - 5.0, head.center().y - b / 2.0), vec2(b, b));
    let plus = dots.translate(vec2(-b - 2.0, 0.0));
    let first_free = track.mixer.inserts.iter().position(Option::is_none);
    let add = icon_button(ui, plus, "plus", first_free.is_some(), "Add an effect", "Inspector: Add effect menu");
    egui::Popup::menu(&add).id(Id::new(("inspector-add-effect", id.0))).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
        if ui.button("Add EQ").on_hover_text("Insert an equaliser in the first free slot").clicked() {
            let _ = app.run("mix.insert", json!({"track": id.0, "plugin": "eq_7band"}));
            ui.close();
        }
        ui.separator();
        if let Some(slot) = first_free {
            mix_window::plugin_menu(app, ui, id, slot, false);
        }
    });
    let more = icon_button(ui, dots, "dots", true, "Insert options", "Inspector: Insert options");
    egui::Popup::menu(&more).id(Id::new(("inspector-insert-options", id.0))).show(|ui| {
        if ui.button("Bypass or restore EQ inserts").clicked() {
            let _ = app.run("track.bypass_eq", json!({"tracks": [id.0]}));
        }
        let mut all = show_all(ui);
        if ui.checkbox(&mut all, "Show all ten slots").changed() {
            ui.data_mut(|d| d.insert_temp(Id::new(SHOW_ALL_KEY), all));
        }
    });
}

fn inserts(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let id = track.id;
    let all = show_all(ui);
    let first_free = track.mixer.inserts.iter().position(Option::is_none);
    for slot in 0..INSERT_SLOTS {
        let ins = track.mixer.inserts.get(slot).cloned().flatten();
        if !(all || ins.is_some() || first_free == Some(slot)) {
            continue;
        }
        let letter = (b'A' + slot as u8) as char;
        let r = row(ui, ROW_H);
        let resp = ui.interact(r, ui.id().with(("ins", id.0, slot)), Sense::click());
        let hot = row_hot(ui, r);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, format!("Insert {letter}")));
        let Some(i) = ins else {
            well(ui, r, resp.hovered(), true);
            clipped_text(ui, r.shrink2(vec2(10.0, 0.0)), "+  Add effect", regular(11.0), if resp.hovered() { t.text } else { t.text_dim });
            egui::Popup::menu(&resp)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| mix_window::plugin_menu(app, ui, id, slot, false));
            continue;
        };
        let info = plugin_info(&i.plugin);
        let name = info.map_or(i.plugin.as_str(), |p| p.short_name);
        well(ui, r, resp.hovered(), false);
        // Actions at the right, drawn and active only while the row is under the pointer.
        let b = 18.0;
        let ay = r.center().y - b / 2.0;
        let acts = |n: f32| Rect::from_min_size(pos2(r.max.x - 3.0 - (3.0 - n) * (b + 1.0) + 1.0, ay), vec2(b, b));
        let name_right = if hot { r.max.x - 3.0 * (b + 1.0) - 4.0 } else { r.max.x - 6.0 };
        let led_r = Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(16.0, ROW_H));
        let led_resp = ui.interact(led_r, ui.id().with(("ins-led", id.0, slot)), Sense::click());
        led_resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, format!("Inspector: Insert {letter} bypass")));
        led(ui, led_r.center(), !i.bypass, t.meter_green, led_resp.hovered());
        if led_resp.clicked() {
            let _ = app.run("mix.insert_bypass", json!({"track": id.0, "slot": slot}));
        }
        let _ = led_resp.clone().on_hover_text(if i.bypass { "Bypassed: click to enable" } else { "Active: click to bypass" });
        let colr = if i.bypass || info.is_none() { t.text_dim } else { t.text };
        clipped_text(ui, Rect::from_min_max(pos2(r.min.x + 26.0, r.min.y), pos2(name_right, r.max.y)), name, regular(11.0), colr);
        if resp.clicked() && !led_resp.hovered() {
            if ui.input(|i| i.modifiers.command) {
                let _ = app.run("mix.insert_bypass", json!({"track": id.0, "slot": slot}));
            } else if !app.ui.plugin_windows.contains(&(id, slot)) {
                app.ui.plugin_windows.push((id, slot));
            }
        }
        let cmd = crate::shortcuts::shortcut_label("Cmd", ui.ctx().os().is_mac());
        let tip = format!("{name}: click to open · {cmd}+click to bypass · right-click to change");
        egui::Popup::context_menu(&resp.clone().on_hover_text(tip))
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| mix_window::plugin_menu(app, ui, id, slot, true));
        // The actions exist only while the row is under the pointer.
        if hot {
            let up = icon_button(ui, acts(0.0), "triangle_up", slot > 0, "Move up", &format!("Inspector: Insert {letter} up"));
            let dn = icon_button(ui, acts(1.0), "triangle_down", slot + 1 < INSERT_SLOTS, "Move down", &format!("Inspector: Insert {letter} down"));
            let rm = icon_button(ui, acts(2.0), "close", true, "Remove", &format!("Inspector: Insert {letter} remove"));
            if up.clicked() {
                let _ = app.run("mix.insert_move", json!({"track": id.0, "from": slot, "to": slot - 1}));
            }
            if dn.clicked() {
                let _ = app.run("mix.insert_move", json!({"track": id.0, "from": slot, "to": slot + 1}));
            }
            if rm.clicked() {
                let _ = app.run("mix.insert_remove", json!({"track": id.0, "slot": slot}));
            }
        }
    }
}

fn sends(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let id = track.id;
    let first_free = track.mixer.sends.iter().position(Option::is_none);
    for slot in 0..SEND_SLOTS {
        let snd = track.mixer.sends.get(slot).and_then(Option::as_ref);
        if snd.is_none() && first_free != Some(slot) {
            continue;
        }
        let letter = (b'A' + slot as u8) as char;
        let r = row(ui, ROW_H);
        let hot = row_hot(ui, r);
        let Some(send) = snd else {
            let resp = ui.interact(r, ui.id().with(("snd-route", id.0, slot)), Sense::click());
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, format!("Send {letter}")));
            well(ui, r, resp.hovered(), true);
            clipped_text(ui, r.shrink2(vec2(10.0, 0.0)), "+  Add send", regular(11.0), if resp.hovered() { t.text } else { t.text_dim });
            egui::Popup::menu(&resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                ui.set_min_width(250.0);
                ui.strong(format!("Send {letter} · Choose destination"));
                mix_window::send_route_menu(app, ui, id, slot, None);
            });
            continue;
        };
        well(ui, r, false, false);
        let chip = Rect::from_center_size(pos2(r.min.x + 13.0, r.center().y), vec2(16.0, 16.0));
        ui.painter().rect_filled(chip, 4.0, if send.mute { t.slot_empty } else { t.accent_dark });
        ui.painter().text(chip.center(), Align2::CENTER_CENTER, letter, bold(9.5), if send.mute { t.text_dim } else { t.accent_text });
        let b = 18.0;
        let rm_r = Rect::from_min_size(pos2(r.max.x - b - 2.0, r.center().y - b / 2.0), vec2(b, b));
        let mute_r = Rect::from_min_size(pos2(rm_r.min.x - b - 1.0, rm_r.min.y), vec2(b, b));
        let level_r = Rect::from_min_size(pos2(mute_r.min.x - 62.0, r.center().y - 8.0), vec2(60.0, 16.0));
        let name_r = Rect::from_min_max(pos2(chip.max.x + 6.0, r.min.y), pos2(level_r.min.x - 4.0, r.max.y));
        let name_resp = ui.interact(name_r, ui.id().with(("snd-route", id.0, slot)), Sense::click());
        name_resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, format!("Send {letter}")));
        let name = route_name(app, &send.target);
        clipped_text(ui, name_r, &name, regular(11.0), if send.mute { t.text_dim } else { t.text });
        egui::Popup::menu(&name_resp).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
            ui.set_min_width(250.0);
            ui.strong(format!("Send {letter} · {name}"));
            mix_window::send_route_menu(app, ui, id, slot, Some(&send.target));
            ui.separator();
            if ui.button("Remove send").clicked() {
                let _ = app.run("mix.send_remove", json!({"track": id.0, "slot": slot}));
                ui.close();
            }
        });
        // Level: a bar with the dB readout; drag to adjust, double-click for 0 dB.
        let lr = ui.interact(level_r, ui.id().with(("snd-level", id.0, slot)), Sense::click_and_drag());
        lr.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, true, format!("Inspector: Send {letter} level")));
        ui.painter().rect(level_r, 4.0, t.slot_empty, Stroke::new(1.0, t.border), StrokeKind::Inside);
        let k = soundcraft_model::fader_db_to_pos(send.level_db).clamp(0.0, 1.0);
        let fill = Rect::from_min_max(level_r.min + vec2(1.0, 1.0), pos2(level_r.min.x + 1.0 + (level_r.width() - 2.0) * k, level_r.max.y - 1.0));
        if fill.width() > 1.0 {
            ui.painter().rect_filled(fill, 3.0, if send.mute { t.text_dim.gamma_multiply(0.35) } else { t.accent.gamma_multiply(0.6) });
        }
        let readout = if send.level_db <= -80.0 { "-inf".to_string() } else { format!("{:.1} dB", send.level_db) };
        ui.painter().text(level_r.center(), Align2::CENTER_CENTER, readout, bold(9.5), t.text);
        if lr.dragged() {
            let (dx, fine) = ui.input(|i| (i.pointer.delta().x, i.modifiers.shift));
            let pos = (k + dx / (level_r.width() * if fine { 4.0 } else { 1.0 })).clamp(0.0, 1.0);
            let db = soundcraft_model::fader_pos_to_db(pos).clamp(-144.0, 12.0);
            let _ = app.engine.execute_merged("mix.send_level", &json!({"track": id.0, "slot": slot, "db": db}), &format!("send:{}:{slot}", id.0));
        }
        if lr.double_clicked() {
            let _ = app.run("mix.send_level", json!({"track": id.0, "slot": slot, "db": 0.0}));
        }
        if lr.drag_stopped() {
            app.engine.end_merge();
        }
        let _ = lr.on_hover_text("Send level: drag to adjust (Shift for fine), double-click for 0 dB");
        let mr = ui.interact(mute_r, ui.id().with(("snd-mute", id.0, slot)), Sense::click());
        mr.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, send.mute, format!("Inspector: Send {letter} mute")));
        if send.mute {
            ui.painter().rect_filled(mute_r, 5.0, t.mute);
        } else {
            raised_key(ui.painter(), mute_r, [shade(t.button, 0.08), shade(t.button, -0.08)], mr.hovered());
        }
        ui.painter().text(mute_r.center(), Align2::CENTER_CENTER, "M", bold(9.5), if send.mute { t.text_dark } else { t.text_dim });
        if mr.clicked() {
            let _ = app.run("mix.send_level", json!({"track": id.0, "slot": slot, "mute": !send.mute}));
        }
        let _ = mr.on_hover_text("Mute send");
        if hot && icon_button(ui, rm_r, "close", true, "Remove send", &format!("Inspector: Send {letter} remove")).clicked() {
            let _ = app.run("mix.send_remove", json!({"track": id.0, "slot": slot}));
        }
    }
}

/// The centre of a stereo pair's two panners (the single panner of a mono track).
pub(crate) fn pan_centre(pan: &[f32]) -> f32 {
    match pan {
        [m] => *m,
        [l, r, ..] => (l + r) * 0.5,
        [] => 0.0,
    }
}

/// The stereo width of a pair of panners, -1..1 (1 = hard left and right, 0 = mono, negative
/// swaps the sides); none on a mono track.
pub(crate) fn pan_width(pan: &[f32]) -> Option<f32> {
    match pan {
        [l, r, ..] => Some((r - l) * 0.5),
        _ => None,
    }
}

/// Moves a stereo pair so its centre is `centre`, each side limited to -1..1.
pub(crate) fn shift_pan(l: f32, r: f32, centre: f32) -> (f32, f32) {
    let d = centre - (l + r) * 0.5;
    ((l + d).clamp(-1.0, 1.0), (r + d).clamp(-1.0, 1.0))
}

/// Sets the width of a stereo pair around its centre, each side limited to -1..1.
pub(crate) fn set_width(l: f32, r: f32, width: f32) -> (f32, f32) {
    let c = (l + r) * 0.5;
    let w = width.clamp(-1.0, 1.0);
    ((c - w).clamp(-1.0, 1.0), (c + w).clamp(-1.0, 1.0))
}

/// Writes a stereo pair of panners as one undo step.
fn write_pan_pair(app: &mut SoundApp, id: TrackId, l: f32, r: f32) {
    let key = format!("pan:{}", id.0);
    let _ = app.engine.execute_merged("mix.pan", &json!({"track": id.0, "pan": l, "index": 0}), &key);
    let _ = app.engine.execute_merged("mix.pan", &json!({"track": id.0, "pan": r, "index": 1}), &key);
}

fn channel(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let id = track.id;
    let (pan_on, trim_on) = channel_rows(track.kind);
    let stereo = track.mixer.pan.len() >= 2;
    let needed = 50.0 + if pan_on { if stereo { 52.0 } else { 26.0 } } else { 0.0 } + if trim_on { 52.0 } else { 0.0 };
    let h = FADER_H.max(needed);
    let r = row(ui, h);
    let fader_r = Rect::from_min_size(r.min, vec2(38.0, FADER_H));
    mix_window::strip_fader(app, ui, fader_r, id, track.mixer.volume_db);
    let fine = crate::shortcuts::shortcut_label("Cmd", ui.ctx().os().is_mac());
    let fr = ui.interact(fader_r, ui.id().with(("inspector-fader-tip", id.0)), Sense::hover());
    fr.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, true, "Inspector: Volume fader"));
    let _ = fr.on_hover_text(format!("Volume: drag to set, hold {fine} for fine adjustment, double-click for 0 dB"));
    let slot = fader_slot(fader_r);
    let md = app.meters.get(&id).copied().unwrap_or_default();
    let mr = Rect::from_min_max(pos2(fader_r.max.x + 6.0, slot.min.y), pos2(fader_r.max.x + 30.0, slot.max.y));
    mix_window::stereo_meters(ui, mr, &md);
    // Right column: volume readout, pan, trim and phase.
    let col = Rect::from_min_max(pos2(mr.max.x + 12.0, r.min.y), r.max);
    let mut c = ui.new_child(UiBuilder::new().id_salt(("inspector-channel", id.0)).max_rect(col).layout(Layout::top_down(Align::Min)));
    c.spacing_mut().item_spacing = vec2(6.0, 6.0);
    c.label(egui::RichText::new("VOLUME").font(bold(9.0)).color(t.text_dim));
    let (rd, _) = c.allocate_exact_size(vec2(c.available_width().min(96.0), 24.0), Sense::hover());
    c.painter().rect(rd, 5.0, t.counter_bg, Stroke::new(1.0, t.border), StrokeKind::Inside);
    // The readout is also a fine control: drag it for 0.1 dB steps, double-click to type a value.
    let mut db = track.mixer.volume_db.max(-144.0);
    let dv = c.put(
        rd.shrink2(vec2(2.0, 2.0)),
        egui::DragValue::new(&mut db)
            .range(-144.0..=12.0)
            .speed(0.1)
            .max_decimals(1)
            .custom_formatter(|v, _| db_text(v as f32))
            .custom_parser(|s| if s.trim().eq_ignore_ascii_case("-inf") { Some(-144.0) } else { s.trim().parse().ok() }),
    );
    dv.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Inspector: Volume"));
    if dv.changed() {
        let _ = app.engine.execute_merged("mix.volume", &json!({"track": id.0, "db": db}), &format!("fader:{}", id.0));
    }
    if dv.drag_stopped() || dv.lost_focus() {
        app.engine.end_merge();
    }
    c.add_space(2.0);
    if pan_on && !track.mixer.pan.is_empty() {
        // One pan control; stereo tracks add a width control (the two panners are derived from both).
        let (pl, pr) = (track.mixer.pan.first().copied().unwrap_or(0.0), track.mixer.pan.get(1).copied().unwrap_or(0.0));
        c.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_sized(vec2(34.0, 20.0), egui::Label::new(egui::RichText::new("Pan").font(regular(10.0)).color(t.text_dim)));
            let before = pan_centre(&track.mixer.pan);
            let mut v = before;
            let resp = pan_knob(ui, 20.0, &mut v, "Pan (drag; double-click centres)");
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Slider, true, "Inspector: Pan"));
            if (v - before).abs() > f32::EPSILON {
                if stereo {
                    let (l, r) = shift_pan(pl, pr, v);
                    write_pan_pair(app, id, l, r);
                } else {
                    let _ = app.engine.execute_merged("mix.pan", &json!({"track": id.0, "pan": v, "index": 0}), &format!("pan:{}", id.0));
                }
            }
            if resp.drag_stopped() {
                app.engine.end_merge();
            }
            ui.label(egui::RichText::new(pan_text(v)).font(regular(10.0)).color(t.counter_text));
        });
        if let Some(width) = pan_width(&track.mixer.pan) {
            c.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.add_sized(vec2(34.0, 20.0), egui::Label::new(egui::RichText::new("Width").font(regular(10.0)).color(t.text_dim)));
                let mut pct = width * 100.0;
                let resp =
                    ui.add_sized(vec2(62.0, 18.0), egui::DragValue::new(&mut pct).range(-100.0..=100.0).speed(0.5).suffix(" %").max_decimals(0));
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Inspector: Width"));
                if resp.changed() {
                    let (l, r) = set_width(pl, pr, pct / 100.0);
                    write_pan_pair(app, id, l, r);
                }
                if resp.drag_stopped() || resp.lost_focus() {
                    app.engine.end_merge();
                }
            });
        }
    }
    if trim_on {
        c.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_sized(vec2(34.0, 20.0), egui::Label::new(egui::RichText::new("Trim").font(regular(10.0)).color(t.text_dim)));
            let mut db = track.mixer.trim_db;
            let resp = ui.add_sized(vec2(62.0, 18.0), egui::DragValue::new(&mut db).range(-24.0..=24.0).speed(0.1).suffix(" dB").max_decimals(1));
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Inspector: Trim"));
            if resp.changed() {
                let _ = app.engine.execute_merged("mix.trim", &json!({"tracks": [id.0], "db": db}), &format!("trim:{}", id.0));
            }
            if resp.drag_stopped() || resp.lost_focus() {
                app.engine.end_merge();
            }
        });
        c.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.add_sized(vec2(34.0, 20.0), egui::Label::new(egui::RichText::new("Phase").font(regular(10.0)).color(t.text_dim)));
            let on = track.mixer.phase_invert;
            let resp = text_toggle(ui, vec2(32.0, 20.0), "\u{d8}", on, t.accent, "Phase invert");
            resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, true, on, "Inspector: Phase invert"));
            if resp.clicked() {
                let _ = app.run("mix.phase_invert", json!({"tracks": [id.0]}));
            }
        });
    }
}

/// The MIDI values of a track as the Inspector shows them: transpose in semitones, velocity
/// offset and the channel override (1..16, none = each note's own channel). These are the values
/// `track.midi_params_get` reports.
pub fn midi_values(s: &soundcraft_model::Session, id: TrackId) -> (i32, i32, Option<u8>) {
    let get = |key: String| s.edit.values.get(&key).copied().filter(|v| v.is_finite());
    let transpose = get(format!("rtp.{}.transpose", id.0)).unwrap_or(0.0).round().clamp(-48.0, 48.0) as i32;
    let velocity = get(format!("rtp.{}.velocity", id.0)).unwrap_or(0.0).round().clamp(-127.0, 127.0) as i32;
    let channel = get(format!("midi.channel.{}", id.0)).filter(|c| (1.0..=16.0).contains(c)).map(|c| c.round() as u8);
    (transpose, velocity, channel)
}

/// The MIDI section: non-destructive transpose, velocity offset and channel override.
fn midi(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let id = track.id;
    let (transpose, velocity, channel) = midi_values(app.engine.session(), id);
    let label = |ui: &mut Ui, text: &str| {
        ui.add_sized(vec2(62.0, 20.0), egui::Label::new(egui::RichText::new(text).font(regular(10.0)).color(t.text_dim)));
    };
    let key = format!("midi:{}", id.0);
    ui.horizontal(|ui| {
        label(ui, "Transpose");
        let mut v = transpose;
        let resp = ui.add_sized(vec2(78.0, 18.0), egui::DragValue::new(&mut v).range(-48..=48).speed(0.1).suffix(" st"));
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Inspector: Transpose"));
        if resp.changed() {
            let _ = app.engine.execute_merged("track.midi_params", &json!({"tracks": [id.0], "transpose": v}), &key);
        }
        if resp.drag_stopped() || resp.lost_focus() {
            app.engine.end_merge();
        }
        let _ = resp.on_hover_text("Transpose the notes this track plays (the clips are untouched)");
    });
    ui.horizontal(|ui| {
        label(ui, "Velocity");
        let mut v = velocity;
        let resp =
            ui.add_sized(vec2(78.0, 18.0), egui::DragValue::new(&mut v).range(-127..=127).speed(0.5).custom_formatter(|n, _| format!("{n:+}")));
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Inspector: Velocity offset"));
        if resp.changed() {
            let _ = app.engine.execute_merged("track.midi_params", &json!({"tracks": [id.0], "velocity": v}), &key);
        }
        if resp.drag_stopped() || resp.lost_focus() {
            app.engine.end_merge();
        }
        let _ = resp.on_hover_text("Add to every note's velocity (the clips are untouched)");
    });
    ui.horizontal(|ui| {
        label(ui, "Channel");
        let mut v = i32::from(channel.unwrap_or(0));
        let resp = ui.add_sized(
            vec2(78.0, 18.0),
            egui::DragValue::new(&mut v)
                .range(0..=16)
                .speed(0.1)
                .custom_formatter(|n, _| if n < 0.5 { "Auto".to_string() } else { format!("{n:.0}") })
                .custom_parser(|s| if s.trim().eq_ignore_ascii_case("auto") { Some(0.0) } else { s.trim().parse().ok() }),
        );
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, true, "Inspector: MIDI channel"));
        if resp.changed() {
            let ch = if v <= 0 { serde_json::Value::Null } else { json!(v) };
            let _ = app.engine.execute_merged("track.midi_params", &json!({"tracks": [id.0], "channel": ch}), &key);
        }
        if resp.drag_stopped() || resp.lost_focus() {
            app.engine.end_merge();
        }
        let _ = resp.on_hover_text("Send everything on one channel (Auto keeps each note's own)");
    });
}

/// The first equaliser insert of a track (slot, insert).
pub(crate) fn first_eq(track: &Track) -> Option<(usize, &soundcraft_model::Insert)> {
    track
        .mixer
        .inserts
        .iter()
        .enumerate()
        .find_map(|(i, ins)| ins.as_ref().filter(|x| x.plugin == "eq_7band" || x.plugin == "eq_1band").map(|x| (i, x)))
}

/// Magnitude response in dB of an equaliser insert at `freqs`, from the filter maths itself.
pub(crate) fn eq_response(ins: &soundcraft_model::Insert, freqs: &[f32]) -> Vec<f32> {
    let params: Vec<(&str, f32)> = ins.params.iter().map(|(k, v)| (k.as_str(), *v)).collect();
    if ins.plugin == "eq_7band" {
        soundcraft_dsp::eq7_response(&params, freqs, 48_000.0)
    } else {
        soundcraft_dsp::eq1_response(&params, freqs, 48_000.0)
    }
}

/// The equaliser card: the response of the track's first EQ insert, drawn read-only. A click
/// opens that insert's window.
fn eq_card(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let t = Tokens::current();
    let r = row(ui, EQ_H);
    let resp = ui.interact(r, ui.id().with(("inspector-eq", track.id.0)), if first_eq(track).is_some() { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, first_eq(track).is_some(), "Inspector: Equaliser curve"));
    ui.painter().rect(r, 5.0, t.inset_bg, Stroke::new(1.0, t.border), StrokeKind::Inside);
    for db in [-12.0f32, 0.0, 12.0] {
        let y = r.center().y - db / 18.0 * r.height() * 0.5;
        ui.painter().hline(r.min.x + 1.0..=r.max.x - 1.0, y, Stroke::new(1.0, t.inset_grid));
    }
    let Some((slot, ins)) = first_eq(track) else {
        ui.painter().text(r.center(), Align2::CENTER_CENTER, "No equaliser: add one from Inserts", regular(10.5), t.text_dim);
        return;
    };
    let n = 96;
    let freqs: Vec<f32> = (0..n).map(|i| 20.0 * 1000f32.powf(i as f32 / (n - 1) as f32)).collect();
    let pts: Vec<egui::Pos2> = eq_response(ins, &freqs)
        .iter()
        .enumerate()
        .map(|(i, db)| {
            pos2(r.min.x + 2.0 + (r.width() - 4.0) * i as f32 / (n - 1) as f32, r.center().y - db.clamp(-18.0, 18.0) / 18.0 * r.height() * 0.5)
        })
        .collect();
    let live = !ins.bypass && ins.active;
    let col = if live { t.counter_text } else { t.text_dim.gamma_multiply(0.6) };
    ui.painter().with_clip_rect(r.shrink(1.0)).add(Shape::line(pts, Stroke::new(1.8, col)));
    if resp.clicked() && !app.ui.plugin_windows.contains(&(track.id, slot)) {
        app.ui.plugin_windows.push((track.id, slot));
    }
    let _ = resp.on_hover_text(if live { "Equaliser response: click to open the equaliser" } else { "Equaliser (bypassed): click to open" });
}

fn notes(app: &mut SoundApp, ui: &mut Ui, track: &Track) {
    let mut c = track.comments.clone();
    let resp = ui.add_sized(
        vec2(ui.available_width(), 60.0),
        egui::TextEdit::multiline(&mut c).id(Id::new(("inspector-notes", track.id.0))).hint_text("Notes...").font(regular(11.0)),
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
        assert_eq!(inspector_sections(TrackKind::Audio), &[Routing, Inserts, Eq, Sends, Channel, Notes]);
        assert!(inspector_sections(TrackKind::Instrument).contains(&Instrument));
        assert!(!inspector_sections(TrackKind::Audio).contains(&Instrument));
        assert_eq!(inspector_sections(TrackKind::Midi), &[Routing, Midi, Notes]);
        assert!(inspector_sections(TrackKind::Instrument).contains(&Midi) && !inspector_sections(TrackKind::Audio).contains(&Midi));
        assert_eq!(inspector_sections(TrackKind::Folder), &[Notes]);
        assert_eq!(inspector_sections(TrackKind::Vca), &[Channel, Notes]);
        let master = inspector_sections(TrackKind::Master);
        assert!(master.contains(&Routing) && master.contains(&Channel) && !master.contains(&Notes) && !master.contains(&Sends));
        assert_eq!(routing_rows(TrackKind::Master), (false, true), "master hides its input");
        assert_eq!(routing_rows(TrackKind::Midi), (true, false));
        assert_eq!(channel_rows(TrackKind::Vca), (false, false));
    }

    #[test]
    fn stereo_pan_and_width_round_trip_inside_the_limits() {
        assert_eq!(pan_centre(&[-1.0, 1.0]), 0.0);
        assert_eq!(pan_width(&[-1.0, 1.0]), Some(1.0));
        assert_eq!(pan_width(&[0.3]), None, "mono has no width");
        assert_eq!(pan_centre(&[0.3]), 0.3);
        assert_eq!(set_width(-1.0, 1.0, 0.5), (-0.5, 0.5));
        assert_eq!(set_width(-1.0, 1.0, 0.0), (0.0, 0.0), "zero width is mono");
        assert_eq!(set_width(-0.5, 0.5, -1.0), (1.0, -1.0), "negative swaps the sides");
        assert_eq!(shift_pan(-0.5, 0.5, 0.25), (-0.25, 0.75));
        let (l, r) = shift_pan(-1.0, 1.0, 0.5);
        assert!(l >= -1.0 && r <= 1.0, "clamped: {l} {r}");
    }

    #[test]
    fn the_eq_curve_follows_the_inserts_parameters() {
        let mut ins = soundcraft_model::Insert::new("eq_7band");
        let freqs = [100.0f32, 1000.0, 10_000.0];
        let flat = eq_response(&ins, &freqs);
        assert!(flat.iter().all(|d| d.abs() < 0.1), "{flat:?}");
        ins.params.insert("mid_gain".into(), 12.0);
        let boosted = eq_response(&ins, &freqs);
        assert!(boosted[1] > 11.0 && boosted[0] < 3.0, "{boosted:?}");
    }

    #[test]
    fn every_section_has_a_unique_id() {
        use Section::*;
        let ids: Vec<_> = [Routing, Instrument, Midi, Inserts, Eq, Sends, Channel, Notes].iter().map(|s| s.id()).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len());
    }
}
