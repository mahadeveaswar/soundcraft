//! Tracks and playlists.

use crate::{AutoParam, AutomationLane, ChannelFormat, Clip, ClipId, Mixer, TrackId};
use soundcraft_time::{Range, Samples};

/// Track types (Track › New).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum TrackKind {
    #[default]
    Audio,
    Aux,
    Master,
    Midi,
    Instrument,
    Vca,
    Folder,
    /// Picture: clips reference a movie ([`crate::VideoSource`]); no audio path.
    Video,
}

impl TrackKind {
    pub const ALL: [TrackKind; 8] = [
        TrackKind::Audio,
        TrackKind::Aux,
        TrackKind::Master,
        TrackKind::Midi,
        TrackKind::Instrument,
        TrackKind::Vca,
        TrackKind::Folder,
        TrackKind::Video,
    ];
    pub fn label(self) -> &'static str {
        match self {
            TrackKind::Audio => "Audio Track",
            TrackKind::Aux => "Aux Input",
            TrackKind::Master => "Master Fader",
            TrackKind::Midi => "MIDI Track",
            TrackKind::Instrument => "Instrument Track",
            TrackKind::Vca => "VCA Master",
            TrackKind::Folder => "Routing Folder Track",
            TrackKind::Video => "Video Track",
        }
    }
    pub fn default_name(self) -> &'static str {
        match self {
            TrackKind::Audio => "Audio",
            TrackKind::Aux => "Aux",
            TrackKind::Master => "Master",
            TrackKind::Midi => "MIDI",
            TrackKind::Instrument => "Inst",
            TrackKind::Vca => "VCA",
            TrackKind::Folder => "Folder",
            TrackKind::Video => "Video",
        }
    }
    pub fn id(self) -> &'static str {
        match self {
            TrackKind::Audio => "audio",
            TrackKind::Aux => "aux",
            TrackKind::Master => "master",
            TrackKind::Midi => "midi",
            TrackKind::Instrument => "instrument",
            TrackKind::Vca => "vca",
            TrackKind::Folder => "folder",
            TrackKind::Video => "video",
        }
    }
    pub fn from_id(s: &str) -> Option<TrackKind> {
        let s = s.to_ascii_lowercase();
        TrackKind::ALL.into_iter().find(|k| k.id() == s || k.label().eq_ignore_ascii_case(&s))
    }
    /// Track kinds that hold clips on a timeline.
    pub fn has_playlist(self) -> bool {
        matches!(self, TrackKind::Audio | TrackKind::Midi | TrackKind::Instrument | TrackKind::Video)
    }
    pub fn is_midi(self) -> bool {
        matches!(self, TrackKind::Midi | TrackKind::Instrument)
    }
    /// Kinds that pass audio through the mixer.
    pub fn is_audio_path(self) -> bool {
        matches!(self, TrackKind::Audio | TrackKind::Aux | TrackKind::Master | TrackKind::Instrument | TrackKind::Folder)
    }
}

/// Track heights (View › Track Height presets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub enum TrackHeight {
    /// Any height from Micro to Extreme, in points.
    Custom(u16),
    Micro,
    Mini,
    Small,
    #[default]
    Medium,
    Large,
    Jumbo,
    Extreme,
    /// Fit To Window: this many points.
    Fit(u16),
}

impl TrackHeight {
    pub const ALL: [TrackHeight; 7] = [
        TrackHeight::Micro,
        TrackHeight::Mini,
        TrackHeight::Small,
        TrackHeight::Medium,
        TrackHeight::Large,
        TrackHeight::Jumbo,
        TrackHeight::Extreme,
    ];
    pub fn points(self) -> f32 {
        match self {
            TrackHeight::Custom(points) => f32::from(points).clamp(TrackHeight::Micro.points(), TrackHeight::Extreme.points()),
            TrackHeight::Micro => 16.0,
            TrackHeight::Mini => 24.0,
            TrackHeight::Small => 46.0,
            TrackHeight::Medium => 88.0,
            TrackHeight::Large => 132.0,
            TrackHeight::Jumbo => 220.0,
            TrackHeight::Extreme => 440.0,
            TrackHeight::Fit(points) => f32::from(points.max(16)),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            TrackHeight::Custom(_) => "custom",
            TrackHeight::Micro => "micro",
            TrackHeight::Mini => "mini",
            TrackHeight::Small => "small",
            TrackHeight::Medium => "medium",
            TrackHeight::Large => "large",
            TrackHeight::Jumbo => "jumbo",
            TrackHeight::Extreme => "extreme",
            TrackHeight::Fit(_) => "fit to window",
        }
    }
    pub fn from_id(s: &str) -> Option<TrackHeight> {
        TrackHeight::ALL.into_iter().find(|h| h.label().eq_ignore_ascii_case(s))
    }
    /// The preset this many points tall, else a custom height clamped to Micro..Extreme.
    pub fn from_points(points: f32) -> TrackHeight {
        let p = points.max(TrackHeight::Micro.points()).min(TrackHeight::Extreme.points()).round() as u16;
        TrackHeight::ALL.into_iter().find(|h| h.points() == f32::from(p)).unwrap_or(TrackHeight::Custom(p))
    }
    /// The next preset taller (or shorter) than this height; Extreme (or Micro) stays.
    pub fn step(self, taller: bool) -> TrackHeight {
        let p = self.points();
        let next =
            if taller { TrackHeight::ALL.into_iter().find(|h| h.points() > p) } else { TrackHeight::ALL.into_iter().rev().find(|h| h.points() < p) };
        next.unwrap_or(self)
    }
}

