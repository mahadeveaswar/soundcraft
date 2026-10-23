//! Context menus built from the command catalogue. An entry names a command; its label, key text (from the
//! active key set, so it follows Settings > Keys) and enabled state come from the registry, and a click runs
//! the command like its key would (through its dialog when it has one). One builder serves every surface:
//! clips, empty lanes, the ruler, the piano roll and its controller lanes, and the track header.

use crate::{SoundApp, keymap};
use egui::{Button, Ui, vec2};
use serde_json::{Value, json};
use soundcraft_model::{ClipId, TrackId};
use soundcraft_time::Samples;

/// One line of a menu.
pub enum Entry {
    /// A command under its own label.
    Cmd(&'static str),
    /// A command under this label: (id, label).
    Named(&'static str, &'static str),
    /// A command with fixed parameters (JSON) under this label: (id, params, label).
    With(&'static str, &'static str, &'static str),
    Sep,
    Sub(&'static str, &'static [Entry]),
    /// Anything the catalogue cannot express (a dialog, a swatch grid, state-dependent lines).
    Custom(fn(&mut SoundApp, &mut Ui)),
}

/// Draw `entries` into a menu.
pub fn show(app: &mut SoundApp, ui: &mut Ui, entries: &[Entry]) {
    for entry in entries {
        match entry {
            Entry::Cmd(id) => item(app, ui, id, Value::Null, None),
            Entry::Named(id, label) => item(app, ui, id, Value::Null, Some(label)),
            Entry::With(id, params, label) => {
                let value = serde_json::from_str(params).unwrap_or(Value::Null);
                item_key(app, ui, &keymap::action_key(id, params), id, value, Some(label), Ok(()));
            }
            Entry::Sep => {
                ui.separator();
            }
            Entry::Sub(label, children) => {
                ui.menu_button(*label, |ui| show(app, ui, children));
            }
            Entry::Custom(f) => f(app, ui),
        }
    }
}

/// A command as a menu line: label, key text, enabled state and its reason. `params` is null for none.
pub fn item(app: &mut SoundApp, ui: &mut Ui, id: &str, params: Value, label: Option<&str>) {
    item_key(app, ui, id, id, params, label, Ok(()));
}

/// Like [`item`] but disabled, with `why` as its hover text, when `ok` is an error (a condition only the
/// surface knows, such as "needs two selected notes").
pub fn item_if(app: &mut SoundApp, ui: &mut Ui, id: &str, params: Value, label: Option<&str>, ok: Result<(), &str>) {
    item_key(app, ui, id, id, params, label, ok.map_err(str::to_string));
}

/// Why a command cannot run now, from the registry's own check.
pub fn availability(app: &SoundApp, id: &str) -> Result<(), String> {
    let Some(c) = soundcraft_engine::find_command(id) else { return Ok(()) };
    (c.enabled)(&app.engine)?;
    if app.engine.blocked_while_recording(id) { Err("not while recording".into()) } else { Ok(()) }
}

/// The label and key text a line for `key` shows (also what tests look at).
pub fn describe(app: &SoundApp, key: &str, id: &str, label: Option<&str>, mac: bool) -> (String, String) {
    let label = label
        .map(str::to_string)
        .or_else(|| soundcraft_engine::find_command(id).map(|c| c.label.to_string()))
        .or_else(|| crate::menus::UI_COMMANDS.iter().find(|c| c.0 == id).map(|c| c.1.to_string()))
        .unwrap_or_else(|| id.to_string());
    (label, crate::menus::shortcut_text(app, key, mac))
}

fn item_key(app: &mut SoundApp, ui: &mut Ui, key: &str, id: &str, params: Value, label: Option<&str>, extra: Result<(), String>) {
    let (label, shortcut) = describe(app, key, id, label, ui.ctx().os().is_mac());
    let state = extra.and_then(|()| availability(app, id));
    let mut response = ui.add_enabled(state.is_ok(), Button::new(label).shortcut_text(shortcut));
    if let Err(why) = &state {
        response = response.on_disabled_hover_text(why);
    }
    if response.clicked() {
        if let Err(e) = crate::shortcuts::run_action(app, id, params) {
            app.ui.status = e;
        }
        ui.close();
    }
}

// ---- arrange: clips and empty lanes -----------------------------------------------------------------

const CLIP: &[Entry] = &[
    Entry::Custom(crate::edit_tools::tools_flat),
    Entry::Sep,
    Entry::Cmd("edit.cut"),
    Entry::Cmd("edit.copy"),
    Entry::Cmd("edit.paste"),
    Entry::Named("edit.delete", "Delete"),
    Entry::Sep,
    Entry::Named("edit.split_at_playhead", "Split at Playhead"),
    Entry::Named("edit.separate", "Split Range"),
    Entry::Named("clip.glue", "Glue"),
    Entry::Cmd("edit.duplicate"),
    Entry::Named("edit.repeat", "Repeat..."),
    Entry::Sep,
    Entry::Named("clip.mute", "Mute"),
    Entry::Named("clip.unmute", "Unmute"),
    Entry::Named("clip.edit_lock", "Lock / Unlock"),
    Entry::Named("clip.time_lock", "Time Lock / Unlock"),
    Entry::Sep,
    Entry::Sub(
        "Fades",
        &[
            Entry::Named("edit.fades_create", "Create Fades"),
            Entry::Cmd("edit.fade_to_start"),
            Entry::Cmd("edit.fade_to_end"),
            Entry::Named("edit.fades_delete", "Remove Fades"),
        ],
    ),
    Entry::Sub(
        "Process",
        &[
            Entry::With("audiosuite.process", r#"{"process":"normalize"}"#, "Normalize"),
            Entry::With("audiosuite.process", r#"{"process":"reverse"}"#, "Reverse"),
            Entry::With("audiosuite.process", r#"{"process":"invert"}"#, "Invert"),
        ],
    ),
    Entry::Sep,
    Entry::Named("clip.quantize_to_grid", "Quantize to Grid"),
    Entry::Named("clip.loop", "Loop..."),
    Entry::Named("clip.unloop", "Unloop"),
    Entry::Cmd("clip.group"),
    Entry::Cmd("clip.ungroup"),
    Entry::Cmd("clip.bring_to_front"),
    Entry::Cmd("clip.send_to_back"),
    Entry::Sep,
    Entry::Custom(clip_colour),
    Entry::Custom(clip_rename),
    Entry::Custom(open_editor),
    Entry::Named("clip.gain_render", "Render Clip Gain"),
    Entry::Named("edit.consolidate", "Bounce (Consolidate)"),
];

const LANE: &[Entry] = &[
    Entry::Custom(crate::edit_tools::tools_flat),
    Entry::Sep,
    Entry::Cmd("edit.paste"),
    Entry::Cmd("edit.select_all"),
    Entry::Named("edit.select_none", "Select None"),
    Entry::Sep,
    Entry::Cmd("edit.locators_to_selection"),
    Entry::Named("markers.add", "Add Marker"),
    Entry::Sep,
    Entry::Custom(add_track),
];

/// The menu of a lane: the clip's (when `clip` is under the pointer) or the empty lane's.
pub fn arrange_menu(app: &mut SoundApp, ui: &mut Ui, clip: Option<ClipId>, track: TrackId) {
    match clip {
        Some(_) => show(app, ui, CLIP),
        None => {
            ui.data_mut(|d| d.insert_temp(egui::Id::new("context_menu_lane_track"), track));
            show(app, ui, LANE);
        }
    }
}

/// A right-click on a clip selects it first (unless it is already part of the selection); on an empty
/// lane it selects the lane's track, so the menu's commands act on what was clicked.
pub fn select_target(app: &mut SoundApp, clip: Option<ClipId>, track: TrackId) {
    let ed = &app.engine.session().edit;
    let params = match clip {
        Some(c) if ed.selected_clips.contains(&c) => return,
        Some(c) => json!({"clips": [c.0]}),
        None if ed.selected_tracks.contains(&track) => return,
        None => json!({"tracks": [track.0], "exact": true}),
    };
    let _ = app.run("edit.select", params);
}

fn add_track(app: &mut SoundApp, ui: &mut Ui) {
    let track = ui.data(|d| d.get_temp::<TrackId>(egui::Id::new("context_menu_lane_track")));
    ui.menu_button("Add Track", |ui| crate::track_add_menu::add_track_items(app, ui, track));
}

fn first_selected_clip(app: &SoundApp) -> Option<ClipId> {
    app.engine.session().edit.selected_clips.first().copied()
}

fn clip_rename(app: &mut SoundApp, ui: &mut Ui) {
    if ui.add_enabled(first_selected_clip(app).is_some(), Button::new("Rename...")).clicked() {
        if let Some(c) = first_selected_clip(app) {
            let name = app.engine.session().find_clip(c).map(|(_, x)| x.name.clone()).unwrap_or_default();
            app.dialogs.open_rename_clip(c, &name);
        }
        ui.close();
    }
}

/// "Open Editor" appears for MIDI clips only.
fn open_editor(app: &mut SoundApp, ui: &mut Ui) {
    let midi = first_selected_clip(app)
        .and_then(|c| app.engine.session().find_clip(c).map(|(_, x)| matches!(x.content, soundcraft_model::ClipContent::Midi { .. })))
        .unwrap_or(false);
    if midi {
        item_key(app, ui, "window.midi_editor", "window.midi_editor", json!({"value": true}), Some("Open Editor"), Ok(()));
    }
}

fn clip_colour(app: &mut SoundApp, ui: &mut Ui) {
    let palette: Vec<[u8; 3]> =
        app.engine.track_palette().filter(|c| !c.is_empty()).map_or_else(|| soundcraft_model::TRACK_COLORS.to_vec(), <[_]>::to_vec);
    ui.menu_button("Colour", |ui| {
        if ui.button("Default").clicked() {
            let _ = app.run("clip.color", json!({"color": null}));
            ui.close();
        }
        ui.horizontal_wrapped(|ui| {
            ui.set_max_width(168.0);
            for [r, g, b] in palette.into_iter().take(16) {
                let swatch = Button::new("").fill(egui::Color32::from_rgb(r, g, b)).min_size(vec2(18.0, 18.0));
                if ui.add(swatch).on_hover_text(format!("#{r:02x}{g:02x}{b:02x}")).clicked() {
                    let _ = app.run("clip.color", json!({"color": [r, g, b]}));
                    ui.close();
                }
            }
        });
    });
}

// ---- ruler ----------------------------------------------------------------------------------------------

/// The ruler's menu for a right-click at `at` (`marker_ruler`: the marker ruler's lane, for Add Marker).
pub fn ruler_menu(app: &mut SoundApp, ui: &mut Ui, at: Samples, marker_ruler: Option<u8>) {
    let marker = match marker_ruler {
        Some(lane) => json!({"at": at, "ruler": lane}),
        None => json!({"at": at}),
    };
    item(app, ui, "markers.add", marker, Some("Add Marker"));
    item(app, ui, "transport.locate", json!({"at": at}), Some("Locate Here"));
    item(app, ui, "transport.set_left_locator", json!({"at": at}), Some("Set Left Locator Here"));
    item(app, ui, "transport.set_right_locator", json!({"at": at}), Some("Set Right Locator Here"));
    ui.separator();
    item(app, ui, "options.loop_playback", Value::Null, Some("Loop On/Off"));
    item(app, ui, "view.zoom_to_selection", Value::Null, None);
    item(app, ui, "view.zoom_fit", Value::Null, Some("Zoom to Fit Session"));
}

// ---- track header ---------------------------------------------------------------------------------------------

/// The lines at the top of a track's menu: mute, solo, record enable and selecting its clips. `selection` are
/// the tracks the menu acts on.
pub fn track_top(app: &mut SoundApp, ui: &mut Ui, track: TrackId, selection: &[u64]) {
    let tracks = json!({"tracks": selection});
    item(app, ui, "mix.mute", tracks.clone(), None);
    item(app, ui, "mix.solo", tracks.clone(), None);
    item(app, ui, "mix.record_arm", tracks, None);
    let clips: Vec<u64> = app.engine.session().track(track).map(|t| t.clips().iter().map(|c| c.id.0).collect()).unwrap_or_default();
    if ui.add_enabled(!clips.is_empty(), Button::new("Select All Clips on Track")).clicked() {
        let _ = app.run("edit.select", json!({"clips": clips}));
        ui.close();
    }
    ui.separator();
}

// ---- piano roll ------------------------------------------------------------------------------------------------

/// What a right-click in the piano roll landed on: the note under the pointer and the tick (relative to the clip).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RollTarget {
    pub note: Option<usize>,
    pub tick: i64,
}

/// The piano roll's menu. Note lines act on the selected notes (a right-click on an unselected note selects it
/// first); on empty roll only the lines that need no note stay enabled.
pub fn roll_menu(app: &mut SoundApp, ui: &mut Ui, clip: ClipId, target: RollTarget) {
    let selected: Vec<usize> = app.midi.selected.clone();
    let count = app.engine.session().find_clip(clip).map_or(0, |(_, c)| match &c.content {
        soundcraft_model::ClipContent::Midi { sequence } => sequence.notes.len(),
        _ => 0,
    });
    let none: Result<(), &str> = if selected.is_empty() { Err("select a note first") } else { Ok(()) };
    let two: Result<(), &str> = if selected.len() >= 2 { Ok(()) } else { Err("select two or more notes") };
    // With no note selected the note operations apply to the whole clip.
    let scope = |extra: Value| {
        let mut p = json!({"clips": [clip.0]});
        if !selected.is_empty() {
            p["indices"] = json!(selected);
        }
        if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) {
            o.extend(e.clone());
        }
        p
    };
    let by_index = |extra: Value| {
        let mut p = json!({"clip": clip.0, "indices": selected});
        if let (Some(o), Some(e)) = (p.as_object_mut(), extra.as_object()) {
            o.extend(e.clone());
        }
        p
    };
    // Cut, Copy and Paste send the note commands but show the Edit keys (the roll owns them while the pointer is over it).
    let paste_ok: Result<(), String> = if crate::midi_clipboard::has_notes(app) { Ok(()) } else { Err("copy notes first".into()) };
    let sel_ok: Result<(), String> = none.map_err(str::to_string);
    action_item(app, ui, "edit.cut", "Cut", sel_ok.clone(), |a| {
        crate::midi_clipboard::copy_selection(a, clip, true);
    });
    action_item(app, ui, "edit.copy", "Copy", sel_ok.clone(), |a| {
        crate::midi_clipboard::copy_selection(a, clip, false);
    });
    action_item(app, ui, "edit.paste", "Paste", paste_ok, |a| {
        crate::midi_clipboard::paste(a, clip, Some(target.tick));
    });
    action_item(app, ui, "edit.duplicate", "Duplicate", sel_ok, |a| {
        crate::midi_clipboard::duplicate(a, clip);
    });
    ui.separator();
    item_if(app, ui, "midi.note_delete", by_index(json!({})), Some("Delete"), none);
    ui.separator();
    let split = match target.note {
        Some(_) => Ok(()),
        None => Err("right-click a note to split it"),
    };
    item_if(app, ui, "midi.note_split", json!({"clip": clip.0, "index": target.note.unwrap_or(0), "tick": target.tick}), Some("Split"), split);
    item_if(app, ui, "midi.notes_glue", by_index(json!({})), Some("Glue"), two);
    item_if(app, ui, "midi.notes_mute", by_index(json!({})), Some("Mute / Unmute"), none);
    ui.separator();
    item(app, ui, "event.quantize", scope(json!({})), Some("Quantize"));
    ui.menu_button("Transpose", |ui| {
        for (label, semitones) in [("Up a Semitone", 1), ("Down a Semitone", -1), ("Up an Octave", 12), ("Down an Octave", -12)] {
            item(app, ui, "event.transpose", scope(json!({"semitones": semitones})), Some(label));
        }
    });
    ui.menu_button("Velocity", |ui| {
        for (label, add) in [("Louder", 10), ("Softer", -10)] {
            item(app, ui, "event.change_velocity", scope(json!({"add": add})), Some(label));
        }
    });
    ui.separator();
    if ui.add_enabled(count > 0, Button::new("Select All")).clicked() {
        app.midi.selected = (0..count).collect();
        ui.close();
    }
}

/// A line that runs `act` (UI-side work around a command) and shows the key of `key`; disabled with the reason
/// when `ok` is an error.
fn action_item(app: &mut SoundApp, ui: &mut Ui, key: &str, label: &str, ok: Result<(), String>, act: impl FnOnce(&mut SoundApp)) {
    let (label, shortcut) = describe(app, key, key, Some(label), ui.ctx().os().is_mac());
    let mut response = ui.add_enabled(ok.is_ok(), Button::new(label).shortcut_text(shortcut));
    if let Err(why) = &ok {
        response = response.on_disabled_hover_text(why);
    }
    if response.clicked() {
        act(app);
        ui.close();
    }
}

/// A controller lane's menu: copy, cut and paste its events, clear them, or remove the lane. Copy and Cut take the
/// events under the selected notes, or the whole lane when no note is selected.
pub fn controller_lane_menu(app: &mut SoundApp, ui: &mut Ui, clip: ClipId, lane: &str, cc: i64, index: usize) {
    if let Some(kind) = app.midi.lanes.get(index).map(|l| l.lane.kind()) {
        action_item(app, ui, "edit.cut", "Cut Events", Ok(()), |a| {
            crate::midi_clipboard::copy_lane(a, clip, kind, true);
        });
        action_item(app, ui, "edit.copy", "Copy Events", Ok(()), |a| {
            crate::midi_clipboard::copy_lane(a, clip, kind, false);
        });
        let paste_ok = if crate::midi_clipboard::has_notes(app) { Ok(()) } else { Err("copy notes or events first".to_string()) };
        action_item(app, ui, "edit.paste", "Paste", paste_ok, |a| {
            crate::midi_clipboard::paste(a, clip, None);
        });
        ui.separator();
    }
    item(app, ui, "midi.ctrl_clear", json!({"clip": clip.0, "lane": lane, "cc": cc}), Some("Clear Lane"));
    item(app, ui, "ui.midi_lane_remove", json!({"index": index}), Some("Remove Lane"));
}
