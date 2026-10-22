use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A MIDI note. Times are in model ticks ([`crate::PPQ`] per quarter note).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Note {
    /// MIDI pitch 0..=127 (60 = middle C, shown as "C3").
    pub pitch: u8,
    /// Note-on velocity 1..=127.
    pub velocity: u8,
    /// Note-off (release) velocity 0..=127; 64 when the source had none.
    pub release_velocity: u8,
    /// MIDI channel 0..=15.
    pub channel: u8,
    /// Start in ticks.
    pub start: i64,
    /// Length in ticks; always > 0.
    pub length: i64,
}

impl Note {
    /// Tick at which the note ends (`start + length`, saturating).
    pub fn end(&self) -> i64 {
        self.start.saturating_add(self.length)
    }
}

/// Kind of a continuous / channel controller event.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum CtrlKind {
    /// Control change with the controller number 0..=127. Value 0..=127.
    Cc(u8),
    /// Pitch bend. Value is centred: -8192..=8191 (0 = no bend).
    PitchBend,
    /// Channel (mono) aftertouch. Value 0..=127.
    ChannelPressure,
    /// Polyphonic aftertouch for the given pitch. Value 0..=127.
    PolyPressure(u8),
    /// Program change. Value is the program number 0..=127.
    Program,
}

/// A controller / channel event at a tick.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CtrlEvent {
    pub tick: i64,
    pub channel: u8,
    pub kind: CtrlKind,
    pub value: i32,
}

/// The MIDI content of a region or track.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Sequence {
    /// Notes, sorted by `(start, pitch)` after [`Sequence::sort`].
    pub notes: Vec<Note>,
    /// Controller events, sorted by tick after [`Sequence::sort`].
    pub ctrls: Vec<CtrlEvent>,
    /// Indices into `notes` of muted notes (they are kept but never sound). A side table so
    /// `Note` stays a plain value: every path that reorders, removes or inserts notes keeps it in
    /// step (see [`Sequence::sort_tracked`] and [`Sequence::retain_notes`]).
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub muted: BTreeSet<u32>,
}

fn kind_key(k: CtrlKind) -> (u8, u8) {
    match k {
        CtrlKind::Cc(n) => (0, n),
        CtrlKind::PitchBend => (1, 0),
        CtrlKind::ChannelPressure => (2, 0),
        CtrlKind::PolyPressure(p) => (3, p),
        CtrlKind::Program => (4, 0),
    }
}

impl Sequence {
    /// Sorts notes by `(start, pitch, channel, length, velocity)` and controller events by tick
    /// (stable, so same-tick events keep their order). Mute flags follow their notes.
    pub fn sort(&mut self) {
        self.sort_tracked();
    }

    /// Like [`Sequence::sort`], returning for every old note index its new index.
    pub fn sort_tracked(&mut self) -> Vec<usize> {
        let key = |a: &Note| (a.start, a.pitch, a.channel, a.length, a.velocity);
        self.ctrls.sort_by_key(|c| c.tick);
        let mut order: Vec<usize> = (0..self.notes.len()).collect();
        order.sort_by_key(|&i| self.notes.get(i).map(key));
        let sorted: Vec<Note> = order.iter().filter_map(|&i| self.notes.get(i).copied()).collect();
        let mut new_of_old = vec![0usize; order.len()];
        for (new, &old) in order.iter().enumerate() {
            if let Some(slot) = new_of_old.get_mut(old) {
                *slot = new;
            }
        }
        self.notes = sorted;
        self.remap_muted(&new_of_old);
        new_of_old
    }

    /// Rewrites the mute table through `new_of_old` (indices past its end are dropped).
    fn remap_muted(&mut self, new_of_old: &[usize]) {
        if self.muted.is_empty() {
            return;
        }
        self.muted = self.muted.iter().filter_map(|&m| new_of_old.get(m as usize).copied()).filter_map(|n| u32::try_from(n).ok()).collect();
    }

    /// Whether the note at `index` is muted.
    pub fn is_muted(&self, index: usize) -> bool {
        u32::try_from(index).is_ok_and(|i| self.muted.contains(&i))
    }

    /// Mutes or unmutes the note at `index` (ignored past the end).
    pub fn set_muted(&mut self, index: usize, muted: bool) {
        let Ok(i) = u32::try_from(index) else { return };
        if index >= self.notes.len() {
            return;
        }
        if muted {
            self.muted.insert(i);
        } else {
            self.muted.remove(&i);
        }
    }

    /// Keeps the notes for which `keep` is true, in order; mute flags follow the survivors.
    pub fn retain_notes(&mut self, mut keep: impl FnMut(&Note) -> bool) {
        let mut new_of_old = Vec::with_capacity(self.notes.len());
        let mut next = 0usize;
        for n in &self.notes {
            if keep(n) {
                new_of_old.push(next);
                next += 1;
            } else {
                new_of_old.push(usize::MAX);
            }
        }
        let mut i = 0usize;
        self.notes.retain(|_| {
            let k = new_of_old.get(i).is_some_and(|&n| n != usize::MAX);
            i += 1;
            k
        });
        if !self.muted.is_empty() {
            self.muted = self
                .muted
                .iter()
                .filter_map(|&m| new_of_old.get(m as usize).copied())
                .filter(|&n| n != usize::MAX)
                .filter_map(|n| u32::try_from(n).ok())
                .collect();
        }
    }

