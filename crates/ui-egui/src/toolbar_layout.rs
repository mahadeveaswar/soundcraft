//! Pure layout of the Standard (one-row) toolbar: which groups fit at a window width, and where.
//! No egui state here, so the responsive rules are unit-testable by sweeping widths.

use egui::{Rect, pos2, vec2};

/// Which toolbar the Edit window shows (`ui.toolbar_style`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolbarStyle {
    /// The original two-row toolbar with counters and the transport cluster.
    Classic,
    /// One 34 pt row of compact groups; the transport lives in the bottom bar.
    #[default]
    Standard,
}

impl ToolbarStyle {
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

/// The toolbar groups, numbered as in the design (1, 3..9; 2 is not built).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    /// 1: window configurations, undo, redo.
    Layout,
    /// 3: M S R W state keys.
    State,
    /// 4: automation mode combo.
    AutoCombo,
    /// 5: edit modes.
    EditModes,
    /// 6: tools.
    Tools,
    /// 6b: split, glue, delete.
    Actions,
    /// 7: snap, grid, adapt, nudge.
    Snap,
    /// 8: zoom.
    Zoom,
    /// 9: panel toggles (right aligned; folded into the gear menu when dropped).
    Panels,
    /// The master output meter (right aligned).
    Meter,
}

pub const ROW_H: f32 = 34.0;
/// Height of a group box inside the row.
pub const GROUP_H: f32 = 30.0;
pub const KEY: f32 = 24.0;
pub const KEY_GAP: f32 = 2.0;
pub const GROUP_PAD: f32 = 3.0;
pub const DIVIDER: f32 = 8.0;
pub const MARGIN: f32 = 8.0;
pub const GEAR_W: f32 = 28.0;
/// Below this width the row keeps this width and scrolls sideways.
pub const MIN_FIT: f32 = 900.0;

/// Left to right order of the groups drawn on the left.
const LEFT: [Group; 7] = [Group::Layout, Group::State, Group::AutoCombo, Group::EditModes, Group::Tools, Group::Actions, Group::Snap];
/// Groups that go first when the row is too narrow, first to go first.
pub const DROP_ORDER: [Group; 5] = [Group::Actions, Group::AutoCombo, Group::State, Group::Layout, Group::Panels];

/// Width of a run of `n` keys.
const fn keys(n: usize) -> f32 {
    n as f32 * KEY + (n.saturating_sub(1)) as f32 * KEY_GAP
}

/// Width of a group box (content plus padding).
pub fn width(g: Group) -> f32 {
    let content = match g {
        Group::Layout => 110.0 + 4.0 + keys(2),
        Group::State => keys(4),
        Group::AutoCombo => 78.0,
        Group::EditModes => 4.0 * 44.0 + 3.0 * KEY_GAP,
        Group::Tools => keys(11),
        Group::Actions => keys(3),
        Group::Snap => KEY + KEY_GAP + 64.0 + KEY_GAP + KEY + KEY_GAP + 80.0,
        Group::Zoom => keys(4),
        Group::Panels => keys(6),
        Group::Meter => 16.0,
    };
    content + 2.0 * GROUP_PAD
}

/// Groups that may never be dropped.
pub fn is_essential(g: Group) -> bool {
    !DROP_ORDER.contains(&g)
}

/// The result of laying the row out.
#[derive(Clone, Debug, PartialEq)]
pub struct Placed {
    /// Kept groups with their rectangles, left cluster first, then the right cluster.
    pub groups: Vec<(Group, Rect)>,
    /// The gear button, always present at the right edge.
    pub gear: Rect,
    /// True when the panel toggles did not fit and live in the gear menu.
    pub panels_in_gear: bool,
}

impl Placed {
    pub fn has(&self, g: Group) -> bool {
        self.groups.iter().any(|(x, _)| *x == g)
    }

    pub fn rect(&self, g: Group) -> Option<Rect> {
        self.groups.iter().find(|(x, _)| *x == g).map(|(_, r)| *r)
    }
}

/// Place the groups in `present` (every group the toolbar can draw) for a row `w` points wide
/// whose top-left is `origin`. Groups are dropped in `DROP_ORDER` until the rest fits; essential
/// groups are never dropped, so `w` is raised to `MIN_FIT` (the caller scrolls below that).
pub fn layout(origin: egui::Pos2, w: f32, present: &[Group]) -> Placed {
    let w = w.max(MIN_FIT);
    let mut kept: Vec<Group> = present.to_vec();
    let needed = |kept: &[Group]| -> f32 {
        let left: Vec<Group> = LEFT.iter().copied().filter(|g| kept.contains(g)).collect();
        let right: Vec<Group> = [Group::Zoom, Group::Panels, Group::Meter].into_iter().filter(|g| kept.contains(g)).collect();
        let sum = |v: &[Group]| v.iter().map(|g| width(*g)).sum::<f32>() + v.len().saturating_sub(1) as f32 * DIVIDER;
        2.0 * MARGIN + sum(&left) + sum(&right) + GEAR_W + DIVIDER * 2.0
    };
    for g in DROP_ORDER {
        if needed(&kept) <= w {
            break;
        }
        kept.retain(|k| *k != g);
    }
    let y = origin.y + (ROW_H - GROUP_H) / 2.0;
    let mut groups = Vec::new();
    let mut x = origin.x + MARGIN;
    for g in LEFT {
        if kept.contains(&g) {
            let gw = width(g);
            groups.push((g, Rect::from_min_size(pos2(x, y), vec2(gw, GROUP_H))));
            x += gw + DIVIDER;
        }
    }
    // Zoom follows the left cluster; panels and the meter sit at the right edge.
    if kept.contains(&Group::Zoom) {
        let gw = width(Group::Zoom);
        groups.push((Group::Zoom, Rect::from_min_size(pos2(x, y), vec2(gw, GROUP_H))));
        x += gw + DIVIDER;
    }
    let left_end = x;
    let right_edge = (origin.x + w - MARGIN).max(left_end + GEAR_W + DIVIDER);
    let gear = Rect::from_min_size(pos2(right_edge - GEAR_W, y), vec2(GEAR_W, GROUP_H));
    let mut rx = gear.min.x - DIVIDER;
    for g in [Group::Meter, Group::Panels] {
        if kept.contains(&g) {
            let gw = width(g);
            groups.push((g, Rect::from_min_size(pos2(rx - gw, y), vec2(gw, GROUP_H))));
            rx -= gw + DIVIDER;
        }
    }
    Placed { groups, gear, panels_in_gear: present.contains(&Group::Panels) && !kept.contains(&Group::Panels) }
}

