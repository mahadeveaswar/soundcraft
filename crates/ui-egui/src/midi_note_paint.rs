//! Piano-roll note painting: velocity colours (a three-stop gradient from the theme) and pitch
//! name labels. The single-colour look stays available behind the `midi.single_colour` view flag.

use crate::theme::{Tokens, regular};
use egui::{Align2, Color32, Painter, Rect, Stroke, StrokeKind};

/// Smallest note width (points) that gets a pitch label.
pub const LABEL_MIN_W: f32 = 22.0;
/// Smallest row height (points) that gets a pitch label.
pub const LABEL_MIN_ROW_H: f32 = 9.0;

/// Colour for a velocity: low blends into middle at the halfway point, middle into high at 127.
pub fn vel_color(velocity: u8, t: &Tokens) -> Color32 {
    let f = f32::from(velocity.clamp(1, 127) - 1) / 126.0;
    if f < 0.5 { t.midi_vel_lo.lerp_to_gamma(t.midi_vel_mid, f * 2.0) } else { t.midi_vel_mid.lerp_to_gamma(t.midi_vel_hi, (f - 0.5) * 2.0) }
}

/// The pitch name printed on a note ("C3" is MIDI 60).
pub fn note_label(pitch: u8) -> String {
    soundcraft_midi::note_name(pitch)
}

/// Whether a note rectangle is big enough to carry its label.
pub fn label_fits(width: f32, row_h: f32) -> bool {
    width >= LABEL_MIN_W && row_h >= LABEL_MIN_ROW_H
}

/// Draw one note. `single` is `Some(track colour)` for the single-colour look (the original
/// shading by velocity, selected notes white); `None` uses the velocity gradient with a bright
/// outline when selected. A muted note is hollow: an outline at 40 percent strength.
#[allow(clippy::too_many_arguments)]
pub fn paint_note(p: &Painter, r: Rect, pitch: u8, velocity: u8, selected: bool, single: Option<Color32>, muted: bool, t: &Tokens) {
    let (fill, stroke) = match single {
        Some(base) => {
            let k = 0.45 + 0.55 * f32::from(velocity) / 127.0;
            let fill = if selected {
                Color32::WHITE
            } else {
                Color32::from_rgb((f32::from(base.r()) * k) as u8, (f32::from(base.g()) * k) as u8, (f32::from(base.b()) * k) as u8)
            };
            (fill, Stroke::new(1.0, Color32::BLACK))
        }
        None => (vel_color(velocity, t), if selected { Stroke::new(2.0, Color32::WHITE) } else { Stroke::new(1.0, Color32::BLACK) }),
    };
    if muted {
        let line = if selected { Color32::WHITE } else { fill };
        p.rect_stroke(r, 2.0, Stroke::new(1.5, line.gamma_multiply(0.4)), StrokeKind::Inside);
    } else {
        p.rect(r, 2.0, fill, stroke, StrokeKind::Inside);
    }
    if label_fits(r.width(), r.height() + 2.0) {
        let ink = if muted {
            fill.gamma_multiply(0.4)
        } else if selected && single.is_some() {
            Color32::from_rgb(20, 20, 20)
        } else {
            Color32::WHITE
        };
        p.with_clip_rect(r.shrink(1.0)).text(egui::pos2(r.min.x + 3.0, r.center().y), Align2::LEFT_CENTER, note_label(pitch), regular(8.0), ink);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_colours_differ_and_run_from_cool_to_hot() {
        let t = Tokens::DARK;
        assert_ne!(vel_color(1, &t), vel_color(127, &t));
        assert_eq!(vel_color(1, &t), t.midi_vel_lo);
        assert_eq!(vel_color(127, &t), t.midi_vel_hi);
        // Red rises monotonically from soft to hard with these stops.
        let mut last = 0;
        for v in (1..=127).step_by(6) {
            let r = vel_color(v, &t).r();
            assert!(r + 1 >= last, "red must not fall: v={v} r={r} last={last}");
            last = r;
        }
        assert_eq!(vel_color(0, &t), vel_color(1, &t), "out-of-range velocity clamps");
    }

    #[test]
    fn labels_and_fit_rules() {
        assert_eq!(note_label(60), "C3");
        assert!(label_fits(22.0, 9.0));
        assert!(!label_fits(21.0, 9.0));
        assert!(!label_fits(40.0, 8.0));
    }
}
