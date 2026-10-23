//! The MIDI export dialog's "Include controller lanes" box reaches `file.export_midi`.
use egui::vec2;
use egui_kittest::{Harness, kittest::Queryable};
use soundcraft_ui_egui::{Services, SoundApp, dialogs::Dialog};

#[test]
fn the_include_controller_lanes_box_is_sent_as_lanes() {
    let dir = std::env::temp_dir().join(format!("soundcraft-ui-lanes-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut sizes = Vec::new();
    for plain in [true, false] {
        let path = dir.join(if plain { "plain.mid" } else { "lanes.mid" });
        let mut a = SoundApp::new(soundcraft_engine::demo::demo_engine(), None, Services::default());
        let keys = a.engine.session().track_by_name("Keys").map(|t| t.id.0).unwrap();
        for (at, v) in [(0, 10.0), (96_000, 120.0)] {
            a.run("automation.set_point", serde_json::json!({"track": keys, "param": "cc1", "at": at, "value": v})).unwrap();
        }
        a.dialogs.open = Some(Dialog::PathPrompt {
            cmd: "file.export_midi".into(),
            title: "Export MIDI".into(),
            path: path.to_string_lossy().into(),
            key: "path".into(),
        });
        let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).build_ui_state(
            |ui, app: &mut SoundApp| {
                app.logic(&ui.ctx().clone());
                app.ui(ui);
            },
            a,
        );
        h.run_steps(3);
        if !plain {
            h.get_by_label("Include controller lanes").click();
            h.run_steps(3);
        }
        h.get_by_label("OK").click();
        h.run_steps(3);
        sizes.push(std::fs::read(&path).map(|b| b.len()).unwrap_or(0));
    }
    assert!(sizes[0] > 0 && sizes[1] > sizes[0], "the lane adds controller events: {sizes:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
