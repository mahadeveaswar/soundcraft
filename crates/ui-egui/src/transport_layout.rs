//! Pure layout of the transport bar: three zones with the transport keys exactly centred.
//!
//! The centre group stays in the middle of the bar; the left and right zones drop their
//! low-priority items independently until they fit beside it. Only when a side has nothing left
//! to drop does the centre group itself shrink. Dropped items live in the gear menu.

/// Everything the bar can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Item {
    RecMode,
    Quantize,
    Lock,
    Primary,
    Secondary,
    Start,
    End,
    Rew,
    FFwd,
    Loop,
    Stop,
    Play,
    Rec,
    LLoc,
    RLoc,
    Click,
    Tempo,
    Meter,
    CountOff,
    PrePost,
    MasterMeter,
    Float,
    Gear,
    Close,
}

impl Item {
    pub const ALL: [Item; 24] = [
        Item::RecMode,
        Item::Quantize,
        Item::Lock,
        Item::Primary,
        Item::Secondary,
        Item::Start,
        Item::End,
        Item::Rew,
        Item::FFwd,
        Item::Loop,
        Item::Stop,
        Item::Play,
        Item::Rec,
        Item::LLoc,
        Item::RLoc,
        Item::Click,
        Item::Tempo,
        Item::Meter,
        Item::CountOff,
        Item::PrePost,
        Item::MasterMeter,
        Item::Float,
        Item::Gear,
        Item::Close,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Item::RecMode => "rec_mode",
            Item::Quantize => "quantize",
            Item::Lock => "lock",
            Item::Primary => "primary",
            Item::Secondary => "secondary",
            Item::Start => "start",
            Item::End => "end",
            Item::Rew => "rewind",
            Item::FFwd => "forward",
            Item::Loop => "loop",
            Item::Stop => "stop",
            Item::Play => "play",
            Item::Rec => "record",
            Item::LLoc => "left_locator",
            Item::RLoc => "right_locator",
            Item::Click => "click",
            Item::Tempo => "tempo",
            Item::Meter => "meter",
            Item::CountOff => "count_off",
            Item::PrePost => "pre_post",
            Item::MasterMeter => "master_meter",
            Item::Float => "float",
            Item::Gear => "gear",
            Item::Close => "close",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Item::RecMode => "Record mode",
            Item::Quantize => "Input quantize",
            Item::Lock => "Keyboard lock",
            Item::Primary => "Primary position",
            Item::Secondary => "Secondary position",
            Item::Start => "Go to start",
            Item::End => "Go to end",
            Item::Rew => "Rewind",
            Item::FFwd => "Fast forward",
            Item::Loop => "Loop playback",
            Item::Stop => "Stop",
            Item::Play => "Play",
            Item::Rec => "Record",
            Item::LLoc => "Left locator",
            Item::RLoc => "Right locator",
            Item::Click => "Click",
            Item::Tempo => "Tempo",
            Item::Meter => "Meter",
            Item::CountOff => "Count off",
            Item::PrePost => "Pre and post roll",
            Item::MasterMeter => "Master level",
            Item::Float => "Float or dock bar",
            Item::Gear => "Bar settings",
            Item::Close => "Close bar",
        }
    }

    pub fn parse(id: &str) -> Option<Item> {
        Item::ALL.into_iter().find(|i| i.id() == id)
    }

    /// Items the user may hide from the gear menu; the transport keys, gear and close stay.
    pub const fn hideable(self) -> bool {
        !matches!(self, Item::Stop | Item::Play | Item::Rec | Item::Gear | Item::Close)
    }

    pub const fn width(self) -> f32 {
        match self {
            Item::RecMode | Item::Quantize => 36.0,
            Item::Lock => 26.0,
            Item::Primary | Item::Secondary => 150.0,
            Item::Start | Item::End | Item::Rew | Item::FFwd | Item::Loop | Item::Stop | Item::Play | Item::Rec => KEY_W,
            Item::LLoc | Item::RLoc => 112.0,
            Item::Click | Item::CountOff | Item::PrePost => 28.0,
            Item::Tempo => 148.0,
            Item::Meter => 44.0,
            Item::MasterMeter => 60.0,
            Item::Float => 22.0,
            Item::Gear => 24.0,
            Item::Close => 20.0,
        }
    }
}

