//! Settings > Track colours page (the 16 default colours for new tracks). Filled in by task S4.

use crate::SoundApp;
use egui::Ui;

pub fn page(_app: &mut SoundApp, ui: &mut Ui) {
    ui.label("Track colours");
}
