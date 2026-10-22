//! Grid and Nudge values: musical or absolute step sizes, grid lines and snapping.

use crate::{FrameRate, SampleRate, Samples, TICKS_PER_QUARTER, TempoMap, to_samples};

/// Note values for Bars|Beats grids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum NoteValue {
    Bar,
    Half,
    Quarter,
    Eighth,
    Sixteenth,
    ThirtySecond,
    SixtyFourth,
}

impl NoteValue {
    pub const ALL: [NoteValue; 7] = [
        NoteValue::Bar,
        NoteValue::Half,
        NoteValue::Quarter,
        NoteValue::Eighth,
        NoteValue::Sixteenth,
        NoteValue::ThirtySecond,
        NoteValue::SixtyFourth,
    ];
    pub fn ticks(self) -> i64 {
        match self {
            NoteValue::Bar => TICKS_PER_QUARTER * 4,
            NoteValue::Half => TICKS_PER_QUARTER * 2,
            NoteValue::Quarter => TICKS_PER_QUARTER,
            NoteValue::Eighth => TICKS_PER_QUARTER / 2,
            NoteValue::Sixteenth => TICKS_PER_QUARTER / 4,
            NoteValue::ThirtySecond => TICKS_PER_QUARTER / 8,
            NoteValue::SixtyFourth => TICKS_PER_QUARTER / 16,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            NoteValue::Bar => "1 bar",
            NoteValue::Half => "1/2 note",
            NoteValue::Quarter => "1/4 note",
            NoteValue::Eighth => "1/8 note",
            NoteValue::Sixteenth => "1/16 note",
            NoteValue::ThirtySecond => "1/32 note",
            NoteValue::SixtyFourth => "1/64 note",
        }
    }
}

/// A grid or nudge step.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum GridValue {
    /// Musical: note value, optionally dotted or triplet.
    Note { value: NoteValue, dotted: bool, triplet: bool },
    /// Absolute seconds (Min:Secs grids: 0.001 … 10 s).
    Seconds(f64),
    /// Frames at the session timecode rate.
    Frames(i64),
    /// Raw samples.
    Samples(i64),
}

impl Default for GridValue {
    fn default() -> Self {
        GridValue::Note { value: NoteValue::Sixteenth, dotted: false, triplet: false }
    }
}

impl GridValue {
    pub fn label(&self) -> String {
        match self {
            GridValue::Note { value, dotted, triplet } => {
                let mut s = value.label().to_string();
                if *dotted {
                    s.push_str(" dotted");
                }
                if *triplet {
                    s.push_str(" triplet");
                }
                s
            }
            GridValue::Seconds(s) => format!("{s:.3} sec"),
            GridValue::Frames(f) => format!("{f} frame{}", if *f == 1 { "" } else { "s" }),
            GridValue::Samples(n) => format!("{n} samples"),
        }
    }

    /// The next larger or smaller value of the same kind, or `self` at either end. Note values keep their
    /// dot or triplet; seconds, frames and samples step by tens.
    pub fn stepped(self, larger: bool) -> GridValue {
        fn next<T: PartialOrd + Copy>(ladder: &[T], v: T, larger: bool) -> Option<T> {
            if larger { ladder.iter().copied().find(|x| *x > v) } else { ladder.iter().rev().copied().find(|x| *x < v) }
        }
        match self {
            GridValue::Note { value, dotted, triplet } => {
                let i = NoteValue::ALL.iter().position(|v| *v == value).unwrap_or(0);
                let j = if larger { i.checked_sub(1) } else { i.checked_add(1) };
                j.and_then(|j| NoteValue::ALL.get(j)).map_or(self, |&value| GridValue::Note { value, dotted, triplet })
            }
            GridValue::Seconds(s) => next(&[0.001, 0.01, 0.1, 1.0, 10.0], s, larger).map_or(self, GridValue::Seconds),
            GridValue::Frames(f) => next(&[1, 10, 100], f, larger).map_or(self, GridValue::Frames),
            GridValue::Samples(n) => next(&[1, 10, 100, 1_000, 10_000, 100_000], n, larger).map_or(self, GridValue::Samples),
        }
    }

    /// The grid to draw and snap to at a zoom level: `self`, coarsened one ladder step at a time until a
    /// step spans at least `min_px` screen pixels at `at`. Never finer than `self`; stops at the largest
    /// value of its kind (a bar for musical grids). Non-finite or non-positive inputs return `self`.
    pub fn effective(self, samples_per_px: f64, min_px: f64, at: Samples, sr: SampleRate, map: &TempoMap, rate: FrameRate) -> GridValue {
        if !(samples_per_px.is_finite() && samples_per_px > 0.0 && min_px.is_finite()) {
            return self;
        }
        let need = min_px * samples_per_px;
        let mut g = self;
        // The ladders are short, so a fixed bound is plenty and guarantees termination.
        for _ in 0..16 {
            if g.step_samples(at, sr, map, rate) as f64 >= need {
                break;
            }
            let next = g.stepped(true);
            if next == g {
                break;
            }
            g = next;
        }
        g
    }

