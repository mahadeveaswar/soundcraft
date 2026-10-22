//! Menu bar presentation layer. `menus::tree()` stays the catalog-ordered source of truth (parity
//! counts, command lookup and the tests walk it); `display_tree` only regroups its nodes under
//! task-oriented roots and renames some labels. Every node keeps its `path`, which is the command
//! key, so dispatch, shortcuts and enabled/checked state work unchanged.

use crate::SoundApp;
use crate::menus::{self, MenuNode};
use serde_json::{Value, json};

/// How the menu bar is arranged (`ui.menu_layout`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MenuLayout {
    /// Catalog order: File, Edit, View, Track, Clip, Event, AudioSuite, Options, Setup, Window.
    Classic,
    /// Grouped by task: File, Edit, Project, Audio, MIDI, Media, Transport, View, Setup, Window, Help.
    #[default]
    Standard,
}

impl MenuLayout {
    pub const ALL: [Self; 2] = [Self::Standard, Self::Classic];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Standard => "Standard",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Classic => "classic",
            Self::Standard => "standard",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.id().eq_ignore_ascii_case(s.trim()))
    }
}

/// Order of the Standard roots. A catalog root that no rule empties and that is not listed here
/// keeps its own place just before Window.
const STANDARD_ROOTS: &[&str] = &["File", "Edit", "Project", "Audio", "MIDI", "Media", "Transport", "View", "Setup", "Window", "Help"];

/// (source node path, display root, new label). The node and everything below it move.
const MOVES: &[(&str, &str, Option<&str>)] = &[
    // Project: tracks, session setup, tempo and time.
    ("Setup > Session", "Project", Some("Project Setup...")),
    ("Event > Tempo Operations", "Project", None),
    ("Event > Time Operations", "Project", None),
    ("Event > Renumber Bars...", "Project", None),
    ("Event > Add Key Change...", "Project", None),
    // Audio: clips and offline processing.
    ("Event > Beat Detective", "Audio", None),
    ("Event > Identify Beat...", "Audio", None),
    // MIDI.
    ("Event > MIDI Operations", "MIDI", None),
    ("Event > MIDI Track Offsets", "MIDI", None),
    ("Event > MIDI Real-Time Properties", "MIDI", None),
    ("Event > Remove Duplicate Notes", "MIDI", None),
    ("Event > Extract Chords from Selection", "MIDI", None),
    ("Event > All MIDI Notes Off", "MIDI", None),
    ("Setup > MIDI", "MIDI", Some("MIDI Setup")),
    ("Options > MIDI Thru", "MIDI", None),
    ("Options > MIDI Live Mode", "MIDI", None),
    ("Options > Mirrored MIDI Editing", "MIDI", None),
    // Transport: loop, record and punch options.
    ("Event > Retrospective Record", "Transport", None),
    ("Options > Destructive Record", "Transport", None),
    ("Options > Loop Record", "Transport", None),
    ("Options > QuickPunch", "Transport", None),
    ("Options > TrackPunch", "Transport", None),
    ("Options > DestructivePunch", "Transport", None),
    ("Options > Pre/Post-Roll", "Transport", None),
    ("Options > Loop Playback", "Transport", None),
    ("Options > Dynamic Transport", "Transport", None),
    ("Options > Transport Online", "Transport", None),
    ("Options > Click", "Transport", None),
    ("Setup > Click/Countoff...", "Transport", None),
    // Whole roots (after the single items above have left them).
    ("Track", "Project", Some("Tracks")),
    ("Clip", "Audio", Some("Clips")),
    ("AudioSuite", "Audio", Some("Offline Processing")),
    ("Options", "Setup", Some("Editing Options")),
    ("Event", "Project", Some("Events")),
];

/// (source node path, display root, new label). The node is cloned; the original stays put, so the
/// same command is reachable from both places. These are the only intentional duplicates.
pub const COPIES: &[(&str, &str, Option<&str>)] = &[
    ("File > Import", "Media", None),
    ("Window > Clip List", "Media", None),
    ("Window > Memory Locations", "Project", None),
    ("Window > MIDI Editor", "MIDI", None),
    ("Window > MIDI Event List", "MIDI", None),
    ("Window > MIDI Keyboard", "MIDI", None),
    ("Window > Score Editor", "MIDI", None),
    ("Window > Transport", "Transport", None),
    ("Window > Big Counter", "Transport", None),
];

/// Leaves the Help menu adds (path, label). Their commands are UI commands with a `Help > ` path.
pub const HELP_ITEMS: &[(&str, &str)] = &[
    ("Help > About SoundCraft", "About SoundCraft"),
    ("Help > Keyboard Shortcuts...", "Keyboard Shortcuts..."),
    ("Help > Session Audio Health", "Session Audio Health"),
];

fn find<'a>(nodes: &'a [MenuNode], path: &str) -> Option<&'a MenuNode> {
    nodes.iter().find_map(|n| {
        if n.path == path {
            Some(n)
        } else if path.starts_with(&format!("{} > ", n.path)) {
            find(&n.children, path)
        } else {
            None
        }
    })
}

