//! The Settings window (`window.settings`): a sidebar with four pages (General, Appearance,
//! Track colours, Keys) and a bottom bar (Reset page, Revert, Close). Every control runs a command.
//! The Appearance, Track colours and Keys pages live in `settings_appearance.rs`, `settings_tracks.rs` and
//! `settings_keys.rs`.

use crate::SoundApp;
use crate::settings_cmds::{SCALE_RANGE, SettingsTab};
use crate::theme::{Tokens, bold, regular};
use egui::{Layout, RichText, Ui, vec2};
use serde_json::json;

const SIDEBAR_W: f32 = 132.0;
const BOTTOM_H: f32 = 34.0;
const DEFAULT_SIZE: egui::Vec2 = vec2(720.0, 560.0);
const MIN_SIZE: egui::Vec2 = vec2(560.0, 420.0);

pub fn show(app: &mut SoundApp, ctx: &egui::Context) {
    let mut open = app.extra.show_settings;
    if !open {
        return;
    }
    let screen = ctx.content_rect().size();
    let size = vec2(DEFAULT_SIZE.x.min((screen.x - 40.0).max(MIN_SIZE.x)), DEFAULT_SIZE.y.min((screen.y - 80.0).max(MIN_SIZE.y)));
    egui::Window::new(RichText::new("Settings").font(bold(13.0)))
        .id(egui::Id::new("settings_window"))
        .open(&mut open)
        .default_size(size)
        .min_size(MIN_SIZE)
        .resizable(true)
        .constrain(true)
        .show(ctx, |ui| {
            let full = ui.available_size();
            let body_h = (full.y - BOTTOM_H - 3.0 * ui.spacing().item_spacing.y - 6.0).max(80.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(vec2(SIDEBAR_W, body_h), Layout::top_down(egui::Align::Min), |ui| sidebar(app, ui));
                // Not `ui.separator()`: in a row it would claim the window's whole free height.
                let (line, _) = ui.allocate_exact_size(vec2(1.0, body_h), egui::Sense::hover());
                ui.painter().vline(line.center().x, line.y_range(), ui.visuals().widgets.noninteractive.bg_stroke);
                ui.allocate_ui_with_layout(vec2(ui.available_width(), body_h), Layout::top_down(egui::Align::Min), |ui| {
                    egui::ScrollArea::vertical().id_salt("settings_page").auto_shrink([false, false]).show(ui, |ui| match app.extra.settings_tab {
                        SettingsTab::General => general(app, ui),
                        SettingsTab::Appearance => crate::settings_appearance::page(app, ui),
                        SettingsTab::Tracks => crate::settings_tracks::page(app, ui),
                        SettingsTab::Keys => crate::settings_keys::page(app, ui),
                    });
                });
            });
            ui.separator();
            bottom_bar(app, ui);
        });
    // The title bar's close button.
    if !open && app.extra.show_settings {
        let _ = app.run("window.settings", json!({"value": false}));
    }
}

fn sidebar(app: &mut SoundApp, ui: &mut Ui) {
    ui.set_width(SIDEBAR_W - 8.0);
    for tab in SettingsTab::ALL {
        let on = app.extra.settings_tab == tab;
        if ui.selectable_label(on, RichText::new(tab.label()).font(regular(12.0))).clicked() && !on {
            let _ = app.run("window.settings", json!({"tab": tab.id()}));
        }
    }
}

fn bottom_bar(app: &mut SoundApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        if ui.button("Reset page").on_hover_text("Put this page's settings back to their defaults").clicked() {
            reset_page(app);
        }
        let dirty = crate::theme_cmds::is_dirty(app);
        if ui.add_enabled(dirty, egui::Button::new("Revert")).on_hover_text("Discard unsaved theme changes").clicked() {
            revert_theme(app);
        }
        ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Close").clicked() {
                let _ = app.run("window.settings", json!({"value": false}));
            }
        });
    });
}

/// Defaults of the General page: the preferences command plus each style command.
fn reset_page(app: &mut SoundApp) {
    match app.extra.settings_tab {
        SettingsTab::General => {
            let _ = app.run("ui.prefs", json!({"reset": true}));
            let _ = app.run("ui.theme", json!({"mode": crate::theme::ThemeMode::default().id()}));
            let _ = app.run("ui.menu_layout", json!({"layout": crate::menu_layout::MenuLayout::default().id()}));
            let _ = app.run("ui.toolbar_style", json!({"value": crate::toolbar_layout::ToolbarStyle::default().id()}));
            let _ = app.run("ui.header_style", json!({"style": crate::track_header_compact::HeaderStyle::default().id()}));
            let _ = app.run("ui.transport_position", json!({"position": crate::transport_bar::TransportPosition::default().id()}));
        }
        SettingsTab::Appearance => {
            let _ = app.run("ui.theme_reset", json!({}));
        }
        SettingsTab::Tracks => {
            let _ = app.run("ui.theme_reset", json!({"key": "track_palette"}));
        }
        SettingsTab::Keys => {
            let _ = app.run("ui.prefs", json!({"keymap_reset": true, "keymap_preset": crate::keymap::Preset::default().id()}));
        }
    }
}