    fn note_ticks(value: NoteValue, dotted: bool, triplet: bool, map: &TempoMap, at_tick: i64) -> i64 {
        let base = if value == NoteValue::Bar { map.meter_at_tick(at_tick).ticks_per_bar() } else { value.ticks() };
        let mut t = base;
        if dotted {
            t = t * 3 / 2;
        }
        if triplet {
            t = t * 2 / 3;
        }
        t.max(1)
    }

    /// The step in samples at position `at` (musical steps depend on tempo).
    pub fn step_samples(&self, at: Samples, sr: SampleRate, map: &TempoMap, rate: FrameRate) -> Samples {
        match *self {
            GridValue::Note { value, dotted, triplet } => {
                let t0 = map.samples_to_ticks(at, sr);
                let dt = Self::note_ticks(value, dotted, triplet, map, t0);
                (map.tick_to_samples(t0 + dt, sr) - map.tick_to_samples(t0, sr)).max(1)
            }
            GridValue::Seconds(s) => sr.samples(s.clamp(0.000_01, 3600.0)).max(1),
            GridValue::Frames(f) => to_samples(rate.samples_per_frame(sr) * f.clamp(1, 1_000_000) as f64).max(1),
            GridValue::Samples(n) => n.max(1),
        }
    }

    /// Nearest grid line to `pos`.
    pub fn snap(&self, pos: Samples, sr: SampleRate, map: &TempoMap, rate: FrameRate) -> Samples {
        let lo = self.line_at_or_before(pos, sr, map, rate);
        let hi = self.next_line(lo, sr, map, rate);
        if (pos - lo) <= (hi - pos) { lo } else { hi }
    }

    /// The grid line at or before `pos`.
    pub fn line_at_or_before(&self, pos: Samples, sr: SampleRate, map: &TempoMap, rate: FrameRate) -> Samples {
        match *self {
            GridValue::Note { value, dotted, triplet } => {
                // Float ticks can land a hair either side of a line's own tick.
                let mut t = map.samples_to_ticks_f(pos, sr).floor() as i64;
                if map.tick_to_samples(t, sr) > pos {
                    t = t.saturating_sub(1);
                } else if map.tick_to_samples(t.saturating_add(1), sr) <= pos {
                    t = t.saturating_add(1);
                }
                // Grid lines restart at each bar line so odd meters stay aligned.
                let bb = map.bar_beat_at_tick(t);
                let bar_tick = map.bar_start_tick(bb.bar);
                let step = Self::note_ticks(value, dotted, triplet, map, bar_tick);
                let k = (t - bar_tick).div_euclid(step);
                map.tick_to_samples(bar_tick + k * step, sr)
            }
            _ => {
                let step = self.step_samples(pos, sr, map, rate);
                pos.div_euclid(step).saturating_mul(step)
            }
        }
    }

    /// The first grid line strictly after `line`.
    pub fn next_line(&self, line: Samples, sr: SampleRate, map: &TempoMap, rate: FrameRate) -> Samples {
        let step = self.step_samples(line, sr, map, rate);
        let candidate = self.line_at_or_before(line.saturating_add(step), sr, map, rate);
        if candidate > line { candidate } else { line.saturating_add(step) }
    }

