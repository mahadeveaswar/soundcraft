//! The theme document: a small JSON file that overrides some of a built-in palette's colours.
//!
//! A document names a `base` palette, lists `#RRGGBB` / `#RRGGBBAA` overrides by key and a few
//! numeric params. [`resolve`] turns it into [`Tokens`]; an untouched document equals its base
//! preset exactly. Unknown colour keys are ignored and reported, missing keys use the base.

use crate::theme::Tokens;
use egui::Color32;
use std::collections::BTreeMap;

/// The `format` string of every theme file.
pub const FORMAT: &str = "soundcraft-theme";
/// The newest document version this build understands; newer files still load, with a warning.
pub const VERSION: u32 = 1;
/// Number of default track colours a theme can carry.
pub const TRACK_COLOR_COUNT: usize = 16;

/// The built-in palette a document starts from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Base {
    #[default]
    Dark,
    Light,
    Midnight,
}

impl Base {
    pub const fn tokens(self) -> Tokens {
        match self {
            Self::Dark => Tokens::DARK,
            Self::Light => Tokens::LIGHT,
            Self::Midnight => Tokens::MIDNIGHT,
        }
    }

    /// Whether egui's light widget visuals go with this base (Midnight maps to dark).
    pub const fn is_light(self) -> bool {
        matches!(self, Self::Light)
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
            Self::Midnight => "midnight",
        }
    }
}

/// Numeric and boolean settings; `None` keeps the base palette's value. Fields are declared in
/// alphabetical order and unset ones are not written, so saved files are canonical.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Params {
    /// Clip corner radius in points, 0..=8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_corner: Option<f32>,
    /// Glossy clips: gloss strength 0..=1 (0 is flat).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_gloss: Option<f32>,
    /// Clip body opacity 0..=1 (1 is an opaque body).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_opacity: Option<f32>,
    /// Clip body opacity while selected, 0..=1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_opacity_selected: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_beat_always: Option<bool>,
    /// Grid line strength 0..=2 relative to the base palette (1 is unchanged).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grid_strength: Option<f32>,
    /// Track header corner radius in points, 0..=8.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_corner: Option<f32>,
    /// Track colour tint 0..=1 in the Edit window.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_tint: Option<f32>,
}

/// A theme file. Fields are declared in alphabetical order: that is the key order on disk.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct ThemeDoc {
    pub base: Base,
    pub colors: BTreeMap<String, String>,
    pub format: String,
    pub name: String,
    pub params: Params,
    /// Default colours for new tracks, `#RRGGBB` each; empty keeps the built-in set.
    pub track_palette: Vec<String>,
    pub version: u32,
}

impl Default for ThemeDoc {
    fn default() -> Self {
        Self {
            format: FORMAT.into(),
            version: VERSION,
            name: String::new(),
            base: Base::Dark,
            colors: BTreeMap::new(),
            params: Params::default(),
            track_palette: Vec::new(),
        }
    }
}

/// One user-facing colour: the key used in files and commands, a label, its window section and
/// the function applying a colour (and the fields derived from it) to the tokens.
pub struct ColorKey {
    pub key: &'static str,
    pub label: &'static str,
    pub section: &'static str,
    set: fn(&mut Tokens, Color32, bool),
    get: fn(&Tokens) -> Color32,
}

impl ColorKey {
    /// The colour this key currently shows in `t` (its primary field).
    pub fn value(&self, t: &Tokens) -> Color32 {
        (self.get)(t)
    }
}

/// Move `c` towards white (dark base) or black (light base) by `k`.
fn lift(c: Color32, k: f32, light: bool) -> Color32 {
    crate::widgets::shade(c, if light { -k } else { k })
}

macro_rules! key {
    ($key:literal, $label:literal, $section:literal, $field:ident, |$t:ident, $c:ident, $light:ident| $body:block) => {
        ColorKey {
            key: $key,
            label: $label,
            section: $section,
            get: |t: &Tokens| t.$field,
            set: |$t: &mut Tokens, $c: Color32, $light: bool| {
                let _ = $light;
                $body
            },
        }
    };
}

