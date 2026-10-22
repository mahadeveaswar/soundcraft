//! `ui.theme_*` commands: the working custom theme, saved user themes and the track palette push.
//!
//! The working copy lives in `UiState.custom_theme` (saved with the other UI prefs, so unsaved
//! edits survive a restart). Saved user themes are JSON files in `Services.themes_dir`; without
//! a folder (tests, `SOUNDCRAFT_NO_PREFS`) they are kept in memory only.

use crate::SoundApp;
use crate::theme::{self, ThemeMode};
use crate::theme_doc::{self, Base, ThemeDoc};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Runtime state that is not saved with the prefs.
#[derive(Default)]
pub struct ThemeState {
    booted: bool,
    /// The document last applied to the tokens, to notice changes made through `ui.set`.
    applied: Option<ThemeDoc>,
    /// The loaded or saved document the working copy is compared with for `dirty`.
    saved: Option<ThemeDoc>,
    /// User themes when there is no themes folder.
    memory: Vec<ThemeDoc>,
}

/// Help text for the control protocol: the valid parameter keys.
const PARAM_KEYS: &[&str] = &["clip_opacity", "clip_opacity_selected", "grid_strength", "track_tint", "grid_beat_always"];

fn same_content(a: &ThemeDoc, b: &ThemeDoc) -> bool {
    a.base == b.base && a.colors == b.colors && a.params == b.params && a.track_palette == b.track_palette
}

/// Validate and clean a theme name: non-empty, at most 64 characters, file-safe.
pub fn clean_name(raw: &str) -> Result<String, String> {
    let name: String = raw.trim().chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    if name.is_empty() {
        return Err("theme name must not be empty".into());
    }
    if name.chars().count() > 64 {
        return Err("theme name must be at most 64 characters".into());
    }
    Ok(name)
}

fn file_name(name: &str) -> String {
    format!("{name}.json")
}

fn io_err(what: &str, e: &std::io::Error) -> String {
    format!("{what}: {e}")
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_err("could not create the themes folder", &e))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| io_err("could not write the theme", &e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io_err("could not write the theme", &e)
    })
}

/// The saved user themes.
fn user_themes(app: &SoundApp) -> Vec<ThemeDoc> {
    let Some(dir) = &app.services.themes_dir else {
        return app.theme_state.memory.clone();
    };
    let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut out: Vec<ThemeDoc> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
        .filter_map(|p| {
            let doc = std::fs::read_to_string(&p).map_err(|e| e.to_string()).and_then(|t| theme_doc::from_json(&t));
            match doc {
                Ok(mut d) => {
                    if d.name.trim().is_empty() {
                        d.name = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    }
                    Some(d)
                }
                Err(e) => {
                    log::warn!("{}: {e}", p.display());
                    None
                }
            }
        })
        .collect();
    out.sort_by_key(|d| d.name.to_lowercase());
    out
}

fn find_user(app: &SoundApp, name: &str) -> Option<ThemeDoc> {
    user_themes(app).into_iter().find(|d| d.name.eq_ignore_ascii_case(name.trim()))
}

fn is_builtin(name: &str) -> bool {
    theme_doc::builtin(name).is_some()
}

fn store_user(app: &mut SoundApp, doc: &ThemeDoc) -> Result<(), String> {
    if let Some(dir) = app.services.themes_dir.clone() {
        // A rename of case only, or an overwrite, replaces the existing file.
        for old in user_themes(app).iter().filter(|d| d.name.eq_ignore_ascii_case(&doc.name)) {
            if old.name != doc.name {
                let _ = std::fs::remove_file(dir.join(file_name(&old.name)));
            }
        }
        write_atomic(&dir.join(file_name(&doc.name)), &theme_doc::to_json(doc)?)
    } else {
        let mem = &mut app.theme_state.memory;
        mem.retain(|d| !d.name.eq_ignore_ascii_case(&doc.name));
        mem.push(doc.clone());
        Ok(())
    }
}

fn remove_user(app: &mut SoundApp, name: &str) -> Result<(), String> {
    if let Some(dir) = app.services.themes_dir.clone() {
        let doc = find_user(app, name).ok_or_else(|| format!("no saved theme `{name}`"))?;
        std::fs::remove_file(dir.join(file_name(&doc.name))).map_err(|e| io_err("could not delete the theme", &e))
    } else {
        let mem = &mut app.theme_state.memory;
        let before = mem.len();
        mem.retain(|d| !d.name.eq_ignore_ascii_case(name.trim()));
        if mem.len() == before { Err(format!("no saved theme `{name}`")) } else { Ok(()) }
    }
}

