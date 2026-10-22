//! Settings > Track colours page: the 16 default colours given to new tracks. Edits run
//! `ui.theme_set_track_color`; "Reset palette" runs `ui.theme_reset {key: "track_palette"}`.
//! Existing tracks keep the colour they have.

use crate::SoundApp;
use crate::settings_appearance::{bad_colour_message, hex_edit};
use crate::theme::{Tokens, bold, regular};
use crate::theme_doc::{self, TRACK_COLOR_COUNT};
use egui::{Color32, Id, RichText, Ui, vec2};
use serde_json::json;

const COLUMNS: usize = 4;

/// The palette new tracks draw from: the theme's, else the built-in set, always 16 entries.
fn palette(app: &SoundApp) -> Vec<[u8; 3]> {
    let live = app.engine.track_palette().filter(|c| !c.is_empty());
    (0..TRACK_COLOR_COUNT)
        .map(|i| match live {
            Some(c) => c.get(i % c.len()).copied().unwrap_or([128; 3]),
            None => soundcraft_model::TRACK_COLORS.get(i).copied().unwrap_or([128; 3]),
        })
        .collect()
}

pub fn page(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let msg_id = Id::new("settings_tracks_message");
    let mut message = ui.data_mut(|d| d.get_temp::<String>(msg_id)).unwrap_or_default();
    ui.label(RichText::new("Track colours").font(bold(12.0)).color(t.text));
    ui.label(RichText::new("The colours new tracks start with. Tracks that already exist keep theirs.").font(regular(11.0)).color(t.text_dim));
    ui.add_space(6.0);
    let colors = palette(app);
    egui::Grid::new("settings_track_palette").num_columns(COLUMNS).spacing(vec2(14.0, 10.0)).show(ui, |ui| {
        for (i, [r, g, b]) in colors.iter().copied().enumerate() {
            let current = Color32::from_rgb(r, g, b);
            ui.vertical(|ui| {
                let mut rgb = [r, g, b];
                if ui.color_edit_button_srgb(&mut rgb).changed() && rgb != [r, g, b] {
                    message = set(app, i, Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                }
                match hex_edit(ui, Id::new(("settings_track_hex", i)), current, 64.0) {
                    Ok(Some(c)) => message = set(app, i, c),
                    Ok(None) => {}
                    Err(text) => message = bad_colour_message(&text),
                }
            });
            if (i + 1) % COLUMNS == 0 {
                ui.end_row();
            }
        }
    });
    ui.add_space(8.0);
    let custom = app.ui.custom_theme.as_ref().is_some_and(|d| !d.track_palette.is_empty());
    if ui.add_enabled(custom, egui::Button::new("Reset palette")).on_hover_text("Go back to the built-in track colours for new tracks").clicked() {
        message = match app.run("ui.theme_reset", json!({"key": "track_palette"})) {
            Ok(_) => String::new(),
            Err(e) => e,
        };
    }
    if !message.is_empty() {
        ui.label(RichText::new(&message).font(regular(11.0)).color(t.rec));
    }
    ui.data_mut(|d| d.insert_temp(msg_id, message));
}

/// Run the command for one swatch; the error text (or nothing) becomes the page message.
fn set(app: &mut SoundApp, index: usize, c: Color32) -> String {
    let hex = format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b());
    debug_assert!(theme_doc::parse_hex(&hex).is_some());
    match app.run("ui.theme_set_track_color", json!({"index": index, "color": hex})) {
        Ok(_) => String::new(),
        Err(e) => e,
    }
}
