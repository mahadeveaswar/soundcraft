//! Settings > Keys page: the key-command preset, a searchable table of every command with its keys, and
//! chord capture. Every change runs `ui.prefs` (`keymap_preset`, `keymap_overrides`, `keymap_reassign`,
//! `keymap_reset`), so the page and scripts share one path and one validation.

use crate::SoundApp;
use crate::keymap::{ActionInfo, KeymapPrefs, Preset, actions};
use crate::theme::{Tokens, bold, mono, regular};
use egui::{RichText, Ui, vec2};
use serde_json::json;
use std::collections::HashMap;

const ROW_H: f32 = 24.0;

pub fn page(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let mac = ui.ctx().os().is_mac();
    ui.label(RichText::new("Keys").font(bold(12.0)).color(t.text));
    ui.label(
        RichText::new(
            "Choose a key set, or give any command its own keys. The Alternative set always lets single letters act as commands; typing in a text field never does.",
        )
        .font(regular(11.0))
        .color(t.text_dim),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Key set");
        let current = app.ui.keymap.preset;
        egui::ComboBox::from_id_salt("settings_keymap_preset").selected_text(current.label()).show_ui(ui, |ui| {
            for p in Preset::ALL {
                if ui.selectable_label(current == p, p.label()).clicked() && current != p {
                    let _ = app.run("ui.prefs", json!({"keymap_preset": p.id()}));
                }
            }
        });
        let custom = !app.ui.keymap.overrides.is_empty();
        if ui.add_enabled(custom, egui::Button::new("Reset all")).on_hover_text("Drop every key you changed and go back to the key set").clicked() {
            let _ = app.run("ui.prefs", json!({"keymap_reset": true}));
        }
        ui.label(RichText::new("Search").font(regular(11.0)).color(t.text_dim));
        ui.add(egui::TextEdit::singleline(&mut app.extra.key_search).id(egui::Id::new("settings_keys_search")).desired_width(180.0));
    });
    ui.add_space(4.0);
    capture_banner(app, ui, mac);
    ui.separator();
    table(app, ui, mac);
}

fn label_of(key: &str, all: &[ActionInfo]) -> String {
    all.iter().find(|a| a.key == key).map_or_else(|| key.to_string(), |a| a.label.clone())
}

/// The prompt while waiting for a key, and the pending chord with its conflict choice.
fn capture_banner(app: &mut SoundApp, ui: &mut Ui, mac: bool) {
    let t = Tokens::current();
    let all = actions();
    if let Some(action) = app.extra.key_capture.clone() {
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Press the new key for {}  (Esc cancels)", label_of(&action, &all))).color(t.accent));
            if ui.button("Cancel").clicked() {
                app.extra.key_capture = None;
            }
        });
        return;
    }
    let Some((action, chord)) = app.extra.key_pending.clone() else { return };
    let holders = app.ui.keymap.holders(&chord, &action);
    let shown = crate::shortcuts::shortcut_label(&chord, mac);
    if holders.is_empty() {
        add_chord(app, &action, &chord, false);
        app.extra.key_pending = None;
        return;
    }
    let names: Vec<String> = holders.iter().map(|h| label_of(h, &all)).collect();
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("{shown} is already used by {}.", names.join(", "))).color(t.error_text));
        if ui.button("Reassign").clicked() {
            add_chord(app, &action, &chord, true);
            app.extra.key_pending = None;
        }
        if ui.button("Cancel").clicked() {
            app.extra.key_pending = None;
        }
    });
}

/// Add `chord` to what an action already answers to.
fn add_chord(app: &mut SoundApp, action: &str, chord: &str, reassign: bool) {
    let mut chords = app.ui.keymap.bindings(action);
    if !chords.iter().any(|c| c == chord) {
        chords.push(chord.to_string());
    }
    let r = app.run("ui.prefs", json!({"keymap_overrides": {action: chords}, "keymap_reassign": reassign}));
    if let Err(e) = r {
        app.ui.status = e;
    }
}

fn table(app: &mut SoundApp, ui: &mut Ui, mac: bool) {
    let t = Tokens::current();
    let km: KeymapPrefs = app.ui.keymap.clone();
    let bound: HashMap<String, Vec<String>> = km.effective().into_iter().collect();
    let needle = app.extra.key_search.trim().to_lowercase();
    let mut rows: Vec<ActionInfo> = actions()
        .into_iter()
        .filter(|a| {
            let chords = bound.get(&a.key).map(Vec::as_slice).unwrap_or_default();
            if needle.is_empty() {
                return !chords.is_empty() || !a.path.is_empty();
            }
            let hay = format!("{} {} {} {}", a.label, a.key, a.path, chords.join(" ")).to_lowercase();
            needle.split_whitespace().all(|w| hay.contains(w))
        })
        .collect();
    rows.sort_by(|a, b| (a.path.is_empty(), a.path.as_str(), a.label.as_str()).cmp(&(b.path.is_empty(), b.path.as_str(), b.label.as_str())));
    if rows.is_empty() {
        ui.label(RichText::new("No command matches.").color(t.text_dim));
        return;
    }
    egui::ScrollArea::vertical().id_salt("settings_keys_table").auto_shrink([false, false]).show_rows(ui, ROW_H, rows.len(), |ui, range| {
        for row in rows.get(range).unwrap_or_default() {
            ui.push_id(&row.key, |ui| {
                ui.horizontal(|ui| {
                    ui.set_min_height(ROW_H);
                    ui.allocate_ui(vec2(190.0, ROW_H), |ui| ui.label(RichText::new(&row.label).font(regular(12.0))));
                    ui.allocate_ui(vec2(150.0, ROW_H), |ui| ui.label(RichText::new(&row.path).font(regular(10.5)).color(t.text_dim)));
                    let chords = bound.get(&row.key).cloned().unwrap_or_default();
                    for chord in &chords {
                        let text = crate::shortcuts::shortcut_label(chord, mac);
                        let text = if text.is_empty() { chord.clone() } else { text };
                        if ui.small_button(RichText::new(format!("{text}  x")).font(mono(11.0))).on_hover_text("Remove this key").clicked() {
                            let rest: Vec<&String> = chords.iter().filter(|c| *c != chord).collect();
                            let _ = app.run("ui.prefs", json!({"keymap_overrides": {row.key.clone(): rest}}));
                        }
                    }
                    if ui.small_button("Assign").on_hover_text("Press the key combination to add").clicked() {
                        app.extra.key_pending = None;
                        app.extra.key_capture = Some(row.key.clone());
                    }
                    if km.overridden(&row.key) && ui.small_button("Reset").on_hover_text("Back to the key set's keys").clicked() {
                        let _ = app.run("ui.prefs", json!({"keymap_overrides": {row.key.clone(): null}}));
                    }
                });
            });
        }
    });
}