/// A name no built-in or saved theme uses: `name`, then `name 2`, `name 3`, ...
fn unique_name(app: &SoundApp, name: &str) -> String {
    let taken = |n: &str| is_builtin(n) || find_user(app, n).is_some();
    if !taken(name) {
        return name.to_string();
    }
    (2..).map(|i| format!("{name} {i}")).find(|n| !taken(n)).unwrap_or_else(|| name.to_string())
}

fn push_track_palette(app: &mut SoundApp, doc: Option<&ThemeDoc>) {
    let colors = doc.and_then(|d| match theme_doc::track_colors(d) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("{e}");
            None
        }
    });
    let result = match colors {
        Some(c) => {
            let hex: Vec<String> = c.iter().map(|[r, g, b]| format!("#{r:02X}{g:02X}{b:02X}")).collect();
            app.engine.execute("track_palette.set", &json!({"colors": hex}))
        }
        None => app.engine.execute("track_palette.clear", &json!({})),
    };
    if let Err(e) = result {
        log::warn!("track palette: {e}");
    }
}

/// Make `doc` the active theme: tokens, custom mode and the engine's new-track palette.
fn apply(app: &mut SoundApp, doc: ThemeDoc) {
    let r = theme_doc::resolve(&doc);
    theme::set_custom(r.tokens, r.light);
    app.ui.theme = ThemeMode::Custom;
    app.ui.theme_name = Some(doc.name.clone());
    if let Some(tint) = doc.params.track_tint {
        app.ui.track_tint = tint.clamp(0.0, 1.0);
    }
    push_track_palette(app, Some(&doc));
    app.theme_state.applied = Some(doc.clone());
    app.ui.custom_theme = Some(doc);
}

/// Switch to a built-in palette mode and drop any custom theme (the engine palette is cleared).
pub fn select_builtin(app: &mut SoundApp, mode: ThemeMode) {
    app.ui.theme = mode;
    app.ui.theme_name = match mode {
        ThemeMode::Dark | ThemeMode::Light | ThemeMode::Midnight => Some(mode.label().to_string()),
        ThemeMode::System | ThemeMode::Custom => None,
    };
    app.ui.custom_theme = None;
    app.theme_state.applied = None;
    app.theme_state.saved = None;
    push_track_palette(app, None);
}

/// The base a built-in mode stands for (System follows the resolved palette).
fn mode_base(mode: ThemeMode) -> Base {
    match mode {
        ThemeMode::Light => Base::Light,
        ThemeMode::Midnight => Base::Midnight,
        ThemeMode::System if theme::is_light() => Base::Light,
        _ => Base::Dark,
    }
}

/// The working document, forking an unsaved "Custom (from X)" from the active built-in if needed.
fn working_or_fork(app: &mut SoundApp) -> ThemeDoc {
    if let Some(doc) = app.ui.custom_theme.clone() {
        return doc;
    }
    let base = mode_base(app.ui.theme);
    let from = theme_doc::builtin(base.id()).map(|d| d.name).unwrap_or_default();
    app.theme_state.saved = Some(ThemeDoc { base, ..ThemeDoc::default() });
    ThemeDoc { name: format!("Custom (from {from})"), base, ..ThemeDoc::default() }
}

fn dirty(app: &SoundApp) -> bool {
    match (&app.ui.custom_theme, &app.theme_state.saved) {
        (Some(w), Some(s)) => !same_content(w, s),
        (Some(_), None) => true,
        _ => false,
    }
}

/// Whether the working theme has unsaved changes (the Settings window's Revert).
pub fn is_dirty(app: &SoundApp) -> bool {
    dirty(app)
}

fn hexes(doc: &ThemeDoc) -> Vec<String> {
    doc.track_palette.clone()
}

fn describe(app: &SoundApp) -> Value {
    let doc = app.ui.custom_theme.clone().unwrap_or_else(|| ThemeDoc { base: mode_base(app.ui.theme), ..ThemeDoc::default() });
    let name = app.ui.theme_name.clone().unwrap_or_else(|| doc.name.clone());
    let r = theme_doc::resolve(&doc);
    let colors: serde_json::Map<String, Value> =
        theme_doc::KEYS.iter().map(|k| (k.key.to_string(), json!(theme_doc::format_hex(k.value(&r.tokens))))).collect();
    let mut warnings = r.warnings.clone();
    warnings.extend(theme_doc::contrast_warnings(&r.tokens));
    json!({
        "name": name,
        "mode": app.ui.theme.id(),
        "base": doc.base.id(),
        "dirty": dirty(app),
        "colors": colors,
        "overrides": doc.colors.keys().collect::<Vec<_>>(),
        "params": {
            "clip_opacity": f32::from(r.tokens.clip_fill_alpha) / 255.0,
            "clip_opacity_selected": f32::from(r.tokens.clip_fill_alpha_selected) / 255.0,
            "grid_strength": doc.params.grid_strength.unwrap_or(1.0),
            "grid_beat_always": r.tokens.grid_beat_always,
            "track_tint": app.ui.track_tint,
        },
        "track_palette": hexes(&doc),
        "ignored": r.ignored,
        "warnings": warnings,
    })
}

