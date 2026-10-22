//! Folder track header pieces for the Edit window: the fold triangle and the guide bars that
//! show which folders a child track sits in.

use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, pos2, vec2};
use soundcraft_model::{Session, Track};

/// Triangle centred in `rect`: pointing down when the folder is open, right when closed.
pub(crate) fn triangle_points(rect: Rect, open: bool) -> [Pos2; 3] {
    let c = rect.center();
    let r = rect.width().min(rect.height()) * 0.38;
    if open {
        [c + vec2(-r, -r * 0.6), c + vec2(r, -r * 0.6), c + vec2(0.0, r * 0.8)]
    } else {
        [c + vec2(-r * 0.6, -r), c + vec2(r * 0.8, 0.0), c + vec2(-r * 0.6, r)]
    }
}

/// Draws the fold triangle on a small rounded plate.
pub(crate) fn draw_fold_triangle(p: &Painter, rect: Rect, open: bool, plate: Color32, ink: Color32) {
    p.rect_filled(rect, 3.0, plate);
    p.add(Shape::convex_polygon(triangle_points(rect, open).to_vec(), ink, Stroke::NONE));
}

/// The chain of folders above a track, outermost first (at most six), without allocating:
/// the folders sit in the first `len` slots of the array.
pub(crate) fn ancestors<'a>(s: &'a Session, track: &Track) -> ([Option<&'a Track>; 6], usize) {
    let mut chain = [None; 6];
    let mut len = 0;
    let mut current = track.folder;
    while let Some(folder) = current.and_then(|id| s.track(id))
        && len < 6
    {
        chain[len] = Some(folder);
        len += 1;
        current = folder.folder;
    }
    chain[..len].reverse();
    (chain, len)
}

/// One thin bar per enclosing folder, in that folder's colour, down the left edge of a header.
pub(crate) fn draw_child_guides(p: &Painter, head: Rect, s: &Session, track: &Track) {
    let (chain, len) = ancestors(s, track);
    for (k, folder) in chain.iter().take(len).flatten().enumerate() {
        let x = head.min.x + 0.5 + k as f32 * 3.0;
        let c = Color32::from_rgb(folder.color[0], folder.color[1], folder.color[2]);
        p.line_segment([pos2(x, head.min.y), pos2(x, head.max.y)], Stroke::new(2.0, c));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangle_points_down_when_open_and_right_when_closed() {
        let r = Rect::from_min_size(pos2(0.0, 0.0), vec2(10.0, 10.0));
        let open = triangle_points(r, true);
        let closed = triangle_points(r, false);
        assert!(open[2].y > open[0].y && (open[0].y - open[1].y).abs() < 1e-3);
        assert!(closed[1].x > closed[0].x && (closed[0].x - closed[2].x).abs() < 1e-3);
    }
}
