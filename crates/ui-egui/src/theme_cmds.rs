//! `ui.theme_*` commands: the working custom theme, saved user themes and the track palette push.
//!
//! The working copy lives in `UiState.custom_theme` (saved with the other UI prefs, so unsaved
//! edits survive a restart). Saved themes are JSON files in the shared folder
//! (`Services.shared_themes_dir` or the `shared_themes_folder` pref) or the user folder
//! (`Services.themes_dir`); with neither (tests, `SOUNDCRAFT_NO_PREFS`, the web) they are kept in
//! memory and every save says so. The shipped themes (`themes/` in the repository) are read-only.

use crate::SoundApp;
use crate::theme::{self, ThemeMode};
use crate::theme_doc::{self, Base, ThemeDoc};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// Where a theme lives. Built-in and shipped themes are read-only; shared and user themes are
/// files; memory themes exist only until the app quits (no folder to write to).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Builtin,
    Shipped,
    Shared,
    User,
    Memory,
}

impl Scope {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Shipped => "shipped",
            Self::Shared => "shared",
            Self::User => "user",
            Self::Memory => "memory",
        }
    }

    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "builtin" => Ok(Self::Builtin),
            "shipped" => Ok(Self::Shipped),
            "shared" => Ok(Self::Shared),
            "user" => Ok(Self::User),
            "memory" => Ok(Self::Memory),
            other => Err(format!("unknown scope `{other}` (builtin, shipped, shared, user or memory)")),
        }
    }

    /// Whether themes of this scope can be saved, renamed and deleted.
    pub const fn writable(self) -> bool {
        matches!(self, Self::Shared | Self::User | Self::Memory)
    }
}

/// Runtime state that is not saved with the prefs.
#[derive(Default)]
pub struct ThemeState {
    booted: bool,
    /// The document last applied to the tokens, to notice changes made through `ui.set`.
    applied: Option<ThemeDoc>,
    /// The loaded or saved document the working copy is compared with for `dirty`.
    saved: Option<ThemeDoc>,
    /// User themes when there is no folder to write to.
    memory: Vec<ThemeDoc>,
    /// Where the active custom theme was loaded from or last saved to.
    scope: Option<Scope>,
}

/// One theme the list shows.
struct Entry {
    doc: ThemeDoc,
    scope: Scope,
    path: Option<PathBuf>,
}

/// Where a save goes.
enum Target {
    Shared(PathBuf),
    User(PathBuf),
    Memory,
}

/// What a store did, reported by save, duplicate, import and move.
struct Stored {
    scope: Scope,
    path: Option<PathBuf>,
    warning: Option<String>,
}

impl Stored {
    fn add_to(&self, v: &mut Value) {
        if let Some(o) = v.as_object_mut() {
            o.insert("saved_to".into(), json!(self.scope.id()));
            if let Some(p) = &self.path {
                o.insert("path".into(), json!(p.to_string_lossy()));
            }
            if let Some(w) = &self.warning {
                o.insert("warning".into(), json!(w));
            }
        }
    }
}

/// Help text for the control protocol: the valid parameter keys.
const PARAM_KEYS: &[&str] =
    &["clip_opacity", "clip_opacity_selected", "grid_strength", "track_tint", "grid_beat_always", "clip_gloss", "clip_corner", "header_corner"];

fn same_content(a: &ThemeDoc, b: &ThemeDoc) -> bool {
    a.base == b.base && a.colors == b.colors && a.params == b.params && a.track_palette == b.track_palette
}

/// Validate and clean a theme name: non-empty, at most 64 characters, file-safe.
pub fn clean_name(raw: &str) -> Result<String, String> {
    let name: String = raw.trim().chars().map(|c| if c.is_control() || "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    if name.is_empty() {
        return Err("theme name must not be empty".into());
    }
    if name.chars().count() > theme_doc::MAX_NAME_CHARS {
        return Err("theme name must be at most 64 characters".into());
    }
    Ok(name)
}

/// The default file name of an exported theme.
fn file_name(name: &str) -> String {
    format!("{}.json", theme_doc::slug(name))
}

fn io_err(what: &str, e: &std::io::Error) -> String {
    format!("{what}: {e}")
}

fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let what = format!("could not write the theme to {}", path.display());
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| io_err(&format!("could not create the folder {}", dir.display()), &e))?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| io_err(&what, &e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io_err(&what, &e)
    })
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.file_name(), b.file_name()) {
        (Some(x), Some(y)) => x.to_string_lossy().eq_ignore_ascii_case(&y.to_string_lossy()),
        _ => false,
    }
}

