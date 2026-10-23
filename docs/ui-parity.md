# UI parity

> **Last reviewed:** 2026-10-13 · **Last updated:** 2026-10-23 · **Change:** minor (alternative key set, arrange context menus) · **Target:** the incumbent professional DAW's menu tree (2026.4.1)

How SoundCraft feels in the hands of a Pro Tools user: tools, modes, modifiers, shortcuts,
windows, meters. Measured from `crates/model/src/session.rs` (tools, modes), the command
registry (shortcuts), `crates/ui-egui/src` (windows, gestures) and user issues; compared with
reference screenshots kept locally under `plan/protools/screenshots/` (16, never committed).

**UI/UX fidelity: ~55 % ready** (estimated). Remaining ≈ 60–100 h.

Legend: ✅ matches · 🟡 partial · ❌ missing.

## Tools and modes

| Feature | Status | Notes |
|---|---|---|
| Tools: Zoom, Trim, Selector, Grabber, Scrubber, Pencil, Smart (7) | ✅ present | Trim variants (TCE, loop, scrub trim) and Grabber variants (separation, object) partial; Multitool fade/trim handles don't adjust audio (#5) |
| Smart-tool Fade In / Fade Out drag bounds and preview | ✅ | One thin curve replaces the dragged side's previous curve and matches its appearance after release; bounded to the clip, including full-clip fades. Hovering the curve's upper endpoint shows the fade cursor and lets the existing fade be dragged |
| Fade midpoint curvature | ✅ | Visible fades retain a small midpoint handle in Smart mode, including after release and extreme bends. Hover shows the vertical resize cursor; repeated drags change curvature without changing length. Curves sample at physical-pixel resolution; preview, playback and bounce share the envelope. Length drags retain curvature |
| Live waveform fade preview | ✅ | Waveform amplitude updates during fade-length and midpoint-curvature drags, sharing the curve's temporary envelope. Mono/stereo, waveform styles, zoom and clip gain remain effective; release preserves the displayed waveform with one undo step and cached peaks |
| Edit modes: Shuffle, Slip, Spot, Grid (absolute/relative) | ✅ | Spot dialog present |
| Grid and nudge values, all timebases | ✅ | Bars/Beats, Min:Secs, Timecode, Feet+Frames, Samples |
| Keyboard focus modes (Commands, Clip List, Groups) | 🟡 | Commands focus single-key editing ✅; list focus partial |
| Link timeline/edit, link track/edit, Tab to transient, insertion follows playback | ✅ | |
| Mirrored MIDI, MIDI merge, layered editing | 🟡 | Stored and partly honoured |

## Keyboard and mouse

