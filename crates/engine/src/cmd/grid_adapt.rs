//! Adaptive grid: the grid coarsens as the view zooms out, so lines never crowd together.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_model::Session;
use soundcraft_time::{GridValue, Samples};

/// Session flag: grid lines and snapping use [`effective_grid`] instead of the raw grid value.
pub const GRID_ADAPTIVE: &str = "grid.adaptive";
/// Smallest on-screen grid step, in points, before the grid coarsens.
pub const MIN_STEP_PX: f64 = 12.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(noundo "edit.grid_adaptive", "Adapt Grid to Zoom", [], None, "{value?: bool}", always, |e, p| {
        let s = e.session_mut();
        let on = p.get("value").and_then(Value::as_bool).unwrap_or(!s.edit.flag(GRID_ADAPTIVE));
        s.edit.set_flag(GRID_ADAPTIVE, on);
        Ok(json!({"value": on}))
    })]
}

/// The grid to draw and snap to at position `at`: the session grid, coarsened to the current zoom when
/// the adaptive flag is on.
pub fn effective_grid(s: &Session, at: Samples) -> GridValue {
    let g = s.edit.grid;
    if !s.edit.flag(GRID_ADAPTIVE) {
        return g;
    }
    g.effective(s.edit.zoom.samples_per_px, MIN_STEP_PX, at, s.sample_rate, &s.tempo, s.frame_rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at_zoom(e: &mut Engine, spp: f64) -> GridValue {
        e.execute("view.zoom_set", &json!({"samples_per_px": spp})).unwrap();
        effective_grid(e.session(), 0)
    }

    #[test]
    fn off_by_default_and_toggles() {
        let mut e = Engine::default();
        assert_eq!(at_zoom(&mut e, 5000.0), e.session().edit.grid);
        assert_eq!(e.execute("edit.grid_adaptive", &json!({})).unwrap(), json!({"value": true}));
        assert_eq!(e.execute("edit.grid_adaptive", &json!({"value": true})).unwrap(), json!({"value": true}));
        assert_eq!(e.execute("edit.grid_adaptive", &json!({})).unwrap(), json!({"value": false}));
    }

    #[test]
    fn effective_grid_follows_the_zoom_level() {
        let mut e = Engine::default();
        e.execute("edit.grid_adaptive", &json!({"value": true})).unwrap();
        let s = |e: &Engine, g: GridValue, pos| g.snap(pos, e.session().sample_rate, &e.session().tempo, e.session().frame_rate);
        let near = at_zoom(&mut e, 10.0);
        assert_eq!(near, e.session().edit.grid);
        assert_eq!(s(&e, near, 8000), 6000); // 1/16 lines every 6000
        let far = at_zoom(&mut e, 2000.0); // 12 px = 24000 samples: a quarter note is 24000
        assert_eq!(far.label(), "1/4 note");
        assert_eq!(s(&e, far, 8000), 0);
        assert_eq!(s(&e, far, 20_000), 24_000);
    }
}