/// The default track colour palette (original, chosen for SoundCraft).
pub const TRACK_COLORS: [[u8; 3]; 16] = [
    [86, 132, 214],
    [94, 170, 214],
    [80, 190, 168],
    [104, 186, 98],
    [168, 196, 76],
    [222, 196, 72],
    [230, 152, 62],
    [222, 104, 72],
    [214, 82, 110],
    [196, 86, 168],
    [150, 98, 206],
    [110, 104, 214],
    [130, 150, 170],
    [178, 140, 110],
    [120, 170, 140],
    [190, 120, 150],
];

/// The extended picker palette: 12 hue columns (red to magenta) by 6 tone rows (pale, light, base,
/// rich, deep, dark), then a row of 12 neutrals from light to dark. Original values. Sessions store
/// RGB, so nothing about this table is persisted.
pub const TRACK_PALETTE_EXT: [[u8; 3]; 84] = [
    [242, 156, 156],
    [242, 196, 156],
    [242, 216, 156],
    [242, 239, 156],
    [207, 242, 156],
    [156, 242, 173],
    [156, 242, 229],
    [156, 224, 242],
    [156, 199, 242],
    [156, 165, 242],
    [189, 156, 242],
    [242, 156, 223],
    [228, 129, 129],
    [228, 175, 129],
    [228, 198, 129],
    [228, 225, 129],
    [188, 228, 129],
    [129, 228, 149],
    [129, 228, 213],
    [129, 208, 228],
    [129, 178, 228],
    [129, 139, 228],
    [167, 129, 228],
    [228, 129, 207],
    [214, 81, 81],
    [214, 143, 81],
    [214, 174, 81],
    [214, 210, 81],
    [161, 214, 81],
    [81, 214, 108],
    [81, 214, 194],
    [81, 188, 214],
    [81, 148, 214],
    [81, 95, 214],
    [132, 81, 214],
    [214, 81, 186],
    [202, 43, 43],
    [202, 117, 43],
    [202, 154, 43],
    [202, 197, 43],
    [138, 202, 43],
    [43, 202, 75],
    [43, 202, 178],
    [43, 170, 202],
    [43, 122, 202],
    [43, 59, 202],
    [104, 43, 202],
    [202, 43, 167],
    [169, 30, 30],
    [169, 95, 30],
    [169, 127, 30],
    [169, 164, 30],
    [113, 169, 30],
    [30, 169, 58],
    [30, 169, 148],
    [30, 141, 169],
    [30, 99, 169],
    [30, 44, 169],
    [83, 30, 169],
    [169, 30, 139],
    [114, 29, 29],
    [114, 69, 29],
    [114, 89, 29],
    [114, 111, 29],
    [80, 114, 29],
    [29, 114, 46],
    [29, 114, 101],
    [29, 97, 114],
    [29, 71, 114],
    [29, 37, 114],
    [61, 29, 114],
    [114, 29, 96],
    [230, 230, 230],
    [213, 213, 213],
    [196, 196, 196],
    [179, 179, 179],
    [163, 163, 163],
    [146, 146, 146],
    [129, 129, 129],
    [113, 113, 113],
    [96, 96, 96],
    [79, 79, 79],
    [63, 63, 63],
    [46, 46, 46],
];

/// Palette index to colour: 0..16 is the default set, 16..100 the extended grid; else `None`.
pub fn track_color_by_index(n: u64) -> Option<[u8; 3]> {
    let n = usize::try_from(n).ok()?;
    match n {
        0..=15 => TRACK_COLORS.get(n).copied(),
        _ => TRACK_PALETTE_EXT.get(n - 16).copied(),
    }
}