/// Remove the node at `path`, and any parent that is left without children.
fn take(nodes: &mut Vec<MenuNode>, path: &str) -> Option<MenuNode> {
    let idx = nodes.iter().position(|n| n.path == path || path.starts_with(&format!("{} > ", n.path)))?;
    if nodes.get(idx)?.path == path {
        return Some(nodes.remove(idx));
    }
    let node = nodes.get_mut(idx)?;
    let taken = take(&mut node.children, path);
    if taken.is_some() && node.children.is_empty() {
        nodes.remove(idx);
    }
    taken
}

fn root_mut<'a>(roots: &'a mut Vec<MenuNode>, name: &str) -> &'a mut MenuNode {
    let idx = roots.iter().position(|n| n.path == name).unwrap_or_else(|| {
        roots.push(MenuNode { label: name.into(), path: name.into(), source_root: String::new(), children: Vec::new() });
        roots.len() - 1
    });
    &mut roots[idx]
}

/// The menu bar's roots for `layout`. Classic is exactly `menus::tree()`.
pub fn display_tree(layout: MenuLayout) -> Vec<MenuNode> {
    let mut roots = menus::tree();
    if layout == MenuLayout::Classic {
        return roots;
    }
    // Copies first, so a later move of the original cannot take the source away.
    let mut copies: Vec<(&str, MenuNode)> = Vec::new();
    for (src, to, label) in COPIES {
        if let Some(mut n) = find(&roots, src).cloned_node() {
            if let Some(l) = label {
                n.label = (*l).into();
            }
            copies.push((to, n));
        }
    }
    let mut moved: Vec<(&str, MenuNode)> = Vec::new();
    for (src, to, label) in MOVES {
        if let Some(mut n) = take(&mut roots, src) {
            if let Some(l) = label {
                n.label = (*l).into();
            }
            moved.push((to, n));
        }
    }
    // Moved nodes first (they are the menu's main content), then copies.
    for (to, n) in moved.into_iter().chain(copies) {
        root_mut(&mut roots, to).children.push(n);
    }
    let help = root_mut(&mut roots, "Help");
    for (path, label) in HELP_ITEMS {
        help.children.push(MenuNode { label: (*label).into(), path: (*path).into(), source_root: String::new(), children: Vec::new() });
    }
    // Order: the Standard roots, with unknown leftovers just before Window.
    let mut out: Vec<MenuNode> = Vec::new();
    let mut rest = roots;
    for name in STANDARD_ROOTS {
        if let Some(i) = rest.iter().position(|n| n.path == *name) {
            out.push(rest.remove(i));
        }
    }
    let at = out.iter().position(|n| n.path == "Window").unwrap_or(out.len());
    for (k, n) in rest.into_iter().enumerate() {
        out.insert(at + k, n);
    }
    out
}

trait ClonedNode {
    fn cloned_node(self) -> Option<MenuNode>;
}

impl ClonedNode for Option<&MenuNode> {
    fn cloned_node(self) -> Option<MenuNode> {
        self.map(clone_node)
    }
}

fn clone_node(n: &MenuNode) -> MenuNode {
    MenuNode {
        label: n.label.clone(),
        path: n.path.clone(),
        source_root: n.source_root.clone(),
        children: n.children.iter().map(clone_node).collect(),
    }
}