pub const KEY_W: f32 = 28.0;
/// Gap between items of one group.
pub const GAP: f32 = 2.0;
/// Space a divider takes between two groups of a side zone (8 + 1 + 8).
pub const DIVIDER: f32 = 17.0;
/// Space between two groups of the centre keys.
pub const CENTRE_DIVIDER: f32 = 6.0;
/// Padding between the centre group's keys and its outline.
pub const OUTLINE_PAD: f32 = 4.0;
pub const PAD_LEFT: f32 = 8.0;
pub const PAD_RIGHT: f32 = 6.0;
/// Room kept between a side zone and the centre outline.
pub const CENTRE_CLEARANCE: f32 = 12.0;

const LEFT_GROUPS: [&[Item]; 2] = [&[Item::RecMode, Item::Quantize, Item::Lock], &[Item::Primary, Item::Secondary]];
const CENTRE_GROUPS: [&[Item]; 3] = [&[Item::Start, Item::End, Item::Rew, Item::FFwd], &[Item::Loop], &[Item::Stop, Item::Play, Item::Rec]];
const RIGHT_GROUPS: [&[Item]; 5] = [
    &[Item::LLoc, Item::RLoc],
    &[Item::Click, Item::Tempo, Item::Meter],
    &[Item::CountOff, Item::PrePost],
    &[Item::MasterMeter],
    &[Item::Float, Item::Gear, Item::Close],
];
/// First to go first.
const LEFT_DROP: [Item; 5] = [Item::Lock, Item::Quantize, Item::RecMode, Item::Secondary, Item::Primary];
const RIGHT_DROP: [Item; 9] =
    [Item::MasterMeter, Item::PrePost, Item::CountOff, Item::Meter, Item::RLoc, Item::LLoc, Item::Click, Item::Tempo, Item::Float];
