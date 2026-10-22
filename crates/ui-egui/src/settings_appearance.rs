//! Settings > Appearance page: theme picker and file buttons, the colour list grouped by section
//! with contrast badges, the theme settings sliders and Reset All. Every control runs a
//! `ui.theme_*` command; the page keeps only transient text-entry state.

use crate::SoundApp;
use crate::icons;
use crate::theme::{Tokens, bold, regular};
use crate::theme_doc::{self, ColorKey};
use egui::{Color32, Id, RichText, Sense, TextEdit, Ui, vec2};
use serde_json::{Value, json};

/// Transient page state kept in egui memory: the name field and the last message.
#[derive(Clone, Default)]
struct PageState {
    name: String,
    message: String,
    error: bool,
}

fn state_id() -> Id {
    Id::new("settings_appearance_state")
}

/// Which contrast checks (by label, see `theme_doc::contrast_checks`) involve a colour key.
fn checks_for(key: &str) -> &'static [&'static str] {
    match key {
        "text" => &["Text on Panel", "Text on raised panel", "Text on Arrange background"],
        "text_dim" => &["Dim text on Panel"],
        "panel" => &["Text on Panel", "Text on raised panel", "Dim text on Panel"],
        "text_on_accent" | "accent" => &["Text on accent"],
        "counter" => &["Counter text on Counter"],
        "ruler" => &["Ruler text on Ruler"],
        "arrange_bg" => &["Text on Arrange background"],
        _ => &[],
    }
}

/// A theme setting slider: (key, label, min, max).
const PARAMS: &[(&str, &str, f32, f32)] = &[
    ("clip_opacity", "Clip opacity", 0.0, 1.0),
    ("clip_opacity_selected", "Selected clip opacity", 0.0, 1.0),
    ("grid_strength", "Grid line strength", 0.0, 2.0),
    ("track_tint", "Track colour tint", 0.0, 1.0),
];

pub fn page(app: &mut SoundApp, ui: &mut Ui) {
    let mut st = ui.data_mut(|d| d.get_temp::<PageState>(state_id())).unwrap_or_default();
    let t = Tokens::current();
    heading(ui, "Theme");
    theme_row(app, ui, &mut st);
    file_row(app, ui, &mut st);
    reset_all(app, ui, &mut st);
    if !st.message.is_empty() {
        let color = if st.error { t.rec } else { t.text_dim };
        ui.label(RichText::new(&st.message).font(regular(11.0)).color(color));
    }
    ui.add_space(6.0);
    contrast_summary(ui, &t);
    ui.add_space(6.0);
    heading(ui, "Colours");
    colour_list(app, ui, &t, &mut st);
    ui.add_space(6.0);
    heading(ui, "Settings");
    params(app, ui, &mut st);
    ui.data_mut(|d| d.insert_temp(state_id(), st));
}

fn heading(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).font(bold(12.0)).color(Tokens::current().text));
    ui.add_space(2.0);
}

/// Run a command and keep its error (or nothing) as the page message.
fn exec(app: &mut SoundApp, st: &mut PageState, id: &str, params: Value) -> Option<Value> {
    match app.run(id, params) {
        Ok(v) => {
            st.message.clear();
            st.error = false;
            Some(v)
        }
        Err(e) => {
            st.message = e;
            st.error = true;
            None
        }
    }
}

fn theme_row(app: &mut SoundApp, ui: &mut Ui, st: &mut PageState) {
    let shown = app.ui.theme_name.clone().unwrap_or_else(|| app.ui.theme.label().to_string());
    let dirty = crate::theme_cmds::is_dirty(app);
    let mut chosen = None;
    ui.horizontal(|ui| {
        ui.label("Theme");
        egui::ComboBox::from_id_salt("settings_theme_picker").selected_text(if dirty { format!("{shown} *") } else { shown }).width(200.0).show_ui(
            ui,
            |ui| {
                // The list reads the themes folder, so it is only fetched while the menu is open.
                let list = app.run("ui.theme_list", json!({})).unwrap_or_default();
                let active = app.ui.theme_name.clone().unwrap_or_default();
                for th in list["themes"].as_array().map(Vec::as_slice).unwrap_or_default() {
                    let Some(name) = th["name"].as_str() else { continue };
                    if ui.selectable_label(active.eq_ignore_ascii_case(name), name).clicked() {
                        chosen = Some(name.to_string());
                    }
                }
            },
        );
    });
    if let Some(name) = chosen {
        exec(app, st, "ui.theme_load", json!({"name": name}));
    }
}