/// The exposed colours, in window order.
pub static KEYS: &[ColorKey] = &[
    key!("window_bg", "Window background", "Surfaces", window_bg, |t, c, _l| {
        t.window_bg = c;
        t.menu_bar = c;
        t.universe_bg = c;
    }),
    key!("panel", "Panel", "Surfaces", panel_bg, |t, c, l| {
        t.panel_bg = c;
        t.strip_bg = c;
        t.strip_section = c;
        t.panel_bg2 = lift(c, 0.04, l);
        t.toolbar_bg = c;
        t.toolbar_group = c;
        t.lane_bg = c;
        t.meter_bg = lift(c, -0.6, l);
        t.meter_clip_off = lift(c, 0.1, l);
    }),
    key!("border", "Border", "Surfaces", border, |t, c, l| {
        t.border = c;
        t.group_border = c;
        t.counter_border = c;
        t.button_border = c;
        t.border_light = lift(c, 0.2, l);
        t.edge_light = lift(c, 0.3, l);
    }),
    key!("button", "Button", "Surfaces", button, |t, c, l| {
        t.button = c;
        t.mode_off = c;
        t.counter_off = c;
        t.knob_bg = c;
        t.slot_empty = c;
        t.button_hi = lift(c, 0.12, l);
    }),
    key!("arrange_bg", "Arrange background", "Surfaces", playlist_bg, |t, c, l| {
        t.playlist_bg = c;
        t.playlist_alt = lift(c, 0.04, l);
        t.playlist_lane = lift(c, -0.04, l);
    }),
    key!("text", "Text", "Text", text, |t, c, _l| {
        t.text = c;
        t.header_text = c;
        t.section_label = c;
        t.marker_text = c;
    }),
    key!("text_dim", "Dim text", "Text", text_dim, |t, c, _l| {
        t.text_dim = c;
        t.mode_off_text = c;
        t.ruler_text = c;
        t.lane_label = c;
    }),
    key!("text_on_accent", "Text on accent", "Text", text_dark, |t, c, _l| {
        t.text_dark = c;
        t.mode_on_text = c;
    }),
    key!("accent", "Accent", "Accents", accent, |t, c, l| {
        t.accent = c;
        t.mode_on = c;
        t.knob_pointer = c;
        t.universe_view = c;
        t.accent_dark = lift(c, -0.35, l);
        t.row_selected = lift(c, -0.6, l);
        t.name_field_sel = lift(c, -0.6, l);
    }),
    key!("selection", "Selection", "Accents", selection, |t, c, _l| {
        t.selection = c;
        t.header_selected = c;
        t.strip_selected = c;
    }),
    key!("playhead", "Playhead", "Accents", playhead, |t, c, _l| {
        t.playhead = c;
        t.insertion = c;
    }),
    key!("ruler", "Ruler", "Grid", ruler_bg, |t, c, l| {
        t.ruler_bg = c;
        t.ruler_label_bg = c;
        t.tempo_ruler = c;
        t.meter_ruler = c;
        t.marker_ruler = c;
        t.ruler_tick = lift(c, 0.35, l);
    }),
    key!("bar_lines", "Bar lines", "Grid", grid_bar, |t, c, _l| {
        t.grid_bar = c;
        t.bar_line = c;
    }),
    key!("beat_lines", "Beat lines", "Grid", grid_beat, |t, c, _l| {
        t.grid_beat = c;
        t.grid_line = c;
    }),
    key!("counter", "Counter", "Transport", counter_bg, |t, c, l| {
        t.counter_bg = c;
        t.big_counter_bg = c;
        t.counter_text = c;
        t.counter_label = lift(c, -0.3, l);
        t.counter_rule = lift(c, -0.5, l);
    }),
    key!("record", "Record", "Transport", rec, |t, c, _l| {
        t.rec = c;
        t.auto_write = c;
    }),
    key!("solo", "Solo", "Transport", solo, |t, c, _l| {
        t.solo = c;
        t.auto_touch = c;
    }),
    key!("mute", "Mute", "Transport", mute, |t, c, _l| {
        t.mute = c;
    }),
    key!("meter_low", "Meter low", "Meters", meter_green, |t, c, _l| {
        t.meter_green = c;
    }),
    key!("meter_mid", "Meter mid", "Meters", meter_yellow, |t, c, _l| {
        t.meter_yellow = c;
    }),
    key!("meter_high", "Meter high", "Meters", meter_red, |t, c, _l| {
        t.meter_red = c;
    }),
    key!("roll_row", "Piano roll rows", "Piano roll", roll_row_white, |t, c, l| {
        t.roll_row_white = c;
        t.roll_row_black = lift(c, -0.15, l);
        t.roll_grid = lift(c, 0.12, l);
        t.roll_octave = lift(c, 0.2, l);
        t.roll_bar = lift(c, 0.3, l);
        t.lane_zero = lift(c, 0.2, l);
    }),
    key!("note_soft", "MIDI notes soft", "Piano roll", midi_vel_lo, |t, c, _l| {
        t.midi_vel_lo = c;
    }),
    key!("note_hard", "MIDI notes hard", "Piano roll", midi_vel_hi, |t, c, _l| {
        t.midi_vel_hi = c;
        t.midi_vel_mid = t.midi_vel_lo.lerp_to_gamma(c, 0.5);
    }),
    key!("automation", "Automation", "Automation", automation_line, |t, c, _l| {
        t.automation_line = c;
        t.auto_read = c;
        t.auto_latch = c;
    }),
];

