//! Template dialogs: File > Save As Template..., New from Template... and Instrument Rack....
//! Each one only collects values and runs the engine's `template.*` commands, so the same work is
//! reachable from the control channel. Errors from a command stay in the dialog and in the status
//! line. Enter confirms and Escape cancels.

use crate::SoundApp;
use crate::theme::{Tokens, bold};
use egui::{Align2, vec2};
use serde_json::{Value, json};

/// One entry of the templates list.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateEntry {
    pub name: String,
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone)]
pub enum TemplateDialog {
    /// `replace` is set once the engine reported that the name is taken, so OK then replaces it.
    Save {
        name: String,
        replace: bool,
        error: Option<String>,
    },
    New {
        list: Vec<TemplateEntry>,
        selected: Option<usize>,
        session_name: String,
        error: Option<String>,
    },
    Rack {
        count: u32,
        names: String,
        error: Option<String>,
    },
}

impl TemplateDialog {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Save { .. } => "template_save",
            Self::New { .. } => "template_new",
            Self::Rack { .. } => "template_rack",
        }
    }
}

#[derive(Debug, Default)]
pub struct TemplateUi {
    pub open: Option<TemplateDialog>,
    /// Use this folder instead of the default templates folders (tests, agents).
    pub dir: Option<String>,
}

/// The command ids this dialog layer takes over when they are invoked from a menu.
pub(crate) fn is_template_command(id: &str, path: &str) -> bool {
    matches!(id, "template.save" | "template.new_session" | "template.instrument_rack") || path == "File > Save As Template..."
}

/// Menu-style invocation of a template item: open its dialog.
pub(crate) fn open_for_menu(app: &mut SoundApp, id: &str, path: &str) {
    if id == "template.new_session" {
        open_new(app);
    } else if id == "template.instrument_rack" {
        app.templates.open = Some(TemplateDialog::Rack { count: 10, names: String::new(), error: None });
    } else if is_template_command(id, path) {
        app.templates.open = Some(TemplateDialog::Save { name: String::new(), replace: false, error: None });
    }
}

fn with_dir(app: &SoundApp, mut p: Value) -> Value {
    if let (Some(d), Some(o)) = (&app.templates.dir, p.as_object_mut()) {
        o.insert("dir".into(), json!(d));
    }
    p
}

/// The templates the engine lists, or the reason it could not.
pub fn list_templates(app: &mut SoundApp) -> Result<Vec<TemplateEntry>, String> {
    let r = app.run("template.list", with_dir(app, json!({})))?;
    let s = |t: &Value, k: &str| t.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    Ok(r.get("templates")
        .and_then(Value::as_array)
        .map(|a| a.iter().map(|t| TemplateEntry { name: s(t, "name"), path: s(t, "path"), source: s(t, "source") }).collect())
        .unwrap_or_default())
}

fn open_new(app: &mut SoundApp) {
    let (list, error) = match list_templates(app) {
        Ok(l) => (l, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    app.templates.open =
        Some(TemplateDialog::New { selected: if list.is_empty() { None } else { Some(0) }, list, session_name: String::new(), error });
}

enum Outcome {
    Keep,
    Close,
    Submit,
}

/// The Cancel / OK row. Returns true when OK was pressed (or Enter, when `ready`).
fn buttons(ui: &mut egui::Ui, ok: &str, enter: bool, ready: bool, cancel: &mut bool) -> bool {
    let mut pressed = enter && ready;
    ui.separator();
    ui.horizontal(|ui| {
        if ui.button("Cancel").clicked() {
            *cancel = true;
        }
        let b = egui::Button::new(egui::RichText::new(ok).strong().color(Tokens::current().accent_text)).fill(Tokens::current().accent);
        if ui.add_enabled(ready, b).clicked() {
            pressed = true;
        }
    });
    pressed
}

fn error_line(ui: &mut egui::Ui, error: &Option<String>) {
    if let Some(e) = error {
        ui.label(egui::RichText::new(e).small().color(Tokens::current().rec));
    }
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text).small().color(Tokens::current().text_dim));
}

