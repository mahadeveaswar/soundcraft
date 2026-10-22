//! Glossy, rounded painting of clip bodies, clip name bars and header colour strips.
//!
//! The drawing functions ([`body`], [`name_bar`], [`strip`]) take a [`Painter`], the rectangle and a
//! [`ClipPaint`] describing the colours and the theme tokens (`clip_gloss`, `clip_corner`,
//! `header_corner`). They draw only the shape (fill, gloss, highlight, outline); waveforms, notes
//! and text are drawn by the caller afterwards. The maths ([`corner`], [`gloss_stops`],
//! [`is_flat`], [`lighten`], [`darken`]) is pure so it is tested without a painter.
//!
//! A glossy fill is one small gradient mesh built in a reused thread-local scratch buffer, so a
//! clip costs no per-call vertex allocation beyond the mesh handed to egui. A flat fill is used
//! for clips narrower than 3 pt or shorter than 6 pt, for muted clips, when gloss is 0 and in a
//! lane with more than [`CROWDED_LANE`] visible clips.

use crate::theme::Tokens;
use egui::epaint::CornerRadius;
use egui::{Color32, Mesh, Painter, Pos2, Rect, Shape, Stroke, StrokeKind, Vec2, pos2};
use std::cell::RefCell;

/// A lane with more visible clips than this draws flat fills (no gradient, no highlight).
pub const CROWDED_LANE: usize = 400;
/// Narrower than this (points): flat, square fill.
pub const MIN_GLOSS_WIDTH: f32 = 3.0;
/// Shorter than this (points): flat, square fill.
pub const MIN_GLOSS_HEIGHT: f32 = 6.0;

/// How a clip is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipState {
    Normal,
    /// Outline in the text colour at 1.5 pt.
    Selected,
    /// Muted or dimmed: base colour dimmed, never glossy.
    Muted,
}

/// Everything the painters need besides the rectangle.
#[derive(Clone, Copy, Debug)]
pub struct ClipPaint {
    /// Fill colour (from `theme::clip_colors`: the body or the bar colour).
    pub base: Color32,
    /// Outline colour for flat bodies (from `theme::clip_outline`).
    pub outline: Color32,
    /// Gloss strength 0..=1 (`Tokens::clip_gloss`; 0 is flat).
    pub gloss: f32,
    /// Corner radius token in points (`Tokens::clip_corner`, or `Tokens::header_corner` for [`strip`]).
    pub radius: u8,
    pub state: ClipState,
    /// Visible clips in this lane; above [`CROWDED_LANE`] the fill is flat.
    pub lane_clips: usize,
}

impl ClipPaint {
    /// Build from the current theme tokens for a clip body or name bar.
    pub fn from_tokens(t: &Tokens, base: Color32, outline: Color32, state: ClipState, lane_clips: usize) -> Self {
        Self { base, outline, gloss: t.clip_gloss, radius: t.clip_corner, state, lane_clips }
    }

    /// Build from the current theme tokens for a header colour strip (uses `header_corner`).
    pub fn strip_from_tokens(t: &Tokens, base: Color32, state: ClipState, lane_clips: usize) -> Self {
        Self { base, outline: base, gloss: t.clip_gloss, radius: t.header_corner, state, lane_clips }
    }
}

/// The corner radius actually used for a rectangle of `size` and token `token` (points): at most a
/// quarter of the height and half the width; 0 for tiny rectangles.
pub fn corner(size: Vec2, token: u8) -> u8 {
    if !(size.x >= MIN_GLOSS_WIDTH && size.y >= MIN_GLOSS_HEIGHT) {
        return 0;
    }
    let r = f32::from(token).min(size.y / 4.0).min(size.x / 2.0);
    r.max(0.0).floor() as u8
}

/// Whether a clip of `size` is drawn with a plain fill.
pub fn is_flat(size: Vec2, gloss: f32, state: ClipState, lane_clips: usize) -> bool {
    gloss <= 0.0
        || !gloss.is_finite()
        || state == ClipState::Muted
        || lane_clips > CROWDED_LANE
        || !(size.x >= MIN_GLOSS_WIDTH && size.y >= MIN_GLOSS_HEIGHT)
}

/// Move `c` towards white by `k` (0..=1), keeping alpha.
pub fn lighten(c: Color32, k: f32) -> Color32 {
    mix(c, 255, k)
}

