//! The Standard toolbar's info row: audio input and output, record time left, session format,
//! frame rate and pan law. Each item may open the window that changes it.

use crate::SoundApp;
use crate::theme::{Tokens, regular};
use egui::{Rect, Sense, Ui};
use soundcraft_model::BitDepthSetting;

/// Height of the row.
pub const HEIGHT: f32 = 20.0;
/// How often the free disk space is read again, in seconds.
const REFRESH_S: f64 = 5.0;
const GAP: f32 = 18.0;
const CHAR_W: f32 = 6.4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Normal,
    Warn,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub label: &'static str,
    pub value: String,
    pub state: State,
    /// Command a click runs (opens a window), if any.
    pub cmd: Option<&'static str>,
}

/// What the row needs from the app, gathered once so [`items`] stays pure.
#[derive(Debug, Clone, PartialEq)]
pub struct Facts {
    pub input: Option<String>,
    pub output: Option<String>,
    pub free_bytes: Option<u64>,
    pub armed_tracks: usize,
    pub sample_rate: u32,
    pub bit_depth: BitDepthSetting,
    pub frame_rate: String,
}

/// Bytes one sample takes on disk.
pub fn bytes_per_sample(d: BitDepthSetting) -> u64 {
    match d {
        BitDepthSetting::Int16 => 2,
        BitDepthSetting::Int24 => 3,
        BitDepthSetting::Float32 => 4,
    }
}

/// Seconds of recording that fit in `free` bytes across `armed` tracks (at least one).
pub fn record_seconds(free: u64, rate: u32, d: BitDepthSetting, armed: usize) -> u64 {
    let per_second = u64::from(rate.max(1)) * bytes_per_sample(d) * (armed.max(1) as u64);
    free / per_second.max(1)
}

/// "3h 20m", "45m 10s" or "-" when unknown.
pub fn format_record_time(seconds: Option<u64>) -> String {
    match seconds {
        None => "-".into(),
        Some(s) if s >= 3600 => format!("{}h {:02}m", s / 3600, s % 3600 / 60),
        Some(s) => format!("{}m {:02}s", s / 60, s % 60),
    }
}

fn depth_label(d: BitDepthSetting) -> &'static str {
    match d {
        BitDepthSetting::Int16 => "16-bit",
        BitDepthSetting::Int24 => "24-bit",
        BitDepthSetting::Float32 => "32-bit float",
    }
}

/// The row's items, left to right.
pub fn items(f: &Facts) -> Vec<Item> {
    let device = |label, name: &Option<String>| match name {
        Some(n) => Item { label, value: n.clone(), state: State::Normal, cmd: Some("ui.audio_device") },
        None => Item { label, value: "not connected".into(), state: State::Warn, cmd: Some("ui.audio_device") },
    };
    let secs = f.free_bytes.map(|b| record_seconds(b, f.sample_rate, f.bit_depth, f.armed_tracks));
    vec![
        device("Input", &f.input),
        device("Output", &f.output),
        Item { label: "Record time", value: format_record_time(secs), state: State::Normal, cmd: Some("setup.disk_allocation") },
        Item {
            label: "Format",
            value: format!("{} Hz, {}", f.sample_rate, depth_label(f.bit_depth)),
            state: State::Normal,
            cmd: Some("setup.session"),
        },
        Item { label: "Frames", value: f.frame_rate.clone(), state: State::Normal, cmd: Some("setup.session") },
        Item { label: "Pan law", value: "-3 dB".into(), state: State::Normal, cmd: None },
    ]
}

/// Estimated width of an item in points.
pub fn item_width(i: &Item) -> f32 {
    (i.label.chars().count() + i.value.chars().count() + 1) as f32 * CHAR_W
}

/// How many of `widths` fit in `avail`, dropping from the right.
pub fn fit_count(widths: &[f32], avail: f32) -> usize {
    let mut used = 0.0;
    for (n, w) in widths.iter().enumerate() {
        used += w + if n > 0 { GAP } else { 0.0 };
        if used > avail {
            return n;
        }
    }
    widths.len()
}

fn record_folder(app: &SoundApp) -> std::path::PathBuf {
    let s = app.engine.session();
    let root = s.edit.flags.iter().find_map(|f| f.strip_prefix("disk.root=")).unwrap_or("Audio Files");
    let root = std::path::PathBuf::from(root);
    if root.is_absolute() {
        return root;
    }
    match app.engine.path.as_deref().and_then(|p| std::path::Path::new(p).parent()) {
        Some(dir) => dir.join(root),
        None => root,
    }
}

