//! Session templates: `template.save` stores the open session (tracks, mixer, plugin slots and
//! their state, no clips) in the per-user templates folder, `template.new_session` starts an
//! unsaved session from one, `template.instrument_rack` adds a row of named instrument tracks.
//!
//! Folders: user templates live in `<per-user app folder>/templates`; templates shipped with the
//! app live in a `templates` folder beside the executable (or in the working directory). Every
//! command also takes `dir` to use one explicit folder instead (agents, tests).

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::{ChannelFormat, Insert, Session, TrackKind};
use std::path::{Path, PathBuf};

const EXT: &str = soundcraft_model::SESSION_EXTENSION;
const MAX_NAME: usize = 64;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "template.dir", "Templates Folder", [], None, "{} -> {user, shipped}: the per-user and the shipped templates folders", always, |_, _| {
            Ok(json!({"user": user_dir(), "shipped": shipped_dir()}))
        }),
        cmd!(query "template.list", "List Templates", [], None, "{dir?} -> {templates: [{name, path, source: user|shipped}]}", always, list),
        cmd!(view "template.save", "Save As Template...", [], None, "{name, replace?: false, dir?} -> {path}: stores the session without clips or media", always, save),
        cmd!(noundo "template.new_session", "New from Template...", [], None, "{name | path, session_name?, dir?}: replaces the session with the template, unsaved (Save asks for a path)", always, new_session),
        cmd!(
            "template.instrument_rack",
            "Instrument Rack...",
            [],
            None,
            "{count?: 10 (1..128), names?: [..], prefix?: 'Instrument', instrument?: plugin id} -> {tracks}: instrument tracks, each named",
            always,
            rack
        ),
    ]
}

/// A template name as a file stem: letters, digits, space, `-`, `_`, `.`; anything else dropped.
pub(crate) fn sanitize(name: &str) -> Option<String> {
    let s: String = name.chars().filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.')).take(MAX_NAME).collect();
    let s = s.trim().trim_matches('.').trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

fn user_dir() -> Option<PathBuf> {
    let env = |k: &str| std::env::var_os(k).filter(|v| !v.is_empty()).map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        env("HOME").map(|h| h.join("Library").join("Application Support").join("SoundCraft"))
    } else if cfg!(windows) {
        env("APPDATA").map(|a| a.join("SoundCraft"))
    } else {
        env("XDG_CONFIG_HOME").or_else(|| env("HOME").map(|h| h.join(".config"))).map(|c| c.join("soundcraft"))
    };
    base.map(|b| b.join("templates"))
}

fn shipped_dir() -> Option<PathBuf> {
    let beside = std::env::current_exe().ok().and_then(|e| e.parent().map(|p| p.join("templates")));
    beside.filter(|p| crate::io::dir_exists(p)).or_else(|| Some(PathBuf::from("templates")).filter(|p| crate::io::dir_exists(p)))
}

fn explicit_dir(p: &Value) -> Option<PathBuf> {
    str_param(p, "dir").map(PathBuf::from)
}

fn templates_in(dir: &Path, source: &str, out: &mut Vec<Value>) {
    for (name, path) in crate::io::files_with_extension(dir, EXT) {
        out.push(json!({"name": name, "path": path, "source": source}));
    }
}

fn list(_: &mut Engine, p: &Value) -> Result<Value> {
    let mut out = Vec::new();
    if let Some(d) = explicit_dir(p) {
        templates_in(&d, "user", &mut out);
    } else {
        if let Some(d) = user_dir() {
            templates_in(&d, "user", &mut out);
        }
        if let Some(d) = shipped_dir() {
            templates_in(&d, "shipped", &mut out);
        }
    }
    Ok(json!({"templates": out}))
}

fn save(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "template.save";
    let name = str_param(p, "name").and_then(sanitize).ok_or_else(|| bad(id, "`name` required (letters, digits, space, - _ .)"))?;
    let dir = explicit_dir(p).or_else(user_dir).ok_or_else(|| bad(id, "no templates folder on this platform: pass `dir`"))?;
    let file = dir.join(&name).with_extension(EXT);
    crate::file_access::check_write(&file).map_err(EngineError::Io)?;
    if crate::io::path_exists(&file) && !bool_or(p, "replace", false) {
        return Err(bad(id, format!("{} already exists: pass `replace: true` to replace it", file.display())));
    }
    if !cfg!(target_arch = "wasm32") {
        crate::io::ensure_dir(&dir)?;
    }
    super::file::write_template(e, &file)?;
    Ok(json!({"path": file}))
}

