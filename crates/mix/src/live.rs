//! Live (audition) notes: notes played into a track's instrument from outside the timeline, for
//! example while clicking a key in the piano roll. They are separate from the notes the clips
//! schedule (`Strip::held`) and sound whether or not the transport runs.
//!
//! Everything here runs on the audio thread: fixed arrays only, no allocation, locking or logging.

use super::{MixEngine, Strip};
use soundcraft_model::TrackId;

/// A live note event for one track's instrument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveNote {
    pub track: TrackId,
    pub pitch: u8,
    pub velocity: u8,
    /// `true` presses the key, `false` releases it.
    pub on: bool,
}

/// Seconds a live note may sound without its release arriving before the engine releases it.
const TIMEOUT_SECS: f32 = 2.0;
/// Default release tail after the last live release, in milliseconds (`options.audition_tail_ms`).
pub const DEFAULT_TAIL_MS: f32 = 300.0;
/// Largest release tail the setting accepts, in milliseconds.
pub const MAX_TAIL_MS: f32 = 2000.0;

/// The live notes sounding on one strip's instrument.
pub(crate) struct LiveHeld {
    /// Samples left before the safety timeout, per pitch (0 = not held).
    left: [u32; 128],
    /// Samples of release tail left after the last live note ended (0 = none).
    tail: u32,
}

impl LiveHeld {
    pub(crate) fn new() -> Self {
        LiveHeld { left: [0; 128], tail: 0 }
    }

    pub(crate) fn any_held(&self) -> bool {
        self.left.iter().any(|l| *l > 0)
    }

    /// A live note is held or its release tail is still ringing.
    pub(crate) fn active(&self) -> bool {
        self.tail > 0 || self.any_held()
    }

    /// Forget everything (the instrument was silenced or replaced).
    pub(crate) fn clear(&mut self) {
        self.left = [0; 128];
        self.tail = 0;
    }
}

fn samples(sample_rate: f32, secs: f32) -> u32 {
    let n = sample_rate * secs;
    if n.is_finite() && n >= 1.0 { n.min(u32::MAX as f32) as u32 } else { 1 }
}

impl MixEngine {
    /// Press or release a live note. Ignored when the track has no strip, no instrument, the
    /// instrument is still loading, or it has failed. A press of a pitch that is already held
    /// releases it first, so the instrument always sees balanced pairs.
    pub fn live_note(&mut self, n: LiveNote) {
        let timeout = samples(self.sample_rate, TIMEOUT_SECS);
        let tail = samples(self.sample_rate, self.audition_tail_ms.clamp(0.0, MAX_TAIL_MS) / 1000.0);
        let Some(strip) = self.strips.get_mut(&n.track) else { return };
        let Strip { instrument, live, .. } = strip;
        let Some(inst) = instrument.as_mut().filter(|i| !i.failed) else { return };
        let pitch = n.pitch.min(127);
        let Some(left) = live.left.get_mut(usize::from(pitch)) else { return };
        if n.on {
            if *left > 0 {
                inst.plugin.note_off(0, pitch);
            }
            inst.plugin.note_on(0, pitch, n.velocity.clamp(1, 127));
            *left = timeout;
            live.tail = 0;
        } else if *left > 0 {
            inst.plugin.note_off(0, pitch);
            *left = 0;
            if !live.any_held() {
                live.tail = tail;
            }
        }
    }

    /// Release every live note on every track.
    pub fn live_all_off(&mut self) {
        let tail = samples(self.sample_rate, self.audition_tail_ms.clamp(0.0, MAX_TAIL_MS) / 1000.0);
        for strip in self.strips.values_mut() {
            let Strip { instrument, live, .. } = strip;
            if !live.any_held() {
                continue;
            }
            if let Some(inst) = instrument.as_mut().filter(|i| !i.failed) {
                for (p, left) in live.left.iter_mut().enumerate() {
                    if *left > 0 {
                        inst.plugin.note_off(0, p as u8);
                        *left = 0;
                    }
                }
            } else {
                live.left = [0; 128];
            }
            live.tail = tail;
        }
    }

    /// True while a live note is held or its release tail rings: the host keeps rendering even
    /// when the transport is stopped.
    pub fn live_active(&self) -> bool {
        self.strips.values().any(|s| s.live.active())
    }