/// Move `c` towards black by `k` (0..=1), keeping alpha.
pub fn darken(c: Color32, k: f32) -> Color32 {
    mix(c, 0, k)
}

fn mix(c: Color32, to: u8, k: f32) -> Color32 {
    let k = if k.is_finite() { k.clamp(0.0, 1.0) } else { 0.0 };
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    let ch = |v: u8| (f32::from(v) + (f32::from(to) - f32::from(v)) * k).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgba_unmultiplied(ch(r), ch(g), ch(b), a)
}

/// The vertical gradient colours, top to bottom, at 0, 45 %, 75 % and 100 % of the height: a
/// lighter top, the base through the middle, a slightly darker bottom. Gloss 0 gives four bases.
pub fn gloss_stops(base: Color32, gloss: f32) -> [Color32; 4] {
    let g = if gloss.is_finite() { gloss.clamp(0.0, 1.0) } else { 0.0 };
    [lighten(base, 0.18 * g), base, base, darken(base, 0.12 * g)]
}

/// Positions of [`gloss_stops`] as fractions of the height.
const STOP_AT: [f32; 4] = [0.0, 0.45, 0.75, 1.0];

fn color_at(stops: &[Color32; 4], t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    for i in 0..3 {
        let (a, b) = (STOP_AT[i], STOP_AT[i + 1]);
        if t <= b {
            let k = if b > a { (t - a) / (b - a) } else { 0.0 };
            return stops[i].lerp_to_gamma(stops[i + 1], k);
        }
    }
    stops[3]
}

/// The outline of a glossy opaque body: a darker shade of the base. Translucent and flat bodies
/// keep the caller's outline.
pub fn gloss_outline(base: Color32, outline: Color32, gloss: f32) -> Color32 {
    if gloss > 0.0 && base.a() == 255 { darken(base, 0.45) } else { outline }
}

thread_local! {
    /// Reused vertex/index storage for gradient meshes.
    static SCRATCH: RefCell<Mesh> = RefCell::new(Mesh::default());
}

/// Fill `r` with the vertical gradient; the top and bottom rows are inset by the chamfers `ct` and
/// `cb` (points) so the corners stay visually round under the outline.
fn gradient(painter: &Painter, r: Rect, stops: &[Color32; 4], ct: f32, cb: f32) {
    let h = r.height().max(1.0);
    let rows = [
        (r.min.y, ct, 0.0),
        (r.min.y + ct, 0.0, ct / h),
        (r.min.y + h * STOP_AT[1], 0.0, STOP_AT[1]),
        (r.min.y + h * STOP_AT[2], 0.0, STOP_AT[2]),
        (r.max.y - cb, 0.0, 1.0 - cb / h),
        (r.max.y, cb, 1.0),
    ];
    let build = |m: &mut Mesh| {
        m.clear();
        for &(y, inset, t) in &rows {
            let c = color_at(stops, t);
            m.colored_vertex(pos2(r.min.x + inset, y), c);
            m.colored_vertex(pos2(r.max.x - inset, y), c);
        }
        for i in 0..rows.len() as u32 - 1 {
            let k = i * 2;
            m.add_triangle(k, k + 1, k + 2);
            m.add_triangle(k + 1, k + 3, k + 2);
        }
    };
    let shape = SCRATCH.with(|cell| match cell.try_borrow_mut() {
        Ok(mut m) => {
            build(&mut m);
            Shape::mesh(m.clone())
        }
        Err(_) => {
            let mut m = Mesh::default();
            build(&mut m);
            Shape::mesh(m)
        }
    });
    painter.add(shape);
}

fn highlight(painter: &Painter, r: Rect, p: &ClipPaint, radius: f32) {
    let (x0, x1) = (r.min.x + radius.max(1.0), r.max.x - radius.max(1.0));
    if x1 <= x0 {
        return;
    }
    let [lr, lg, lb, _] = lighten(p.base, 0.35).to_srgba_unmultiplied();
    let alpha = (90.0 * p.gloss.clamp(0.0, 1.0)).round() as u8;
    let line = Color32::from_rgba_unmultiplied(lr, lg, lb, alpha);
    let y = r.min.y + 1.0;
    painter.line_segment([Pos2::new(x0, y), Pos2::new(x1, y)], Stroke::new(1.0, line));
}