/// Write `doc` into `dir` as `<slug>.json` (`-2`, `-3` ... when another theme's file has that
/// name), replacing the file that already holds a theme of this name and removing any older file
/// for it (a legacy `<Name>.json`), so a theme is never stored twice.
fn write_theme(dir: &Path, doc: &ThemeDoc) -> Result<PathBuf, String> {
    let (existing, _) = theme_doc::load_dir(dir);
    let same: Vec<PathBuf> = existing.iter().filter(|l| l.doc.name.eq_ignore_ascii_case(&doc.name)).map(|l| l.path.clone()).collect();
    let stem = theme_doc::slug(&doc.name);
    let target = (1..)
        .map(|i| if i == 1 { dir.join(format!("{stem}.json")) } else { dir.join(format!("{stem}-{i}.json")) })
        .find(|p| !p.exists() || same.iter().any(|s| same_file(s, p)))
        .ok_or("no free file name")?;
    write_atomic(&target, &theme_doc::to_json(doc)?)?;
    for old in same.iter().filter(|s| !same_file(s, &target)) {
        let _ = std::fs::remove_file(old);
    }
    Ok(target)
}

/// The shared themes folder in effect: the preference, else the one the app detected. Always
/// `None` on the web.
pub fn shared_dir(app: &SoundApp) -> Option<PathBuf> {
    if cfg!(target_arch = "wasm32") {
        return None;
    }
    match &app.ui.shared_themes_folder {
        Some(s) if s.trim().is_empty() => None,
        Some(s) => Some(PathBuf::from(s.trim())),
        None => app.services.shared_themes_dir.clone(),
    }
}

/// Where the shared folder setting comes from: `pref`, `detected` or `off`.
fn folder_source(app: &SoundApp) -> &'static str {
    match &app.ui.shared_themes_folder {
        Some(s) if s.trim().is_empty() => "off",
        Some(_) => "pref",
        None if shared_dir(app).is_some() => "detected",
        None => "off",
    }
}

/// The scope a new theme goes to when none is named: the preference when usable, else the shared
/// folder when there is one, else this computer.
pub fn default_scope(app: &SoundApp) -> Scope {
    let shared = shared_dir(app).is_some();
    let user = app.services.themes_dir.is_some();
    match app.ui.theme_default_scope.as_deref() {
        Some("user") if user => Scope::User,
        Some("shared") | None if shared => Scope::Shared,
        _ if shared && !user => Scope::Shared,
        _ => Scope::User,
    }
}

/// The scope of the active custom theme, `None` for a built-in palette or an unsaved working copy.
pub fn current_scope(app: &SoundApp) -> Option<Scope> {
    app.theme_state.scope.filter(|_| app.ui.custom_theme.is_some() || app.ui.theme == ThemeMode::Custom)
}

fn resolve_target(app: &SoundApp, requested: Option<&str>, prefer: Option<Scope>) -> Result<Target, String> {
    let scope = match requested {
        Some(s) => match Scope::parse(s)? {
            sc @ (Scope::Shared | Scope::User) => sc,
            _ => return Err("`scope` must be `shared` or `user`".into()),
        },
        None => match prefer {
            Some(Scope::Shared) if shared_dir(app).is_some() => Scope::Shared,
            Some(Scope::User) if app.services.themes_dir.is_some() => Scope::User,
            _ => default_scope(app),
        },
    };
    target_for(app, scope)
}

fn target_for(app: &SoundApp, scope: Scope) -> Result<Target, String> {
    match scope {
        Scope::Shared => {
            shared_dir(app).map(Target::Shared).ok_or_else(|| "there is no shared themes folder; choose one first (ui.theme_folder)".to_string())
        }
        Scope::User => Ok(app.services.themes_dir.clone().map_or(Target::Memory, Target::User)),
        Scope::Memory => Ok(Target::Memory),
        Scope::Builtin | Scope::Shipped => Err("built-in and shipped themes are read-only".into()),
    }
}

fn memory_warning() -> String {
    if cfg!(target_arch = "wasm32") {
        "This web build cannot save files: the theme is kept until you close the page. Use Export... to keep it.".into()
    } else {
        "Preferences are disabled for this run: the theme is kept until you quit but is NOT saved to disk. Use Export... to keep it.".into()
    }
}

