//! Editor-owned settings of the piano roll: the snap value and the `ui.midi_tool` /
//! `ui.midi_snap` commands. The snap is the editor's own (it does not change the session edit
//! grid): `Grid` follows the session grid, which is the default.

use crate::SoundApp;
use crate::midi_editor::MidiTool;
use serde_json::{Value, json};
use soundcraft_time::TICKS_PER_QUARTER;

/// How a note division is modified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    Straight,
    Triplet,
    Dotted,
}

impl Flavor {
    fn suffix(self) -> &'static str {
        match self {
            Flavor::Straight => "",
            Flavor::Triplet => "t",
            Flavor::Dotted => "d",
        }
    }
}

/// What note positions and lengths snap to in the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MidiSnap {
    /// Follow the session edit grid.
    #[default]
    Grid,
    /// No snapping (one tick); the roll still draws the session grid.
    Off,
    /// One bar.
    Bar,
    /// A note division: denominator 2, 4, 8, 16, 32 or 64.
    Div(u8, Flavor),
}

/// Every choice in menu order.
pub fn all_snaps() -> Vec<MidiSnap> {
    let mut v = vec![MidiSnap::Grid, MidiSnap::Off, MidiSnap::Bar];
    for flavor in [Flavor::Straight, Flavor::Triplet, Flavor::Dotted] {
        for d in [2, 4, 8, 16, 32, 64] {
            v.push(MidiSnap::Div(d, flavor));
        }
    }
    v
}

impl MidiSnap {
    /// The id used by `ui.midi_snap {snap}`: `grid`, `off`, `bar`, `1/8`, `1/8t`, `1/8d`.
    pub fn id(self) -> String {
        match self {
            MidiSnap::Grid => "grid".into(),
            MidiSnap::Off => "off".into(),
            MidiSnap::Bar => "bar".into(),
            MidiSnap::Div(d, f) => format!("1/{d}{}", f.suffix()),
        }
    }

    pub fn parse(text: &str) -> Option<MidiSnap> {
        let t = text.trim().to_ascii_lowercase();
        all_snaps().into_iter().find(|s| s.id() == t)
    }

    pub fn label(self) -> String {
        match self {
            MidiSnap::Grid => "Grid".into(),
            MidiSnap::Off => "Off".into(),
            MidiSnap::Bar => "Bar".into(),
            MidiSnap::Div(d, Flavor::Straight) => format!("1/{d}"),
            MidiSnap::Div(d, Flavor::Triplet) => format!("1/{d} triplet"),
            MidiSnap::Div(d, Flavor::Dotted) => format!("1/{d} dotted"),
        }
    }

    /// Ticks of this snap. `session_grid` is the session grid in ticks and `bar` the length of
    /// the bar in ticks. `Off` snaps to one tick.
    pub fn ticks(self, session_grid: i64, bar: i64) -> i64 {
        match self {
            MidiSnap::Grid => session_grid.max(1),
            MidiSnap::Off => 1,
            MidiSnap::Bar => bar.max(1),
            MidiSnap::Div(d, f) => {
                let base = TICKS_PER_QUARTER * 4 / i64::from(d.max(1));
                match f {
                    Flavor::Straight => base,
                    Flavor::Triplet => base * 2 / 3,
                    Flavor::Dotted => base * 3 / 2,
                }
                .max(1)
            }
        }
    }

    /// The grid the roll draws and uses for default note lengths: `Off` still shows the session grid.
    pub fn display_ticks(self, session_grid: i64, bar: i64) -> i64 {
        if self == MidiSnap::Off { session_grid.max(1) } else { self.ticks(session_grid, bar) }
    }
}

impl MidiTool {
    pub const ALL: [MidiTool; 8] =
        [MidiTool::Select, MidiTool::Pencil, MidiTool::Erase, MidiTool::Split, MidiTool::Glue, MidiTool::Mute, MidiTool::Zoom, MidiTool::Line];

    /// The id used by `ui.midi_tool {tool}`. The Draw tool is `pencil` (`draw` is accepted too).
    pub fn id(self) -> &'static str {
        match self {
            MidiTool::Select => "select",
            MidiTool::Pencil => "pencil",
            MidiTool::Erase => "erase",
            MidiTool::Split => "split",
            MidiTool::Glue => "glue",
            MidiTool::Mute => "mute",
            MidiTool::Zoom => "zoom",
            MidiTool::Line => "line",
        }
    }

    pub fn parse(text: &str) -> Option<MidiTool> {
        let t = text.trim().to_ascii_lowercase();
        let t = if t == "draw" { "pencil".to_string() } else { t };
        Self::ALL.into_iter().find(|m| m.id() == t)
    }
}

fn lanes_json(app: &SoundApp) -> Value {
    json!({"lanes": app.midi.lanes.iter().map(|l| json!({"lane": l.lane.id(), "height": l.height})).collect::<Vec<_>>()})
}