/// Effective colours and gloss after the state rules (muted: dimmed base, no gloss).
fn effective(p: &ClipPaint) -> (Color32, f32) {
    if p.state == ClipState::Muted { (p.base.gamma_multiply(0.45), 0.0) } else { (p.base, p.gloss) }
}

/// Draw a clip body: rounded fill, gradient and top highlight when glossy, then the outline.
pub fn body(painter: &Painter, r: Rect, p: &ClipPaint) {
    let (base, gloss) = effective(p);
    let flat = is_flat(r.size(), p.gloss, p.state, p.lane_clips);
    let rad = corner(r.size(), p.radius);
    if r.width() < MIN_GLOSS_WIDTH || r.height() < MIN_GLOSS_HEIGHT {
        painter.rect_filled(r, 0.0, base);
        return;
    }
    let cr = CornerRadius::same(rad);
    let selected = p.state == ClipState::Selected;
    let (stroke_color, stroke_w) = if selected { (Tokens::current().text, 1.5) } else { (gloss_outline(base, p.outline, gloss), 1.0) };
    if flat {
        painter.rect(r, cr, base, Stroke::new(stroke_w, if selected { stroke_color } else { p.outline }), StrokeKind::Inside);
        return;
    }
    if base.a() == 255 {
        painter.rect_filled(r, cr, base);
    }
    let c = f32::from(rad) * 0.3;
    gradient(painter, r, &gloss_stops(base, gloss), c, c);
    highlight(painter, r, &ClipPaint { base, gloss, ..*p }, f32::from(rad));
    painter.rect_stroke(r, cr, Stroke::new(stroke_w, stroke_color), StrokeKind::Inside);
}

/// Draw a clip name bar (the bar over the top of a clip): rounded top corners only, same gradient,
/// no outline. `p.radius` is the clip corner token.
pub fn name_bar(painter: &Painter, r: Rect, p: &ClipPaint) {
    let (base, gloss) = effective(p);
    let rad = corner(r.size(), p.radius);
    let cr = CornerRadius { nw: rad, ne: rad, sw: 0, se: 0 };
    if is_flat(r.size(), p.gloss, p.state, p.lane_clips) {
        painter.rect_filled(r, cr, base);
        return;
    }
    if base.a() == 255 {
        painter.rect_filled(r, cr, base);
    }
    gradient(painter, r, &gloss_stops(base, gloss), f32::from(rad) * 0.3, 0.0);
}