/// Centre items that leave, in steps, once the sides cannot give anything more.
const CENTRE_STEPS: [&[Item]; 3] = [&[Item::End, Item::FFwd], &[Item::Rew, Item::Start], &[Item::Loop]];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placed {
    pub item: Item,
    pub x0: f32,
    pub x1: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layout {
    pub left: Vec<Placed>,
    pub centre: Vec<Placed>,
    pub right: Vec<Placed>,
    /// Outline around the centre keys (x range).
    pub outline: Option<(f32, f32)>,
    /// x of the 1 pt dividers between groups of the side zones.
    pub dividers: Vec<f32>,
    /// x of the dividers between groups of centre keys.
    pub centre_dividers: Vec<f32>,
    /// Items that do not fit (and are not hidden by the user): they live in the gear menu.
    pub dropped: Vec<Item>,
}

impl Layout {
    pub fn all(&self) -> impl Iterator<Item = &Placed> {
        self.left.iter().chain(&self.centre).chain(&self.right)
    }

    pub fn find(&self, item: Item) -> Option<&Placed> {
        self.all().find(|p| p.item == item)
    }

    pub fn kept(&self) -> Vec<Item> {
        self.all().map(|p| p.item).collect()
    }
}

fn groups_of(groups: &[&[Item]], keep: &dyn Fn(Item) -> bool) -> Vec<Vec<Item>> {
    groups.iter().map(|g| g.iter().copied().filter(|i| keep(*i)).collect::<Vec<_>>()).filter(|g| !g.is_empty()).collect()
}

fn group_w(g: &[Item]) -> f32 {
    g.iter().map(|i| i.width()).sum::<f32>() + GAP * g.len().saturating_sub(1) as f32
}

fn zone_w(groups: &[Vec<Item>], divider: f32) -> f32 {
    groups.iter().map(|g| group_w(g)).sum::<f32>() + divider * groups.len().saturating_sub(1) as f32
}

fn left_need(groups: &[Vec<Item>]) -> f32 {
    PAD_LEFT + zone_w(groups, DIVIDER)
}

fn right_need(groups: &[Vec<Item>]) -> f32 {
    PAD_RIGHT + zone_w(groups, DIVIDER)
}

/// Where everything goes in a bar `w` points wide (x from the bar's left edge). `hidden` items
/// are the user's choice and are left out entirely.
pub fn layout(w: f32, hidden: &[Item]) -> Layout {
    let shown = |i: Item| !hidden.contains(&i) || !i.hideable();
    // Centre level: 0 = all keys; each further level removes one step.
    let mut level = 0;
    let mut centre_items;
    loop {
        let gone: Vec<Item> = CENTRE_STEPS.iter().take(level).flat_map(|s| s.iter().copied()).collect();
        centre_items = groups_of(&CENTRE_GROUPS, &|i| shown(i) && !gone.contains(&i));
        let cw = zone_w(&centre_items, CENTRE_DIVIDER) + 2.0 * OUTLINE_PAD;
        let budget = (w - cw) / 2.0 - CENTRE_CLEARANCE;
        let mandatory = groups_of(&RIGHT_GROUPS, &|i| matches!(i, Item::Gear | Item::Close));
        if level >= CENTRE_STEPS.len() || right_need(&mandatory) <= budget {
            break;
        }
        level += 1;
    }
    let cw = zone_w(&centre_items, CENTRE_DIVIDER) + 2.0 * OUTLINE_PAD;
    let budget = (w - cw) / 2.0 - CENTRE_CLEARANCE;
    let optional_ok = level == 0;
    let side = |groups: &[&[Item]], drop: &[Item], need: &dyn Fn(&[Vec<Item>]) -> f32, mandatory: &dyn Fn(Item) -> bool| {
        let mut gone: Vec<Item> = Vec::new();
        loop {
            let g = groups_of(groups, &|i| shown(i) && !gone.contains(&i) && (optional_ok || mandatory(i)));
            if need(&g) <= budget || !drop.iter().any(|d| !gone.contains(d) && shown(*d)) {
                return g;
            }
            if let Some(next) = drop.iter().find(|d| !gone.contains(d) && shown(**d)) {
                gone.push(*next);
            }
        }
    };
    let left_groups = side(&LEFT_GROUPS, &LEFT_DROP, &left_need, &|_| false);
    let right_groups = side(&RIGHT_GROUPS, &RIGHT_DROP, &right_need, &|i| matches!(i, Item::Gear | Item::Close));

    let mut out = Layout::default();
    // Left zone from the left edge.
    let mut x = PAD_LEFT;
    for (gi, g) in left_groups.iter().enumerate() {
        if gi > 0 {
            out.dividers.push(x - DIVIDER / 2.0 + 0.5);
        }
        for (ii, item) in g.iter().enumerate() {
            if ii > 0 {
                x += GAP;
            }
            out.left.push(Placed { item: *item, x0: x, x1: x + item.width() });
            x += item.width();
        }
        x += DIVIDER;
    }
    // Centre group, centred on the bar width.
    let items_w = zone_w(&centre_items, CENTRE_DIVIDER);
    let mut x = (w - items_w) / 2.0;
    out.outline = Some((x - OUTLINE_PAD, x + items_w + OUTLINE_PAD));
    for (gi, g) in centre_items.iter().enumerate() {
        if gi > 0 {
            out.centre_dividers.push(x - CENTRE_DIVIDER / 2.0);
        }
        for (ii, item) in g.iter().enumerate() {
            if ii > 0 {
                x += GAP;
            }
            out.centre.push(Placed { item: *item, x0: x, x1: x + item.width() });
            x += item.width();
        }
        x += CENTRE_DIVIDER;
    }
    // Right zone to the right edge.
    let mut x = w - right_need(&right_groups);
    for (gi, g) in right_groups.iter().enumerate() {
        if gi > 0 {
            out.dividers.push(x - DIVIDER / 2.0 + 0.5);
        }
        for (ii, item) in g.iter().enumerate() {
            if ii > 0 {
                x += GAP;
            }
            out.right.push(Placed { item: *item, x0: x, x1: x + item.width() });
            x += item.width();
        }
        x += DIVIDER;
    }
    let kept = out.kept();
    out.dropped = Item::ALL.into_iter().filter(|i| !kept.contains(i) && shown(*i)).collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sweep() -> impl Iterator<Item = f32> {
        (200..=3000).map(|w| w as f32)
    }

    #[test]
    fn no_overlaps_at_any_width() {
        for w in sweep() {
            let l = layout(w, &[]);
            let mut all: Vec<&Placed> = l.all().collect();
            all.sort_by(|a, b| a.x0.total_cmp(&b.x0));
            for pair in all.windows(2) {
                assert!(pair[1].x0 >= pair[0].x1 - 0.001, "{:?} overlaps {:?} at {w}", pair[0], pair[1]);
            }
            for p in &all {
                assert!(p.x1 > p.x0);
            }
        }
    }

    #[test]
    fn everything_stays_inside_the_bar() {
        for w in sweep().filter(|w| *w >= 300.0) {
            for p in layout(w, &[]).all() {
                assert!(p.x0 >= 0.0 && p.x1 <= w, "{:?} outside 0..{w}", p);
            }
        }
    }

    #[test]
    fn centre_keys_are_centred() {
        for w in sweep().filter(|w| *w >= 400.0) {
            let l = layout(w, &[]);
            let (Some(first), Some(last)) = (l.centre.first(), l.centre.last()) else { panic!("no centre keys at {w}") };
            let mid = (first.x0 + last.x1) / 2.0;
            assert!((mid - w / 2.0).abs() < 0.5, "centre {mid} vs {} at {w}", w / 2.0);
        }
    }

    #[test]
    fn kept_set_only_grows_with_width() {
        let mut previous = layout(200.0, &[]).kept();
        for w in sweep().skip(1) {
            let now = layout(w, &[]).kept();
            for item in &previous {
                assert!(now.contains(item), "{item:?} kept at {} but dropped at {w}", w - 1.0);
            }
            previous = now;
        }
    }

    #[test]
    fn stop_play_record_gear_and_close_are_always_there() {
        for w in sweep() {
            let l = layout(w, &[]);
            for item in [Item::Stop, Item::Play, Item::Rec] {
                assert!(l.centre.iter().any(|p| p.item == item), "{item:?} missing at {w}");
            }
            assert!(l.right.iter().any(|p| p.item == Item::Gear), "gear missing at {w}");
            assert!(l.right.iter().any(|p| p.item == Item::Close), "close missing at {w}");
        }
    }

    #[test]
    fn wide_bar_shows_everything_and_narrow_bar_lists_the_rest() {
        let wide = layout(2400.0, &[]);
        assert!(wide.dropped.is_empty(), "{:?}", wide.dropped);
        assert_eq!(wide.kept().len(), Item::ALL.len());
        let narrow = layout(600.0, &[]);
        assert!(!narrow.dropped.is_empty());
        assert!(narrow.dropped.contains(&Item::MasterMeter));
        // Every item is either placed, dropped to the gear menu, or nothing else.
        assert_eq!(narrow.kept().len() + narrow.dropped.len(), Item::ALL.len());
    }

    #[test]
    fn drop_order_follows_the_priority_list() {
        // Master meter goes before the lock, the lock before the secondary position.
        let widths_without = |item: Item| (200..=3000).map(|w| w as f32).find(|w| layout(*w, &[]).kept().contains(&item)).unwrap_or(f32::MAX);
        assert!(widths_without(Item::MasterMeter) > widths_without(Item::PrePost));
        assert!(widths_without(Item::PrePost) >= widths_without(Item::CountOff));
        assert!(widths_without(Item::Secondary) >= widths_without(Item::Primary));
        assert!(widths_without(Item::Lock) >= widths_without(Item::Quantize));
        assert!(widths_without(Item::Tempo) <= widths_without(Item::Click));
    }

    #[test]
    fn hidden_items_are_left_out_but_the_keys_stay() {
        let hidden = [Item::Start, Item::Secondary, Item::MasterMeter, Item::Stop, Item::Gear];
        let l = layout(2400.0, &hidden);
        assert!(l.find(Item::Start).is_none() && l.find(Item::Secondary).is_none() && l.find(Item::MasterMeter).is_none());
        assert!(l.find(Item::Stop).is_some() && l.find(Item::Gear).is_some(), "stop and gear cannot be hidden");
        assert!(!l.dropped.contains(&Item::Start), "hidden is not dropped");
    }

    #[test]
    fn item_ids_round_trip() {
        for item in Item::ALL {
            assert_eq!(Item::parse(item.id()), Some(item));
        }
        assert_eq!(Item::parse("nope"), None);
    }
}