/// All valid colour keys, for error messages.
pub fn key_names() -> Vec<&'static str> {
    KEYS.iter().map(|k| k.key).collect()
}

pub fn find_key(key: &str) -> Option<&'static ColorKey> {
    KEYS.iter().find(|k| k.key == key)
}

/// Parse `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
pub fn parse_hex(s: &str) -> Option<Color32> {
    let h = s.trim().strip_prefix('#').unwrap_or_else(|| s.trim());
    if !h.is_ascii() || !(h.len() == 6 || h.len() == 8) {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    let (r, g, b) = (byte(0)?, byte(2)?, byte(4)?);
    Some(if h.len() == 8 { Color32::from_rgba_unmultiplied(r, g, b, byte(6)?) } else { Color32::from_rgb(r, g, b) })
}

/// `#RRGGBB`, or `#RRGGBBAA` when not opaque.
pub fn format_hex(c: Color32) -> String {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    if a == 255 { format!("#{r:02X}{g:02X}{b:02X}") } else { format!("#{r:02X}{g:02X}{b:02X}{a:02X}") }
}

fn linear(v: u8) -> f64 {
    let s = f64::from(v) / 255.0;
    if s <= 0.04045 { s / 12.92 } else { ((s + 0.055) / 1.055).powf(2.4) }
}

/// WCAG relative luminance (alpha ignored).
pub fn luminance(c: Color32) -> f64 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b)
}

/// WCAG contrast ratio of two colours, 1..=21.
pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let (la, lb) = (luminance(a), luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// What [`resolve`] produced.
#[derive(Clone, Debug)]
pub struct Resolved {
    pub tokens: Tokens,
    /// egui light widget visuals (the base is Light).
    pub light: bool,
    /// Colour keys in the document that this build does not know.
    pub ignored: Vec<String>,
    /// Colour values that were not valid hex (ignored) and similar notes.
    pub warnings: Vec<String>,
}

fn scale_towards(c: Color32, base: Color32, k: f32) -> Color32 {
    let ch = |c: u8, b: u8| (f32::from(b) + (f32::from(c) - f32::from(b)) * k).round().clamp(0.0, 255.0) as u8;
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let [br, bg, bb, _] = base.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(ch(r, br), ch(g, bg), ch(b, bb), a)
}

fn alpha_of(opacity: f32) -> u8 {
    (opacity.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// A corner radius param (points) as the token value, clamped to 0..=8.
fn radius_of(points: f32) -> u8 {
    if points.is_finite() { points.clamp(0.0, 8.0).round() as u8 } else { 0 }
}

/// Resolve a document to tokens: the base preset, then colour overrides, then params.
pub fn resolve(doc: &ThemeDoc) -> Resolved {
    let mut tokens = doc.base.tokens();
    let light = doc.base.is_light();
    let mut ignored = Vec::new();
    let mut warnings = Vec::new();
    if doc.version > VERSION {
        warnings.push(format!("theme version {} is newer than {VERSION}; unknown settings are ignored", doc.version));
    }
    for (key, value) in &doc.colors {
        match (find_key(key), parse_hex(value)) {
            (Some(k), Some(c)) => (k.set)(&mut tokens, c, light),
            (Some(_), None) => warnings.push(format!("colour `{key}`: `{value}` is not #RRGGBB or #RRGGBBAA")),
            (None, _) => ignored.push(key.clone()),
        }
    }
    let p = &doc.params;
    if let Some(o) = p.clip_opacity {
        tokens.clip_fill_alpha = alpha_of(o);
    }
    if let Some(o) = p.clip_opacity_selected {
        tokens.clip_fill_alpha_selected = alpha_of(o);
    }
    if let Some(g) = p.clip_gloss {
        tokens.clip_gloss = g.clamp(0.0, 1.0);
    }
    if let Some(c) = p.clip_corner {
        tokens.clip_corner = radius_of(c);
    }
    if let Some(c) = p.header_corner {
        tokens.header_corner = radius_of(c);
    }
    if let Some(b) = p.grid_beat_always {
        tokens.grid_beat_always = b;
    }
    if let Some(k) = p.grid_strength {
        let k = k.clamp(0.0, 2.0);
        tokens.grid_bar = scale_towards(tokens.grid_bar, tokens.playlist_bg, k);
        tokens.grid_beat = scale_towards(tokens.grid_beat, tokens.playlist_bg, k);
    }
    Resolved { tokens, light, ignored, warnings }
}

/// Parse a theme file. Fails only on malformed JSON or a foreign `format`.
pub fn from_json(text: &str) -> Result<ThemeDoc, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|e| format!("not a theme file: {e}"))?;
    if !value.is_object() {
        return Err("not a theme file: expected a JSON object".into());
    }
    let doc: ThemeDoc = serde_json::from_value(value).map_err(|e| format!("not a theme file: {e}"))?;
    if doc.format != FORMAT {
        return Err(format!("not a SoundCraft theme file (format `{}`)", doc.format));
    }
    Ok(doc)
}

/// The canonical text of a theme file: pretty with 2-space indents, keys sorted, unset settings
/// left out, LF line ends and one trailing newline. Saving the same theme twice gives the same
/// bytes, so a theme in git only changes when the theme does.
pub fn to_json(doc: &ThemeDoc) -> Result<String, String> {
    let mut text = serde_json::to_string_pretty(doc).map_err(|e| e.to_string())?;
    text.push('\n');
    Ok(text)
}

/// The most a theme file may hold (bytes); bigger files are skipped when a folder is scanned.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;
/// The longest theme name (characters).
pub const MAX_NAME_CHARS: usize = 64;

/// The file stem for a theme name: lowercase, letters and digits kept, space, `-` and `_` become
/// one `-`, everything else is dropped, at most 64 characters, `theme` when nothing is left.
/// (`cargo xtask themes` carries a copy of this rule; keep them in step.)
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.trim().chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if (c == ' ' || c == '-' || c == '_') && !out.is_empty() && !out.ends_with('-') {
            out.push('-');
        }
    }
    let out: String = out.chars().take(MAX_NAME_CHARS).collect();
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "theme".into() } else { out }
}

