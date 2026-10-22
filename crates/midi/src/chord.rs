//! Chord shapes for chord entry: the pitches of a chord from a root, a kind and an inversion.

/// Supported chord kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChordKind {
    Maj,
    Min,
    Dim,
    Aug,
    Sus2,
    Sus4,
    Dom7,
    Maj7,
    Min7,
}

impl ChordKind {
    /// Every kind, in menu order.
    pub const ALL: [ChordKind; 9] = [
        ChordKind::Maj,
        ChordKind::Min,
        ChordKind::Dim,
        ChordKind::Aug,
        ChordKind::Sus2,
        ChordKind::Sus4,
        ChordKind::Dom7,
        ChordKind::Maj7,
        ChordKind::Min7,
    ];

    /// Semitone offsets from the root, ascending.
    pub fn intervals(self) -> &'static [u8] {
        match self {
            ChordKind::Maj => &[0, 4, 7],
            ChordKind::Min => &[0, 3, 7],
            ChordKind::Dim => &[0, 3, 6],
            ChordKind::Aug => &[0, 4, 8],
            ChordKind::Sus2 => &[0, 2, 7],
            ChordKind::Sus4 => &[0, 5, 7],
            ChordKind::Dom7 => &[0, 4, 7, 10],
            ChordKind::Maj7 => &[0, 4, 7, 11],
            ChordKind::Min7 => &[0, 3, 7, 10],
        }
    }

    /// Stable id used in commands: `maj`, `min`, `dim`, `aug`, `sus2`, `sus4`, `7`, `maj7`, `m7`.
    pub fn id(self) -> &'static str {
        match self {
            ChordKind::Maj => "maj",
            ChordKind::Min => "min",
            ChordKind::Dim => "dim",
            ChordKind::Aug => "aug",
            ChordKind::Sus2 => "sus2",
            ChordKind::Sus4 => "sus4",
            ChordKind::Dom7 => "7",
            ChordKind::Maj7 => "maj7",
            ChordKind::Min7 => "m7",
        }
    }

    /// Short name for menus.
    pub fn label(self) -> &'static str {
        match self {
            ChordKind::Maj => "Major",
            ChordKind::Min => "Minor",
            ChordKind::Dim => "Diminished",
            ChordKind::Aug => "Augmented",
            ChordKind::Sus2 => "Sus2",
            ChordKind::Sus4 => "Sus4",
            ChordKind::Dom7 => "Dominant 7",
            ChordKind::Maj7 => "Major 7",
            ChordKind::Min7 => "Minor 7",
        }
    }

    /// Parses an id (case-insensitive).
    pub fn parse(text: &str) -> Option<ChordKind> {
        let t = text.trim();
        if t == "M7" {
            return Some(ChordKind::Maj7);
        }
        let t = t.to_ascii_lowercase();
        Self::ALL.into_iter().find(|k| k.id() == t)
    }
}

/// The MIDI pitches of a chord with its root at `root` (0..=127), in ascending order. Inversion
/// `n` moves the lowest `n` notes up an octave (wrapping past the chord size). Notes that would
/// pass 127 are dropped.
pub fn notes(root: u8, kind: ChordKind, inversion: u8) -> Vec<u8> {
    let iv = kind.intervals();
    let inv = usize::from(inversion) % iv.len().max(1);
    let mut pitches: Vec<u16> = iv.iter().enumerate().map(|(i, off)| u16::from(root) + u16::from(*off) + if i < inv { 12 } else { 0 }).collect();
    pitches.sort_unstable();
    pitches.into_iter().filter(|p| *p <= 127).map(|p| p as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_major_seventh_and_its_first_inversion() {
        assert_eq!(notes(60, ChordKind::Maj7, 0), vec![60, 64, 67, 71]);
        assert_eq!(notes(60, ChordKind::Maj7, 1), vec![64, 67, 71, 72]);
        assert_eq!(notes(60, ChordKind::Maj7, 4), vec![60, 64, 67, 71], "inversion wraps");
    }

    #[test]
    fn shapes_and_limits() {
        assert_eq!(notes(60, ChordKind::Min, 0), vec![60, 63, 67]);
        assert_eq!(notes(60, ChordKind::Sus4, 0), vec![60, 65, 67]);
        assert_eq!(notes(60, ChordKind::Dom7, 2), vec![67, 70, 72, 76]);
        assert_eq!(notes(126, ChordKind::Maj, 0), vec![126], "notes past 127 are dropped");
        for k in ChordKind::ALL {
            assert_eq!(ChordKind::parse(k.id()), Some(k));
        }
        assert_eq!(ChordKind::parse("M7"), Some(ChordKind::Maj7));
    }
}
