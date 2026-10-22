//! Settings > Appearance page (theme picker, colour list, contrast badges). Filled in by task S3.

use crate::SoundApp;
use egui::Ui;

pub fn page(_app: &mut SoundApp, ui: &mut Ui) {
    ui.label("Appearance");
}