/// A playlist: an alternative take of a track's clips.
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize, serde::Deserialize)]
pub struct Playlist {
    pub name: String,
    /// Clips sorted by start; non-overlapping except at crossfades.
    pub clips: Vec<Clip>,
}

impl Playlist {
    pub fn new(name: impl Into<String>) -> Self {
        Playlist { name: name.into(), clips: Vec::new() }
    }
    pub fn sort(&mut self) {
        self.clips.sort_by_key(|c| (c.start, c.id.0));
    }
    pub fn clip(&self, id: ClipId) -> Option<&Clip> {
        self.clips.iter().find(|c| c.id == id)
    }
    pub fn clip_mut(&mut self, id: ClipId) -> Option<&mut Clip> {
        self.clips.iter_mut().find(|c| c.id == id)
    }
    pub fn clips_in(&self, r: Range) -> impl Iterator<Item = &Clip> {
        self.clips.iter().filter(move |c| c.range().overlaps(&r))
    }
    pub fn clip_at(&self, at: Samples) -> Option<&Clip> {
        self.clips.iter().rev().find(|c| c.range().contains(at))
    }
    pub fn end(&self) -> Samples {
        self.clips.iter().map(Clip::end).max().unwrap_or(0)
    }
}

/// A track.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub id: TrackId,
    pub name: String,
    pub kind: TrackKind,
    pub format: ChannelFormat,
    pub color: [u8; 3],
    pub height: TrackHeight,
    pub playlists: Vec<Playlist>,
    pub active_playlist: usize,
    pub mixer: Mixer,
    pub automation: Vec<AutomationLane>,
    /// Which lane the track shows: `waveform`/`blocks`/`notes`/`regions` or an automation param id.
    pub view: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub inactive: bool,
    #[serde(default)]
    pub frozen: bool,
    #[serde(default)]
    pub comments: String,
    /// Folder that contains this track.
    #[serde(default)]
    pub folder: Option<TrackId>,
    #[serde(default = "yes")]
    pub folder_open: bool,
    /// Instrument plugin (instrument tracks) — registry id.
    #[serde(default)]
    pub instrument: Option<crate::Insert>,
    /// Ticks (musical) or samples timebase for clips.
    #[serde(default)]
    pub ticks_timebase: bool,
    /// Elastic Audio algorithm, if enabled.
    #[serde(default)]
    pub elastic: Option<String>,
    /// Pinned to top in the Edit window.
    #[serde(default)]
    pub pinned: bool,
    /// Automation lanes shown under the track (`automation.lane_show`).
    #[serde(default)]
    pub shown_lanes: Vec<crate::ShownLane>,
}

fn yes() -> bool {
    true
}

impl Track {
    pub fn new(id: TrackId, name: impl Into<String>, kind: TrackKind, format: ChannelFormat, color: [u8; 3]) -> Self {
        let name = name.into();
        let format = if kind.is_midi() && kind == TrackKind::Midi { ChannelFormat::Mono } else { format };
        let view = if kind.is_midi() {
            "notes"
        } else if kind.has_playlist() {
            "waveform"
        } else {
            "volume"
        };
        Track {
            id,
            playlists: vec![Playlist::new(name.clone())],
            name,
            kind,
            format,
            color,
            height: TrackHeight::Medium,
            active_playlist: 0,
            mixer: Mixer::new(format),
            automation: Vec::new(),
            view: view.into(),
            hidden: false,
            inactive: false,
            frozen: false,
            comments: String::new(),
            folder: None,
            folder_open: true,
            instrument: None,
            ticks_timebase: kind.is_midi(),
            elastic: None,
            pinned: false,
            shown_lanes: Vec::new(),
        }
    }

    pub fn channels(&self) -> usize {
        self.format.channels()
    }

    pub fn playlist(&self) -> Option<&Playlist> {
        self.playlists.get(self.active_playlist)
    }

    pub fn playlist_mut(&mut self) -> Option<&mut Playlist> {
        self.playlists.get_mut(self.active_playlist)
    }

    pub fn clips(&self) -> &[Clip] {
        self.playlist().map_or(&[], |p| p.clips.as_slice())
    }

    pub fn lane(&self, p: &AutoParam) -> Option<&AutomationLane> {
        self.automation.iter().find(|l| &l.param == p)
    }

    pub fn lane_mut(&mut self, p: &AutoParam) -> &mut AutomationLane {
        let idx = match self.automation.iter().position(|l| &l.param == p) {
            Some(i) => i,
            None => {
                self.automation.push(AutomationLane::new(p.clone()));
                self.automation.len() - 1
            }
        };
        let len = self.automation.len();
        // `idx` is in range: it came from `position` or is the index of the element just pushed.
        &mut self.automation[idx.min(len - 1)]
    }

