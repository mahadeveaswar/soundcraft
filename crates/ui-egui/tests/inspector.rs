//! The Inspector dock panel: closed by default, opened through `ui.dock` and `window.inspector`,
//! Edit window only, old saved preferences still load, and it shows the selected track's slots.
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use serde_json::json;
use soundcraft_ui_egui::{MainWindow, Services, SoundApp, UiState, docking::Panel, menus, panel_menu};

fn harness() -> Harness<'static, SoundApp> {
    let app = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1500.0, 1000.0)).build_ui_state(
        |ui, app: &mut SoundApp| {
            app.logic(&ui.ctx().clone());
            app.ui(ui);
        },
        app,
    );
    h.run_steps(5);
    h
}

fn count(h: &Harness<'_, SoundApp>, label: &str) -> usize {
    h.query_all_by_label(label).count()
}

#[test]
fn closed_by_default_opens_left_and_closes_through_ui_dock() {
    let mut h = harness();
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector));
    assert!(!h.state().ui.show_inspector);
    assert!(!panel_menu::is_visible(h.state(), Panel::Inspector));
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    let ui = &h.state().ui;
    assert!(ui.docking.layout.contains(&Panel::Inspector) && ui.show_inspector);
    assert!(ui.docking.validate().is_ok());
    h.state_mut().run("ui.dock", json!({"operation": "close", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector) && !h.state().ui.show_inspector);
    // The panel remembers where it was: reopening restores a valid layout.
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    assert!(h.state().ui.docking.validate().is_ok());
    // Reset brings the default layout back, with the Inspector closed.
    h.state_mut().run("ui.dock.reset", json!({})).unwrap();
    h.run_steps(4);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector));
}

#[test]
fn the_window_command_and_menu_state_follow_the_panel() {
    let mut h = harness();
    assert!(!menus::item_state(h.state(), "", Some("window.inspector")).1);
    h.state_mut().run("window.inspector", json!({})).unwrap();
    h.run_steps(4);
    assert!(h.state().ui.docking.layout.contains(&Panel::Inspector));
    assert!(menus::item_state(h.state(), "", Some("window.inspector")).1);
    h.state_mut().run("window.inspector", json!({})).unwrap();
    h.run_steps(4);
    assert!(!h.state().ui.docking.layout.contains(&Panel::Inspector));
}

#[test]
fn the_mix_window_rejects_the_inspector() {
    let mut h = harness();
    h.state_mut().ui.window = MainWindow::Mix;
    h.run_steps(3);
    assert!(h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).is_err());
    assert!(!panel_menu::panels_for(false).contains(&Panel::Inspector));
}

#[test]
fn preferences_saved_before_the_inspector_still_load() {
    let mut value = serde_json::to_value(UiState::default()).unwrap();
    let object = value.as_object_mut().unwrap();
    assert!(object.remove("show_inspector").is_some(), "the flag is saved");
    let old: UiState = serde_json::from_value(value).unwrap();
    assert!(!old.show_inspector);
    assert!(old.docking.validate().is_ok());
    // A saved layout that predates the panel has no "inspector" anywhere and stays valid.
    let text = serde_json::to_string(&old.docking).unwrap();
    assert!(!text.contains("inspector"));
}

#[test]
fn it_shows_the_selected_tracks_inserts_and_sends() {
    let mut h = harness();
    h.state_mut().run("edit.select", json!({"tracks": ["Keys"]})).unwrap();
    h.run_steps(3);
    let (inserts, sends) = (count(&h, "Insert A"), count(&h, "Send A"));
    let all_before = count(&h, "Insert J");
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(4);
    assert_eq!(count(&h, "Insert A"), inserts + 1, "the Inspector adds one Insert A slot");
    assert_eq!(count(&h, "Send A"), sends + 1, "the Inspector adds one Send A slot");
    // Filled slots plus one empty by default; "All" shows every slot.
    assert_eq!(count(&h, "Insert J"), all_before, "only filled slots and one empty show");
    h.get_by_label("Inspector: Insert options").click();
    h.run_steps(3);
    h.get_by_label("Show all ten slots").click();
    h.run_steps(3);
    assert_eq!(count(&h, "Insert J"), all_before + 1);
    // With nothing selected it shows no slots.
    h.state_mut().run("edit.select", json!({"tracks": []})).unwrap();
    h.run_steps(4);
    assert_eq!(count(&h, "Insert A"), inserts);
    assert_eq!(count(&h, "Insert J"), all_before);
}

fn open_inspector(track: &str) -> Harness<'static, SoundApp> {
    let mut h = harness();
    h.state_mut().run("edit.select", json!({"tracks": [track]})).unwrap();
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(5);
    h
}