fn reset_all(app: &mut SoundApp, ui: &mut Ui, st: &mut PageState) {
    if ui.button("Reset All").on_hover_text("Put every colour and theme setting back to the theme's base").clicked() {
        exec(app, st, "ui.theme_reset", json!({}));
    }
}

fn file_row(app: &mut SoundApp, ui: &mut Ui, st: &mut PageState) {
    let current = app.ui.theme_name.clone().unwrap_or_else(|| app.ui.theme.label().to_string());
    let user_theme = !current.is_empty() && theme_doc::builtin(&current).is_none();
    let has_working = app.ui.custom_theme.is_some();
    ui.horizontal(|ui| {
        ui.label("Name");
        ui.add(TextEdit::singleline(&mut st.name).desired_width(160.0).hint_text("theme name"));
    });
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(has_working, egui::Button::new("Save")).on_hover_text("Save the working theme under its own name").clicked() {
            exec(app, st, "ui.theme_save", json!({}));
        }
        if ui.add_enabled(has_working, egui::Button::new("Save As")).on_hover_text("Save the working theme under the name typed above").clicked() {
            if st.name.trim().is_empty() {
                st.message = "Type a theme name first".into();
                st.error = true;
            } else if exec(app, st, "ui.theme_save", json!({"name": st.name.clone()})).is_some() {
                st.name.clear();
            }
        }
        if ui.button("Duplicate").on_hover_text("Copy this theme under the typed name (or `<name> copy`)").clicked() {
            let to = if st.name.trim().is_empty() { format!("{current} copy") } else { st.name.clone() };
            if exec(app, st, "ui.theme_duplicate", json!({"name": current.clone(), "to": to})).is_some() {
                st.name.clear();
            }
        }
        if ui.add_enabled(user_theme, egui::Button::new("Rename")).on_hover_text("Rename this saved theme to the typed name").clicked() {
            if st.name.trim().is_empty() {
                st.message = "Type the new name first".into();
                st.error = true;
            } else if exec(app, st, "ui.theme_rename", json!({"from": current.clone(), "to": st.name.clone()})).is_some() {
                st.name.clear();
            }
        }
        if ui.add_enabled(user_theme, egui::Button::new("Delete")).on_hover_text("Delete this saved theme file").clicked() {
            exec(app, st, "ui.theme_delete", json!({"name": current.clone()}));
        }
        if ui.button("Import...").clicked() {
            exec(app, st, "ui.theme_import", json!({}));
        }
        if ui.button("Export...").clicked() {
            exec(app, st, "ui.theme_export", json!({}));
        }
    });
}

fn contrast_summary(ui: &mut Ui, t: &Tokens) {
    let bad: Vec<_> = theme_doc::contrast_checks(t).into_iter().filter(|(_, ratio, min)| ratio < min).collect();
    if bad.is_empty() {
        ui.label(RichText::new("Contrast checks passed").font(regular(11.0)).color(t.text_dim));
        return;
    }
    for (label, ratio, min) in bad {
        ui.horizontal(|ui| {
            warning_icon(ui, t);
            ui.label(RichText::new(format!("Low contrast {ratio:.1}:1, needs {min}:1: {label}")).font(regular(11.0)).color(t.meter_yellow));
        });
    }
}

fn warning_icon(ui: &mut Ui, t: &Tokens) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
    icons::draw(ui.painter(), r, "warning", t.meter_yellow);
    resp
}

fn colour_list(app: &mut SoundApp, ui: &mut Ui, t: &Tokens, st: &mut PageState) {
    let mut sections: Vec<&'static str> = Vec::new();
    for k in theme_doc::KEYS {
        if !sections.contains(&k.section) {
            sections.push(k.section);
        }
    }
    let checks = theme_doc::contrast_checks(t);
    for section in sections {
        egui::CollapsingHeader::new(RichText::new(section).font(bold(11.0)).color(t.text)).default_open(true).show(ui, |ui| {
            for k in theme_doc::KEYS.iter().filter(|k| k.section == section) {
                colour_row(app, ui, t, st, k, &checks);
            }
        });
    }
}