/// Everything the list shows, in display order, and the files that could not be read.
fn scan(app: &SoundApp) -> (Vec<Entry>, Vec<(Scope, theme_doc::Skipped)>) {
    let mut out: Vec<Entry> =
        theme_doc::builtin_names().iter().filter_map(|n| theme_doc::builtin(n)).map(|doc| Entry { doc, scope: Scope::Builtin, path: None }).collect();
    let mut skipped = Vec::new();
    let mut files = |dir: Option<PathBuf>, scope: Scope| -> Vec<Entry> {
        let Some(dir) = dir else { return Vec::new() };
        let (loaded, bad) = theme_doc::load_dir(&dir);
        skipped.extend(bad.into_iter().map(|b| (scope, b)));
        loaded.into_iter().map(|l| Entry { doc: l.doc, scope, path: Some(l.path) }).collect()
    };
    let shared = files(shared_dir(app), Scope::Shared);
    let user = files(app.services.themes_dir.clone(), Scope::User);
    // A shared theme of the same name hides the shipped one.
    out.extend(theme_doc::shipped_docs().iter().filter(|d| !shared.iter().any(|s| s.doc.name.eq_ignore_ascii_case(&d.name))).map(|d| Entry {
        doc: d.clone(),
        scope: Scope::Shipped,
        path: None,
    }));
    out.extend(shared);
    out.extend(user);
    let mut mem = app.theme_state.memory.clone();
    mem.sort_by_key(|d| d.name.to_lowercase());
    out.extend(mem.into_iter().map(|doc| Entry { doc, scope: Scope::Memory, path: None }));
    (out, skipped)
}

/// The theme of this name (case-insensitive). Without a scope: shared, user, memory, shipped,
/// then built-in.
fn find(app: &SoundApp, name: &str, scope: Option<Scope>) -> Option<Entry> {
    let name = name.trim();
    let mut hits: Vec<Entry> =
        scan(app).0.into_iter().filter(|e| e.doc.name.eq_ignore_ascii_case(name) && scope.is_none_or(|s| s == e.scope)).collect();
    let rank = |s: Scope| match s {
        Scope::Shared => 0,
        Scope::User => 1,
        Scope::Memory => 2,
        Scope::Shipped => 3,
        Scope::Builtin => 4,
    };
    hits.sort_by_key(|e| rank(e.scope));
    hits.into_iter().next()
}

fn scope_param(p: &Value) -> Result<Option<Scope>, String> {
    p.get("scope").and_then(Value::as_str).map(Scope::parse).transpose()
}

fn is_builtin(name: &str) -> bool {
    theme_doc::builtin(name).is_some()
}

/// Whether any listed theme (built-in, shipped, shared, user or memory) already has this name.
fn name_taken(app: &SoundApp, name: &str) -> bool {
    find(app, name, None).is_some()
}

fn store(app: &mut SoundApp, doc: &ThemeDoc, target: Target) -> Result<Stored, String> {
    match target {
        Target::Shared(dir) => write_theme(&dir, doc).map(|p| Stored { scope: Scope::Shared, path: Some(p), warning: None }),
        Target::User(dir) => write_theme(&dir, doc).map(|p| Stored { scope: Scope::User, path: Some(p), warning: None }),
        Target::Memory => {
            let mem = &mut app.theme_state.memory;
            mem.retain(|d| !d.name.eq_ignore_ascii_case(&doc.name));
            mem.push(doc.clone());
            Ok(Stored { scope: Scope::Memory, path: None, warning: Some(memory_warning()) })
        }
    }
}

fn remove_entry(app: &mut SoundApp, e: &Entry) -> Result<(), String> {
    match (&e.path, e.scope) {
        (Some(path), _) => std::fs::remove_file(path).map_err(|err| io_err(&format!("could not delete {}", path.display()), &err)),
        (None, Scope::Memory) => {
            app.theme_state.memory.retain(|d| !d.name.eq_ignore_ascii_case(&e.doc.name));
            Ok(())
        }
        _ => Err(format!("`{}` is read-only", e.doc.name)),
    }
}

/// The writable theme `name` in `scope` (any writable scope when none is named), or why not.
fn find_writable(app: &SoundApp, name: &str, scope: Option<Scope>, verb: &str) -> Result<Entry, String> {
    let hit = match scope {
        Some(s) => find(app, name, Some(s)),
        None => [Scope::Shared, Scope::User, Scope::Memory].into_iter().find_map(|s| find(app, name, Some(s))),
    };
    match hit {
        Some(e) if e.scope.writable() => Ok(e),
        Some(e) => {
            Err(format!("`{}` is a {} theme and cannot be {verb}", e.doc.name, if e.scope == Scope::Builtin { "built-in" } else { "shipped" }))
        }
        None => match find(app, name, None) {
            Some(e) => {
                Err(format!("`{}` is a {} theme and cannot be {verb}", e.doc.name, if e.scope == Scope::Builtin { "built-in" } else { "shipped" }))
            }
            None => Err(format!("no saved theme `{name}`")),
        },
    }
}