fn new_session(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "template.new_session";
    let file = if let Some(path) = str_param(p, "path") {
        let file = PathBuf::from(path);
        crate::file_access::check_read(&file).map_err(EngineError::Io)?;
        file
    } else {
        let name = str_param(p, "name").and_then(sanitize).ok_or_else(|| bad(id, "`name` or `path` required"))?;
        let stem = format!("{name}.{EXT}");
        let dirs: Vec<PathBuf> = match explicit_dir(p) {
            Some(d) => vec![d],
            None => user_dir().into_iter().chain(shipped_dir()).collect(),
        };
        dirs.into_iter().map(|d| d.join(&stem)).find(|f| crate::io::file_found(f)).ok_or_else(|| bad(id, format!("no template named `{name}`")))?
    };
    let text = crate::io::read_text(&file)?;
    let mut s = Session::from_json(&text).map_err(|err| EngineError::Io(format!("{}: {err}", file.display())))?;
    // A template carries no media, whatever its file holds.
    for t in &mut s.tracks {
        for pl in &mut t.playlists {
            pl.clips.clear();
        }
    }
    s.sources.clear();
    s.videos.clear();
    s.name = match str_param(p, "session_name").map(str::trim).filter(|n| !n.is_empty()) {
        Some(n) => n.chars().take(soundcraft_model::MAX_NAME_CHARS).collect(),
        None => "Untitled".to_string(),
    };
    e.replace_session(s);
    e.path = None;
    Ok(json!({"name": e.session().name, "tracks": e.session().tracks.len()}))
}