fn str_param<'a>(p: &'a Value, key: &str) -> Result<&'a str, String> {
    p.get(key).and_then(Value::as_str).ok_or_else(|| format!("`{key}` required"))
}

fn color_param(p: &Value, key: &str) -> Result<egui::Color32, String> {
    let s = str_param(p, key)?;
    theme_doc::parse_hex(s).ok_or_else(|| format!("`{s}` is not a colour (use #RRGGBB or #RRGGBBAA)"))
}

fn set_color(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let key = str_param(p, "key")?;
    if theme_doc::find_key(key).is_none() {
        return Err(format!("unknown colour `{key}`; valid keys: {}", theme_doc::key_names().join(", ")));
    }
    let c = color_param(p, "color")?;
    let mut doc = working_or_fork(app);
    doc.colors.insert(key.to_string(), theme_doc::format_hex(c));
    apply(app, doc);
    Ok(describe(app))
}

fn set_param(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let key = str_param(p, "key")?;
    let value = p.get("value").ok_or("`value` required")?;
    let num = |lo: f32, hi: f32| -> Result<f32, String> {
        let v = value.as_f64().ok_or_else(|| format!("`{key}` must be a number"))? as f32;
        if (lo..=hi).contains(&v) { Ok(v) } else { Err(format!("`{key}` must be between {lo} and {hi}")) }
    };
    let mut doc = working_or_fork(app);
    match key {
        "clip_opacity" => doc.params.clip_opacity = Some(num(0.0, 1.0)?),
        "clip_opacity_selected" => doc.params.clip_opacity_selected = Some(num(0.0, 1.0)?),
        "grid_strength" => doc.params.grid_strength = Some(num(0.0, 2.0)?),
        "track_tint" => doc.params.track_tint = Some(num(0.0, 1.0)?),
        "grid_beat_always" => doc.params.grid_beat_always = Some(value.as_bool().ok_or("`grid_beat_always` must be true or false")?),
        _ => return Err(format!("unknown setting `{key}`; valid keys: {}", PARAM_KEYS.join(", "))),
    }
    apply(app, doc);
    Ok(describe(app))
}

fn set_track_color(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let index = p.get("index").and_then(Value::as_u64).ok_or("`index` required (0-15)")? as usize;
    if index >= theme_doc::TRACK_COLOR_COUNT {
        return Err(format!("`index` must be 0-{}", theme_doc::TRACK_COLOR_COUNT - 1));
    }
    let c = color_param(p, "color")?;
    let mut doc = working_or_fork(app);
    if doc.track_palette.len() != theme_doc::TRACK_COLOR_COUNT {
        let current = app.engine.track_palette().map(<[[u8; 3]]>::to_vec).unwrap_or_else(|| soundcraft_model::TRACK_COLORS.to_vec());
        doc.track_palette = (0..theme_doc::TRACK_COLOR_COUNT)
            .map(|i| {
                let [r, g, b] = current.get(i % current.len().max(1)).copied().unwrap_or([128; 3]);
                format!("#{r:02X}{g:02X}{b:02X}")
            })
            .collect();
    }
    doc.track_palette[index] = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
    apply(app, doc);
    Ok(describe(app))
}

fn reset(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let key = p.get("key").and_then(Value::as_str);
    if app.ui.custom_theme.is_none() {
        // A built-in is already at its base: nothing to reset.
        return Ok(describe(app));
    }
    let mut doc = working_or_fork(app);
    match key {
        None => {
            doc.colors.clear();
            doc.params = theme_doc::Params::default();
            doc.track_palette.clear();
        }
        Some(k) if theme_doc::find_key(k).is_some() => {
            doc.colors.remove(k);
        }
        Some("clip_opacity") => doc.params.clip_opacity = None,
        Some("clip_opacity_selected") => doc.params.clip_opacity_selected = None,
        Some("grid_strength") => doc.params.grid_strength = None,
        Some("grid_beat_always") => doc.params.grid_beat_always = None,
        Some("track_tint") => doc.params.track_tint = None,
        Some("track_palette") => doc.track_palette.clear(),
        Some(k) => {
            return Err(format!("unknown key `{k}`; valid colours: {}; settings: {}", theme_doc::key_names().join(", "), PARAM_KEYS.join(", ")));
        }
    }
    apply(app, doc);
    Ok(describe(app))
}

