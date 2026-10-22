//! Master loudness (BS.1770 K-weighted LUFS): `mix.loudness` reads the live meter the playback
//! side feeds (`Engine::set_live_loudness`) or measures the session offline; `mix.loudness_reset`
//! restarts the live measurement.

use super::*;
use crate::cmd;
use serde_json::json;
use soundcraft_dsp::meter::{LoudnessMeter, LoudnessReading};
use soundcraft_time::Range;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(query "mix.loudness", "Master Loudness", [], None, "{offline?: false} -> {momentary, short_term, integrated, max_momentary, true_peak_db, source: live|offline} in LUFS / dBTP (-144 = nothing measured). `offline: true` renders the session and measures it whole", always, loudness),
        cmd!(view "mix.loudness_reset", "Reset Loudness Meter", [], None, "{}: restarts the live integrated, maximum momentary and true-peak measurement", always, |e, _| {
            e.set_live_loudness(LoudnessReading::default());
            e.request_loudness_reset();
            Ok(json!({}))
        }),
    ]
}

fn reading_json(r: LoudnessReading, source: &str) -> Value {
    json!({
        "momentary": r.momentary, "short_term": r.short_term, "integrated": r.integrated,
        "max_momentary": r.max_momentary, "true_peak_db": r.true_peak_db, "source": source,
    })
}

fn loudness(e: &mut Engine, p: &Value) -> Result<Value> {
    if !bool_or(p, "offline", false) {
        return Ok(reading_json(e.live_loudness(), "live"));
    }
    let s = e.session();
    let end = s.content_end();
    if end <= 0 {
        return Ok(reading_json(LoudnessReading::default(), "offline"));
    }
    let mix = soundcraft_mix::render_range(s, Range::new(0, end), 4096);
    let frames = mix.first().map_or(0, Vec::len);
    let mut m = LoudnessMeter::new(s.sample_rate.as_f64() as f32, mix.len());
    m.process(&mix, frames);
    Ok(reading_json(m.reading(), "offline"))
}

#[cfg(test)]
mod tests {
    use crate::Engine;
    use serde_json::json;
    use soundcraft_dsp::meter::LoudnessReading;

    #[test]
    fn live_reading_round_trips_and_reset_is_requested_once() {
        let mut e = Engine::default();
        assert_eq!(e.execute("mix.loudness", &json!({})).unwrap()["momentary"], -144.0);
        e.set_live_loudness(LoudnessReading { momentary: -23.0, short_term: -22.5, integrated: -23.5, max_momentary: -20.0, true_peak_db: -3.0 });
        let r = e.execute("mix.loudness", &json!({})).unwrap();
        assert_eq!((r["momentary"].as_f64(), r["integrated"].as_f64(), r["source"].as_str()), (Some(-23.0), Some(-23.5), Some("live")));
        assert!(!e.take_loudness_reset());
        e.execute("mix.loudness_reset", &json!({})).unwrap();
        assert!(e.take_loudness_reset());
        assert!(!e.take_loudness_reset());
        assert_eq!(e.execute("mix.loudness", &json!({})).unwrap()["integrated"], -144.0);
    }

    #[test]
    fn offline_measures_the_demo_session() {
        let mut e = crate::demo::demo_engine();
        let r = e.execute("mix.loudness", &json!({"offline": true})).unwrap();
        assert_eq!(r["source"], "offline");
        let i = r["integrated"].as_f64().unwrap();
        assert!(i > -60.0 && i < 0.0, "{i}");
        let empty = Engine::default().execute("mix.loudness", &json!({"offline": true})).unwrap();
        assert_eq!(empty["integrated"], -144.0);
    }
}
