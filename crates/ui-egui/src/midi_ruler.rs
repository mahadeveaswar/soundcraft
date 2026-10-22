//! The piano-roll ruler: a coloured band spanning the clip with its name, bar numbers below it,
//! and the stronger bar lines through the roll.

use crate::theme::{Tokens, bold, regular};
use egui::{Align2, Color32, Rect, Stroke, Ui, pos2};
use soundcraft_model::Session;

/// Height of the ruler (clip band on top, bar numbers under it).
pub const RULER_H: f32 = 26.0;
const BAND_H: f32 = 12.0;

/// Which bars get a number: every `n`th so numbers stay about 30 points apart.
pub fn label_every(bar_px: f32) -> i64 {
    ((30.0 / bar_px.max(1.0)).ceil() as i64).max(1)
}

/// What to draw: the clip's name and colour, its length in ticks, and where its tick 0 sits.
pub struct RulerClip<'a> {
    pub name: &'a str,
    pub color: Color32,
    pub base_tick: i64,
    pub len_ticks: i64,
    pub view_start: f64,
}

/// Paint the ruler and the bar lines through `roll`. `x_of` maps a clip tick to screen x.
pub fn draw(ui: &Ui, ruler: Rect, roll: Rect, s: &Session, clip: &RulerClip, x_of: &dyn Fn(i64) -> f32) {
    let t = Tokens::current();
    let rp = ui.painter().with_clip_rect(ruler);
    let roll_p = ui.painter().with_clip_rect(roll);
    rp.rect_filled(ruler, 0.0, t.panel_bg2);
    // The clip band over its span, with the name at its left edge.
    let band_y = Rect::from_min_max(ruler.min, pos2(ruler.max.x, ruler.min.y + BAND_H));
    let x0 = x_of(0).max(ruler.min.x);
    let x1 = x_of(clip.len_ticks).min(ruler.max.x);
    if x1 > x0 {
        let band = Rect::from_min_max(pos2(x0, band_y.min.y + 1.0), pos2(x1, band_y.max.y));
        rp.rect_filled(band, 0.0, clip.color.gamma_multiply(0.8));
        let ink =
            if clip.color.r() as u32 + clip.color.g() as u32 + clip.color.b() as u32 > 420 { Tokens::current().ink_dark } else { Color32::WHITE };
        rp.with_clip_rect(band).text(pos2(band.min.x + 4.0, band.center().y), Align2::LEFT_CENTER, clip.name, bold(9.5), ink);
    }
    let numbers_y = ruler.min.y + BAND_H + (ruler.height() - BAND_H) / 2.0;
    let first_bar = s.tempo.bar_beat_at_tick(clip.base_tick.saturating_add(clip.view_start.floor() as i64)).bar;
    let bar_px = (x_of(s.tempo.bar_start_tick(first_bar + 1) - clip.base_tick) - x_of(s.tempo.bar_start_tick(first_bar) - clip.base_tick)).max(1.0);
    let every = label_every(bar_px);
    for bar in first_bar..first_bar.saturating_add(4096) {
        let tk = s.tempo.bar_start_tick(bar) - clip.base_tick;
        let x = x_of(tk);
        if x > roll.max.x || tk > clip.len_ticks {
            break;
        }
        if x < roll.min.x - 1.0 {
            continue;
        }
        roll_p.line_segment([pos2(x, roll.min.y), pos2(x, roll.max.y)], Stroke::new(1.0, Tokens::current().roll_bar));
        rp.line_segment([pos2(x, ruler.min.y + BAND_H), pos2(x, ruler.max.y)], Stroke::new(1.0, t.text_dim));
        if (bar - first_bar) % every == 0 {
            rp.text(pos2(x + 3.0, numbers_y), Align2::LEFT_CENTER, bar.to_string(), regular(9.5), t.text_dim);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_spacing_grows_as_bars_shrink() {
        assert_eq!(label_every(100.0), 1);
        assert_eq!(label_every(30.0), 1);
        assert_eq!(label_every(10.0), 3);
        assert_eq!(label_every(0.0), 30);
    }
}