/// A theme file that a folder scan did not accept, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Skipped {
    pub file: String,
    pub reason: String,
}

/// A theme read from a file.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub doc: ThemeDoc,
    pub path: std::path::PathBuf,
}

/// Parse one theme file for a folder scan: size capped, UTF-8, valid JSON of our format, a usable
/// name (an empty one falls back to the file stem). A newer version still loads.
pub fn load_file(path: &std::path::Path) -> Result<ThemeDoc, String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len > MAX_FILE_BYTES {
        return Err(format!("larger than {} KiB", MAX_FILE_BYTES / 1024));
    }
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    let text = String::from_utf8(bytes).map_err(|_| "not valid UTF-8 text".to_string())?;
    let mut doc = from_json(&text)?;
    if doc.name.trim().is_empty() {
        doc.name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    }
    doc.name = doc.name.trim().to_string();
    if doc.name.is_empty() || doc.name.chars().count() > MAX_NAME_CHARS || doc.name.chars().any(char::is_control) {
        return Err("the theme name is empty, longer than 64 characters or has control characters".into());
    }
    Ok(doc)
}

/// Every acceptable `*.json` theme in `dir` (hidden files and `*.json.tmp` leftovers ignored),
/// sorted by name, plus the files that were skipped. A missing folder is empty. Never panics.
pub fn load_dir(dir: &std::path::Path) -> (Vec<Loaded>, Vec<Skipped>) {
    let mut loaded: Vec<Loaded> = Vec::new();
    let mut skipped = Vec::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return (loaded, skipped) };
    let mut paths: Vec<std::path::PathBuf> = rd
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
        .filter(|p| !p.file_name().is_some_and(|n| n.to_string_lossy().starts_with('.')))
        .collect();
    paths.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
    for path in paths {
        let file = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        match load_file(&path) {
            Ok(doc) if loaded.iter().any(|l| l.doc.name.eq_ignore_ascii_case(&doc.name)) => {
                skipped.push(Skipped { file, reason: format!("another file already defines `{}`", doc.name) });
            }
            Ok(doc) => loaded.push(Loaded { doc, path }),
            Err(reason) => {
                log::warn!("{}: {reason}", path.display());
                skipped.push(Skipped { file, reason });
            }
        }
    }
    loaded.sort_by_key(|l| l.doc.name.to_lowercase());
    (loaded, skipped)
}

mod shipped_data {
    include!(concat!(env!("OUT_DIR"), "/shipped_themes.rs"));
}

