//! Key-command presets. The engine registry's `shortcut` column is the **Default** preset and stays as it
//! is; this UI-layer overlay adds the **Alternative** preset (single-letter commands, number-row tools,
//! Delete, Insert, arrows) and the user's own per-command overrides, which work on top of either preset.
//!
//! An *action* is a command id, optionally with fixed parameters (`edit.tool#{"tool":"zoom"}`), so one
//! command can have several keys. The saved form is [`KeymapPrefs`] (in `ui.json`, scriptable through
//! `ui.prefs`). egui reports numpad keys as their main-row twins (digits, `/`, `.`, Enter), so the
//! preset's transport keys use plain keys instead; see `docs/ui-parity.md`.

use crate::shortcuts::{mods_match, parse};
use egui::{Key, Modifiers};
use serde_json::Value;
use std::collections::BTreeMap;

/// Which built-in key set is active.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Preset {
    #[default]
    Default,
    Alternative,
}

impl Preset {
    pub const ALL: [Self; 2] = [Self::Default, Self::Alternative];

    pub const fn id(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Alternative => "alternative",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Default => "Default",
            Self::Alternative => "Alternative",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.id().eq_ignore_ascii_case(s.trim()))
    }
}

/// One row of the Alternative preset: a command (with optional fixed parameters) and its keys.
struct Entry {
    id: &'static str,
    /// JSON parameters, or "" for none.
    params: &'static str,
    /// Row label when the command's own label would be ambiguous (parameterised actions).
    label: &'static str,
    chords: &'static [&'static str],
}