fn load(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let name = str_param(p, "name")?.trim().to_string();
    let doc = if let Some(user) = find_user(app, &name) {
        Some(user)
    } else if let Some(b) = theme_doc::builtin(&name) {
        match (b.colors.is_empty() && b.params == theme_doc::Params::default() && b.track_palette.is_empty(), b.base) {
            (true, base) => {
                let mode = match base {
                    Base::Dark => ThemeMode::Dark,
                    Base::Light => ThemeMode::Light,
                    Base::Midnight => ThemeMode::Midnight,
                };
                select_builtin(app, mode);
                return Ok(describe(app));
            }
            _ => Some(b),
        }
    } else {
        None
    };
    let doc = doc.ok_or_else(|| format!("no theme named `{name}`"))?;
    app.theme_state.saved = Some(doc.clone());
    apply(app, doc);
    Ok(describe(app))
}

fn save(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let mut doc = match &app.ui.custom_theme {
        Some(d) => d.clone(),
        None => return Err("nothing to save: the active theme is a built-in; change a colour first".into()),
    };
    let name = clean_name(p.get("name").and_then(Value::as_str).unwrap_or(&doc.name))?;
    if is_builtin(&name) {
        return Err(format!("`{name}` is a built-in theme and cannot be overwritten; save under another name"));
    }
    doc.name = name;
    store_user(app, &doc)?;
    app.theme_state.saved = Some(doc.clone());
    apply(app, doc);
    Ok(describe(app))
}

fn rename(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let from = str_param(p, "from")?;
    let to = clean_name(str_param(p, "to")?)?;
    if is_builtin(from) {
        return Err(format!("`{from}` is a built-in theme and cannot be renamed"));
    }
    let mut doc = find_user(app, from).ok_or_else(|| format!("no saved theme `{from}`"))?;
    if (is_builtin(&to) || find_user(app, &to).is_some()) && !doc.name.eq_ignore_ascii_case(&to) {
        return Err(format!("a theme named `{to}` already exists"));
    }
    let old = doc.name.clone();
    doc.name.clone_from(&to);
    store_user(app, &doc)?;
    if !old.eq_ignore_ascii_case(&to) {
        remove_user(app, &old)?;
    }
    if app.ui.theme_name.as_deref() == Some(old.as_str()) {
        if let Some(w) = &mut app.ui.custom_theme {
            w.name.clone_from(&to);
        }
        app.ui.theme_name = Some(to.clone());
        if let Some(s) = &mut app.theme_state.saved {
            s.name.clone_from(&to);
        }
    }
    Ok(json!({"name": to}))
}

fn delete(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let name = str_param(p, "name")?;
    if is_builtin(name) {
        return Err(format!("`{name}` is a built-in theme and cannot be deleted"));
    }
    remove_user(app, name)?;
    Ok(json!({"deleted": name}))
}

fn duplicate(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let name = str_param(p, "name")?;
    let to = clean_name(str_param(p, "to")?)?;
    if is_builtin(&to) || find_user(app, &to).is_some() {
        return Err(format!("a theme named `{to}` already exists"));
    }
    let mut doc = find_user(app, name).or_else(|| theme_doc::builtin(name)).ok_or_else(|| format!("no theme named `{name}`"))?;
    doc.name.clone_from(&to);
    store_user(app, &doc)?;
    Ok(json!({"name": to}))
}

fn export(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let mut doc = app.ui.custom_theme.clone().unwrap_or_else(|| ThemeDoc { base: mode_base(app.ui.theme), ..ThemeDoc::default() });
    if doc.name.is_empty() {
        doc.name = app.ui.theme_name.clone().unwrap_or_else(|| "Theme".into());
    }
    let path: PathBuf = match p.get("path").and_then(Value::as_str) {
        Some(path) => PathBuf::from(path),
        None => {
            let pick = app.services.pick_save.as_ref().ok_or("no file dialog available; pass `path`")?;
            let default = file_name(&clean_name(&doc.name)?);
            PathBuf::from(pick("ui.theme_export", &default).ok_or("export cancelled")?)
        }
    };
    write_atomic(&path, &theme_doc::to_json(&doc)?)?;
    Ok(json!({"path": path.to_string_lossy(), "name": doc.name}))
}