/// `ui.midi_tool {tool?}`, `ui.midi_snap {snap?}`, `ui.midi_lane_add {lane?}`,
/// `ui.midi_lane_remove {index | lane}`, `ui.midi_lane_height {index, height}`,
/// `ui.midi_inspector {open?}` and `ui.midi_assist {highlight_rows?, snap_new?, draw_chords?}`.
/// Without parameters they report the current value.
pub fn run_ui_command(app: &mut SoundApp, id: &str, p: &Value) -> Option<Result<Value, String>> {
    use crate::midi_lanes::{LANE_H_MAX, LANE_H_MIN, Lane};
    match id {
        "ui.midi_tool" => {
            if let Some(name) = p.get("tool").and_then(Value::as_str) {
                let Some(tool) = MidiTool::parse(name) else {
                    return Some(Err(format!("unknown tool `{name}` (select, pencil/draw, erase, split, glue, mute, zoom, line)")));
                };
                if app.midi.tool != tool {
                    crate::midi_audition::stop(app);
                }
                app.midi.tool = tool;
            }
            Some(Ok(json!({"tool": app.midi.tool.id()})))
        }
        "ui.midi_snap" => {
            if let Some(name) = p.get("snap").and_then(Value::as_str) {
                let Some(snap) = MidiSnap::parse(name) else {
                    return Some(Err(format!("unknown snap `{name}` (grid, off, bar, 1/2 to 1/64 with t or d)")));
                };
                app.midi.snap = snap;
            }
            Some(Ok(json!({"snap": app.midi.snap.id()})))
        }
        "ui.midi_lane_add" => {
            let lane = match p.get("lane").and_then(Value::as_str) {
                Some(name) => match Lane::parse(name) {
                    Some(l) => Some(l),
                    None => return Some(Err(format!("unknown lane `{name}` (velocity, modulation, expression, sustain, pitch_bend)"))),
                },
                None => None,
            };
            if !crate::midi_lanes::add(&mut app.midi.lanes, lane) {
                return Some(Err("no lane to add (already shown, or all lanes are in use)".into()));
            }
            Some(Ok(lanes_json(app)))
        }
        "ui.midi_lane_remove" => {
            let index = match (p.get("index").and_then(Value::as_u64), p.get("lane").and_then(Value::as_str)) {
                (Some(i), _) => usize::try_from(i).ok(),
                (None, Some(name)) => Lane::parse(name).and_then(|l| app.midi.lanes.iter().position(|c| c.lane == l)),
                _ => return Some(Err("give `index` or `lane`".into())),
            };
            if !index.is_some_and(|i| crate::midi_lanes::remove(&mut app.midi.lanes, i)) {
                return Some(Err("no such lane".into()));
            }
            app.midi.active_lane = 0;
            Some(Ok(lanes_json(app)))
        }
        "ui.midi_lane_height" => {
            let (Some(i), Some(h)) = (p.get("index").and_then(Value::as_u64), p.get("height").and_then(Value::as_f64)) else {
                return Some(Err("give `index` and `height`".into()));
            };
            let Some(lane) = usize::try_from(i).ok().and_then(|i| app.midi.lanes.get_mut(i)) else {
                return Some(Err("no such lane".into()));
            };
            if !h.is_finite() {
                return Some(Err("`height` must be a number".into()));
            }
            lane.height = (h as f32).clamp(LANE_H_MIN, LANE_H_MAX);
            Some(Ok(lanes_json(app)))
        }
        "ui.midi_inspector" => {
            if let Some(open) = p.get("open").and_then(Value::as_bool) {
                app.midi.inspector_open = open;
            }
            Some(Ok(json!({"open": app.midi.inspector_open})))
        }
        "ui.midi_assist" => {
            let i = &mut app.midi.insp;
            if let Some(v) = p.get("highlight_rows").and_then(Value::as_bool) {
                i.highlight_rows = v;
            }
            if let Some(v) = p.get("snap_new").and_then(Value::as_bool) {
                i.snap_new = v;
            }
            if let Some(v) = p.get("draw_chords").and_then(Value::as_bool) {
                i.draw_chords = v;
            }
            Some(Ok(json!({"highlight_rows": i.highlight_rows, "snap_new": i.snap_new, "draw_chords": i.draw_chords})))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_ids_round_trip_and_are_unique() {
        let all = all_snaps();
        assert_eq!(all.len(), 21);
        for s in &all {
            assert_eq!(MidiSnap::parse(&s.id()), Some(*s), "{}", s.id());
        }
        let mut ids: Vec<String> = all.iter().map(|s| s.id()).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
        assert_eq!(MidiSnap::parse("1/7"), None);
        assert_eq!(MidiSnap::parse(" 1/16T "), Some(MidiSnap::Div(16, Flavor::Triplet)));
    }

    #[test]
    fn snap_ticks() {
        assert_eq!(MidiSnap::Div(4, Flavor::Straight).ticks(0, 3840), 960);
        assert_eq!(MidiSnap::Div(8, Flavor::Triplet).ticks(0, 3840), 320);
        assert_eq!(MidiSnap::Div(8, Flavor::Dotted).ticks(0, 3840), 720);
        assert_eq!(MidiSnap::Div(32, Flavor::Straight).ticks(0, 3840), 120);
        assert_eq!(MidiSnap::Div(64, Flavor::Straight).ticks(0, 3840), 60);
        assert_eq!(MidiSnap::Div(64, Flavor::Triplet).ticks(0, 3840), 40);
        assert_eq!(MidiSnap::Div(64, Flavor::Dotted).ticks(0, 3840), 90);
        assert_eq!(MidiSnap::Bar.ticks(240, 2880), 2880);
        assert_eq!(MidiSnap::Grid.ticks(240, 3840), 240);
        assert_eq!(MidiSnap::Off.ticks(240, 3840), 1);
        assert_eq!(MidiSnap::Off.display_ticks(240, 3840), 240);
        assert_eq!(MidiSnap::Grid.ticks(0, 0), 1);
    }
}