/// A name no listed theme uses: `name`, then `name 2`, `name 3`, ...
fn unique_name(app: &SoundApp, name: &str) -> String {
    if !name_taken(app, name) {
        return name.to_string();
    }
    (2..).map(|i| format!("{name} {i}")).find(|n| !name_taken(app, n)).unwrap_or_else(|| name.to_string())
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
    app.theme_state.scope = None;
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
        "scope": current_scope(app).map(Scope::id),
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
            "clip_gloss": r.tokens.clip_gloss,
            "clip_corner": r.tokens.clip_corner,
            "header_corner": r.tokens.header_corner,
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
        "clip_gloss" => doc.params.clip_gloss = Some(num(0.0, 1.0)?),
        "clip_corner" => doc.params.clip_corner = Some(num(0.0, 8.0)?),
        "header_corner" => doc.params.header_corner = Some(num(0.0, 8.0)?),
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
        Some("clip_gloss") => doc.params.clip_gloss = None,
        Some("clip_corner") => doc.params.clip_corner = None,
        Some("header_corner") => doc.params.header_corner = None,
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
    let found = find(app, &name, scope_param(p)?).ok_or_else(|| format!("no theme named `{name}`"))?;
    let (scope, doc) = (found.scope, found.doc);
    if scope == Scope::Builtin && doc.colors.is_empty() && doc.params == theme_doc::Params::default() && doc.track_palette.is_empty() {
        let mode = match doc.base {
            Base::Dark => ThemeMode::Dark,
            Base::Light => ThemeMode::Light,
            Base::Midnight => ThemeMode::Midnight,
        };
        select_builtin(app, mode);
        return Ok(describe(app));
    }
    app.theme_state.saved = Some(doc.clone());
    apply(app, doc);
    app.theme_state.scope = Some(scope);
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
    let target = resolve_target(app, p.get("scope").and_then(Value::as_str), app.theme_state.scope)?;
    let stored = store(app, &doc, target)?;
    app.theme_state.saved = Some(doc.clone());
    apply(app, doc);
    app.theme_state.scope = Some(stored.scope);
    let mut out = describe(app);
    stored.add_to(&mut out);
    Ok(out)
}