const fn k(id: &'static str, chords: &'static [&'static str]) -> Entry {
    Entry { id, params: "", label: "", chords }
}

const fn kp(id: &'static str, params: &'static str, label: &'static str, chords: &'static [&'static str]) -> Entry {
    Entry { id, params, label, chords }
}

/// The Alternative preset. Commands not listed keep their Default key unless another row takes it.
static ALTERNATIVE: &[Entry] = &[
    k("transport.toggle", &["Space"]),
    k("transport.play", &["Enter"]),
    k("transport.stop", &["0"]),
    k("transport.record", &["Cmd+Space", "F12"]),
    k("options.loop_playback", &["/"]),
    k("transport.rtz", &[".", "Home"]),
    k("transport.go_to_end", &["End"]),
    k("transport.locate_left", &["Shift+1"]),
    k("transport.locate_right", &["Shift+2"]),
    k("edit.locators_to_selection", &["P"]),
    k("markers.add", &["Insert"]),
    k("edit.undo", &["Cmd+Z"]),
    k("edit.redo", &["Cmd+Shift+Z", "Cmd+Y"]),
    k("edit.cut", &["Cmd+X"]),
    k("edit.copy", &["Cmd+C"]),
    k("edit.paste", &["Cmd+V"]),
    k("edit.delete", &["Delete", "Backspace"]),
    k("edit.select_all", &["Cmd+A"]),
    k("edit.select_none", &["Cmd+Shift+A"]),
    k("edit.split_at_playhead", &["Alt+X"]),
    k("edit.separate", &["Shift+X"]),
    k("clip.glue", &["Alt+G"]),
    k("edit.duplicate", &["Cmd+D"]),
    k("edit.repeat", &["Cmd+K"]),
    k("clip.mute", &["Shift+M"]),
    k("clip.unmute", &["Shift+U"]),
    k("mix.mute", &["M"]),
    k("mix.solo", &["S"]),
    k("mix.record_arm", &["R"]),
    k("clip.edit_lock", &["Cmd+Shift+L"]),
    k("clip.quantize_to_grid", &["Q"]),
    k("event.quantize", &["Shift+Q"]),
    k("view.zoom_in", &["H", "Cmd+]"]),
    k("view.zoom_out", &["G", "Cmd+["]),
    k("view.zoom_fit", &["Shift+F"]),
    k("window.midi_editor", &["Cmd+E"]),
    k("window.mixer_panel", &["F3"]),
    k("window.inspector", &["Alt+I"]),
    k("options.click", &["C"]),
    kp("edit.tool", r#"{"tool":"grabber"}"#, "Edit Tool: Object Select", &["1"]),
    kp("edit.tool", r#"{"tool":"selector"}"#, "Edit Tool: Range Select", &["2"]),
    kp("edit.tool", r#"{"tool":"split"}"#, "Edit Tool: Split", &["3"]),
    kp("edit.tool", r#"{"tool":"glue"}"#, "Edit Tool: Glue", &["4"]),
    kp("edit.tool", r#"{"tool":"erase"}"#, "Edit Tool: Erase", &["5"]),
    kp("edit.tool", r#"{"tool":"zoom"}"#, "Edit Tool: Zoom", &["6"]),
    kp("edit.tool", r#"{"tool":"mute"}"#, "Edit Tool: Mute", &["7"]),
    kp("edit.tool", r#"{"tool":"pencil"}"#, "Edit Tool: Draw", &["8"]),
    kp("edit.tool", r#"{"tool":"scrubber"}"#, "Edit Tool: Scrub", &["9"]),
    kp("edit.nudge", r#"{"direction":-1}"#, "Nudge Earlier", &["Cmd+Left"]),
    kp("edit.nudge", r#"{"direction":1}"#, "Nudge Later", &["Cmd+Right"]),
    k("clip.select_prev", &["Left"]),
    k("clip.select_next", &["Right"]),
    k("track.select_prev", &["Up"]),
    k("track.select_next", &["Down"]),
    k("session.new", &["Cmd+N"]),
    k("session.open", &["Cmd+O"]),
    k("session.save", &["Cmd+S"]),
    k("session.save_as", &["Cmd+Shift+S"]),
];

/// The key of an action in prefs and tables: the command id, plus `#` and the JSON parameters.
pub fn action_key(id: &str, params: &str) -> String {
    if params.is_empty() { id.to_string() } else { format!("{id}#{params}") }
}

/// A command id and its fixed parameters, from an [`action_key`].
pub fn split_key(key: &str) -> (&str, Value) {
    match key.split_once('#') {
        Some((id, params)) => (id, serde_json::from_str(params).unwrap_or(Value::Null)),
        None => (key, Value::Null),
    }
}

/// A row for the Keys page and the shortcut list.
#[derive(Debug, Clone)]
pub struct ActionInfo {
    pub key: String,
    pub label: String,
    pub path: String,
}

/// Every action that can carry a key: registry commands, UI commands with a menu place or key, and the
/// preset's parameterised actions.
pub fn actions() -> Vec<ActionInfo> {
    let mut out: Vec<ActionInfo> = Vec::new();
    for c in soundcraft_engine::command_specs() {
        out.push(ActionInfo { key: c.id.into(), label: c.label.into(), path: c.menu.join(" > ") });
    }
    for c in crate::menus::UI_COMMANDS.iter().filter(|c| !c.2.is_empty() || c.3.is_some()) {
        if !out.iter().any(|a| a.key == c.0) {
            out.push(ActionInfo { key: c.0.into(), label: c.1.into(), path: c.2.to_string() });
        }
    }
    for e in ALTERNATIVE.iter().filter(|e| !e.params.is_empty()) {
        out.push(ActionInfo { key: action_key(e.id, e.params), label: e.label.into(), path: String::new() });
    }
    out
}

/// Whether `id` names a registry or UI command.
pub fn is_known(id: &str) -> bool {
    soundcraft_engine::find_command(id).is_some() || crate::menus::UI_COMMANDS.iter().any(|c| c.0 == id)
}

/// A chord as comparable flags and key.
fn norm(chord: &str) -> Option<(bool, bool, bool, bool, Key)> {
    let (m, key) = parse(chord)?;
    Some((m.command, m.ctrl, m.alt, m.shift, key))
}

/// The saved key preferences.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct KeymapPrefs {
    pub preset: Preset,
    /// Action key to its chords (an empty list unbinds the action). Wins over the preset.
    pub overrides: BTreeMap<String, Vec<String>>,
}

impl KeymapPrefs {
    /// Every action's effective chords, in lookup order: the preset's own rows first, then the other
    /// commands' Default keys that no preset row has taken. Overrides replace an action's chords.
    pub fn effective(&self) -> Vec<(String, Vec<String>)> {
        let mut rows: Vec<(String, Vec<String>)> = Vec::new();
        let mut taken: Vec<(bool, bool, bool, bool, Key)> = Vec::new();
        if self.preset == Preset::Alternative {
            for e in ALTERNATIVE {
                taken.extend(e.chords.iter().filter_map(|c| norm(c)));
                rows.push((action_key(e.id, e.params), e.chords.iter().map(|c| (*c).to_string()).collect()));
            }
        }
        let in_preset = |key: &str, rows: &[(String, Vec<String>)]| rows.iter().any(|(k, _)| k == key);
        let defaults =
            soundcraft_engine::command_specs().iter().map(|c| (c.id, c.shortcut)).chain(crate::menus::UI_COMMANDS.iter().map(|c| (c.0, c.3)));
        for (id, sc) in defaults {
            let Some(chord) = sc.filter(|c| parse(c).is_some()) else { continue };
            if in_preset(id, &rows) && self.preset == Preset::Alternative {
                continue;
            }
            if self.preset == Preset::Alternative && norm(chord).is_some_and(|n| taken.contains(&n)) {
                continue;
            }
            match rows.iter_mut().find(|(k, _)| k == id) {
                Some((_, v)) => v.push(chord.to_string()),
                None => rows.push((id.to_string(), vec![chord.to_string()])),
            }
        }
        for (key, chords) in &self.overrides {
            match rows.iter_mut().find(|(k, _)| k == key) {
                Some((_, v)) => v.clone_from(chords),
                None => rows.push((key.clone(), chords.clone())),
            }
        }
        rows
    }

    /// The chords an action answers to.
    pub fn bindings(&self, key: &str) -> Vec<String> {
        self.effective().into_iter().find(|(k, _)| k == key).map(|(_, v)| v).unwrap_or_default()
    }

    /// Whether the user rebound this action (the Default key dispatch then skips its old key).
    pub fn overridden(&self, key: &str) -> bool {
        self.overrides.contains_key(key)
    }

    fn hit(chords: &[String], key: Key, mods: Modifiers, mac: bool) -> bool {
        chords.iter().any(|c| parse(c).is_some_and(|(m, k)| k == key && mods_match(m, mods, mac)))
    }

    /// The action the user bound to a pressed key, if any.
    pub fn lookup_override(&self, key: Key, mods: Modifiers, mac: bool) -> Option<(String, Value)> {
        let (a, _) = self.overrides.iter().find(|(_, v)| Self::hit(v, key, mods, mac))?;
        let (id, params) = split_key(a);
        Some((id.to_string(), params))
    }

    /// The action a pressed key triggers under the Alternative preset or an override.
    pub fn lookup(&self, key: Key, mods: Modifiers, mac: bool) -> Option<(String, Value)> {
        if let Some(found) = self.lookup_override(key, mods, mac) {
            return Some(found);
        }
        if self.preset != Preset::Alternative {
            return None;
        }
        let (a, _) = self.effective().into_iter().find(|(a, v)| !self.overridden(a) && Self::hit(v, key, mods, mac))?;
        let (id, params) = split_key(&a);
        Some((id.to_string(), params))
    }

    /// Other actions holding `chord` (compared as parsed chords, not text).
    pub fn holders(&self, chord: &str, except: &str) -> Vec<String> {
        let Some(n) = norm(chord) else { return Vec::new() };
        self.effective().into_iter().filter(|(k, v)| k != except && v.iter().any(|c| norm(c) == Some(n))).map(|(k, _)| k).collect()
    }

    /// Give an action exactly these chords. Chords that do not parse are an error; chords another action
    /// holds are an error naming it unless `reassign`, which takes them from it.
    pub fn assign(&mut self, key: &str, chords: &[String], reassign: bool) -> Result<(), String> {
        let (id, _) = split_key(key);
        if !is_known(id) {
            return Err(format!("unknown command `{id}`"));
        }
        for c in chords {
            if norm(c).is_none() {
                return Err(format!("`{c}` is not a key chord (examples: `Cmd+Shift+K`, `Alt+G`, `F3`, `Delete`)"));
            }
        }
        let mut stolen: Vec<(String, String)> = Vec::new();
        for c in chords {
            for holder in self.holders(c, key) {
                stolen.push((holder, c.clone()));
            }
        }
        if !reassign && let Some((holder, chord)) = stolen.first() {
            return Err(format!("`{chord}` is already used by `{holder}`; pass keymap_reassign to take it"));
        }
        for (holder, chord) in stolen {
            let Some(n) = norm(&chord) else { continue };
            let left: Vec<String> = self.bindings(&holder).into_iter().filter(|c| norm(c) != Some(n)).collect();
            self.overrides.insert(holder, left);
        }
        let mut seen = Vec::new();
        let unique: Vec<String> = chords
            .iter()
            .filter(|c| {
                norm(c).is_some_and(|n| {
                    !seen.contains(&n) && {
                        seen.push(n);
                        true
                    }
                })
            })
            .cloned()
            .collect();
        self.overrides.insert(key.to_string(), unique);
        Ok(())
    }

    /// Drop the override of one action, restoring the preset's keys.
    pub fn reset(&mut self, key: &str) {
        self.overrides.remove(key);
    }
}

/// A key and modifiers as a chord string, or None for keys that cannot be written and parsed back.
pub fn chord_for(key: Key, m: Modifiers, mac: bool) -> Option<String> {
    let name = match key {
        Key::OpenBracket => "[".to_string(),
        Key::CloseBracket => "]".to_string(),
        Key::Plus => "Plus".to_string(),
        other => other.name().to_string(),
    };
    let mut parts: Vec<&str> = Vec::new();
    if m.command {
        parts.push("Cmd");
    }
    if mac && m.ctrl {
        parts.push("Ctrl");
    }
    if m.alt {
        parts.push("Alt");
    }
    if m.shift {
        parts.push("Shift");
    }
    parts.push(&name);
    let chord = parts.join("+");
    (parse(&chord).is_some_and(|(_, k)| k == key)).then_some(chord)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alt() -> KeymapPrefs {
        KeymapPrefs { preset: Preset::Alternative, ..Default::default() }
    }

    #[test]
    fn every_preset_row_names_a_known_command_and_parseable_chords() {
        for e in ALTERNATIVE {
            assert!(is_known(e.id), "{}", e.id);
            for c in e.chords {
                assert!(parse(c).is_some(), "{} {c}", e.id);
            }
            if !e.params.is_empty() {
                assert!(serde_json::from_str::<Value>(e.params).is_ok(), "{}", e.params);
            }
        }
    }

    #[test]
    fn no_chord_is_held_by_two_actions_in_either_preset() {
        let allowed = ["transport.play", "transport.stop", "transport.toggle", "window.mix", "window.edit", "window.toggle_mix_edit"];
        for km in [KeymapPrefs::default(), alt()] {
            let mut seen: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for (key, chords) in km.effective() {
                for c in chords {
                    let n = norm(&c).map(|n| format!("{n:?}"));
                    seen.entry(n.unwrap_or(c)).or_default().push(key.clone());
                }
            }
            for (chord, keys) in seen {
                assert!(keys.len() < 2 || keys.iter().all(|k| allowed.contains(&k.as_str())), "{:?}: {chord} is held by {keys:?}", km.preset);
            }
        }
    }

    #[test]
    fn alternative_takes_over_the_keys_it_lists_and_frees_the_old_ones() {
        let km = alt();
        assert_eq!(km.bindings("mix.mute"), vec!["M".to_string()]);
        let (id, _) = km.lookup(Key::M, Modifiers::NONE, false).unwrap();
        assert_eq!(id, "mix.mute");
        // Default's Shift+M (mute) now belongs to the clip mute.
        let (id, _) = km.lookup(Key::M, Modifiers::SHIFT, false).unwrap();
        assert_eq!(id, "clip.mute");
        // A command the preset does not list keeps its Default key.
        assert_eq!(km.bindings("edit.trim_to_selection"), vec!["Cmd+T".to_string()]);
        // The default preset has no lookups of its own.
        assert!(KeymapPrefs::default().lookup(Key::M, Modifiers::NONE, false).is_none());
    }

    #[test]
    fn parameterised_actions_carry_their_parameters() {
        let (id, params) = alt().lookup(Key::Num6, Modifiers::NONE, false).unwrap();
        assert_eq!((id.as_str(), params["tool"].as_str()), ("edit.tool", Some("zoom")));
        let pc_ctrl = Modifiers { ctrl: true, command: true, ..Modifiers::NONE };
        let (id, params) = alt().lookup(Key::ArrowRight, pc_ctrl, false).unwrap();
        assert_eq!((id.as_str(), params["direction"].as_i64()), ("edit.nudge", Some(1)));
    }

    #[test]
    fn assigning_reports_conflicts_and_reassign_takes_the_chord() {
        let mut km = alt();
        let err = km.assign("clip.glue", &["M".to_string()], false).unwrap_err();
        assert!(err.contains("mix.mute"), "{err}");
        assert!(km.overrides.is_empty(), "a refused assignment changes nothing");
        km.assign("clip.glue", &["M".to_string()], true).unwrap();
        assert_eq!(km.bindings("clip.glue"), vec!["M".to_string()]);
        assert!(km.bindings("mix.mute").is_empty(), "the old holder lost it");
        assert_eq!(km.lookup(Key::M, Modifiers::NONE, false).unwrap().0, "clip.glue");
        km.reset("clip.glue");
        assert_eq!(km.bindings("clip.glue"), vec!["Alt+G".to_string()]);
    }

    #[test]
    fn bad_chords_and_unknown_commands_are_rejected() {
        let mut km = KeymapPrefs::default();
        assert!(km.assign("clip.glue", &["Hyper+Q".to_string()], false).is_err());
        assert!(km.assign("clip.glue", &["Cmd+".to_string()], false).is_err());
        assert!(km.assign("no.such.command", &["Cmd+Q".to_string()], false).is_err());
        assert!(km.overrides.is_empty());
        km.assign("clip.glue", &[], false).unwrap();
        assert!(km.bindings("clip.glue").is_empty(), "an empty list unbinds");
    }

    #[test]
    fn captured_chords_parse_back() {
        let pc_ctrl = Modifiers { ctrl: true, command: true, shift: true, ..Modifiers::NONE };
        assert_eq!(chord_for(Key::K, pc_ctrl, false).as_deref(), Some("Cmd+Shift+K"));
        assert_eq!(chord_for(Key::F3, Modifiers::NONE, false).as_deref(), Some("F3"));
        assert_eq!(chord_for(Key::Delete, Modifiers::ALT, false).as_deref(), Some("Alt+Delete"));
        assert_eq!(chord_for(Key::OpenBracket, Modifiers::NONE, false).as_deref(), Some("["));
        assert_eq!(chord_for(Key::ArrowLeft, Modifiers::NONE, false).as_deref(), Some("Left"));
        for key in
            [Key::A, Key::Num0, Key::Space, Key::Enter, Key::Insert, Key::Backspace, Key::Comma, Key::Period, Key::Slash, Key::Minus, Key::Equals]
        {
            let chord = chord_for(key, Modifiers::NONE, false).unwrap_or_default();
            assert!(parse(&chord).is_some_and(|(_, k)| k == key), "{key:?} -> {chord}");
        }
    }
}