fn track<'a>(h: &'a Harness<'_, SoundApp>, name: &str) -> &'a soundcraft_model::Track {
    h.state().engine.session().tracks.iter().find(|t| t.name == name).expect("track")
}

#[test]
fn rename_commits_on_enter_and_cancels_on_escape() {
    let mut h = open_inspector("Keys");
    h.get_by_label("Inspector: Track name").click();
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.event(egui::Event::Text("Piano".into()));
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.state().engine.session().tracks.iter().any(|t| t.name == "Piano"));
    assert!(!h.state().engine.session().tracks.iter().any(|t| t.name == "Keys"));
    // Escape leaves the name alone.
    h.get_by_label("Inspector: Track name").click();
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(1);
    h.event(egui::Event::Text("Nope".into()));
    h.run_steps(1);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().engine.session().tracks.iter().any(|t| t.name == "Piano"));
    assert!(!h.state().engine.session().tracks.iter().any(|t| t.name == "Nope"));
    assert!(h.query_by_label("Inspector: Track name").is_some(), "back to the plain name");
}

#[test]
fn a_taken_name_shows_the_engines_error() {
    let mut h = open_inspector("Keys");
    h.get_by_label("Inspector: Track name").click();
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("Kick".into()));
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(h.query_by_label_contains("already exists").is_some());
    assert_eq!(track(&h, "Keys").name, "Keys");
}

/// Insert slots drawn in the Inspector (the left of the window), not the Mix strips.
fn inspector_insert_slots(h: &Harness<'_, SoundApp>) -> usize {
    ["Insert A", "Insert B", "Insert C"].iter().map(|l| h.query_all_by_label(l).filter(|n| n.rect().min.x < 330.0).count()).sum()
}

#[test]
fn collapse_toggles_and_persists_through_ui_json() {
    let mut h = open_inspector("Keys");
    assert!(h.state().ui.inspector_collapsed.contains("notes"), "notes start collapsed");
    assert!(!h.state().ui.inspector_collapsed.contains("inserts"));
    h.get_by_label("Inspector section Inserts").click();
    h.run_steps(3);
    assert!(h.state().ui.inspector_collapsed.contains("inserts"));
    assert_eq!(inspector_insert_slots(&h), 0, "the body is hidden");
    let text = serde_json::to_string(&h.state().ui).unwrap();
    let back: UiState = serde_json::from_str(&text).unwrap();
    assert!(back.inspector_collapsed.contains("inserts"));
    h.get_by_label("Inspector section Inserts").click();
    h.run_steps(3);
    assert!(!h.state().ui.inspector_collapsed.contains("inserts"));
    assert!(inspector_insert_slots(&h) > 0);
}

#[test]
fn an_old_ui_json_without_the_new_fields_loads() {
    let mut value = serde_json::to_value(UiState::default()).unwrap();
    let object = value.as_object_mut().unwrap();
    assert!(object.remove("inspector_collapsed").is_some());
    assert!(object.remove("recent_track_colors").is_some());
    let old: UiState = serde_json::from_value(value).unwrap();
    assert!(old.recent_track_colors.is_empty());
    assert!(old.inspector_collapsed.contains("notes"));
}

#[test]
fn add_eq_runs_mix_insert_and_remove_clears_the_slot() {
    let mut h = open_inspector("Keys");
    let before = track(&h, "Keys").mixer.inserts.iter().flatten().count();
    h.get_by_label("Inspector: Add effect menu").click();
    h.run_steps(3);
    h.get_by_label("Add EQ").click();
    h.run_steps(3);
    let t = track(&h, "Keys");
    assert_eq!(t.mixer.inserts.iter().flatten().count(), before + 1);
    let slot = t.mixer.inserts.iter().position(|i| i.as_ref().is_some_and(|i| i.plugin == "eq_7band")).expect("EQ slot");
    let letter = (b'A' + slot as u8) as char;
    h.get_by_label(&format!("Inspector: Insert {letter} bypass")).click();
    h.run_steps(3);
    assert!(track(&h, "Keys").mixer.inserts[slot].as_ref().unwrap().bypass);
    h.get_by_label(&format!("Inspector: Insert {letter} remove")).click();
    h.run_steps(3);
    assert!(track(&h, "Keys").mixer.inserts[slot].is_none());
}

