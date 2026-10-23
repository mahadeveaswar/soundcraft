//! UI text must not hard-code modifier names: macOS says Cmd where the other platforms say Ctrl,
//! so labels go through `shortcuts::shortcut_label`. Chord strings of the key registry and the
//! parser (`shortcuts.rs`, `keymap.rs`) are data, not labels, and are exempt.
use std::path::Path;

#[test]
fn no_hard_coded_modifier_in_ui_text() {
    let mut bad = Vec::new();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for entry in std::fs::read_dir(&src).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if path.extension().is_none_or(|e| e != "rs") || name == "shortcuts.rs" || name == "keymap.rs" {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        // Unit tests (after the first `#[cfg(test)]`) may spell out chords.
        let code = text.split("#[cfg(test)]").next().unwrap_or("");
        for (n, line) in code.lines().enumerate() {
            let t = line.trim_start();
            if t.starts_with("//")
                || line.contains("shortcut_label")
                || line.contains("describe(")
                || line.contains("label(\"Cmd")
                || line.contains("Some(\"C")
            {
                continue;
            }
            let hard = line.contains("Ctrl+") || line.contains('⌘') || line.contains("Cmd-") || line.contains("Ctrl-");
            let chord_label = line.contains("\"Cmd+") && (line.contains("hover_text") || line.contains("format!") || line.contains("tip"));
            if hard || chord_label {
                bad.push(format!("{name}:{}: {}", n + 1, line.trim()));
            }
        }
    }
    assert!(bad.is_empty(), "hard-coded modifier names in UI text (use shortcut_label):\n{}", bad.join("\n"));
}
