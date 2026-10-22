//! Track default palette: the colours new tracks cycle through, set by the UI theme.

use super::*;
use crate::cmd;
use serde_json::json;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(view "track_palette.set", "Set Track Default Palette", [], None, "{colors: [[r,g,b] | \"#RRGGBB\", ...]}", always, set),
        cmd!(view "track_palette.clear", "Reset Track Default Palette", [], None, "{}", always, clear),
        cmd!(query "track_palette.get", "Get Track Default Palette", [], None, "{}", always, get),
    ]
}

pub(super) fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#').unwrap_or(s);
    if h.len() != 6 || !h.is_ascii() {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
    Some([c(0)?, c(2)?, c(4)?])
}

pub(super) fn parse_color(v: &Value) -> Option<[u8; 3]> {
    match v {
        Value::String(s) => parse_hex(s),
        Value::Array(a) if a.len() == 3 => {
            let mut out = [0u8; 3];
            for (o, x) in out.iter_mut().zip(a) {
                *o = u8::try_from(x.as_u64()?).ok()?;
            }
            Some(out)
        }
        _ => None,
    }
}

fn set(e: &mut Engine, p: &Value) -> Result<Value> {
    let list = p.get("colors").and_then(Value::as_array).ok_or_else(|| bad("track_palette.set", "`colors` must be an array"))?;
    let mut colors = Vec::with_capacity(list.len());
    for v in list {
        colors.push(parse_color(v).ok_or_else(|| bad("track_palette.set", "each colour must be [r,g,b] (0-255) or \"#RRGGBB\""))?);
    }
    e.set_track_palette(Some(colors));
    get(e, p)
}

fn clear(e: &mut Engine, _: &Value) -> Result<Value> {
    e.set_track_palette(None);
    Ok(json!({"colors": null}))
}

fn get(e: &mut Engine, _: &Value) -> Result<Value> {
    Ok(json!({"colors": e.track_palette()}))
}

#[cfg(test)]
mod tests {
    use super::*;
    use soundcraft_model::TRACK_COLORS;

    fn color_of(e: &Engine, name: &str) -> [u8; 3] {
        e.session().tracks.iter().find(|t| t.name == name).map_or([0; 3], |t| t.color)
    }

    #[test]
    fn new_tracks_use_the_override_and_fall_back() {
        let mut e = Engine::default();
        e.execute("track.new", &json!({})).unwrap();
        assert_eq!(color_of(&e, "Audio 1"), TRACK_COLORS[0]);
        e.execute("track_palette.set", &json!({"colors": [[1, 2, 3], "#0A0B0C"]})).unwrap();
        e.execute("track.new", &json!({"count": 3})).unwrap();
        assert_eq!(color_of(&e, "Audio 2"), [0x0A, 0x0B, 0x0C]);
        assert_eq!(color_of(&e, "Audio 3"), [1, 2, 3]);
        assert_eq!(color_of(&e, "Audio 4"), [0x0A, 0x0B, 0x0C]);
        e.execute("track_palette.clear", &json!({})).unwrap();
        e.execute("track.new", &json!({})).unwrap();
        assert_eq!(color_of(&e, "Audio 5"), TRACK_COLORS[4]);
        assert!(e.track_palette().is_none());
    }

    #[test]
    fn bad_colours_are_errors_and_empty_clears() {
        let mut e = Engine::default();
        assert!(e.execute("track_palette.set", &json!({})).is_err());
        assert!(e.execute("track_palette.set", &json!({"colors": [[1, 2, 300]]})).is_err());
        assert!(e.execute("track_palette.set", &json!({"colors": ["#12"]})).is_err());
        assert!(e.track_palette().is_none());
        e.execute("track_palette.set", &json!({"colors": [[1, 2, 3]]})).unwrap();
        e.execute("track_palette.set", &json!({"colors": []})).unwrap();
        assert!(e.track_palette().is_none());
    }

    #[test]
    fn the_demo_is_unchanged_without_an_override() {
        let a = crate::demo::demo_session();
        assert!(a.track_palette.is_none());
        assert_eq!(
            a.tracks.iter().find(|t| t.name == "Drums").map_or(TRACK_COLORS[0], |t| t.color),
            crate::demo::demo_session().tracks.iter().find(|t| t.name == "Drums").map_or(TRACK_COLORS[0], |t| t.color)
        );
        let mut e = crate::demo::demo_engine();
        let before: Vec<_> = e.session().tracks.iter().map(|t| t.color).collect();
        e.execute("track_palette.set", &json!({"colors": [[9, 9, 9]]})).unwrap();
        let after: Vec<_> = e.session().tracks.iter().map(|t| t.color).collect();
        assert_eq!(before, after, "existing tracks keep their colours");
    }

    #[test]
    fn the_override_survives_undo() {
        let mut e = Engine::default();
        e.execute("track_palette.set", &json!({"colors": [[7, 7, 7]]})).unwrap();
        e.execute("track.new", &json!({})).unwrap();
        e.undo();
        e.execute("track.new", &json!({})).unwrap();
        assert_eq!(color_of(&e, "Audio 1"), [7, 7, 7]);
    }
}