    /// Advance the safety timeouts and release tails by one block.
    pub(crate) fn live_tick(&mut self, frames: usize) {
        let frames = u32::try_from(frames).unwrap_or(u32::MAX);
        let tail = samples(self.sample_rate, self.audition_tail_ms.clamp(0.0, MAX_TAIL_MS) / 1000.0);
        for strip in self.strips.values_mut() {
            let Strip { instrument, live, .. } = strip;
            if !live.active() {
                continue;
            }
            if live.any_held() {
                for (p, left) in live.left.iter_mut().enumerate() {
                    if *left == 0 {
                        continue;
                    }
                    if *left <= frames {
                        *left = 0;
                        if let Some(inst) = instrument.as_mut().filter(|i| !i.failed) {
                            inst.plugin.note_off(0, p as u8);
                        }
                    } else {
                        *left -= frames;
                    }
                }
                if !live.any_held() {
                    live.tail = tail;
                }
            } else {
                live.tail = live.tail.saturating_sub(frames);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use soundcraft_model::Insert;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Ev {
        On(usize, u8, u8),
        Off(usize, u8),
    }

    struct Rec {
        inner: Box<dyn Plugin>,
        heard: Arc<Mutex<Vec<Ev>>>,
    }

    impl Plugin for Rec {
        fn info(&self) -> &'static soundcraft_dsp::PluginInfo {
            self.inner.info()
        }
        fn prepare(&mut self, sr: f32, mb: usize, ch: usize) {
            self.inner.prepare(sr, mb, ch);
        }
        fn reset(&mut self) {
            self.inner.reset();
        }
        fn set_param(&mut self, id: &str, v: f32) -> bool {
            self.inner.set_param(id, v)
        }
        fn param(&self, id: &str) -> Option<f32> {
            self.inner.param(id)
        }
        fn process(&mut self, io: &mut [Vec<f32>], frames: usize) {
            self.inner.process(io, frames);
        }
        fn note_on(&mut self, offset: usize, note: u8, velocity: u8) {
            self.heard.lock().unwrap().push(Ev::On(offset, note, velocity));
        }
        fn note_off(&mut self, offset: usize, note: u8) {
            self.heard.lock().unwrap().push(Ev::Off(offset, note));
        }
    }

    const BLOCK: usize = 512;

    fn block(eng: &mut MixEngine, s: &Session, pos: i64) -> Vec<Vec<f32>> {
        let mut out = vec![vec![0.0f32; BLOCK]; 2];
        eng.render(s, pos, BLOCK, &mut out);
        out
    }

    fn rms(v: &[f32]) -> f32 {
        (v.iter().map(|x| x * x).sum::<f32>() / v.len().max(1) as f32).sqrt()
    }

    /// An instrument track whose instrument records what it is sent. The strip exists after one
    /// render, which the helper does.
    fn recorder() -> (Session, TrackId, MixEngine, Arc<Mutex<Vec<Ev>>>) {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Instrument, ChannelFormat::Stereo, None);
        s.track_mut(t).unwrap().instrument = Some(Insert::new("clap:test.rec"));
        let heard = Arc::new(Mutex::new(Vec::new()));
        let rec = Rec { inner: soundcraft_dsp::create("gain").unwrap(), heard: Arc::clone(&heard) };
        let mut eng = MixEngine::new(48_000.0, BLOCK);
        eng.set_external_instances(true);
        eng.adopt(vec![PreparedInstance { track: t, slot: INSTRUMENT_SLOT, id: "clap:test.rec".into(), ch: 2, plugin: Box::new(rec) }]);
        eng.monitor_only = true;
        block(&mut eng, &s, 0);
        heard.lock().unwrap().clear();
        (s, t, eng, heard)
    }

    fn take(h: &Arc<Mutex<Vec<Ev>>>) -> Vec<Ev> {
        std::mem::take(&mut *h.lock().unwrap())
    }

    fn note(t: TrackId, pitch: u8, on: bool) -> LiveNote {
        LiveNote { track: t, pitch, velocity: 100, on }
    }

    #[test]
    fn live_note_reaches_the_instrument_while_stopped() {
        let (s, t, mut eng, heard) = recorder();
        eng.live_note(note(t, 60, true));
        block(&mut eng, &s, 0);
        assert_eq!(take(&heard), vec![Ev::On(0, 60, 100)]);
        assert!(eng.live_active());
    }

    #[test]
    fn stopped_blocks_do_not_kill_a_held_live_note() {
        let (s, t, mut eng, heard) = recorder();
        eng.live_note(note(t, 60, true));
        for _ in 0..4 {
            block(&mut eng, &s, 0);
        }
        assert_eq!(take(&heard), vec![Ev::On(0, 60, 100)]);
        assert!(eng.live_active());
    }

    #[test]
    fn on_and_off_are_balanced() {
        let (s, t, mut eng, heard) = recorder();
        eng.live_note(note(t, 60, false));
        assert!(take(&heard).is_empty(), "a release for a pitch that is not held sends nothing");
        eng.live_note(note(t, 60, true));
        eng.live_note(note(t, 60, true));
        eng.live_note(note(t, 60, false));
        eng.live_note(note(t, 60, false));
        block(&mut eng, &s, 0);
        assert_eq!(take(&heard), vec![Ev::On(0, 60, 100), Ev::Off(0, 60), Ev::On(0, 60, 100), Ev::Off(0, 60)]);
    }

    #[test]
    fn a_missed_release_times_out_after_two_seconds_and_the_tail_ends() {
        let (s, t, mut eng, heard) = recorder();
        eng.live_note(note(t, 64, true));
        take(&heard);
        for _ in 0..(96_000 / BLOCK + 1) {
            block(&mut eng, &s, 0);
        }
        assert_eq!(take(&heard), vec![Ev::Off(0, 64)]);
        assert!(eng.live_active(), "the release tail still rings");
        for _ in 0..(72_000 / BLOCK + 2) {
            block(&mut eng, &s, 0);
        }
        assert!(!eng.live_active());
        assert!(take(&heard).is_empty());
    }

    #[test]
    fn a_failed_instrument_gets_no_note_off_from_all_off_or_the_timeout() {
        let (s, t, mut eng, heard) = recorder();
        eng.live_note(note(t, 60, true));
        eng.live_note(note(t, 64, true));
        take(&heard);
        if let Some(i) = eng.strips.get_mut(&t).and_then(|st| st.instrument.as_mut()) {
            i.failed = true;
        }
        eng.live_all_off();
        assert!(take(&heard).is_empty(), "nothing is sent to a failed instrument");
        assert!(eng.live_active(), "the table still empties through the tail");
        // The timeout path too.
        let (s2, t2, mut eng2, heard2) = recorder();
        eng2.live_note(note(t2, 60, true));
        take(&heard2);
        if let Some(i) = eng2.strips.get_mut(&t2).and_then(|st| st.instrument.as_mut()) {
            i.failed = true;
        }
        for _ in 0..(96_000 / BLOCK + 1) {
            block(&mut eng2, &s2, 0);
        }
        assert!(take(&heard2).is_empty());
        for _ in 0..(100_000 / BLOCK) {
            block(&mut eng, &s, 0);
        }
        assert!(!eng.live_active());
    }

    #[test]
    fn the_release_tail_defaults_to_300_ms_and_follows_the_setting() {
        let (s, t, mut eng, _heard) = recorder();
        eng.live_note(note(t, 60, true));
        eng.live_note(note(t, 60, false));
        for _ in 0..(12_000 / BLOCK) {
            block(&mut eng, &s, 0);
        }
        assert!(eng.live_active(), "250 ms in, the tail still rings");
        for _ in 0..(6_000 / BLOCK + 1) {
            block(&mut eng, &s, 0);
        }
        assert!(!eng.live_active(), "gone after 300 ms");
        let (mut s, t, mut eng, _heard) = recorder();
        s.edit.values.insert("options.audition_tail_ms".into(), 1000.0);
        block(&mut eng, &s, 0);
        eng.live_note(note(t, 60, true));
        eng.live_note(note(t, 60, false));
        for _ in 0..(40_000 / BLOCK) {
            block(&mut eng, &s, 0);
        }
        assert!(eng.live_active(), "a 1000 ms tail is still ringing at 830 ms");
        for _ in 0..(10_000 / BLOCK + 1) {
            block(&mut eng, &s, 0);
        }
        assert!(!eng.live_active());
    }

    #[test]
    fn all_off_releases_every_held_note() {
        let (s, t, mut eng, heard) = recorder();
        eng.live_note(note(t, 60, true));
        eng.live_note(note(t, 67, true));
        take(&heard);
        eng.live_all_off();
        assert_eq!(take(&heard), vec![Ev::Off(0, 60), Ev::Off(0, 67)]);
        block(&mut eng, &s, 0);
        assert!(take(&heard).is_empty());
    }

    #[test]
    fn reset_clears_the_table() {
        let (s, t, mut eng, heard) = recorder();
        eng.live_note(note(t, 60, true));
        eng.reset();
        assert!(!eng.live_active());
        take(&heard);
        for _ in 0..(100_000 / BLOCK) {
            block(&mut eng, &s, 0);
        }
        assert!(take(&heard).is_empty(), "no stray release after reset");
    }

    #[test]
    fn replacing_the_instrument_clears_the_table() {
        let (mut s, t, mut eng, _heard) = recorder();
        eng.live_note(note(t, 60, true));
        assert!(eng.live_active());
        s.track_mut(t).unwrap().instrument = Some(Insert::new("subtractive_synth"));
        block(&mut eng, &s, 0);
        assert!(!eng.live_active());
    }

    #[test]
    fn unknown_track_no_instrument_and_failed_instrument_are_no_ops() {
        let (mut s, t, mut eng, _heard) = recorder();
        eng.live_note(note(TrackId(9_999), 60, true));
        assert!(!eng.live_active());
        // Instrument still loading: the external instance has not been adopted.
        let t2 = s.add_track(TrackKind::Instrument, ChannelFormat::Stereo, None);
        s.track_mut(t2).unwrap().instrument = Some(Insert::new("clap:test.missing"));
        block(&mut eng, &s, 0);
        eng.live_note(note(t2, 60, true));
        assert!(!eng.live_active());
        // A failed instrument.
        if let Some(i) = eng.strips.get_mut(&t).and_then(|st| st.instrument.as_mut()) {
            i.failed = true;
        }
        eng.live_note(note(t, 60, true));
        assert!(!eng.live_active());
        eng.live_all_off();
        eng.live_note(note(t, 200, false));
    }

    #[test]
    fn built_in_synth_sounds_from_a_live_note_while_stopped() {
        let mut s = Session::default();
        let t = s.add_track(TrackKind::Instrument, ChannelFormat::Stereo, None);
        s.track_mut(t).unwrap().instrument = Some(Insert::new("subtractive_synth"));
        let mut eng = MixEngine::new(48_000.0, BLOCK);
        eng.monitor_only = true;
        block(&mut eng, &s, 0);
        for _ in 0..4 {
            assert_eq!(rms(&block(&mut eng, &s, 0)[0]), 0.0, "silent without a live note");
        }
        eng.live_note(note(t, 60, true));
        let mut last = 0.0;
        for _ in 0..4 {
            last = rms(&block(&mut eng, &s, 0)[0]);
        }
        assert!(last > 1e-4, "the synth should sound at a fixed position while stopped: {last}");
        eng.live_note(note(t, 60, false));
        assert!(eng.live_active());
    }

    #[test]
    fn playing_clip_notes_are_unchanged_by_live_notes() {
        let (mut s, t, mut eng, heard) = recorder();
        eng.monitor_only = false;
        let mut seq = soundcraft_midi::Sequence::default();
        // 24 ticks = 600 samples: 88 samples into the second block.
        seq.notes.push(soundcraft_midi::Note { pitch: 72, velocity: 90, release_velocity: 64, channel: 0, start: 24, length: 480 });
        let cid = s.new_clip_id();
        s.track_mut(t).unwrap().playlist_mut().unwrap().clips.push(Clip::midi(cid, "m", 0, 48_000, seq));
        block(&mut eng, &s, 0);
        take(&heard);
        eng.live_note(note(t, 60, true));
        block(&mut eng, &s, BLOCK as i64);
        assert_eq!(take(&heard), vec![Ev::On(0, 60, 100), Ev::On(88, 72, 90)]);
        assert!(eng.strips.get(&t).is_some_and(|st| st.held == vec![72]));
    }
}