fn free_bytes(app: &SoundApp, ui: &Ui) -> Option<u64> {
    let free = app.services.free_space.as_ref()?;
    let now = ui.ctx().input(|i| i.time);
    let id = egui::Id::new("info_row_free_space");
    let cached = ui.data(|d| d.get_temp::<(f64, Option<u64>)>(id));
    if let Some((at, v)) = cached
        && now - at < REFRESH_S
    {
        return v;
    }
    let mut folder = record_folder(app);
    while !folder.exists() && folder.pop() {}
    let v = free(&folder);
    ui.data_mut(|d| d.insert_temp(id, (now, v)));
    v
}

/// Gather the row's facts from the app.
pub fn facts(app: &SoundApp, ui: &Ui) -> Facts {
    let s = app.engine.session();
    Facts {
        input: app.recorder.as_ref().map(|r| r.device_name.clone()),
        output: app.player.as_ref().filter(|p| !p.silent).map(|p| p.device_name.clone()),
        free_bytes: free_bytes(app, ui),
        armed_tracks: s.tracks.iter().filter(|t| t.mixer.record_arm).count(),
        sample_rate: s.sample_rate.hz(),
        bit_depth: s.bit_depth,
        frame_rate: s.frame_rate.label().to_string(),
    }
}

pub fn show(app: &mut SoundApp, ui: &mut Ui) {
    let t = Tokens::current();
    let list = items(&facts(app, ui));
    let widths: Vec<f32> = list.iter().map(item_width).collect();
    let n = fit_count(&widths, ui.max_rect().width() - 20.0);
    let rect = ui.max_rect();
    let mut x = rect.min.x + 10.0;
    let mut run = None;
    for (idx, item) in list.iter().take(n).enumerate() {
        let r = Rect::from_min_size(egui::pos2(x, rect.min.y), egui::vec2(widths[idx], rect.height()));
        let resp = ui.interact(r, ui.id().with(("info", idx)), if item.cmd.is_some() { Sense::click() } else { Sense::hover() });
        let p = ui.painter();
        let label = p.text(egui::pos2(x, rect.center().y), egui::Align2::LEFT_CENTER, format!("{} ", item.label), regular(11.0), t.text_dim);
        let col = if item.state == State::Warn { t.status_warn } else { t.text };
        p.text(egui::pos2(label.max.x, rect.center().y), egui::Align2::LEFT_CENTER, &item.value, regular(11.0), col);
        let resp = resp.on_hover_text(match item.cmd {
            Some(_) => format!("{}: click to change", item.label),
            None => item.label.to_string(),
        });
        if resp.clicked() {
            run = item.cmd;
        }
        x += widths[idx] + GAP;
    }
    if let Some(cmd) = run {
        let _ = app.run(cmd, serde_json::json!({}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            input: None,
            output: Some("Speakers".into()),
            free_bytes: Some(48_000 * 3 * 3600),
            armed_tracks: 0,
            sample_rate: 48_000,
            bit_depth: BitDepthSetting::Int24,
            frame_rate: "24 fps".into(),
        }
    }

    #[test]
    fn items_in_order_with_warning_for_missing_input() {
        let v = items(&facts());
        let labels: Vec<_> = v.iter().map(|i| i.label).collect();
        assert_eq!(labels, ["Input", "Output", "Record time", "Format", "Frames", "Pan law"]);
        assert_eq!((v[0].value.as_str(), v[0].state), ("not connected", State::Warn));
        assert_eq!((v[1].value.as_str(), v[1].state), ("Speakers", State::Normal));
        assert_eq!(v[3].value, "48000 Hz, 24-bit");
        assert_eq!(v[3].cmd, Some("setup.session"));
        assert_eq!(v[5].cmd, None);
    }

    #[test]
    fn record_time_divides_by_armed_tracks_and_shows_dash_without_a_service() {
        let mut f = facts();
        assert_eq!(items(&f)[2].value, "1h 00m");
        f.armed_tracks = 2;
        assert_eq!(items(&f)[2].value, "30m 00s");
        f.free_bytes = None;
        assert_eq!(items(&f)[2].value, "-");
    }

    #[test]
    fn narrow_rows_drop_items_from_the_right() {
        let w = [100.0, 100.0, 100.0];
        assert_eq!(fit_count(&w, 1000.0), 3);
        assert_eq!(fit_count(&w, 250.0), 2);
        assert_eq!(fit_count(&w, 50.0), 0);
    }
}
