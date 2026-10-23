//! Regression: the clip context menu must stay open while the pointer moves onto it, so its
//! Delete item can be used. It used to vanish once the pointer left the lane, because the lane
//! returned early (no hover position) before the menu was drawn. The menu is found by its labels, so its
//! length can change without breaking the test.

use egui::{Event, Modifiers, PointerButton, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use soundcraft_ui_egui::{Services, SoundApp};

/// One UI frame, with queued synthetic pointer input applied first (as the control channel does).
fn step(h: &mut Harness<'_, SoundApp>) {
    let mut raw = std::mem::take(h.input_mut());
    h.state_mut().raw_input_hook(&mut raw);
    *h.input_mut() = raw;
    h.step();
}

fn press(h: &mut Harness<'_, SoundApp>, x: f32, y: f32, button: PointerButton) {
    let pos = pos2(x, y);
    let s = &mut h.state_mut().synthetic;
    s.push(Event::PointerMoved(pos));
    s.push(Event::PointerButton { pos, button, pressed: true, modifiers: Modifiers::NONE });
    s.push(Event::PointerButton { pos, button, pressed: false, modifiers: Modifiers::NONE });
}

/// The menu line with exactly this label (lines carry their key text after it).
fn menu_line<'a>(h: &'a Harness<'_, SoundApp>, name: &'a str) -> egui_kittest::Node<'a> {
    h.get_all_by_label_contains(name).find(|n| n.accesskit_node().label().is_some_and(|l| l.trim() == name)).unwrap()
}

fn kick_clip_count(h: &Harness<'_, SoundApp>) -> usize {
    let s = h.state().engine.session();
    s.tracks.iter().find(|t| t.name == "Kick").map_or(0, |t| t.clips().len())
}

#[test]
fn clip_menu_stays_open_while_moving_onto_delete() {
    let engine = soundcraft_engine::demo::demo_engine();
    let app = SoundApp::new(engine, None, Services::default());
    let mut h = Harness::builder().with_size(vec2(1600.0, 1000.0)).with_pixels_per_point(2.0).build_ui_state(
        |ui, app: &mut SoundApp| {
            let ctx = ui.ctx().clone();
            app.logic(&ctx);
            app.ui(ui);
        },
        app,
    );
    for _ in 0..6 {
        step(&mut h);
    }
    let before = kick_clip_count(&h);
    assert!(before > 0, "the demo session should have a recorded Kick clip");

    // Right-click the Kick clip, then move the pointer into the menu and down to Delete.
    press(&mut h, 880.0, 240.0, PointerButton::Secondary);
    for _ in 0..10 {
        step(&mut h);
    }
    // Move (no click) onto the Delete line, as a real pointer would, so the lane loses its hover.
    let delete = menu_line(&h, "Delete").rect();
    for to in [pos2(delete.center().x, delete.min.y - 60.0), delete.center()] {
        h.state_mut().synthetic.push(Event::PointerMoved(to));
        for _ in 0..10 {
            step(&mut h);
        }
    }
    press(&mut h, delete.center().x, delete.center().y, PointerButton::Primary);
    for _ in 0..15 {
        step(&mut h);
    }

    assert_eq!(kick_clip_count(&h), before - 1, "clicking Delete in the clip menu should remove the clip");
}