/// Show the open template dialog, if any.
pub fn show(app: &mut SoundApp, ctx: &egui::Context) {
    let Some(mut d) = app.templates.open.take() else { return };
    let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
    let esc = ctx.input(|i| i.key_pressed(egui::Key::Escape));
    let title = match &d {
        TemplateDialog::Save { .. } => "Save As Template",
        TemplateDialog::New { .. } => "New from Template",
        TemplateDialog::Rack { .. } => "Instrument Rack",
    };
    let mut cancel = esc;
    let mut submit = false;
    egui::Window::new(egui::RichText::new(title).font(bold(13.0)))
        .collapsible(false)
        .resizable(false)
        .anchor(Align2::CENTER_CENTER, vec2(0.0, -80.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
            match &mut d {
                TemplateDialog::Save { name, replace, error } => {
                    ui.horizontal(|ui| {
                        ui.label("Name");
                        let r = ui.add(egui::TextEdit::singleline(name).desired_width(240.0));
                        if !r.has_focus() && !*replace {
                            r.request_focus();
                        }
                        if r.changed() {
                            *replace = false;
                            *error = None;
                        }
                    });
                    hint(ui, "Stores tracks, mixer and plugin settings, without clips or media.");
                    error_line(ui, error);
                    let label = if *replace { "Replace" } else { "Save" };
                    submit = buttons(ui, label, enter, !name.trim().is_empty(), &mut cancel);
                }
                TemplateDialog::New { list, selected, session_name, error } => {
                    if list.is_empty() {
                        hint(ui, "No templates yet. Use File > Save As Template... first.");
                    }
                    egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                        for (i, t) in list.iter().enumerate() {
                            let text = if t.source == "shipped" { format!("{}  (built in)", t.name) } else { t.name.clone() };
                            let r = ui.selectable_label(*selected == Some(i), text);
                            if r.clicked() {
                                *selected = Some(i);
                            }
                            if r.double_clicked() {
                                submit = true;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Session name");
                        ui.add(egui::TextEdit::singleline(session_name).hint_text("Untitled").desired_width(200.0));
                    });
                    error_line(ui, error);
                    submit |= buttons(ui, "Create", enter, selected.is_some(), &mut cancel);
                }
                TemplateDialog::Rack { count, names, error } => {
                    ui.horizontal(|ui| {
                        ui.label("Tracks");
                        ui.add(egui::DragValue::new(count).range(1..=128));
                    });
                    ui.horizontal(|ui| {
                        ui.label("Names");
                        let r = ui.add(egui::TextEdit::singleline(names).hint_text("Piano, Strings, ...").desired_width(260.0));
                        if !r.has_focus() {
                            r.request_focus();
                        }
                    });
                    hint(ui, "Comma separated. Tracks without a name are called Instrument 1, 2, ...");
                    error_line(ui, error);
                    submit = buttons(ui, "Create", enter, true, &mut cancel);
                }
            }
        });
    let outcome = if cancel {
        Outcome::Close
    } else if submit {
        Outcome::Submit
    } else {
        Outcome::Keep
    };
    match outcome {
        Outcome::Close => {}
        Outcome::Keep => app.templates.open = Some(d),
        Outcome::Submit => {
            if let Some(kept) = submit_dialog(app, d) {
                app.templates.open = Some(kept);
            }
        }
    }
}

/// Run the dialog's command. Returns the dialog again when it must stay open (an error).
fn submit_dialog(app: &mut SoundApp, mut d: TemplateDialog) -> Option<TemplateDialog> {
    match &mut d {
        TemplateDialog::Save { name, replace, error } => {
            let p = with_dir(app, json!({"name": name.trim(), "replace": *replace}));
            match app.run("template.save", p) {
                Ok(r) => {
                    let path = r.get("path").and_then(Value::as_str).unwrap_or("");
                    app.ui.status = format!("Saved template {path}");
                    None
                }
                Err(e) => {
                    *replace = e.contains("already exists");
                    *error = Some(if *replace { "A template with this name exists. Press Replace to overwrite it.".into() } else { e });
                    Some(d)
                }
            }
        }
        TemplateDialog::New { list, selected, session_name, error } => {
            let Some(t) = selected.and_then(|i| list.get(i)) else { return Some(d) };
            let mut p = json!({"path": t.path});
            if !session_name.trim().is_empty() {
                p["session_name"] = json!(session_name.trim());
            }
            match crate::dialogs::run_or_ask(app, "template.new_session", p) {
                Ok(_) => None,
                Err(e) => {
                    *error = Some(e);
                    Some(d)
                }
            }
        }
        TemplateDialog::Rack { count, names, error } => {
            let list: Vec<&str> = names.split(',').map(str::trim).collect();
            let list = if list.iter().all(|n| n.is_empty()) { Vec::new() } else { list };
            match app.run("template.instrument_rack", json!({"count": count, "names": list})) {
                Ok(_) => {
                    app.ui.status = format!("Added {count} instrument tracks");
                    None
                }
                Err(e) => {
                    *error = Some(e);
                    Some(d)
                }
            }
        }
    }
}
