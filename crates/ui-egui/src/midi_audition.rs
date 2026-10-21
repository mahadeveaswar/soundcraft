//! Note audition in the piano roll: sound the instrument while the user clicks, draws or drags a
//! note, or presses a key in the gutter. A small state machine decides which `midi.audition`
//! commands to send; it guarantees that every note-on is followed by a note-off.

use crate::SoundApp;
use serde_json::json;
use soundcraft_model::TrackId;

/// A plain click holds the note this long (seconds) before it is released.
pub const CLICK_HOLD: f64 = 0.15;
const DEFAULT_VELOCITY: u8 = 100;

/// One `midi.audition` command to send.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Msg {
    pub track: TrackId,
    pub pitch: u8,
    pub velocity: u8,
    pub on: bool,
}

/// The note currently sounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sounding {
    pub track: TrackId,
    pub pitch: u8,
    pub velocity: u8,
}

/// Audition state kept in the piano-roll state (`app.midi.audition`).
#[derive(Debug, Clone, Default)]
pub struct Audition {
    pub sounding: Option<Sounding>,
    /// When a plain click note is released (time in seconds), if one is pending.
    release_at: Option<f64>,
}

impl Audition {
    fn off(&mut self) -> Option<Msg> {
        self.release_at = None;
        self.sounding.take().map(|s| Msg { track: s.track, pitch: s.pitch, velocity: s.velocity, on: false })
    }

    /// Sound `pitch` on `track` (releasing any note that is already sounding first). With
    /// `hold`, the note is released by `tick` that long after `now`; without it the caller
    /// releases it with `stop`. Nothing is sent when `enabled` is false.
    pub fn start(&mut self, enabled: bool, track: TrackId, pitch: i32, velocity: u8, hold: Option<f64>, now: f64) -> Vec<Msg> {
        if !enabled {
            return Vec::new();
        }
        let mut out: Vec<Msg> = self.off().into_iter().collect();
        let pitch = pitch.clamp(0, 127) as u8;
        let velocity = velocity.clamp(1, 127);
        self.sounding = Some(Sounding { track, pitch, velocity });
        self.release_at = hold.map(|h| now + h);
        out.push(Msg { track, pitch, velocity, on: true });
        out
    }

    /// Release the sounding note, if any.
    pub fn stop(&mut self) -> Vec<Msg> {
        self.off().into_iter().collect()
    }

    /// The sounding note follows the pointer to another pitch: off for the old pitch, on for the
    /// new. Nothing happens when the pitch is unchanged or no note is sounding.
    pub fn moved(&mut self, pitch: i32) -> Vec<Msg> {
        let pitch = pitch.clamp(0, 127) as u8;
        match self.sounding {
            Some(s) if s.pitch != pitch => self.start(true, s.track, i32::from(pitch), s.velocity, None, 0.0),
            _ => Vec::new(),
        }
    }

    /// Per-frame safety: release a click note whose time is up, and any held note that has no
    /// pointer down (a lost release), when the editor is gone, or when auditioning was turned off.
    pub fn tick(&mut self, now: f64, pointer_down: bool, editor_shown: bool, enabled: bool) -> Vec<Msg> {
        if self.sounding.is_none() {
            return Vec::new();
        }
        let timed_out = self.release_at.is_some_and(|t| now >= t);
        let orphan = self.release_at.is_none() && !pointer_down;
        if timed_out || orphan || !editor_shown || !enabled { self.stop() } else { Vec::new() }
    }
}

fn send(app: &mut SoundApp, msgs: Vec<Msg>) {
    for m in msgs {
        let _ = app.run("midi.audition", json!({"track": m.track.0, "pitch": m.pitch, "velocity": m.velocity, "on": m.on}));
    }
}

/// Start auditioning a note (see [`Audition::start`]); `click` holds it for a short time.
pub fn start(app: &mut SoundApp, ctx: &egui::Context, track: TrackId, pitch: i32, velocity: u8, click: bool) {
    let now = ctx.input(|i| i.time);
    let msgs = app.midi.audition.start(app.ui.audition, track, pitch, velocity, click.then_some(CLICK_HOLD), now);
    if click && !msgs.is_empty() {
        ctx.request_repaint_after(std::time::Duration::from_secs_f64(CLICK_HOLD));
    }
    send(app, msgs);
}

/// Start with the default velocity (pencil, gutter and double-click).
pub fn start_default(app: &mut SoundApp, ctx: &egui::Context, track: TrackId, pitch: i32, click: bool) {
    start(app, ctx, track, pitch, DEFAULT_VELOCITY, click);
}

/// Release the sounding note.
pub fn stop(app: &mut SoundApp) {
    let msgs = app.midi.audition.stop();
    send(app, msgs);
}

/// Move the sounding note to another pitch.
pub fn moved(app: &mut SoundApp, pitch: i32) {
    let msgs = app.midi.audition.moved(pitch);
    send(app, msgs);
}