/// The themes in the repository's `themes/` folder, embedded at build time (file name, text).
pub fn shipped_sources() -> &'static [(&'static str, &'static str)] {
    shipped_data::SHIPPED
}

/// The shipped themes, parsed once. An invalid file is skipped (a unit test fails on one).
pub fn shipped_docs() -> &'static [ThemeDoc] {
    static DOCS: std::sync::OnceLock<Vec<ThemeDoc>> = std::sync::OnceLock::new();
    DOCS.get_or_init(|| {
        let mut docs: Vec<ThemeDoc> = shipped_sources()
            .iter()
            .filter_map(|(file, text)| match from_json(text) {
                Ok(d) if !d.name.trim().is_empty() => Some(d),
                Ok(_) => {
                    log::warn!("shipped theme {file}: no name");
                    None
                }
                Err(e) => {
                    log::warn!("shipped theme {file}: {e}");
                    None
                }
            })
            .collect();
        docs.sort_by_key(|d| d.name.to_lowercase());
        docs
    })
}

/// The track palette of a document as RGB triples; `None` when it carries none. Invalid entries
/// make the whole palette invalid (a warning is better than a half-applied set).
pub fn track_colors(doc: &ThemeDoc) -> Result<Option<Vec<[u8; 3]>>, String> {
    if doc.track_palette.is_empty() {
        return Ok(None);
    }
    doc.track_palette
        .iter()
        .map(|h| parse_hex(h).map(|c| [c.r(), c.g(), c.b()]).ok_or_else(|| format!("track colour `{h}` is not #RRGGBB")))
        .collect::<Result<Vec<_>, _>>()
        .map(Some)
}

/// Contrast checks shown as warnings: (label, foreground, background, minimum ratio).
pub fn contrast_checks(t: &Tokens) -> Vec<(&'static str, f64, f64)> {
    [
        ("Text on Panel", t.text, t.panel_bg, 4.5),
        ("Text on raised panel", t.text, t.panel_bg2, 4.5),
        ("Dim text on Panel", t.text_dim, t.panel_bg, 3.0),
        ("Counter text on Counter", t.counter_text, t.counter_bg, 4.5),
        ("Text on accent", t.text_dark, t.accent, 4.5),
        ("Ruler text on Ruler", t.ruler_text, t.ruler_bg, 4.5),
        ("Text on Arrange background", t.text, t.playlist_bg, 4.5),
    ]
    .into_iter()
    .map(|(label, fg, bg, min)| (label, contrast_ratio(fg, bg), min))
    .collect()
}

/// Warnings for every contrast check below its threshold.
pub fn contrast_warnings(t: &Tokens) -> Vec<String> {
    contrast_checks(t).into_iter().filter(|(_, ratio, min)| ratio < min).map(|(label, ratio, _)| format!("Contrast {ratio:.1}:1 - {label}")).collect()
}

/// The built-in themes beyond Dark, Light and Midnight (read-only; selected through the custom palette).
pub fn builtin_docs() -> Vec<ThemeDoc> {
    let colors = [
        ("text", "#FFFFFF"),
        ("text_dim", "#D8D8D8"),
        ("meter_low", "#0072B2"),
        ("meter_mid", "#E69F00"),
        ("meter_high", "#CC79A7"),
        ("record", "#D55E00"),
        ("solo", "#F0E442"),
        ("mute", "#56B4E9"),
    ];
    let mut doc = ThemeDoc { name: "Colour-safe".into(), base: Base::Dark, ..ThemeDoc::default() };
    doc.colors = colors.iter().map(|(k, v)| ((*k).to_string(), (*v).to_string())).collect();
    vec![doc]
}

/// Names of the built-in themes: the three palettes plus [`builtin_docs`].
pub fn builtin_names() -> Vec<String> {
    ["Dark", "Light", "Midnight"].iter().map(|s| (*s).to_string()).chain(builtin_docs().into_iter().map(|d| d.name)).collect()
}

