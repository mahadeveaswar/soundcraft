//! The piano-roll keyboard gutter: key shapes, octave labels on every C, and a tint for the
//! pitches sounding at the playhead (on the key and across its row).

use crate::theme::{Tokens, regular};
use egui::{Align2, Color32, Painter, Rect, pos2, vec2};
use soundcraft_midi::Note;

/// Which of the 128 pitches have a note sounding at `tick` (a note sounds from its start up to
/// but not including its end).
pub fn active_pitches(notes: &[Note], tick: i64) -> [bool; 128] {
    let mut on = [false; 128];
    for n in notes {
        if n.start <= tick
            && tick < n.start.saturating_add(n.length)
            && let Some(slot) = on.get_mut(usize::from(n.pitch))
        {
            *slot = true;
        }
    }
    on
}

/// Shade laid over the rows that are not in the clip's scale.
pub const OUT_OF_SCALE: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 96);

/// Background of a roll row.
pub fn row_fill(black: bool, active: bool) -> Color32 {
    let t = Tokens::current();
    let base = if black { t.roll_row_black } else { t.roll_row_white };
    if active { base.lerp_to_gamma(Tokens::current().roll_accent, 0.16) } else { base }
}

/// One key of the gutter at vertical position `y`. `key_w` is the full gutter width.
pub fn draw_key(p: &Painter, gutter_left: f32, key_w: f32, y: f32, row_h: f32, pitch: i32, black: bool, active: bool) {
    let key = Rect::from_min_size(pos2(gutter_left, y), vec2(key_w - 2.0, row_h - 1.0));
    let base = if black { Tokens::current().roll_key_black } else { Tokens::current().roll_key_white };
    p.rect_filled(key, 1.0, if active { base.lerp_to_gamma(Tokens::current().roll_accent, 0.85) } else { base });
    if pitch.rem_euclid(12) == 0 && (0..=127).contains(&pitch) && row_h >= 7.0 {
        p.text(
            pos2(key.max.x - 3.0, key.center().y),
            Align2::RIGHT_CENTER,
            soundcraft_midi::note_name(pitch as u8),
            regular(8.0),
            Tokens::current().roll_key_label,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note(pitch: u8, start: i64, length: i64) -> Note {
        Note { pitch, velocity: 90, release_velocity: 64, channel: 0, start, length }
    }

    #[test]
    fn active_pitches_follow_the_playhead() {
        let notes = [note(60, 0, 480), note(64, 480, 480), note(67, 0, 960), note(72, 2000, 10)];
        let at = |t| active_pitches(&notes, t);
        assert!(at(0)[60] && at(0)[67] && !at(0)[64]);
        assert!(!at(480)[60], "a note ends before its end tick");
        assert!(at(480)[64] && at(959)[67] && !at(960)[67]);
        assert!(at(2005)[72] && !at(1999)[72]);
        assert!(!at(-5).iter().any(|a| *a), "nothing sounds before the clip");
        assert!(active_pitches(&[], 0).iter().all(|a| !a));
    }

    #[test]
    fn huge_lengths_do_not_overflow() {
        assert!(active_pitches(&[note(10, 5, i64::MAX)], 100)[10]);
    }
}