/// `ui.menu_layout {layout?}`: report or set the menu bar arrangement (no undo, not in the session).
pub(crate) fn command(app: &mut SoundApp, p: &Value) -> Result<Value, String> {
    if let Some(name) = p.get("layout").or_else(|| p.get("value")).and_then(Value::as_str) {
        let Some(layout) = MenuLayout::parse(name) else {
            return Err(format!("unknown menu layout `{name}` (standard or classic)"));
        };
        app.ui.menu_layout = layout;
    }
    Ok(json!({"layout": app.ui.menu_layout.id()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn leaf_counts(nodes: &[MenuNode], out: &mut BTreeMap<String, usize>) {
        for n in nodes {
            if n.children.is_empty() {
                *out.entry(n.path.clone()).or_default() += 1;
            }
            leaf_counts(&n.children, out);
        }
    }

    fn counts(nodes: &[MenuNode]) -> BTreeMap<String, usize> {
        let mut m = BTreeMap::new();
        leaf_counts(nodes, &mut m);
        m
    }

    fn copy_leaf_counts() -> BTreeMap<String, usize> {
        let src = menus::tree();
        let mut m = BTreeMap::new();
        for (path, ..) in COPIES {
            if let Some(n) = find(&src, path) {
                if n.children.is_empty() {
                    *m.entry(n.path.clone()).or_default() += 1;
                } else {
                    leaf_counts(&n.children, &mut m);
                }
            }
        }
        m
    }

    fn same_labels(a: &[MenuNode], b: &[MenuNode]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.label == y.label && x.path == y.path && same_labels(&x.children, &y.children))
    }

    #[test]
    fn classic_is_the_catalog_tree() {
        assert!(same_labels(&display_tree(MenuLayout::Classic), &menus::tree()));
    }

    #[test]
    fn standard_keeps_every_leaf_path_and_only_adds_whitelisted_copies_and_help() {
        let mut expected = counts(&menus::tree());
        for (p, n) in copy_leaf_counts() {
            *expected.entry(p).or_default() += n;
        }
        for (p, _) in HELP_ITEMS {
            *expected.entry((*p).into()).or_default() += 1;
        }
        assert_eq!(counts(&display_tree(MenuLayout::Standard)), expected);
    }

    #[test]
    fn standard_roots_are_in_order_and_non_empty() {
        let roots = display_tree(MenuLayout::Standard);
        let labels: Vec<&str> = roots.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, STANDARD_ROOTS);
        assert!(roots.iter().all(|n| !n.children.is_empty()));
    }

    #[test]
    fn every_standard_leaf_resolves_like_in_the_catalog_tree() {
        let extra = menus::ui_aliases();
        let mut classic = BTreeMap::new();
        fn walk(nodes: &[MenuNode], extra: &[(&str, &str)], out: &mut BTreeMap<String, Option<String>>) {
            for n in nodes {
                if n.children.is_empty() {
                    out.insert(n.path.clone(), menus::command_for_path(&n.path, extra));
                }
                walk(&n.children, extra, out);
            }
        }
        walk(&menus::tree(), &extra, &mut classic);
        let mut standard = BTreeMap::new();
        walk(&display_tree(MenuLayout::Standard), &extra, &mut standard);
        for (path, cmd) in &standard {
            match classic.get(path) {
                Some(c) => assert_eq!(c, cmd, "{path}"),
                None => assert!(cmd.is_some(), "{path} has no command"),
            }
        }
    }

    #[test]
    fn standard_places_items_in_the_expected_roots() {
        let roots = display_tree(MenuLayout::Standard);
        let root = |name: &str| roots.iter().find(|n| n.label == name).unwrap();
        assert!(find(&root("Project").children, "Track").is_some_and(|n| n.label == "Tracks"));
        assert!(find(&root("Audio").children, "Clip").is_some_and(|n| n.label == "Clips"));
        assert!(find(&root("Audio").children, "AudioSuite").is_some_and(|n| n.label == "Offline Processing"));
        assert!(find(&root("MIDI").children, "Event > MIDI Operations").is_some());
        assert!(find(&root("Transport").children, "Options > Loop Playback").is_some());
        assert!(find(&root("Media").children, "File > Import").is_some());
        // Window stays whole.
        assert!(find(&root("Window").children, "Window > MIDI Editor").is_some());
        // The Clip menu keeps its internal order (new clip first).
        let clips = find(&root("Audio").children, "Clip").unwrap();
        assert_eq!(clips.children[0].label, "New MIDI Clip");
    }

    #[test]
    fn template_items_follow_save_as_template_in_both_layouts() {
        let extra = menus::ui_aliases();
        for layout in [MenuLayout::Standard, MenuLayout::Classic] {
            let roots = display_tree(layout);
            let file = roots.iter().find(|n| n.label == "File").unwrap();
            let labels: Vec<&str> = file.children.iter().map(|n| n.label.as_str()).collect();
            let at = labels.iter().position(|l| *l == "Save As Template...").unwrap();
            assert_eq!(&labels[at + 1..at + 3], ["New from Template...", "Instrument Rack..."], "{layout:?}");
        }
        assert_eq!(menus::command_for_path("File > New from Template...", &extra).as_deref(), Some("template.new_session"));
        assert_eq!(menus::command_for_path("File > Instrument Rack...", &extra).as_deref(), Some("template.instrument_rack"));
    }

    #[test]
    fn help_commands_open_their_window_and_the_layout_command_round_trips() {
        let mut app = SoundApp::new(soundcraft_engine::Engine::default(), None, crate::Services::default());
        assert_eq!(app.ui.menu_layout, MenuLayout::Standard);
        let r = app.run("ui.menu_layout", json!({"layout": "classic"})).unwrap();
        assert_eq!(r["layout"], "classic");
        assert_eq!(app.ui.menu_layout, MenuLayout::Classic);
        assert!(app.run("ui.menu_layout", json!({"layout": "nope"})).is_err());
        menus::invoke_menu(&mut app, "window.about", "Help > About SoundCraft");
        assert!(app.ui.show_about);
        menus::invoke_menu(&mut app, "window.about", "Help > About SoundCraft");
        assert!(app.ui.show_about, "Help never closes the window");
    }

    #[test]
    fn ui_state_without_the_field_defaults_to_standard() {
        let ui: crate::UiState = serde_json::from_str("{}").unwrap();
        assert_eq!(ui.menu_layout, MenuLayout::Standard);
    }
}