    /// All grid lines within `[start, end)`, capped at `max` lines.
    pub fn lines(&self, start: Samples, end: Samples, sr: SampleRate, map: &TempoMap, rate: FrameRate, max: usize) -> Vec<Samples> {
        let mut out = Vec::new();
        let mut p = self.line_at_or_before(start, sr, map, rate);
        while p < end && out.len() < max {
            if p >= start {
                out.push(p);
            }
            let n = self.next_line(p, sr, map, rate);
            if n <= p {
                break;
            }
            p = n;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_lines_are_their_own_line_at_odd_tempos() {
        let (g, sr, rate) = (GridValue::default(), SampleRate::HZ_48000, FrameRate::Fps30);
        for bpm in [90.0, 95.0, 110.0, 130.0, 140.0] {
            let map = TempoMap::new(bpm, 4, 4).unwrap();
            let mut line = 0;
            for _ in 0..64 {
                assert_eq!(g.line_at_or_before(line, sr, &map, rate), line, "{bpm} bpm");
                line = g.next_line(line, sr, &map, rate);
            }
        }
    }

    #[test]
    fn sixteenth_grid_at_120() {
        let g = GridValue::default();
        let (sr, map, rate) = (SampleRate::HZ_48000, TempoMap::default(), FrameRate::Fps30);
        assert_eq!(g.step_samples(0, sr, &map, rate), 6000);
        assert_eq!(g.snap(2900, sr, &map, rate), 0);
        assert_eq!(g.snap(3100, sr, &map, rate), 6000);
        assert_eq!(g.lines(0, 24_000, sr, &map, rate, 100), vec![0, 6000, 12000, 18000]);
    }

    #[test]
    fn absolute_grids() {
        let (sr, map, rate) = (SampleRate::HZ_48000, TempoMap::default(), FrameRate::Fps25);
        assert_eq!(GridValue::Seconds(1.0).snap(70_000, sr, &map, rate), 48_000);
        assert_eq!(GridValue::Frames(1).step_samples(0, sr, &map, rate), 1920);
        assert_eq!(GridValue::Samples(0).step_samples(0, sr, &map, rate), 1);
    }

    #[test]
    fn lines_are_capped() {
        let (sr, map, rate) = (SampleRate::HZ_48000, TempoMap::default(), FrameRate::Fps25);
        assert_eq!(GridValue::Samples(1).lines(0, 1_000_000, sr, &map, rate, 50).len(), 50);
    }

    #[test]
    fn triplets_and_dots() {
        let (sr, map, rate) = (SampleRate::HZ_48000, TempoMap::default(), FrameRate::Fps25);
        let t = GridValue::Note { value: NoteValue::Quarter, dotted: false, triplet: true };
        assert_eq!(t.step_samples(0, sr, &map, rate), 16_000);
        let d = GridValue::Note { value: NoteValue::Quarter, dotted: true, triplet: false };
        assert_eq!(d.step_samples(0, sr, &map, rate), 36_000);
    }

    #[test]
    fn values_step_to_the_next_larger_or_smaller() {
        let note = |value, triplet| GridValue::Note { value, dotted: false, triplet };
        assert_eq!(note(NoteValue::Sixteenth, false).stepped(true), note(NoteValue::Eighth, false));
        assert_eq!(note(NoteValue::Eighth, true).stepped(false), note(NoteValue::Sixteenth, true));
        assert_eq!(note(NoteValue::Bar, false).stepped(true), note(NoteValue::Bar, false));
        assert_eq!(note(NoteValue::SixtyFourth, false).stepped(false), note(NoteValue::SixtyFourth, false));
        assert_eq!(GridValue::Seconds(0.01).stepped(true), GridValue::Seconds(0.1));
        assert_eq!(GridValue::Seconds(0.5).stepped(false), GridValue::Seconds(0.1));
        assert_eq!(GridValue::Seconds(10.0).stepped(true), GridValue::Seconds(10.0));
        assert_eq!(GridValue::Frames(1).stepped(true), GridValue::Frames(10));
        assert_eq!(GridValue::Samples(100).stepped(false), GridValue::Samples(10));
        assert_eq!(GridValue::Samples(1).stepped(false), GridValue::Samples(1));
    }

    #[test]
    fn effective_grid_coarsens_with_zoom_out_and_never_refines() {
        let (sr, map, rate) = (SampleRate::HZ_48000, TempoMap::default(), FrameRate::Fps25);
        let g = GridValue::default(); // 1/16 = 6000 samples at 120 bpm
        let eff = |spp| g.effective(spp, 12.0, 0, sr, &map, rate);
        assert_eq!(eff(100.0), g); // 60 px
        assert_eq!(eff(1000.0), GridValue::Note { value: NoteValue::Eighth, dotted: false, triplet: false }); // 12000 >= 12000
        assert_eq!(eff(2000.0), GridValue::Note { value: NoteValue::Quarter, dotted: false, triplet: false });
        assert_eq!(eff(1e9), GridValue::Note { value: NoteValue::Bar, dotted: false, triplet: false });
        assert_eq!(GridValue::Seconds(0.01).effective(100.0, 12.0, 0, sr, &map, rate), GridValue::Seconds(0.1));
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(g.effective(bad, 12.0, 0, sr, &map, rate), g);
        }
    }

    #[test]
    fn effective_grid_keeps_triplets() {
        let (sr, map, rate) = (SampleRate::HZ_48000, TempoMap::default(), FrameRate::Fps25);
        let t = GridValue::Note { value: NoteValue::Sixteenth, dotted: false, triplet: true };
        assert_eq!(t.effective(1000.0, 12.0, 0, sr, &map, rate), GridValue::Note { value: NoteValue::Quarter, dotted: false, triplet: true }); // 8000 < 12000, 16000 ok
    }
}