#[test]
fn choosing_an_effect_from_the_empty_slot_menu_runs_mix_insert() {
    let mut h = open_inspector("Keys");
    let before = track(&h, "Keys").mixer.inserts.iter().flatten().count();
    let slot = h.query_all_by_label("Insert A").chain(h.query_all_by_label("Insert B")).find(|n| n.rect().min.x < 330.0).expect("inspector slot");
    slot.click();
    h.run_steps(3);
    h.event(egui::Event::Text("reverb".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(track(&h, "Keys").mixer.inserts.iter().flatten().count(), before + 1);
}

#[test]
fn mute_solo_and_the_colour_popup_run_commands() {
    let mut h = open_inspector("Keys");
    h.get_by_label("Inspector: Mute").click();
    h.run_steps(3);
    assert!(track(&h, "Keys").mixer.mute);
    h.get_by_label("Inspector: Solo").click();
    h.run_steps(3);
    assert!(track(&h, "Keys").mixer.solo);
    // The swatch opens the popup; a pick recolours the track.
    let swatch = h.query_all_by_label("Track colour").find(|n| n.rect().min.x < 330.0).expect("inspector swatch");
    swatch.click();
    h.run_steps(3);
    let [r, g, b] = soundcraft_model::TRACK_PALETTE_EXT[40];
    h.get_by_label(&format!("Colour #{r:02X}{g:02X}{b:02X}")).click();
    h.run_steps(3);
    assert_eq!(track(&h, "Keys").color, [r, g, b]);
    assert_eq!(h.state().ui.recent_track_colors.first().copied(), Some([r, g, b]));
}

#[test]
fn apply_to_clips_clears_own_colours_in_one_undo_step() {
    let mut h = open_inspector("Keys");
    let clip = h
        .state()
        .engine
        .session()
        .tracks
        .iter()
        .find(|t| t.name == "Kick")
        .and_then(|t| t.playlist())
        .and_then(|p| p.clips.first())
        .map(|c| c.id.0)
        .expect("clip");
    h.state_mut().run("edit.select", json!({"tracks": ["Kick"]})).unwrap();
    h.state_mut().run("clip.color", json!({"clips": [clip], "color": [1, 2, 3]})).unwrap();
    h.run_steps(4);
    let own = |h: &Harness<'_, SoundApp>| track(h, "Kick").playlists.iter().flat_map(|p| p.clips.iter()).filter(|c| c.color.is_some()).count();
    assert_eq!(own(&h), 1);
    let swatch = h.query_all_by_label("Track colour").find(|n| n.rect().min.x < 330.0).expect("inspector swatch");
    swatch.click();
    h.run_steps(3);
    h.get_by_label("Apply to this track's clips").click();
    h.run_steps(3);
    assert_eq!(own(&h), 0);
    h.state_mut().run("edit.undo", json!({})).unwrap();
    assert_eq!(own(&h), 1, "one undo step restores the clip colour");
}

#[test]
fn master_has_no_input_row_and_an_instrument_has_its_row() {
    let mut h = harness();
    let master = h.state().engine.session().tracks.iter().find(|t| t.kind == soundcraft_model::TrackKind::Master).map(|t| t.id.0).expect("master");
    h.state_mut().run("edit.select", json!({"tracks": [master]})).unwrap();
    h.state_mut().run("ui.dock", json!({"operation": "open", "panel": "inspector"})).unwrap();
    h.run_steps(5);
    assert!(h.query_by_label("Inspector: Input").is_none());
    assert!(h.query_by_label("Inspector: Output").is_some());
    assert!(h.query_by_label("Inspector: Solo").is_none());
    let h = open_inspector("Keys");
    assert!(h.query_by_label("Inspector: Input").is_some());
    assert!(h.query_by_label("Inspector: Open instrument editor").is_some(), "instrument tracks get the Instrument row");
}

#[test]
fn a_send_row_mutes_sets_its_level_and_removes() {
    let mut h = open_inspector("Keys");
    assert!(track(&h, "Keys").mixer.sends[0].is_some(), "the demo Keys track has Send A");
    h.get_by_label("Inspector: Send A mute").click();
    h.run_steps(3);
    assert!(track(&h, "Keys").mixer.sends[0].as_ref().unwrap().mute);
    h.get_by_label("Inspector: Send A level").click();
    h.run_steps(2);
    h.get_by_label("Inspector: Send A remove").click();
    h.run_steps(3);
    assert!(track(&h, "Keys").mixer.sends[0].is_none());
    assert!(h.query_by_label("Inspector: Send A remove").is_none());
}

#[test]
fn the_header_shows_the_type_and_an_empty_instrument_says_so() {
    let h = open_inspector("Verb");
    assert!(h.query_by_label("Inspector: Record enable").is_none(), "a bus has no record button");
    assert!(h.query_by_label("Inspector: Input monitor").is_none());
    let h = open_inspector("Keys");
    assert!(h.query_by_label("Inspector: Record enable").is_some());
    assert!(h.query_by_label("Inspector: Choose instrument").is_some());
}