/// The built-in document of this name (case-insensitive): the palettes are empty documents.
pub fn builtin(name: &str) -> Option<ThemeDoc> {
    let n = name.trim();
    for base in [Base::Dark, Base::Light, Base::Midnight] {
        if n.eq_ignore_ascii_case(base.id()) {
            return Some(ThemeDoc { name: base.id()[..1].to_uppercase() + &base.id()[1..], base, ..ThemeDoc::default() });
        }
    }
    builtin_docs().into_iter().find(|d| d.name.eq_ignore_ascii_case(n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_document_equals_its_base_preset() {
        for base in [Base::Dark, Base::Light, Base::Midnight] {
            let doc = ThemeDoc { base, ..ThemeDoc::default() };
            let r = resolve(&doc);
            assert_eq!(r.tokens, base.tokens());
            assert_eq!(r.light, base == Base::Light);
            assert!(r.ignored.is_empty() && r.warnings.is_empty());
        }
    }

    #[test]
    fn hex_round_trips_and_bad_hex_is_rejected() {
        for s in ["#000000", "#FF8800", "#123456", "#10203040"] {
            let c = parse_hex(s).expect("valid hex");
            let again = parse_hex(&format_hex(c)).expect("round trip");
            assert_eq!(c, again);
        }
        assert_eq!(format_hex(Color32::from_rgb(255, 136, 0)), "#FF8800");
        assert_eq!(parse_hex("ff8800"), Some(Color32::from_rgb(255, 136, 0)));
        for bad in ["", "#", "#12345", "#GG0000", "#1234567", "#12345é", "red"] {
            assert!(parse_hex(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn unknown_keys_are_ignored_and_reported() {
        let mut doc = ThemeDoc::default();
        doc.colors.insert("accent".into(), "#FF8800".into());
        doc.colors.insert("no_such_key".into(), "#000000".into());
        doc.colors.insert("text".into(), "nonsense".into());
        let r = resolve(&doc);
        assert_eq!(r.ignored, vec!["no_such_key".to_string()]);
        assert_eq!(r.warnings.len(), 1);
        assert_eq!(r.tokens.accent, Color32::from_rgb(255, 136, 0));
        assert_eq!(r.tokens.text, Tokens::DARK.text);
    }

    #[test]
    fn a_newer_version_loads_with_a_warning() {
        let doc = from_json(r##"{"format":"soundcraft-theme","version":99,"colors":{"accent":"#FF8800"},"future":true}"##).expect("loads");
        assert_eq!(doc.version, 99);
        let r = resolve(&doc);
        assert_eq!(r.tokens.accent, Color32::from_rgb(255, 136, 0));
        assert_eq!(r.warnings.len(), 1);
        assert!(from_json("[]").is_err());
        assert!(from_json(r#"{"format":"other"}"#).is_err());
    }

    #[test]
    fn every_key_changes_at_least_one_field() {
        // A colour no preset uses, so every setter must move something.
        let odd = "#0A0B0C";
        for base in [Base::Dark, Base::Light, Base::Midnight] {
            for k in KEYS {
                let mut doc = ThemeDoc { base, ..ThemeDoc::default() };
                doc.colors.insert(k.key.into(), odd.into());
                let r = resolve(&doc);
                assert_ne!(r.tokens, base.tokens(), "{} on {:?}", k.key, base);
            }
        }
        let names = key_names();
        assert_eq!(names.len(), 25);
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "keys are unique");
    }

    #[test]
    fn params_drive_clip_opacity_grid_and_beat_lines() {
        let doc = ThemeDoc {
            params: Params {
                clip_opacity: Some(0.5),
                clip_opacity_selected: Some(1.0),
                grid_strength: Some(0.0),
                grid_beat_always: Some(true),
                track_tint: None,
                ..Params::default()
            },
            ..ThemeDoc::default()
        };
        let r = resolve(&doc);
        assert_eq!((r.tokens.clip_fill_alpha, r.tokens.clip_fill_alpha_selected), (128, 255));
        assert!(r.tokens.grid_beat_always);
        assert_eq!(r.tokens.grid_bar, Tokens::DARK.playlist_bg);
        let same = resolve(&ThemeDoc { params: Params { grid_strength: Some(1.0), ..Params::default() }, ..ThemeDoc::default() });
        assert_eq!(same.tokens, Tokens::DARK);
    }

    #[test]
    fn gloss_and_corner_params_resolve_clamp_and_round_trip() {
        let doc = ThemeDoc {
            base: Base::Midnight,
            params: Params { clip_gloss: Some(0.0), clip_corner: Some(7.6), header_corner: Some(99.0), ..Params::default() },
            ..ThemeDoc::default()
        };
        let r = resolve(&doc);
        assert_eq!((r.tokens.clip_gloss, r.tokens.clip_corner, r.tokens.header_corner), (0.0, 8, 8));
        let back = from_json(&to_json(&doc).unwrap_or_default()).unwrap_or_default();
        assert_eq!(back.params, doc.params);
        let nan = resolve(&ThemeDoc { params: Params { clip_corner: Some(f32::NAN), ..Params::default() }, ..ThemeDoc::default() });
        assert_eq!(nan.tokens.clip_corner, 0);
        // An older file without the keys keeps the base values.
        let old = from_json(r#"{"format":"soundcraft-theme","version":1,"base":"midnight","params":{"clip_opacity":0.5}}"#).unwrap_or_default();
        let (o, m) = (resolve(&old).tokens, Tokens::MIDNIGHT);
        assert_eq!((o.clip_gloss, o.clip_corner, o.header_corner), (m.clip_gloss, m.clip_corner, m.header_corner));
    }

    #[test]
    fn contrast_of_black_white_and_equal_colours() {
        assert!((contrast_ratio(Color32::WHITE, Color32::BLACK) - 21.0).abs() < 1e-9);
        assert!((contrast_ratio(Color32::from_rgb(90, 90, 90), Color32::from_rgb(90, 90, 90)) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn contrast_warnings_flag_text_equal_to_panel() {
        let mut doc = ThemeDoc::default();
        doc.colors.insert("text".into(), format_hex(Tokens::DARK.panel_bg));
        let r = resolve(&doc);
        assert!(contrast_warnings(&r.tokens).iter().any(|w| w.contains("Text on Panel")));
        assert!(contrast_warnings(&Tokens::DARK).is_empty());
    }

    #[test]
    fn colour_safe_preset_has_high_contrast_text() {
        let doc = builtin("colour-safe").expect("built in");
        let t = resolve(&doc).tokens;
        for bg in [t.panel_bg, t.panel_bg2, t.playlist_bg] {
            assert!(contrast_ratio(t.text, bg) >= 7.0);
        }
        let lum = [t.rec, t.solo, t.mute].map(luminance);
        assert!(lum[0] != lum[1] && lum[1] != lum[2] && lum[0] != lum[2], "signals differ in luminance");
        assert!(builtin("DARK").is_some() && builtin("nope").is_none());
    }

    #[test]
    fn track_palette_parses_or_reports() {
        let mut doc = ThemeDoc::default();
        assert_eq!(track_colors(&doc), Ok(None));
        doc.track_palette = vec!["#FF0000".into(), "#00FF00".into()];
        assert_eq!(track_colors(&doc), Ok(Some(vec![[255, 0, 0], [0, 255, 0]])));
        doc.track_palette.push("zzz".into());
        assert!(track_colors(&doc).is_err());
    }
}

#[cfg(test)]
mod file_tests {
    use super::*;

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("soundcraft-themedoc-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp folder");
        dir
    }

    fn doc(name: &str) -> ThemeDoc {
        let mut d = ThemeDoc { name: name.into(), ..ThemeDoc::default() };
        d.colors.insert("accent".into(), "#FF8800".into());
        d.params.clip_opacity = Some(0.3);
        d
    }

    #[test]
    fn slugs_are_lowercase_dashed_and_never_empty() {
        for (name, want) in [
            ("Sunset", "sunset"),
            ("My Theme", "my-theme"),
            ("  a__b - c  ", "a-b-c"),
            ("--x--", "x"),
            ("Colour-safe", "colour-safe"),
            ("Dark 2", "dark-2"),
            ("!!!", "theme"),
            ("", "theme"),
            ("Caf\u{e9} Noir", "caf\u{e9}-noir"),
        ] {
            assert_eq!(slug(name), want, "{name:?}");
        }
        let long = slug(&"ab ".repeat(40));
        assert!(long.chars().count() <= MAX_NAME_CHARS && !long.ends_with('-'), "{long}");
    }

    #[test]
    fn saved_text_is_canonical_sorted_and_identical_when_saved_twice() {
        let d = doc("Sunset");
        let (a, b) = (to_json(&d).expect("json"), to_json(&d).expect("json"));
        assert_eq!(a, b);
        assert!(a.ends_with("}\n") && !a.ends_with("\n\n") && !a.contains('\r'));
        let keys = ["\"base\"", "\"colors\"", "\"format\"", "\"name\"", "\"params\"", "\"track_palette\"", "\"version\""];
        let at: Vec<usize> = keys.iter().map(|k| a.find(k).expect("key present")).collect();
        assert!(at.windows(2).all(|w| w[0] < w[1]), "top-level keys sorted: {a}");
        assert!(!a.contains("null"), "unset settings are left out: {a}");
        assert_eq!(from_json(&a).expect("reads back"), d);
        // Settings keep alphabetical order however they were set.
        let mut p = doc("P");
        p.params = Params { track_tint: Some(0.5), clip_corner: Some(2.0), grid_strength: Some(1.0), ..Params::default() };
        let t = to_json(&p).expect("json");
        assert!(t.find("clip_corner") < t.find("grid_strength") && t.find("grid_strength") < t.find("track_tint"), "{t}");
    }

    #[test]
    fn a_folder_scan_keeps_good_files_and_reports_the_rest() {
        let dir = temp_dir("scan");
        let good = to_json(&doc("Good")).expect("json");
        std::fs::write(dir.join("good.json"), &good).expect("write");
        std::fs::write(dir.join("Same.json"), to_json(&doc("good")).expect("json")).expect("write");
        std::fs::write(dir.join("broken.json"), "{not json").expect("write");
        std::fs::write(dir.join("foreign.json"), r#"{"format":"something-else","name":"X"}"#).expect("write");
        std::fs::write(dir.join("big.json"), format!("{{\"format\":\"soundcraft-theme\",\"name\":\"Big\",\"pad\":\"{}\"}}", "x".repeat(300 * 1024)))
            .expect("write");
        std::fs::write(dir.join("binary.json"), [0xff_u8, 0xfe, 0x00, 0x80]).expect("write");
        std::fs::write(dir.join("longname.json"), format!("{{\"format\":\"soundcraft-theme\",\"name\":\"{}\"}}", "n".repeat(65))).expect("write");
        std::fs::write(dir.join("newer.json"), r##"{"format":"soundcraft-theme","version":99,"name":"Newer","future":1}"##).expect("write");
        std::fs::write(dir.join("nameless.json"), r#"{"format":"soundcraft-theme"}"#).expect("write");
        std::fs::write(dir.join("good.json.tmp"), "leftover").expect("write");
        std::fs::write(dir.join(".hidden.json"), good).expect("write");
        std::fs::write(dir.join("notes.txt"), "hello").expect("write");
        std::fs::create_dir_all(dir.join("sub.json")).expect("a folder named like a theme");
        let (loaded, skipped) = load_dir(&dir);
        let names: Vec<&str> = loaded.iter().map(|l| l.doc.name.as_str()).collect();
        assert_eq!(names, ["Good", "nameless", "Newer"], "{skipped:?}");
        assert_eq!(loaded.iter().find(|l| l.doc.name == "Newer").map(|l| l.doc.version), Some(99));
        let files: Vec<&str> = skipped.iter().map(|s| s.file.as_str()).collect();
        for f in ["Same.json", "big.json", "binary.json", "broken.json", "foreign.json", "longname.json"] {
            assert!(files.contains(&f), "{f} should be skipped: {files:?}");
        }
        assert_eq!(skipped.len(), 6, "{skipped:?}");
        assert!(skipped.iter().find(|s| s.file == "big.json").is_some_and(|s| s.reason.contains("256")));
        assert_eq!(load_dir(&dir.join("missing")).0.len(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn shipped_themes_parse_are_canonical_generic_and_resolve_cleanly() {
        assert!(!shipped_sources().is_empty(), "the themes folder ships at least one theme");
        assert_eq!(shipped_docs().len(), shipped_sources().len(), "every shipped file parses");
        for (file, text) in shipped_sources() {
            let d = from_json(text).unwrap_or_else(|e| panic!("{file}: {e}"));
            assert_eq!(format!("{}.json", slug(&d.name)), *file, "file name is the slug of the name");
            assert_eq!(&to_json(&d).expect("json"), text, "{file} is in canonical form");
            assert!(!text.contains('\r'), "{file} uses LF");
            let r = resolve(&d);
            assert!(r.ignored.is_empty() && r.warnings.is_empty(), "{file}: {:?} {:?}", r.ignored, r.warnings);
            assert!(contrast_warnings(&r.tokens).is_empty(), "{file}: contrast");
            assert!(track_colors(&d).is_ok(), "{file}: track colours");
            assert!(builtin(&d.name).is_none(), "{file}: a built-in name is reserved");
        }
    }

    /// The rule behind the names policy, as far as a committed test can state it: shipped theme
    /// names are plain words (letters, digits, spaces, hyphens), short, and carry no links.
    /// The word list of products to avoid lives outside the repository and is applied by the
    /// check script that runs before every commit.
    #[test]
    fn shipped_theme_names_are_generic() {
        for d in shipped_docs() {
            let n = &d.name;
            assert!(n.chars().count() <= 32, "{n}: short");
            assert!(n.chars().all(|c| c.is_alphanumeric() || c == ' ' || c == '-'), "{n}: plain words only");
            assert!(!n.to_lowercase().contains("http") && !n.contains('.'), "{n}: no links");
        }
    }
}