| Feature | Status | Notes |
|---|---|---|
| Default shortcuts | 🟡 146 bound (measured: distinct `Some("…")` shortcuts in the registry) | Pro Tools' default set is several hundred incl. numeric keypad and focus keys. Collisions: Mix/Edit toggle vs zoom (#144), Heal (#140), Export (#7) |
| Ctrl as Cmd on Windows/Linux | ✅ since #15, #41 still open in some builds | #62 shows Ctrl in labels |
| Numeric keypad transport and memory-location recall | 🟡 | Classic / Transport / Shuttle modes not all present |
| Modifier gestures (Alt for all tracks, Shift-Alt for selected, Cmd-click fine faders, Ctrl-click defaults) | 🟡 | Main ones on faders and pan; not across every control |
| Scroll/zoom: wheel, trackpad pinch | 🟡 | Anchored trackpad zoom in open PRs #112, #137 |
| Drag and drop: Clip List → tracks, files → tracks | ✅ macOS/Windows; 🟡 Wayland (PR #150) | |
| Context menus (clip, empty lane, track, ruler, piano roll, controller lane, mixer strip, Clip List) | 🟡 | Built from the command catalogue (`context_menu.rs`): labels, key text and enabled state come from the registry and follow the active key set. Clip: cut/copy/paste/delete, split at playhead, split range, glue, duplicate, repeat, mute/unmute, locks, fades, process, quantize, loop, group, order, colour, rename, open editor (MIDI), render clip gain, bounce. Empty lane: paste, select all/none, locators to selection, add marker, add track. Track: mute, solo, record enable, select all clips plus the existing items; the mixer strip body opens it too. Ruler: marker, locate here, left/right locator here, loop, zoom. Piano roll: delete, split, glue, mute, quantize, transpose, velocity, select all (no note clipboard yet). Controller lanes: clear, remove. Clip List delete (#4) |
| Key sets (Settings > Keys) | 🟡 | Two built-in sets: Default (the registry's keys, unchanged) and Alternative (single-letter commands, number-row tools, Delete/Insert, arrows, `Cmd+K` repeat and so on), plus per-command keys that work on top of either set (several keys per command, conflict warning with Reassign, reset). Saved in the UI preferences; scriptable with `ui.prefs`. The window toolkit reports numeric-keypad keys as their main-row twins, so the Alternative set uses plain keys for transport (Enter plays, 0 stops, `/` loops, `.` returns to zero); a separate keypad set needs toolkit support |
| Drag across checkboxes to check or uncheck several | ✅ | #399; every checkbox, as in the other Craft apps |

## Windows and panels

| Window | Status | Notes |
|---|---|---|
| Edit window: rulers, track headers, view columns, Clip List, Groups, Tracks list | ✅ | Track Pin (new in 2026.4) ✅ (`track.pin`) |
| Mix window: strips, views (inserts, sends, I/O, comments, EQ curve, preamps, instruments, object) | ✅ | Narrow Mix ✅; Mix and Edit on screen at once (Window › Mix in Separate Window, its own native window) ✅; Delay Compensation view and HEAT missing |
| Inspector (dock panel), Mixer pane (bottom dock), track colour palette, glossy clips | 🟡 | Per-track Inspector sections, an 84-colour palette popup, glossy rounded clips and headers (Settings switch) and a bottom Mixer pane sharing the lower zone with the MIDI editor as exclusive tabs are in; EQ curve view and MIDI track controls are later |
| MIDI Editor, Score Editor, Event List | 🟡 | Dock height bug fixed (#113); notation editing shallow |
| Transport, Big Counter, Memory Locations, Automation, Video, Universe, Renderer, Settings window | ✅ present | Settings: Setup > Preferences..., Setup > Appearance, Window > Settings... (General, Appearance, Track colours, Keys) |
| Workspace / sound browser with previews | ❌ | G-16 |
| I/O Setup | 🟡 | Busses and paths; no hardware channel matrix (G-01) |
| Preferences | 🟡 | Many settings stored, fewer honoured; the Settings window holds interface scale, reduce motion, tooltips (saved, not yet read by the UI), menu and toolbar style, themes and track colours |
| Window configurations, Tile/Cascade | 🟡 | Configurations ✅; Tile/Cascade in PR #151 |
| Plugin windows | 🟡 | Generic parameter sliders; Pro Tools plugins have bespoke GUIs (#138 asks for skeuomorphic design); multi-page plugin views (#6) |
| A plugin opens when it's inserted | ✅ | #486; a hosted plugin's own editor, else its plugin window |
| Command palette (Search) | ✅ | Over every command |

## Look and meters

| Feature | Status | Notes |
|---|---|---|
| Dark theme close to Pro Tools' | ✅ | Light theme (#32) and live system appearance (#85) beyond Pro Tools |
| Interface scaling | 🟡 | Slider in PR #147 |
| Meter types and ballistics (peak, RMS, VU, K-scales, gain reduction) | 🟡 | Peak/RMS; meter type choice and GR meters partial |
| Clip indicators, track colours, waveform views (peak/power/rectified/outlines) | ✅ | |
| Fader resolution, fine adjust, numeric entry | 🟡 | |
| macOS system menu bar | ✅ | #10 |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-13 | minor | A plugin opens when it's inserted (#486) |
| 2026-10-13 | minor | Drag across checkboxes (#399) |
| 2026-10-22 | minor | Mac queue complete and promoted (tag rel-2026-10-22): Standard and Classic menus and toolbars, Settings window, themes, panels; later items: Inspector EQ curve, MIDI track controls, Mix-window panels, side-by-side docking. Readiness estimate unchanged |
| 2026-10-22 | minor | Inspector sections, 84-colour palette, glossy rounded clips and headers, bottom Mixer pane (Editor / Mixer tabs). Readiness estimate unchanged |
| 2026-10-23 | minor | Alternative key set, Settings > Keys page, per-command keys, catalogue-driven context menus for clips, empty lanes, tracks, the ruler, the piano roll and controller lanes. Readiness estimate unchanged |
| 2026-10-13 | minor | Waveforms and fade curves share temporary envelopes during drag, without rebuilding media or editing the document before release. Readiness estimate unchanged |
| 2026-10-13 | minor | Fade curves use bounded physical-pixel sampling; midpoint handles stay visible and inside the clip, with repeat-drag coverage after extreme bends. Readiness estimate unchanged |
| 2026-10-13 | minor | Continuous fade curvature from a midpoint drag, shared UI/audio envelope, one-step undo and saved session data. Readiness estimate unchanged |
| 2026-10-13 | minor | Existing fades have draggable upper endpoints with matching hover/drag cursors, including at adjacent clip boundaries. Readiness estimate unchanged |
| 2026-10-13 | minor | Fade drags use the same thin S-curve renderer as committed fades, replacing the dragged side's old curve. Readiness estimate unchanged |
| 2026-10-13 | minor | Bounded Smart-tool fade previews and commits; full-clip fades target only the dragged clip and side. Readiness estimate unchanged |
| 2026-10-12 | minor | Mix window: Mix in Separate Window (Edit and Mix on screen at once) |
| 2026-10-10 | major | Created: tools, modes, shortcuts, windows and meters checklist against Pro Tools 2026.4.1 |