/// Draw a track header colour strip: fully rounded with radius `min(token, w/2)`, same gradient.
/// Pass a [`ClipPaint`] made with [`ClipPaint::strip_from_tokens`].
pub fn strip(painter: &Painter, r: Rect, p: &ClipPaint) {
    let (base, gloss) = effective(p);
    let rad = if r.width() < MIN_GLOSS_WIDTH || r.height() < MIN_GLOSS_HEIGHT { 0 } else { p.radius.min((r.width() / 2.0).floor() as u8) };
    let cr = CornerRadius::same(rad);
    if is_flat(r.size(), p.gloss, p.state, p.lane_clips) {
        painter.rect_filled(r, cr, base);
        return;
    }
    if base.a() == 255 {
        painter.rect_filled(r, cr, base);
    }
    let c = f32::from(rad) * 0.3;
    gradient(painter, r, &gloss_stops(base, gloss), c, c);
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    #[test]
    fn corner_shrinks_with_the_rectangle_and_vanishes_when_tiny() {
        assert_eq!(corner(vec2(100.0, 40.0), 4), 4);
        assert_eq!(corner(vec2(100.0, 8.0), 4), 2);
        assert_eq!(corner(vec2(5.0, 40.0), 4), 2);
        assert_eq!(corner(vec2(100.0, 5.9), 4), 0);
        assert_eq!(corner(vec2(2.9, 40.0), 4), 0);
        assert_eq!(corner(vec2(100.0, 40.0), 0), 0);
        assert_eq!(corner(vec2(f32::NAN, 40.0), 4), 0);
    }

    #[test]
    fn stops_run_light_to_dark_and_flatten_at_zero_gloss() {
        let base = Color32::from_rgb(100, 120, 140);
        let s = gloss_stops(base, 1.0);
        let sum = |c: Color32| u32::from(c.r()) + u32::from(c.g()) + u32::from(c.b());
        assert!(sum(s[0]) > sum(s[1]) && sum(s[1]) == sum(s[2]) && sum(s[2]) > sum(s[3]));
        assert_eq!(gloss_stops(base, 0.0), [base; 4]);
        let soft = gloss_stops(base, 0.5);
        assert!(sum(soft[0]) < sum(s[0]) && sum(soft[0]) > sum(base));
        assert_eq!(gloss_stops(base, f32::NAN), [base; 4]);
        assert_eq!(gloss_stops(base.gamma_multiply(0.5), 1.0)[0].a(), base.gamma_multiply(0.5).a());
    }

    #[test]
    fn flat_fallbacks() {
        let big = vec2(100.0, 40.0);
        assert!(!is_flat(big, 1.0, ClipState::Normal, 10));
        assert!(!is_flat(big, 0.3, ClipState::Selected, CROWDED_LANE));
        assert!(is_flat(big, 0.0, ClipState::Normal, 1));
        assert!(is_flat(big, 1.0, ClipState::Muted, 1));
        assert!(is_flat(big, 1.0, ClipState::Normal, CROWDED_LANE + 1));
        assert!(is_flat(vec2(2.0, 40.0), 1.0, ClipState::Normal, 1));
        assert!(is_flat(vec2(100.0, 5.0), 1.0, ClipState::Normal, 1));
    }

    #[test]
    fn color_at_hits_the_stops() {
        let s = gloss_stops(Color32::from_rgb(80, 80, 80), 1.0);
        assert_eq!(color_at(&s, 0.0), s[0]);
        assert_eq!(color_at(&s, 0.45), s[1]);
        assert_eq!(color_at(&s, 1.0), s[3]);
    }

    #[test]
    fn outline_is_darker_only_for_glossy_opaque_bodies() {
        let base = Color32::from_rgb(90, 140, 200);
        let o = Color32::from_rgb(10, 10, 10);
        assert!(gloss_outline(base, o, 1.0).b() < base.b());
        assert_eq!(gloss_outline(base, o, 0.0), o);
        assert_eq!(gloss_outline(base.gamma_multiply(0.5), o, 1.0), o);
    }

    /// Run the painters in a headless context: shapes are produced, the scratch mesh is reused and
    /// a crowded lane or tiny clip adds no mesh.
    #[test]
    fn painters_emit_meshes_only_on_the_glossy_path() {
        let ctx = egui::Context::default();
        let mut counts = Vec::new();
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let ctx = ui.ctx();
            let painter = ctx.layer_painter(egui::LayerId::background());
            let t = Tokens::DARK;
            let base = Color32::from_rgb(40, 90, 60);
            let mk = |state, lane| ClipPaint::from_tokens(&t, base, t.clip_outline_dark, state, lane);
            let meshes = |ctx: &egui::Context| {
                ctx.graphics_mut(|g| g.entry(egui::LayerId::background()).all_entries().filter(|s| matches!(s.shape, Shape::Mesh(_))).count())
            };
            let big = Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 40.0));
            let before = meshes(ctx);
            body(&painter, big, &mk(ClipState::Normal, 3));
            counts.push(meshes(ctx) - before);
            body(&painter, big, &mk(ClipState::Selected, 3));
            name_bar(&painter, Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 12.0)), &mk(ClipState::Normal, 3));
            strip(&painter, Rect::from_min_size(pos2(0.0, 0.0), vec2(6.0, 40.0)), &ClipPaint::strip_from_tokens(&t, base, ClipState::Normal, 1));
            let n = meshes(ctx);
            counts.push(n - before);
            body(&painter, big, &mk(ClipState::Muted, 3));
            body(&painter, big, &mk(ClipState::Normal, CROWDED_LANE + 1));
            body(&painter, Rect::from_min_size(pos2(0.0, 0.0), vec2(2.0, 40.0)), &mk(ClipState::Normal, 3));
            body(&painter, Rect::from_min_size(pos2(0.0, 0.0), vec2(120.0, 5.0)), &mk(ClipState::Normal, 3));
            counts.push(meshes(ctx) - n);
            // A translucent (Midnight style) body: gradient only, no extra opaque fill.
            let tr = ClipPaint::from_tokens(&Tokens::MIDNIGHT, base.gamma_multiply(0.4), base, ClipState::Normal, 1);
            body(&painter, big, &tr);
        });
        out.textures_delta.clear();
        assert_eq!(counts, vec![1, 4, 0]);
    }
}