/// The groups the Standard toolbar draws today.
pub const DRAWN: [Group; 10] = [
    Group::Layout,
    Group::State,
    Group::AutoCombo,
    Group::EditModes,
    Group::Tools,
    Group::Actions,
    Group::Snap,
    Group::Zoom,
    Group::Panels,
    Group::Meter,
];

/// Width the row needs so that nothing essential is dropped.
pub fn essential_width() -> f32 {
    let present: Vec<Group> = DRAWN.iter().copied().filter(|g| is_essential(*g)).collect();
    let left: f32 = LEFT.iter().filter(|g| present.contains(g)).map(|g| width(*g) + DIVIDER).sum();
    2.0 * MARGIN + left + width(Group::Zoom) + DIVIDER + width(Group::Meter) + DIVIDER + GEAR_W
}

/// `ui.toolbar_style {value?}`: report or set the toolbar style (no undo, not in the session).
pub(crate) fn command(app: &mut crate::SoundApp, p: &serde_json::Value) -> Result<serde_json::Value, String> {
    if let Some(name) = p.get("value").or_else(|| p.get("style")).and_then(serde_json::Value::as_str) {
        let Some(style) = ToolbarStyle::parse(name) else {
            return Err(format!("unknown toolbar style `{name}` (standard or classic)"));
        };
        app.ui.toolbar_style = style;
    }
    Ok(serde_json::json!({"style": app.ui.toolbar_style.id()}))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(w: f32) -> Placed {
        layout(pos2(0.0, 0.0), w, &DRAWN)
    }

    #[test]
    fn essential_groups_always_stay_and_the_row_fits_from_the_scroll_threshold() {
        assert!(essential_width() <= MIN_FIT, "essential groups need {} pt", essential_width());
        let mut w = 300.0;
        while w <= 3000.0 {
            let p = at(w);
            for g in DRAWN.iter().filter(|g| is_essential(**g)) {
                assert!(p.has(*g), "{g:?} dropped at {w}");
            }
            w += 7.0;
        }
    }

    #[test]
    fn groups_never_overlap_and_stay_inside_the_row_when_it_fits() {
        let mut w = 300.0;
        while w <= 3000.0 {
            let p = at(w);
            let mut rects: Vec<Rect> = p.groups.iter().map(|(_, r)| *r).collect();
            rects.push(p.gear);
            for (i, a) in rects.iter().enumerate() {
                for b in &rects[i + 1..] {
                    assert!(!a.intersects(*b), "overlap at {w}: {a:?} {b:?}");
                }
                let row = w.max(MIN_FIT);
                assert!(a.min.x >= 0.0 && a.max.x <= row && a.min.y >= 0.0 && a.max.y <= ROW_H, "{a:?} outside {row}");
            }
            w += 1.0;
        }
    }

    #[test]
    fn the_kept_set_only_grows_with_the_width() {
        let mut prev: Vec<Group> = Vec::new();
        let mut w = 300.0;
        while w <= 3000.0 {
            let now: Vec<Group> = at(w).groups.iter().map(|(g, _)| *g).collect();
            for g in &prev {
                assert!(now.contains(g), "{g:?} reappeared as missing at {w}");
            }
            prev = now;
            w += 1.0;
        }
        assert_eq!(prev.len(), DRAWN.len(), "everything fits at 3000");
    }

    #[test]
    fn groups_drop_in_the_documented_order() {
        let mut dropped: Vec<Group> = Vec::new();
        let mut w = 3000.0;
        while w >= 300.0 {
            let p = at(w);
            for g in DROP_ORDER {
                if !p.has(g) && !dropped.contains(&g) {
                    dropped.push(g);
                }
            }
            w -= 1.0;
        }
        assert_eq!(dropped, DROP_ORDER);
    }

    #[test]
    fn panels_fold_into_the_gear_when_dropped() {
        assert!(!at(3000.0).panels_in_gear);
        assert!(at(MIN_FIT).panels_in_gear || at(MIN_FIT).has(Group::Panels));
        assert!(at(500.0).panels_in_gear);
    }

    #[test]
    fn style_ids_round_trip() {
        for s in ToolbarStyle::ALL {
            assert_eq!(ToolbarStyle::parse(s.id()), Some(s));
        }
        assert_eq!(ToolbarStyle::default(), ToolbarStyle::Standard);
    }
}