fn import(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let path: PathBuf = match p.get("path").and_then(Value::as_str) {
        Some(path) => PathBuf::from(path),
        None => {
            let pick = app.services.pick_open.as_ref().ok_or("no file dialog available; pass `path`")?;
            PathBuf::from(pick("ui.theme_import", &["json"]).ok_or("import cancelled")?)
        }
    };
    let len = std::fs::metadata(&path).map_err(|e| io_err("could not read the theme", &e))?.len();
    if len > 1 << 20 {
        return Err("theme file is larger than 1 MiB".into());
    }
    let text = std::fs::read_to_string(&path).map_err(|e| io_err("could not read the theme", &e))?;
    let mut doc = theme_doc::from_json(&text)?;
    let base_name =
        if doc.name.trim().is_empty() { path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default() } else { doc.name.clone() };
    let base_name = clean_name(if base_name.trim().is_empty() { "Imported" } else { &base_name })?;
    doc.name = unique_name(app, &base_name);
    let r = theme_doc::resolve(&doc);
    store_user(app, &doc)?;
    app.theme_state.saved = Some(doc.clone());
    let name = doc.name.clone();
    apply(app, doc);
    Ok(json!({"name": name, "ignored": r.ignored, "warnings": r.warnings}))
}

fn list(app: &SoundApp) -> Value {
    let active = app.ui.theme_name.clone();
    let is_active = |n: &str| active.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(n));
    let mut themes: Vec<Value> =
        theme_doc::builtin_names().into_iter().map(|n| json!({"name": n, "builtin": true, "active": is_active(&n)})).collect();
    themes.extend(user_themes(app).into_iter().map(|d| json!({"name": d.name, "builtin": false, "active": is_active(&d.name)})));
    json!({
        "themes": themes,
        "active": active,
        "themes_dir": app.services.themes_dir.as_ref().map(|d| d.to_string_lossy().into_owned()),
    })
}

/// Handle a `ui.theme_*` command.
pub fn run(app: &mut SoundApp, id: &str, p: &Value) -> Result<Value, String> {
    match id {
        "ui.theme_get" => Ok(describe(app)),
        "ui.theme_list" => Ok(list(app)),
        "ui.theme_set_color" => set_color(app, p),
        "ui.theme_set_param" => set_param(app, p),
        "ui.theme_set_track_color" => set_track_color(app, p),
        "ui.theme_reset" => reset(app, p),
        "ui.theme_load" => load(app, p),
        "ui.theme_save" => save(app, p),
        "ui.theme_rename" => rename(app, p),
        "ui.theme_delete" => delete(app, p),
        "ui.theme_duplicate" => duplicate(app, p),
        "ui.theme_export" => export(app, p),
        "ui.theme_import" => import(app, p),
        _ => Err(format!("unknown theme command `{id}`")),
    }
}

/// Per-frame housekeeping, before the palette is resolved: restore the saved theme on the first
/// frame (and re-push its track palette, which the engine does not persist) and pick up
/// changes made to `UiState.custom_theme` by other routes such as `ui.set`.
pub fn frame(app: &mut SoundApp) {
    if !app.theme_state.booted {
        app.theme_state.booted = true;
        boot(app);
        return;
    }
    if app.ui.theme == ThemeMode::Custom && app.ui.custom_theme != app.theme_state.applied {
        match app.ui.custom_theme.clone() {
            Some(doc) => apply(app, doc),
            None => select_builtin(app, ThemeMode::Dark),
        }
    }
}

fn boot(app: &mut SoundApp) {
    if app.ui.theme != ThemeMode::Custom {
        push_track_palette(app, None);
        return;
    }
    let doc = app.ui.custom_theme.clone().or_else(|| app.ui.theme_name.as_deref().and_then(|n| find_user(app, n)));
    match doc {
        Some(doc) => {
            let saved = find_user(app, &doc.name).or_else(|| Some(ThemeDoc { base: doc.base, ..ThemeDoc::default() }));
            app.theme_state.saved = saved;
            apply(app, doc);
        }
        None => {
            log::warn!("the saved custom theme is missing; using Dark");
            select_builtin(app, ThemeMode::Dark);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_cleaned_and_validated() {
        assert_eq!(clean_name("  My/Theme: 1 ").as_deref(), Ok("My_Theme_ 1"));
        assert!(clean_name("   ").is_err());
        assert!(clean_name(&"x".repeat(65)).is_err());
        assert!(clean_name(&"x".repeat(64)).is_ok());
    }
}
