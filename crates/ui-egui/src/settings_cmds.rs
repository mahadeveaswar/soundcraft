//! Settings window commands: `window.settings` (with its `window.ui_customization` alias) and
//! `ui.prefs` (interface scale, reduce motion, tooltips). The preferences are saved in `UiState`
//! (one source of truth) and mirrored into `ExtraState`, which applies them to the egui context.

use crate::SoundApp;
use serde_json::{Value, json};

/// Lowest and highest interface scale.
pub const SCALE_RANGE: (f32, f32) = (0.75, 1.5);

/// The pages of the Settings window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    General,
    Appearance,
    Tracks,
}

impl SettingsTab {
    pub const ALL: [Self; 3] = [Self::General, Self::Appearance, Self::Tracks];

    pub const fn id(self) -> &'static str {
        match self {
            Self::General => "general",
            Self::Appearance => "appearance",
            Self::Tracks => "tracks",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::General => "General",
            Self::Appearance => "Appearance",
            Self::Tracks => "Track colours",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.id().eq_ignore_ascii_case(s.trim()))
    }
}

/// Handle `ui.prefs`, `window.settings` and `window.ui_customization`.
pub fn run(app: &mut SoundApp, id: &str, p: &Value) -> Result<Value, String> {
    match id {
        "ui.prefs" => prefs(app, p),
        "window.settings" => window(app, p, None),
        _ => window(app, p, Some(SettingsTab::Appearance)),
    }
}

/// `window.settings {tab?: "general"|"appearance"|"tracks", value?}`: with a `tab` the window opens
/// on that page; otherwise `value` (or a toggle) shows or hides it, and a closed window opens on
/// General. `window.ui_customization` is the same command with Appearance as its default page.
fn window(app: &mut SoundApp, p: &Value, default_tab: Option<SettingsTab>) -> Result<Value, String> {
    let tab = match p.get("tab") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let s = v.as_str().ok_or("`tab` must be a string")?;
            Some(SettingsTab::parse(s).ok_or_else(|| format!("unknown tab `{s}` (general, appearance or tracks)"))?)
        }
    };
    let value = match p.get("value") {
        None | Some(Value::Null) => None,
        Some(v) => Some(v.as_bool().ok_or("`value` must be true or false")?),
    };
    let was_open = app.extra.show_settings;
    let open = match (tab, value) {
        (Some(_), None) => true,
        (_, v) => v.unwrap_or(!was_open),
    };
    if open {
        if let Some(t) = tab.or(default_tab) {
            app.extra.settings_tab = t;
        } else if !was_open {
            app.extra.settings_tab = SettingsTab::General;
        }
    }
    app.extra.show_settings = open;
    Ok(json!({"value": open, "tab": app.extra.settings_tab.id()}))
}

fn bool_param(p: &Value, key: &str) -> Result<Option<bool>, String> {
    match p.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(v) => v.as_bool().map(Some).ok_or_else(|| format!("`{key}` must be true or false")),
    }
}

/// `ui.prefs {ui_scale?, reduce_motion?, tooltips?, reset?}`: set interface preferences (saved in
/// the UI preferences, no undo) and report all three. `ui_scale` is clamped to 0.75..=1.5.
fn prefs(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    let scale = match p.get("ui_scale") {
        None | Some(Value::Null) => None,
        Some(v) => {
            let s = v.as_f64().filter(|s| s.is_finite()).ok_or("`ui_scale` must be a number")?;
            Some((s as f32).clamp(SCALE_RANGE.0, SCALE_RANGE.1))
        }
    };
    let motion = bool_param(p, "reduce_motion")?;
    let tips = bool_param(p, "tooltips")?;
    if bool_param(p, "reset")? == Some(true) {
        set(app, 1.0, false, true);
    }
    set(app, scale.unwrap_or(app.ui.ui_scale), motion.unwrap_or(app.ui.reduce_motion), tips.unwrap_or(app.ui.tooltips));
    Ok(json!({"ui_scale": app.ui.ui_scale, "reduce_motion": app.ui.reduce_motion, "tooltips": app.ui.tooltips}))
}

fn set(app: &mut SoundApp, scale: f32, reduce_motion: bool, tooltips: bool) {
    app.ui.ui_scale = scale;
    app.ui.reduce_motion = reduce_motion;
    app.ui.tooltips = tooltips;
    app.extra.ui_scale = scale;
    app.extra.reduce_motion = reduce_motion;
    app.extra.tooltips = tooltips;
}