    pub fn is_folder(&self) -> bool {
        self.kind == TrackKind::Folder
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oklab(c: [u8; 3]) -> [f32; 3] {
        let lin = |v: u8| {
            let x = f32::from(v) / 255.0;
            if x <= 0.04045 { x / 12.92 } else { ((x + 0.055) / 1.055).powf(2.4) }
        };
        let (r, g, b) = (lin(c[0]), lin(c[1]), lin(c[2]));
        let l = (0.412_221_47 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
        let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
        let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
        [
            0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
            1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
            0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
        ]
    }

    fn hue(c: [u8; 3]) -> f32 {
        let (r, g, b) = (f32::from(c[0]), f32::from(c[1]), f32::from(c[2]));
        let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
        let d = mx - mn;
        if d == 0.0 {
            return 0.0;
        }
        let h = if mx == r {
            ((g - b) / d).rem_euclid(6.0)
        } else if mx == g {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };
        h * 60.0
    }

    #[test]
    fn extended_palette_is_distinct_and_hue_ordered() {
        let p = TRACK_PALETTE_EXT;
        for (i, a) in p.iter().enumerate() {
            for b in &p[i + 1..] {
                assert_ne!(a, b);
                let (x, y) = (oklab(*a), oklab(*b));
                let d = ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt();
                assert!(d > 0.04, "{a:?} and {b:?} are too close ({d})");
            }
        }
        for row in p[..72].chunks(12) {
            assert!(row.windows(2).all(|w| hue(w[0]) <= hue(w[1])), "row not in hue order");
        }
    }

    #[test]
    fn palette_index_mapping() {
        assert_eq!(track_color_by_index(3), Some(TRACK_COLORS[3]));
        assert_eq!(track_color_by_index(20), Some(TRACK_PALETTE_EXT[4]));
        assert_eq!(track_color_by_index(99), Some(TRACK_PALETTE_EXT[83]));
        assert_eq!(track_color_by_index(100), None);
    }

    #[test]
    fn new_track_defaults() {
        let t = Track::new(TrackId(1), "Gtr", TrackKind::Audio, ChannelFormat::Stereo, TRACK_COLORS[0]);
        assert_eq!(t.channels(), 2);
        assert_eq!(t.view, "waveform");
        assert_eq!(t.playlists.len(), 1);
        let m = Track::new(TrackId(2), "Keys", TrackKind::Midi, ChannelFormat::Stereo, TRACK_COLORS[0]);
        assert_eq!(m.format, ChannelFormat::Mono);
        assert_eq!(m.view, "notes");
    }

    #[test]
    fn lane_mut_creates_once() {
        let mut t = Track::new(TrackId(1), "Gtr", TrackKind::Audio, ChannelFormat::Mono, TRACK_COLORS[0]);
        t.lane_mut(&AutoParam::Volume).set_point(0, -3.0);
        t.lane_mut(&AutoParam::Volume).set_point(10, -6.0);
        assert_eq!(t.automation.len(), 1);
        assert_eq!(t.lane(&AutoParam::Volume).map(|l| l.points.len()), Some(2));
    }

    #[test]
    fn kind_ids() {
        for k in TrackKind::ALL {
            assert_eq!(TrackKind::from_id(k.id()), Some(k));
        }
    }

    #[test]
    fn custom_heights_round_to_presets_and_step_through_them() {
        assert_eq!(TrackHeight::from_points(88.0), TrackHeight::Medium);
        assert_eq!(TrackHeight::from_points(57.4), TrackHeight::Custom(57));
        assert_eq!(TrackHeight::from_points(9999.0), TrackHeight::Extreme);
        assert_eq!(TrackHeight::from_points(f32::NAN), TrackHeight::Micro);
        assert_eq!(TrackHeight::Custom(u16::MAX).points(), 440.0);
        assert_eq!(TrackHeight::Custom(57).step(true), TrackHeight::Medium);
        assert_eq!(TrackHeight::Custom(57).step(false), TrackHeight::Small);
        assert_eq!(TrackHeight::Extreme.step(true), TrackHeight::Extreme);
        assert_eq!(TrackHeight::Micro.step(false), TrackHeight::Micro);
        let json = serde_json::to_string(&TrackHeight::Custom(57)).unwrap();
        assert_eq!(serde_json::from_str::<TrackHeight>(&json).unwrap(), TrackHeight::Custom(57));
        assert_eq!(serde_json::from_str::<TrackHeight>("\"Large\"").unwrap(), TrackHeight::Large);
    }
}
