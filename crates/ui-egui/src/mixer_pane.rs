//! The bottom Mixer pane: a horizontally scrolling row of channel strips (one per visible track,
//! in track-list order) with the Master strip pinned at the right. The strips are the Mix window's
//! own, drawn through `mix_window::strip` with pane options; only the strips that intersect the
//! scroll viewport are drawn.

use crate::SoundApp;
use crate::mix_window::{self, StripOpts};
use crate::theme::{Tokens, bold, regular};
use egui::{Rect, Sense, Ui, UiBuilder, pos2, vec2};
use soundcraft_model::{TrackId, TrackKind};

/// Sections a pane strip shows: inserts, sends, routing summary and the track colour.
const ROWS: [&str; 4] = ["inserts_ae", "sends_ae", "io", "color"];
/// The least strip heights; a shorter pane scrolls vertically instead of squeezing the fader.
const FULL_HEIGHT: f32 = 250.0;
const COMPACT_HEIGHT: f32 = 250.0;
/// Height of the bar above the strips.
const BAR_HEIGHT: f32 = 22.0;
/// Space between strips.
const GAP: f32 = 1.0;

fn opts(compact: bool) -> StripOpts {
    StripOpts {
        compact,
        rows: Some(ROWS.iter().map(|r| (*r).to_owned()).collect()),
        min_height: Some(if compact { COMPACT_HEIGHT } else { FULL_HEIGHT }),
        select_on_click: true,
        outline_selected: true,
        color_chip: true,
        stacked: true,
    }
}

/// The strips the pane lists, in track-list order: visible tracks, not Master (pinned apart).
fn visible_tracks(app: &SoundApp) -> Vec<TrackId> {
    app.engine.session().tracks.iter().filter(|t| !t.hidden && !matches!(t.kind, TrackKind::Video | TrackKind::Master)).map(|t| t.id).collect()
}

/// The range of strip indices whose columns intersect the horizontal span `[min_x, max_x]`.
pub(crate) fn visible_range(count: usize, step: f32, min_x: f32, max_x: f32) -> std::ops::Range<usize> {
    if count == 0 || step <= 0.0 {
        return 0..0;
    }
    let first = (min_x / step).floor().max(0.0) as usize;
    let last = ((max_x / step).ceil().max(0.0) as usize).min(count);
    first.min(count)..last.max(first.min(count))
}

/// The pane body. Called from the dock's panel match.
pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let full = ui.max_rect();
    ui.painter().rect_filled(full, 0.0, t.strip_bg);
    let compact = app.ui.mixer_compact;
    let o = opts(compact);
    let bar = Rect::from_min_size(full.min, vec2(full.width(), BAR_HEIGHT));
    let mut bar_ui = ui.new_child(UiBuilder::new().max_rect(bar).layout(egui::Layout::left_to_right(egui::Align::Center)));
    bar_ui.add_space(4.0);
    bar_ui.label(egui::RichText::new("MIXER").font(bold(10.0)).color(t.section_label));
    if bar_ui
        .selectable_label(compact, egui::RichText::new("Compact").font(regular(10.0)))
        .on_hover_text("Narrow strips; inserts and sends show as counts")
        .clicked()
    {
        app.ui.mixer_compact = !compact;
    }
    let body = Rect::from_min_max(pos2(full.min.x, bar.max.y), full.max);
    let step = mix_window::opts_width(app, &o) + GAP;
    let master_w = step;
    let master_rect = Rect::from_min_max(pos2((body.max.x - master_w).max(body.min.x), body.min.y), body.max);
    let scroll_rect = Rect::from_min_max(body.min, pos2(master_rect.min.x, body.max.y));
    // The polled snapshot is taken out for the frame (strips borrow the app) and put back after.
    let snap = app.meters_live().then(|| std::mem::take(&mut app.meter_snapshot));
    let tracks = visible_tracks(app);
    let height = body.height().max(if compact { COMPACT_HEIGHT } else { FULL_HEIGHT });
    let mut scroll_ui = ui.new_child(UiBuilder::new().max_rect(scroll_rect));
    egui::ScrollArea::both().id_salt("mixer-pane-scroll").auto_shrink([false, false]).show_viewport(&mut scroll_ui, |ui, viewport| {
        let (area, _) = ui.allocate_exact_size(vec2(step * tracks.len() as f32, height), Sense::hover());
        for i in visible_range(tracks.len(), step, viewport.min.x, viewport.max.x) {
            let Some(id) = tracks.get(i).copied() else { continue };
            let cell = Rect::from_min_size(pos2(area.min.x + i as f32 * step, area.min.y), vec2(step - GAP, height));
            let mut cell_ui = ui.new_child(UiBuilder::new().max_rect(cell));
            mix_window::strip(app, &mut cell_ui, id, snap.as_ref(), &o);
        }
    });
    let master = app.engine.session().tracks.iter().find(|t| t.kind == TrackKind::Master).map(|t| t.id);
    if let Some(id) = master {
        let mut master_ui = ui.new_child(UiBuilder::new().max_rect(Rect::from_min_size(master_rect.min, vec2(master_rect.width(), height))));
        mix_window::strip(app, &mut master_ui, id, snap.as_ref(), &o);
    }
    if let Some(s) = snap {
        app.meter_snapshot = s;
    }
}

#[cfg(test)]
mod tests {
    use super::visible_range;

    #[test]
    fn only_strips_in_the_viewport_are_drawn() {
        assert_eq!(visible_range(0, 99.0, 0.0, 500.0), 0..0);
        assert_eq!(visible_range(10, 100.0, 0.0, 250.0), 0..3);
        assert_eq!(visible_range(10, 100.0, 250.0, 450.0), 2..5);
        assert_eq!(visible_range(10, 100.0, 950.0, 5000.0), 9..10);
        assert_eq!(visible_range(3, 100.0, 5000.0, 6000.0).len(), 0);
    }
}