/// Once, on the first frame: take the saved preferences into the live state.
pub fn sync_on_load(app: &mut SoundApp) {
    if app.extra.prefs_synced {
        return;
    }
    app.extra.prefs_synced = true;
    let ui = (app.ui.ui_scale.clamp(SCALE_RANGE.0, SCALE_RANGE.1), app.ui.reduce_motion, app.ui.tooltips);
    set(app, ui.0, ui.1, ui.2);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Services, UiState};

    fn app() -> SoundApp {
        SoundApp::new(soundcraft_engine::Engine::default(), None, Services::default())
    }

    #[test]
    fn prefs_set_report_and_reset() {
        let mut app = app();
        let r = app.run("ui.prefs", json!({})).unwrap();
        assert_eq!((r["ui_scale"].as_f64(), r["reduce_motion"].as_bool(), r["tooltips"].as_bool()), (Some(1.0), Some(false), Some(true)));
        let r = app.run("ui.prefs", json!({"ui_scale": 1.25, "reduce_motion": true, "tooltips": false})).unwrap();
        assert_eq!(r["ui_scale"].as_f64(), Some(1.25));
        assert_eq!((app.ui.ui_scale, app.ui.reduce_motion, app.ui.tooltips), (1.25, true, false));
        assert_eq!((app.extra.ui_scale, app.extra.reduce_motion, app.extra.tooltips), (1.25, true, false));
        app.run("ui.prefs", json!({"reset": true})).unwrap();
        assert_eq!((app.ui.ui_scale, app.ui.reduce_motion, app.ui.tooltips), (1.0, false, true));
    }

    #[test]
    fn scale_is_clamped_and_bad_values_change_nothing() {
        let mut app = app();
        app.run("ui.prefs", json!({"ui_scale": 9.0})).unwrap();
        assert_eq!(app.ui.ui_scale, 1.5);
        app.run("ui.prefs", json!({"ui_scale": 0.1})).unwrap();
        assert_eq!(app.ui.ui_scale, 0.75);
        assert!(app.run("ui.prefs", json!({"ui_scale": "big", "reduce_motion": true})).is_err());
        assert!(app.run("ui.prefs", json!({"reduce_motion": 3})).is_err());
        assert!(!app.ui.reduce_motion);
    }

    #[test]
    fn window_settings_tabs_toggle_and_errors() {
        let mut app = app();
        let r = app.run("window.settings", json!({})).unwrap();
        assert_eq!((r["value"].as_bool(), r["tab"].as_str()), (Some(true), Some("general")));
        let r = app.run("window.settings", json!({"tab": "tracks"})).unwrap();
        assert_eq!(r["tab"], "tracks");
        assert!(app.extra.show_settings);
        assert!(app.run("window.settings", json!({"tab": "audio"})).is_err());
        assert_eq!(app.extra.settings_tab, SettingsTab::Tracks);
        let r = app.run("window.settings", json!({})).unwrap();
        assert_eq!(r["value"], false);
        assert!(!app.extra.show_settings);
        // Reopening without a page starts on General.
        app.run("window.settings", json!({"value": true})).unwrap();
        assert_eq!(app.extra.settings_tab, SettingsTab::General);
    }

    #[test]
    fn ui_customization_is_an_alias_for_the_appearance_page() {
        let mut app = app();
        let r = app.run("window.ui_customization", json!({"value": true})).unwrap();
        assert_eq!((r["value"].as_bool(), r["tab"].as_str()), (Some(true), Some("appearance")));
        let r = app.run("window.ui_customization", json!({"tab": "tracks"})).unwrap();
        assert_eq!(r["tab"], "tracks");
        assert!(app.run("window.ui_customization", json!({"tab": "x"})).is_err());
        app.run("window.ui_customization", json!({"value": false})).unwrap();
        assert!(!app.extra.show_settings);
    }

    #[test]
    fn saved_preferences_reach_the_live_state_on_the_first_frame() {
        let mut app = app();
        app.ui = serde_json::from_value::<UiState>(json!({"ui_scale": 1.25, "reduce_motion": true, "tooltips": false})).unwrap();
        sync_on_load(&mut app);
        assert_eq!((app.extra.ui_scale, app.extra.reduce_motion, app.extra.tooltips), (1.25, true, false));
    }

    #[test]
    fn old_preferences_without_the_new_fields_load_unchanged() {
        let ui: UiState = serde_json::from_value(json!({"show_transport": true, "ui_scale": 1.1})).unwrap();
        assert!(ui.show_transport);
        assert_eq!((ui.ui_scale, ui.reduce_motion, ui.tooltips), (1.1, false, true));
        let none: UiState = serde_json::from_value(json!({})).unwrap();
        assert!(!none.reduce_motion && none.tooltips);
    }
}
