//! Musical scales for note-entry assistance: membership tests and snapping a pitch to the nearest
//! scale tone.

/// The scale families the editor offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScaleKind {
    Major,
    Minor,
    Dorian,
    Phrygian,
    Lydian,
    Mixolydian,
    Locrian,
    HarmonicMinor,
    MelodicMinor,
    PentaMajor,
    PentaMinor,
    Blues,
    Chromatic,
}

impl ScaleKind {
    /// Every kind, in menu order.
    pub const ALL: [ScaleKind; 13] = [
        ScaleKind::Major,
        ScaleKind::Minor,
        ScaleKind::Dorian,
        ScaleKind::Phrygian,
        ScaleKind::Lydian,
        ScaleKind::Mixolydian,
        ScaleKind::Locrian,
        ScaleKind::HarmonicMinor,
        ScaleKind::MelodicMinor,
        ScaleKind::PentaMajor,
        ScaleKind::PentaMinor,
        ScaleKind::Blues,
        ScaleKind::Chromatic,
    ];

    /// Semitone offsets from the root, ascending, within one octave.
    pub fn intervals(self) -> &'static [u8] {
        match self {
            ScaleKind::Major => &[0, 2, 4, 5, 7, 9, 11],
            ScaleKind::Minor => &[0, 2, 3, 5, 7, 8, 10],
            ScaleKind::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            ScaleKind::Phrygian => &[0, 1, 3, 5, 7, 8, 10],
            ScaleKind::Lydian => &[0, 2, 4, 6, 7, 9, 11],
            ScaleKind::Mixolydian => &[0, 2, 4, 5, 7, 9, 10],
            ScaleKind::Locrian => &[0, 1, 3, 5, 6, 8, 10],
            ScaleKind::HarmonicMinor => &[0, 2, 3, 5, 7, 8, 11],
            ScaleKind::MelodicMinor => &[0, 2, 3, 5, 7, 9, 11],
            ScaleKind::PentaMajor => &[0, 2, 4, 7, 9],
            ScaleKind::PentaMinor => &[0, 3, 5, 7, 10],
            ScaleKind::Blues => &[0, 3, 5, 6, 7, 10],
            ScaleKind::Chromatic => &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
        }
    }

    /// Stable id used in commands and stored data.
    pub fn id(self) -> &'static str {
        match self {
            ScaleKind::Major => "major",
            ScaleKind::Minor => "minor",
            ScaleKind::Dorian => "dorian",
            ScaleKind::Phrygian => "phrygian",
            ScaleKind::Lydian => "lydian",
            ScaleKind::Mixolydian => "mixolydian",
            ScaleKind::Locrian => "locrian",
            ScaleKind::HarmonicMinor => "harmonic_minor",
            ScaleKind::MelodicMinor => "melodic_minor",
            ScaleKind::PentaMajor => "penta_major",
            ScaleKind::PentaMinor => "penta_minor",
            ScaleKind::Blues => "blues",
            ScaleKind::Chromatic => "chromatic",
        }
    }

    /// Name for menus.
    pub fn label(self) -> &'static str {
        match self {
            ScaleKind::Major => "Major",
            ScaleKind::Minor => "Minor",
            ScaleKind::Dorian => "Dorian",
            ScaleKind::Phrygian => "Phrygian",
            ScaleKind::Lydian => "Lydian",
            ScaleKind::Mixolydian => "Mixolydian",
            ScaleKind::Locrian => "Locrian",
            ScaleKind::HarmonicMinor => "Harmonic minor",
            ScaleKind::MelodicMinor => "Melodic minor",
            ScaleKind::PentaMajor => "Pentatonic major",
            ScaleKind::PentaMinor => "Pentatonic minor",
            ScaleKind::Blues => "Blues",
            ScaleKind::Chromatic => "Chromatic",
        }
    }

    /// Parses an id (case-insensitive; spaces and dashes count as underscores).
    pub fn parse(text: &str) -> Option<ScaleKind> {
        let t = text.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Self::ALL.into_iter().find(|k| k.id() == t)
    }

    fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap_or(0)
    }
}

/// A scale: a root pitch class (0 = C .. 11 = B) and a kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Scale {
    pub root: u8,
    pub kind: ScaleKind,
}

impl Scale {
    /// A scale; `root` wraps into 0..=11.
    pub fn new(root: u8, kind: ScaleKind) -> Self {
        Self { root: root % 12, kind }
    }

    /// Whether `pitch` is a tone of the scale (in any octave).
    pub fn contains(&self, pitch: u8) -> bool {
        let rel = (pitch % 12 + 12 - self.root) % 12;
        self.kind.intervals().contains(&rel)
    }

    /// The nearest scale tone to `pitch` within 0..=127; ties go down.
    pub fn snap(&self, pitch: u8) -> u8 {
        let p = i16::from(pitch.min(127));
        for d in 0..=127i16 {
            for cand in [p - d, p + d] {
                if (0..=127).contains(&cand) && self.contains(cand as u8) {
                    return cand as u8;
                }
            }
        }
        pitch.min(127)
    }

    /// A single number for storage: 1 + root + 12 x kind position (0 means no scale).
    pub fn to_code(&self) -> f64 {
        (1 + usize::from(self.root) + 12 * self.kind.index()) as f64
    }

    /// The inverse of [`Scale::to_code`]; `None` for 0, non-finite or out-of-range codes.
    pub fn from_code(code: f64) -> Option<Scale> {
        if !code.is_finite() || !(1.0..=156.5).contains(&code) {
            return None;
        }
        let n = code.round() as usize - 1;
        let kind = ScaleKind::ALL.get(n / 12).copied()?;
        Some(Scale { root: (n % 12) as u8, kind })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_major_snap_table() {
        let c = Scale::new(0, ScaleKind::Major);
        // (input, expected): out-of-scale tones tie down.
        let table =
            [(60, 60), (61, 60), (62, 62), (63, 62), (64, 64), (65, 65), (66, 65), (67, 67), (68, 67), (69, 69), (70, 69), (71, 71), (72, 72)];
        for (p, want) in table {
            assert_eq!(c.snap(p), want, "snap {p}");
        }
        assert!(c.contains(0) && c.contains(120) && !c.contains(61));
    }

    #[test]
    fn rooted_and_other_kinds() {
        let a_minor = Scale::new(9, ScaleKind::Minor);
        assert!(a_minor.contains(57) && a_minor.contains(60) && !a_minor.contains(61));
        assert_eq!(Scale::new(0, ScaleKind::Chromatic).snap(61), 61);
        assert_eq!(Scale::new(0, ScaleKind::PentaMajor).snap(65), 64, "F is out of C pentatonic: the tie goes down to E");
        assert_eq!(Scale::new(2, ScaleKind::Major).snap(0), 1, "C snaps up to C# in D major");
        assert_eq!(Scale::new(0, ScaleKind::Major).snap(127), 127, "G9 is in C major");
    }

    #[test]
    fn codes_and_ids_round_trip() {
        for k in ScaleKind::ALL {
            assert_eq!(ScaleKind::parse(k.id()), Some(k));
            for root in 0..12 {
                let s = Scale::new(root, k);
                assert_eq!(Scale::from_code(s.to_code()), Some(s));
            }
        }
        assert_eq!(Scale::from_code(0.0), None);
        assert_eq!(Scale::from_code(f64::NAN), None);
        assert_eq!(Scale::from_code(1e9), None);
        assert_eq!(ScaleKind::parse("Harmonic Minor"), Some(ScaleKind::HarmonicMinor));
    }
}