    /// The last tick covered by any note end or controller event (0 when empty).
    pub fn end_tick(&self) -> i64 {
        let n = self.notes.iter().map(Note::end).max();
        let c = self.ctrls.iter().map(|c| c.tick).max();
        match (n, c) {
            (Some(a), Some(b)) => a.max(b),
            (Some(a), None) | (None, Some(a)) => a,
            (None, None) => 0,
        }
    }

    /// Notes whose start lies in the half-open range `[start, end)`.
    pub fn notes_in(&self, start: i64, end: i64) -> impl Iterator<Item = &Note> + '_ {
        self.notes.iter().filter(move |n| n.start >= start && n.start < end)
    }

    /// Notes that sound anywhere inside the half-open range `[start, end)`.
    pub fn notes_overlapping(&self, start: i64, end: i64) -> impl Iterator<Item = &Note> + '_ {
        self.notes.iter().filter(move |n| n.start < end && n.end() > start)
    }

    /// Removes duplicate notes (same channel, pitch and start; the longest one is kept) and
    /// duplicate controller events (same tick, channel and kind; the last one is kept, matching
    /// what a synth would end up with). Returns how many events were removed. The sequence is
    /// left sorted.
    pub fn remove_duplicates(&mut self) -> usize {
        let before = self.notes.len() + self.ctrls.len();
        // Longest first within each (start, pitch, channel) group, then keep the first. The mute
        // flag travels with its note.
        let mut tagged: Vec<(Note, bool)> = self.notes.iter().enumerate().map(|(i, n)| (*n, self.is_muted(i))).collect();
        tagged.sort_by(|(a, _), (b, _)| (a.start, a.pitch, a.channel, b.length).cmp(&(b.start, b.pitch, b.channel, a.length)));
        tagged.dedup_by(|(later, _), (kept, _)| later.start == kept.start && later.pitch == kept.pitch && later.channel == kept.channel);
        self.muted = tagged.iter().enumerate().filter(|(_, (_, m))| *m).filter_map(|(i, _)| u32::try_from(i).ok()).collect();
        self.notes = tagged.into_iter().map(|(n, _)| n).collect();

        self.ctrls.sort_by_key(|c| c.tick);
        let mut kept: Vec<CtrlEvent> = Vec::with_capacity(self.ctrls.len());
        for c in self.ctrls.drain(..) {
            // Within a tick, replace an earlier event of the same channel+kind.
            // Only events at the same tick (the tail of `kept`) can match.
            let slot =
                kept.iter_mut().rev().take_while(|k| k.tick == c.tick).find(|k| k.channel == c.channel && kind_key(k.kind) == kind_key(c.kind));
            match slot {
                Some(slot) => *slot = c,
                None => kept.push(c),
            }
        }
        self.ctrls = kept;
        self.sort();
        before - (self.notes.len() + self.ctrls.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn n(pitch: u8, start: i64) -> Note {
        Note { pitch, velocity: 100, release_velocity: 64, channel: 0, start, length: 100 }
    }

    #[test]
    fn mute_follows_its_note_through_sort_and_retain() {
        let mut s = Sequence { notes: vec![n(60, 0), n(62, 100), n(64, 200)], ..Default::default() };
        s.set_muted(2, true);
        s.notes.insert(0, n(40, 500));
        s.muted = s.muted.iter().map(|m| m + 1).collect();
        s.sort();
        let idx = s.notes.iter().position(|x| x.pitch == 64).unwrap_or(99);
        assert!(s.is_muted(idx));
        assert_eq!(s.muted.len(), 1);
        s.retain_notes(|x| x.pitch != 60);
        let idx = s.notes.iter().position(|x| x.pitch == 64).unwrap_or(99);
        assert!(s.is_muted(idx));
        s.retain_notes(|x| x.pitch != 64);
        assert!(s.muted.is_empty());
    }

    #[test]
    fn duplicates_keep_the_survivors_mute_flag() {
        let mut s = Sequence { notes: vec![n(60, 0), n(60, 0), n(62, 10)], ..Default::default() };
        s.set_muted(2, true);
        assert_eq!(s.remove_duplicates(), 1);
        let idx = s.notes.iter().position(|x| x.pitch == 62).unwrap_or(99);
        assert!(s.is_muted(idx) && s.muted.len() == 1);
    }

    #[test]
    fn json_round_trip_and_old_sessions() {
        let mut s = Sequence { notes: vec![n(60, 0), n(62, 10)], ..Default::default() };
        let plain = serde_json::to_string(&s).unwrap_or_default();
        assert!(!plain.contains("muted"), "empty table is not written");
        let old: Sequence = serde_json::from_str(&plain).unwrap_or_default();
        assert!(old.muted.is_empty() && old.notes.len() == 2);
        s.set_muted(1, true);
        let json = serde_json::to_string(&s).unwrap_or_default();
        let back: Sequence = serde_json::from_str(&json).unwrap_or_default();
        assert_eq!(back, s);
        assert!(back.is_muted(1) && !back.is_muted(0));
        s.set_muted(9, true);
        assert_eq!(s.muted.len(), 1, "out-of-range index ignored");
    }
}