/// Reload the active theme from where it was loaded or saved; an unsaved fork goes back to its base.
fn revert_theme(app: &mut SoundApp) {
    let name = app.ui.theme_name.clone().unwrap_or_default();
    if app.run("ui.theme_load", json!({"name": name})).is_err() {
        let base = app.ui.custom_theme.as_ref().map_or("dark", |d| d.base.id());
        let _ = app.run("ui.theme_load", json!({"name": base}));
    }
}

fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(4.0);
    ui.label(RichText::new(text).font(bold(12.0)).color(Tokens::current().text));
    ui.add_space(2.0);
}

/// A drop-down; returns the item chosen this frame.
fn combo<T: Copy + PartialEq>(ui: &mut Ui, salt: &str, current: T, all: &[T], label: fn(T) -> &'static str) -> Option<T> {
    let mut chosen = None;
    egui::ComboBox::from_id_salt(salt).selected_text(label(current)).show_ui(ui, |ui| {
        for &item in all {
            if ui.selectable_label(current == item, label(item)).clicked() {
                chosen = Some(item);
            }
        }
    });
    chosen
}

fn general(app: &mut SoundApp, ui: &mut Ui) {
    heading(ui, "General");
    egui::Grid::new("settings_general").num_columns(2).spacing(vec2(16.0, 8.0)).show(ui, |ui| {
        ui.label("Interface scale");
        let mut scale = app.extra.ui_scale;
        let slider =
            egui::Slider::new(&mut scale, SCALE_RANGE.0..=SCALE_RANGE.1).step_by(0.05).custom_formatter(|v, _| format!("{:.0} %", v * 100.0));
        if ui.add(slider).changed() {
            let _ = app.run("ui.prefs", json!({"ui_scale": scale}));
        }
        ui.end_row();

        ui.label("Reduce motion");
        let mut on = app.extra.reduce_motion;
        if crate::widgets::checkbox(ui, &mut on, "").changed() {
            let _ = app.run("ui.prefs", json!({"reduce_motion": on}));
        }
        ui.end_row();

        ui.label("Show tooltips");
        let mut on = app.extra.tooltips;
        if crate::widgets::checkbox(ui, &mut on, "").changed() {
            let _ = app.run("ui.prefs", json!({"tooltips": on}));
        }
        ui.end_row();

        ui.label("Theme");
        let mut modes = crate::theme::ThemeMode::ALL.to_vec();
        if app.ui.theme == crate::theme::ThemeMode::Custom {
            modes.push(crate::theme::ThemeMode::Custom);
        }
        if let Some(m) = combo(ui, "settings_theme", app.ui.theme, &modes, |m| m.label()).filter(|m| *m != app.ui.theme) {
            let _ = app.run("ui.theme", json!({"mode": m.id()}));
        }
        ui.end_row();

        ui.label("Menu layout");
        if let Some(m) = combo(ui, "settings_menu_layout", app.ui.menu_layout, &crate::menu_layout::MenuLayout::ALL, |m| m.label()) {
            let _ = app.run("ui.menu_layout", json!({"layout": m.id()}));
        }
        ui.end_row();

        ui.label("Toolbar");
        if let Some(s) = combo(ui, "settings_toolbar", app.ui.toolbar_style, &crate::toolbar_layout::ToolbarStyle::ALL, |s| s.label()) {
            let _ = app.run("ui.toolbar_style", json!({"value": s.id()}));
        }
        ui.end_row();

        ui.label("Track header");
        if let Some(s) = combo(ui, "settings_header", app.ui.header_style, &crate::track_header_compact::HeaderStyle::ALL, |s| s.label()) {
            let _ = app.run("ui.header_style", json!({"style": s.id()}));
        }
        ui.end_row();

        ui.label("Transport");
        if let Some(p) = combo(ui, "settings_transport", app.ui.transport_position, &crate::transport_bar::TransportPosition::ALL, |p| p.label()) {
            let _ = app.run("ui.transport_position", json!({"position": p.id()}));
        }
        ui.end_row();

        ui.label("Auto-update window configuration");
        let mut on = app.extra.auto_update_config;
        if crate::widgets::checkbox(ui, &mut on, "").changed() {
            let _ = app.run("window.config_auto_update", json!({"value": on}));
        }
        ui.end_row();
    });
    ui.add_space(8.0);
    ui.separator();
    heading(ui, "Other settings");
    ui.horizontal_wrapped(|ui| {
        for (label, id) in
            [("Playback Engine...", "window.playback_engine"), ("I/O...", "window.io_setup"), ("Keyboard Shortcuts...", "window.shortcuts")]
        {
            if ui.button(label).clicked() {
                let _ = app.run(id, json!({"value": true}));
            }
        }
        if ui.button("Session Setup...").clicked() {
            crate::menus::invoke_menu(app, "setup.session", "Setup > Session");
        }
    });
}