fn rack(e: &mut Engine, p: &Value) -> Result<Value> {
    let id = "template.instrument_rack";
    let count = i64_or(p, "count", 10).clamp(1, 128) as usize;
    let instrument = str_param(p, "instrument").unwrap_or("subtractive_synth");
    if super::clap::plugin_info(instrument).is_none_or(|i| !i.is_instrument) {
        return Err(bad(id, format!("`{instrument}` is not a known instrument")));
    }
    let names: Vec<String> = match p.get("names") {
        None | Some(Value::Null) => Vec::new(),
        Some(v) => v
            .as_array()
            .ok_or_else(|| bad(id, "`names` must be an array of strings"))?
            .iter()
            .map(|n| n.as_str().map(str::to_string).ok_or_else(|| bad(id, "`names` must be an array of strings")))
            .collect::<Result<_>>()?,
    };
    let prefix = str_param(p, "prefix").map(str::trim).filter(|s| !s.is_empty()).unwrap_or("Instrument").to_string();
    let s = e.session_mut();
    let mut ids = Vec::new();
    for i in 0..count {
        let name = names.get(i).map(|n| n.trim()).filter(|n| !n.is_empty()).map(str::to_string).unwrap_or_else(|| format!("{prefix} {}", i + 1));
        let t = s.add_track(TrackKind::Instrument, ChannelFormat::Stereo, Some(&name));
        if let Some(tr) = s.track_mut(t) {
            tr.instrument = Some(Insert::new(instrument));
        }
        ids.push(t);
    }
    Ok(json!({"tracks": ids}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("sc-template-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn sanitises_names() {
        assert_eq!(sanitize("  My/Rack:1*  ").as_deref(), Some("MyRack1"));
        assert_eq!(sanitize("../..").as_deref(), None);
        assert_eq!(sanitize("???"), None);
        assert_eq!(sanitize(&"a".repeat(200)).map(|s| s.len()), Some(MAX_NAME));
    }

    #[test]
    fn rack_makes_ten_named_instrument_tracks() {
        let mut e = Engine::default();
        let r = e.execute("template.instrument_rack", &json!({"names": ["Piano", "Strings"], "prefix": "Slot"})).unwrap();
        assert_eq!(r["tracks"].as_array().map(Vec::len), Some(10));
        let t = &e.session().tracks;
        assert_eq!(t.len(), 10);
        assert!(t.iter().all(|t| t.kind == TrackKind::Instrument && t.instrument.is_some()));
        assert_eq!(t[0].name, "Piano");
        assert_eq!(t[1].name, "Strings");
        assert_eq!(t[2].name, "Slot 3");
        e.execute("edit.undo", &json!({})).unwrap();
        assert!(e.session().tracks.is_empty(), "one undo step");
        assert!(e.execute("template.instrument_rack", &json!({"instrument": "nope"})).is_err());
        assert!(e.execute("template.instrument_rack", &json!({"names": 3})).is_err());
    }

    #[test]
    fn save_list_new_round_trip_keeps_tracks_names_and_plugin_state() {
        let dir = tmp("rt");
        let d = dir.to_string_lossy().to_string();
        let mut e = crate::demo::demo_engine();
        e.execute("template.instrument_rack", &json!({"count": 2, "names": ["Piano", "Pad"]})).unwrap();
        let piano = e.session().tracks.iter().find(|t| t.name == "Piano").map(|t| t.id.0).unwrap();
        e.execute("mix.insert", &json!({"track": piano, "plugin": "gain", "slot": 3})).unwrap();
        let blob = soundcraft_model::b64::encode(b"\x01\x02vst3 component+controller");
        e.execute("mix.insert_state", &json!({"track": piano, "slot": "instrument", "state": blob})).unwrap();
        let before: Vec<(String, Option<String>, Vec<Option<String>>)> = e
            .session()
            .tracks
            .iter()
            .map(|t| {
                (
                    t.name.clone(),
                    t.instrument.as_ref().map(|i| i.plugin.clone()),
                    t.mixer.inserts.iter().map(|i| i.as_ref().map(|i| i.plugin.clone())).collect(),
                )
            })
            .collect();
        e.path = Some("old.scraft".into());

        let r = e.execute("template.save", &json!({"name": "My: Rack", "dir": d})).unwrap();
        assert!(r["path"].as_str().unwrap().ends_with("My Rack.scraft"));
        assert!(e.execute("template.save", &json!({"name": "My Rack", "dir": d})).is_err(), "no silent replace");
        assert!(e.execute("template.save", &json!({"name": "My Rack", "dir": d, "replace": true})).is_ok());
        assert!(e.execute("template.save", &json!({"name": "///", "dir": d})).is_err());

        let l = e.execute("template.list", &json!({"dir": d})).unwrap();
        assert_eq!(l["templates"][0]["name"], "My Rack");

        let mut f = Engine::default();
        f.execute("template.new_session", &json!({"name": "My Rack", "dir": d, "session_name": "Song"})).unwrap();
        assert_eq!(f.session().name, "Song");
        assert!(f.path.is_none());
        assert!(f.session().tracks.iter().all(|t| t.playlists.iter().all(|p| p.clips.is_empty())));
        assert!(f.session().sources.is_empty());
        let after: Vec<(String, Option<String>, Vec<Option<String>>)> = f
            .session()
            .tracks
            .iter()
            .map(|t| {
                (
                    t.name.clone(),
                    t.instrument.as_ref().map(|i| i.plugin.clone()),
                    t.mixer.inserts.iter().map(|i| i.as_ref().map(|i| i.plugin.clone())).collect(),
                )
            })
            .collect();
        assert_eq!(before, after);
        // The opaque plugin state (what a hosted VST3 stores) survives the session JSON.
        let st = f.session().tracks.iter().find(|t| t.name == "Piano").and_then(|t| t.instrument.as_ref()).and_then(|i| i.state.clone());
        assert_eq!(st.as_deref(), Some(blob.as_str()));
        assert!(f.execute("template.new_session", &json!({"name": "Missing", "dir": d})).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_session_path_outside_the_read_root_is_refused() {
        let dir = tmp("root");
        let inside = dir.join("in");
        let outside = dir.join("out");
        std::fs::create_dir_all(&inside).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let mut src = Engine::default();
        let path = outside.join("t.scraft");
        crate::file_access::unconfined(|| src.execute("template.save", &json!({"name": "T", "dir": outside.to_string_lossy()}))).unwrap();
        assert!(path.exists());
        let roots = crate::file_access::AutomationRoots::new(Some(&inside), None).unwrap().unwrap();
        let mut e = Engine::default();
        let r = crate::file_access::confine(Some(&roots), || e.execute("template.new_session", &json!({"path": path.to_string_lossy()})));
        assert!(r.is_err(), "a path outside the read root is refused");
        let ok = crate::file_access::unconfined(|| e.execute("template.new_session", &json!({"path": path.to_string_lossy()})));
        assert!(ok.is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn new_session_is_journaled_and_not_undoable_like_session_new() {
        let spec = |id: &str| crate::cmd::command_specs().iter().find(|c| c.id == id).map(|c| (c.journal, c.undoable, c.view));
        assert_eq!(spec("template.new_session"), spec("session.new"));
    }
}