fn rename(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let from = str_param(p, "from")?;
    let to = clean_name(str_param(p, "to")?)?;
    let mut e = find_writable(app, from, scope_param(p)?, "renamed")?;
    if name_taken(app, &to) && !e.doc.name.eq_ignore_ascii_case(&to) {
        return Err(format!("a theme named `{to}` already exists"));
    }
    let old = e.doc.name.clone();
    e.doc.name.clone_from(&to);
    let target = target_for(app, e.scope)?;
    let stored = store(app, &e.doc, target)?;
    // The old file goes unless the new name reuses it (a case-only rename); memory is keyed by name.
    if let Some(old_path) = &e.path
        && stored.path.as_ref().is_none_or(|n| !same_file(n, old_path))
    {
        let _ = std::fs::remove_file(old_path);
    }
    if e.scope == Scope::Memory && !old.eq_ignore_ascii_case(&to) {
        app.theme_state.memory.retain(|d| !d.name.eq_ignore_ascii_case(&old));
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
    let e = find_writable(app, name, scope_param(p)?, "deleted")?;
    remove_entry(app, &e)?;
    Ok(json!({"deleted": e.doc.name}))
}

fn duplicate(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let name = str_param(p, "name")?;
    let to = clean_name(str_param(p, "to")?)?;
    if name_taken(app, &to) {
        return Err(format!("a theme named `{to}` already exists"));
    }
    let source = find(app, name, None).ok_or_else(|| format!("no theme named `{name}`"))?;
    let target = resolve_target(app, p.get("scope").and_then(Value::as_str), Some(source.scope))?;
    let mut doc = source.doc;
    doc.name.clone_from(&to);
    let stored = store(app, &doc, target)?;
    let mut out = json!({"name": to});
    stored.add_to(&mut out);
    Ok(out)
}

/// Copy a saved theme to the other folder, then remove the original.
fn move_theme(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let name = str_param(p, "name")?;
    let from = Scope::parse(str_param(p, "from")?)?;
    let to = Scope::parse(str_param(p, "to")?)?;
    if !from.writable() || !matches!(to, Scope::Shared | Scope::User) {
        return Err("`from` must be shared, user or memory and `to` must be shared or user".into());
    }
    if from == to {
        return Err("`from` and `to` are the same".into());
    }
    let e = find(app, name, Some(from)).ok_or_else(|| format!("no {} theme `{name}`", from.id()))?;
    if find(app, &e.doc.name, Some(to)).is_some() {
        return Err(format!("a {} theme named `{}` already exists", to.id(), e.doc.name));
    }
    let target = target_for(app, to)?;
    if matches!(target, Target::Memory) {
        return Err("this computer has no themes folder to move the theme to".into());
    }
    let stored = store(app, &e.doc, target)?;
    remove_entry(app, &e)?;
    if app.ui.theme_name.as_deref().is_some_and(|n| n.eq_ignore_ascii_case(&e.doc.name)) && app.theme_state.scope == Some(from) {
        app.theme_state.scope = Some(stored.scope);
    }
    let mut out = json!({"name": e.doc.name});
    stored.add_to(&mut out);
    Ok(out)
}

/// Report or change the shared themes folder.
fn folder(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    if p.get("detect").and_then(Value::as_bool) == Some(true) {
        app.ui.shared_themes_folder = None;
    } else if let Some(v) = p.get("path") {
        match v {
            Value::Null => app.ui.shared_themes_folder = None,
            Value::String(s) => {
                let s = s.trim();
                if !s.is_empty() && Path::new(s).is_file() {
                    return Err(format!("`{s}` is a file, not a folder"));
                }
                app.ui.shared_themes_folder = Some(s.to_string());
            }
            _ => return Err("`path` must be a string (empty turns the shared folder off) or null".into()),
        }
    }
    Ok(json!({
        "shared_dir": shared_dir(app).map(|d| d.to_string_lossy().into_owned()),
        "detected": app.services.shared_themes_dir.as_ref().map(|d| d.to_string_lossy().into_owned()),
        "source": folder_source(app),
    }))
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
    let target = resolve_target(app, p.get("scope").and_then(Value::as_str), app.theme_state.scope)?;
    let stored = store(app, &doc, target)?;
    app.theme_state.saved = Some(doc.clone());
    let name = doc.name.clone();
    apply(app, doc);
    app.theme_state.scope = Some(stored.scope);
    let mut out = json!({"name": name, "ignored": r.ignored, "warnings": r.warnings});
    stored.add_to(&mut out);
    Ok(out)
}

fn list(app: &SoundApp) -> Value {
    let active = app.ui.theme_name.clone();
    let cur = app.theme_state.scope;
    let (entries, skipped) = scan(app);
    let themes: Vec<Value> = entries
        .iter()
        .map(|e| {
            let is_active = active.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(&e.doc.name)) && cur.is_none_or(|c| c == e.scope);
            let mut v = json!({
                "name": e.doc.name,
                "builtin": e.scope == Scope::Builtin,
                "active": is_active,
                "scope": e.scope.id(),
                "read_only": !e.scope.writable(),
            });
            if let (Some(path), Some(o)) = (&e.path, v.as_object_mut()) {
                o.insert("path".into(), json!(path.to_string_lossy()));
            }
            v
        })
        .collect();
    let path_of = |d: Option<PathBuf>| d.map(|d| d.to_string_lossy().into_owned());
    json!({
        "themes": themes,
        "active": active,
        "themes_dir": path_of(app.services.themes_dir.clone()),
        "user_dir": path_of(app.services.themes_dir.clone()),
        "shared_dir": path_of(shared_dir(app)),
        "default_scope": default_scope(app).id(),
        "prefs_enabled": app.services.prefs_enabled,
        "skipped": skipped.iter().map(|(s, b)| json!({"scope": s.id(), "file": b.file, "reason": b.reason})).collect::<Vec<_>>(),
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
        "ui.theme_move" => move_theme(app, p),
        "ui.theme_folder" => folder(app, p),
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
    let found = app.ui.theme_name.as_deref().and_then(|n| find(app, n, None));
    let doc = app.ui.custom_theme.clone().or_else(|| found.as_ref().map(|e| e.doc.clone()));
    match doc {
        Some(doc) => {
            let entry = find(app, &doc.name, None);
            app.theme_state.scope = entry.as_ref().map(|e| e.scope);
            app.theme_state.saved = entry.map(|e| e.doc).or_else(|| Some(ThemeDoc { base: doc.base, ..ThemeDoc::default() }));
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