/// Call once per frame after the panels: releases notes whose click time is up, whose pointer
/// went away, or whose editor was closed. `app.midi.shown_frame` is set by the editor each frame.
pub fn frame_end(app: &mut SoundApp, ctx: &egui::Context) {
    if app.midi.audition.sounding.is_none() {
        return;
    }
    let (now, down) = ctx.input(|i| (i.time, i.pointer.any_down()));
    let shown = app.midi.shown_frame == Some(ctx.cumulative_frame_nr());
    let msgs = app.midi.audition.tick(now, down, shown, app.ui.audition);
    if app.midi.audition.sounding.is_some() {
        ctx.request_repaint_after(std::time::Duration::from_millis(50));
    }
    send(app, msgs);
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: TrackId = TrackId(7);

    /// Count of ons minus offs per pitch over a message list.
    fn balance(msgs: &[Msg]) -> std::collections::HashMap<u8, i32> {
        let mut m = std::collections::HashMap::new();
        for x in msgs {
            *m.entry(x.pitch).or_insert(0) += if x.on { 1 } else { -1 };
        }
        m
    }

    fn balanced(msgs: &[Msg]) -> bool {
        balance(msgs).values().all(|v| *v == 0)
    }

    #[test]
    fn a_plain_click_sounds_then_releases_after_the_hold_time() {
        let mut a = Audition::default();
        let mut log = a.start(true, T, 60, 90, Some(CLICK_HOLD), 1.0);
        assert_eq!(log.len(), 1);
        assert!(log[0].on && log[0].pitch == 60 && log[0].velocity == 90);
        // The pointer is already up, but the note is held for the click time.
        assert!(a.tick(1.1, false, true, true).is_empty());
        log.extend(a.tick(1.2, false, true, true));
        assert!(a.sounding.is_none());
        assert!(balanced(&log) && log.len() == 2);
    }

    #[test]
    fn a_drag_holds_until_stop_and_the_safety_covers_a_lost_release() {
        let mut a = Audition::default();
        let mut log = a.start(true, T, 64, 100, None, 0.0);
        assert!(a.tick(5.0, true, true, true).is_empty(), "pointer still down: keeps sounding");
        log.extend(a.stop());
        assert!(balanced(&log));
        assert!(a.stop().is_empty(), "a second stop sends nothing");
        // A lost release: no pointer down, nothing pending.
        let mut log = a.start(true, T, 64, 100, None, 0.0);
        log.extend(a.tick(0.1, false, true, true));
        assert!(balanced(&log) && a.sounding.is_none());
    }

    #[test]
    fn a_vertical_drag_sends_off_for_the_old_pitch_then_on_for_the_new() {
        let mut a = Audition::default();
        let mut log = a.start(true, T, 60, 100, None, 0.0);
        assert!(a.moved(60).is_empty(), "same pitch: nothing");
        let step = a.moved(62);
        assert_eq!(step.len(), 2);
        assert!(!step[0].on && step[0].pitch == 60 && step[1].on && step[1].pitch == 62);
        log.extend(step);
        log.extend(a.moved(55));
        log.extend(a.stop());
        assert!(balanced(&log), "{log:?}");
        assert!(a.moved(70).is_empty(), "nothing sounding: moving does nothing");
    }

    #[test]
    fn pencil_and_gutter_drags_balance() {
        let mut a = Audition::default();
        let mut log = a.start(true, T, 72, 100, None, 0.0);
        log.extend(a.stop());
        // Gutter: press on a key, slide along several keys, release.
        log.extend(a.start(true, T, 50, 100, None, 0.0));
        for p in [51, 52, 52, 53, 52] {
            log.extend(a.moved(p));
        }
        log.extend(a.tick(0.5, false, true, true));
        assert!(balanced(&log), "{log:?}");
        assert!(a.sounding.is_none());
    }

    #[test]
    fn closing_the_editor_or_switching_the_toggle_off_releases_the_note() {
        let mut a = Audition::default();
        let mut log = a.start(true, T, 60, 100, None, 0.0);
        log.extend(a.tick(0.1, true, false, true));
        assert!(balanced(&log) && a.sounding.is_none(), "editor closed");
        let mut log = a.start(true, T, 61, 100, Some(CLICK_HOLD), 0.0);
        log.extend(a.tick(0.01, true, true, false));
        assert!(balanced(&log) && a.sounding.is_none(), "toggled off");
    }

    #[test]
    fn a_new_note_releases_the_previous_one_first() {
        let mut a = Audition::default();
        let mut log = a.start(true, T, 60, 100, None, 0.0);
        log.extend(a.start(true, T, 60, 100, None, 0.0));
        log.extend(a.stop());
        assert_eq!(log.iter().map(|m| m.on).collect::<Vec<_>>(), [true, false, true, false]);
    }

    #[test]
    fn nothing_is_sent_when_auditioning_is_off() {
        let mut a = Audition::default();
        assert!(a.start(false, T, 60, 100, None, 0.0).is_empty());
        assert!(a.sounding.is_none());
        assert!(a.moved(61).is_empty());
        assert!(a.tick(1.0, false, true, false).is_empty());
        assert!(a.stop().is_empty());
    }

    #[test]
    fn pitch_and_velocity_are_clamped() {
        let mut a = Audition::default();
        let m = a.start(true, T, 300, 0, None, 0.0);
        assert_eq!((m[0].pitch, m[0].velocity), (127, 1));
        let m = a.start(true, T, -4, 255, None, 0.0);
        assert_eq!((m[1].pitch, m[1].velocity), (0, 127));
    }
}