fn colour_row(app: &mut SoundApp, ui: &mut Ui, t: &Tokens, st: &mut PageState, k: &ColorKey, checks: &[(&'static str, f64, f64)]) {
    let current = k.value(t);
    let overridden = app.ui.custom_theme.as_ref().is_some_and(|d| d.colors.contains_key(k.key));
    let failing: Vec<String> = checks
        .iter()
        .filter(|(label, ratio, min)| ratio < min && checks_for(k.key).contains(label))
        .map(|(label, ratio, _)| format!("Contrast {ratio:.1}:1 - {label}"))
        .collect();
    ui.horizontal(|ui| {
        ui.add_sized(vec2(150.0, 18.0), egui::Label::new(RichText::new(k.label).font(regular(11.0)).color(t.text)).truncate());
        let mut c = current;
        if ui.color_edit_button_srgba(&mut c).changed() && c != current {
            exec(app, st, "ui.theme_set_color", json!({"key": k.key, "color": theme_doc::format_hex(c)}));
        }
        hex_field(app, ui, st, k.key, current);
        let r = ui
            .add_enabled_ui(overridden, |ui| crate::widgets::icon_button(ui, vec2(20.0, 20.0), "reset", false, &format!("Reset {}", k.label)))
            .inner;
        if r.clicked() {
            exec(app, st, "ui.theme_reset", json!({"key": k.key}));
        }
        if !failing.is_empty() {
            warning_icon(ui, t).on_hover_text(failing.join("\n"));
        }
    });
}

/// The hex entry: it follows the theme until focused; Enter or leaving the field applies it, and
/// anything that is not `#RRGGBB` / `#RRGGBBAA` is rejected with a message.
fn hex_field(app: &mut SoundApp, ui: &mut Ui, st: &mut PageState, key: &str, current: Color32) {
    let id = Id::new(("settings_hex", key));
    let shown = theme_doc::format_hex(current);
    let mut text = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| shown.clone());
    let valid = theme_doc::parse_hex(&text).is_some();
    let color = if valid { Tokens::current().text } else { Tokens::current().rec };
    let resp = ui.add(TextEdit::singleline(&mut text).id(id.with("edit")).desired_width(76.0).char_limit(9).font(regular(11.0)).text_color(color));
    if resp.lost_focus() {
        match theme_doc::parse_hex(&text) {
            Some(c) if c != current => {
                exec(app, st, "ui.theme_set_color", json!({"key": key, "color": theme_doc::format_hex(c)}));
            }
            Some(_) => {}
            None => {
                st.message = format!("`{}` is not a colour (use #RRGGBB or #RRGGBBAA)", text.trim());
                st.error = true;
            }
        }
        text = String::new();
    }
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text));
    } else {
        ui.data_mut(|d| d.remove::<String>(id));
    }
}

fn params(app: &mut SoundApp, ui: &mut Ui, st: &mut PageState) {
    let t = Tokens::current();
    let doc = app.ui.custom_theme.clone().unwrap_or_default();
    let overridden = |key: &str| match key {
        "clip_opacity" => doc.params.clip_opacity.is_some(),
        "clip_opacity_selected" => doc.params.clip_opacity_selected.is_some(),
        "grid_strength" => doc.params.grid_strength.is_some(),
        "track_tint" => doc.params.track_tint.is_some(),
        _ => doc.params.grid_beat_always.is_some(),
    };
    egui::Grid::new("settings_theme_params").num_columns(3).spacing(vec2(12.0, 6.0)).show(ui, |ui| {
        for &(key, label, lo, hi) in PARAMS {
            ui.label(label);
            let mut v = match key {
                "clip_opacity" => f32::from(t.clip_fill_alpha) / 255.0,
                "clip_opacity_selected" => f32::from(t.clip_fill_alpha_selected) / 255.0,
                "grid_strength" => doc.params.grid_strength.unwrap_or(1.0),
                _ => app.ui.track_tint,
            };
            if ui.add(egui::Slider::new(&mut v, lo..=hi).fixed_decimals(2)).changed() {
                exec(app, st, "ui.theme_set_param", json!({"key": key, "value": v}));
            }
            if ui.add_enabled(overridden(key), egui::Button::new("Reset")).clicked() {
                exec(app, st, "ui.theme_reset", json!({"key": key}));
            }
            ui.end_row();
        }
        ui.label("Beat lines at every zoom");
        let mut on = t.grid_beat_always;
        if crate::widgets::checkbox(ui, &mut on, "").changed() {
            exec(app, st, "ui.theme_set_param", json!({"key": "grid_beat_always", "value": on}));
        }
        if ui.add_enabled(overridden("grid_beat_always"), egui::Button::new("Reset")).clicked() {
            exec(app, st, "ui.theme_reset", json!({"key": "grid_beat_always"}));
        }
        ui.end_row();
    });
}
