# Control protocol

SoundCraft can be driven completely by other programs: scripts, test harnesses and AI agents.
Every menu item, button, mouse gesture and keystroke maps to a **command** (or to synthetic
input), and every command is reachable from:

| Surface | How |
|---|---|
| Desktop app control channel | `soundcraft --control <port>` (`0` = pick a free port; printed as `SOUNDCRAFT_CONTROL_PORT=…`) |
| CLI against a running app | `soundcraft-cli app --port <port> <method or command id> [JSON]` |
| Headless CLI | `soundcraft-cli run --demo --cmd 'mix.volume={"track":"Kick","db":-6}' --bounce out.wav` |
| MCP (headless) | `soundcraft-cli mcp [--demo]` |
| MCP (bridged to the app) | `soundcraft-cli mcp --connect <port>` |

## Isolated automation profiles

Set `SOUNDCRAFT_CONFIG_DIR` to a fresh directory to isolate UI preferences, `Autosave`, `Presets`, and `logs`.
The nonempty value is used as the complete settings directory, even in portable mode (`portable.txt` beside the executable, which otherwise uses `SoundCraftData` next to it). Unset retains the normal platform paths;
an explicit empty value disables the settings directory instead of falling back to a personal profile. Save synthetic sessions to explicit scratch paths. Do not use `SOUNDCRAFT_NO_PREFS`
when testing preference persistence across restarts.

Run UI acceptance with `--no-audio`. This disables the output player; it does not disable recording
input opened by arm/monitor commands. An automated UI smoke run must never arm a track, enable input
monitoring, or start recording. Control input injects egui events; it does not establish operating-system
input delivery or screen-reader acceptance. Native screenshots use the presented viewport frame.

## Wire format

Newline-delimited JSON over TCP on `127.0.0.1`. One request per line, one reply per line:

```json
{"id": 1, "method": "engine.execute", "params": {"command": "track.new", "params": {"count": 2, "format": "Stereo"}}}
{"id": 1, "ok": true, "result": {"tracks": [12, 13]}}
```

Errors: `{"id": 1, "ok": false, "error": "bad parameters for `mix.volume`: no track named `Nope`"}`.

## Methods

| Method | Params | What it does |
|---|---|---|
| `engine.execute` (alias `command`) | `{command, params}` | Run a command programmatically. **Never opens a dialog**; empty params use defaults or the current selection. |
| `engine.commands` | `{filter?}` | Every command: id, label, menu path, shortcut, params doc, enabled + reason. Includes UI-layer commands (`window.*`). |
| `engine.parity` | `{}` | Parity report against the incumbent's menu catalog. |
| `session.inspect` | `{detail?: "summary"|"full"}` | The document: tracks, clips (with times), inserts (+params when full), sends, routing, automation, markers, groups, busses, sources (`loaded` true when decoded audio is in the pool), selection, zoom (`samples_per_px`, `scroll`), transport, undo label. |
| `ui.inspect` | `{}` | UI state (`window`, panels, dialogs), window size, playing/position, audio device, and `edit_layout` (timeline rect and every track row's rect, for clicking). |
| `ui.set` | any `UiState` fields | e.g. `{"window": "Mix", "narrow_mix": true}`. |
| `ui.menu.list` | `{}` | Every catalog menu path and the command that implements it (or null). |
| `ui.menu.invoke` | `{path}` or `{id}` | Like clicking the menu item: may open its dialog (e.g. `"Track > New..."`). |
| `ui.click` | `{x, y, button?, count?, cmd?, shift?, alt?, ctrl?}` | Synthetic click (window points). |
| `ui.drag` | `{x, y, to_x, to_y, steps?}` | Synthetic drag with the primary button. |
| `ui.scroll` | `{x, y, dx?, dy?, unit?: "point"\|"line", steps?, cmd?, shift?, alt?, ctrl?}` | A two-finger trackpad swipe over `(x, y)`: `dx`/`dy` points spread over `steps` frames (default 8). Positive values move the content right/down (natural scrolling). `unit: "line"` sends mouse-wheel notches instead. With `cmd` it zooms; with `shift` it scrolls sideways. |
| `ui.zoom` | `{x, y, factor, steps?}` | A trackpad pinch around `(x, y)`: `factor` > 1 spreads the fingers (zooms in). |
| `ui.move` | `{x, y}` | Move the pointer. |
| `ui.key` | `{key, cmd?, shift?, alt?, ctrl?}` | Key press (`Space`, `Enter`, `A`, `F7`, `=`…). |
| `ui.text` | `{text}` | Type text into the focused field. |
| `ui.screenshot` | `{path?}` | PNG of the window (base64 in `png_base64` when no path). |
| `app.quit` | `{}` | Quit. |

## Positions and references

- Tracks: id (number) or name (`"Kick"`).
- Clips: id (from `session.inspect`).
- Positions: samples (`96000`), seconds (`{"seconds": 2.5}`), or a time string in any timebase,
  optionally prefixed: `"bars_beats:5|1|000"`, `"min_secs:0:12.500"`, `"timecode:00:00:10:00"`,
  `"samples:48000"`.
- Ranges: `start` + `end` (or `start` + `length`); omitted = the current edit selection.

## Examples

```sh
P=$(soundcraft --demo --control 0 | sed -n 's/SOUNDCRAFT_CONTROL_PORT=//p')   # or read stderr
soundcraft-cli app --port $P mix.solo '{"track":"Bass"}'
soundcraft-cli app --port $P transport.play
soundcraft-cli app --port $P ui.screenshot '{"path":"/tmp/shot.png"}'
soundcraft-cli app --port $P ui.menu.invoke '{"path":"Track > New..."}'
```

Offscreen (no window): `cargo run -p soundcraft-ui-egui --example ui_shot -- out.png script.jsonl`,
where each script line is a request (`{"method": …, "params": …}`), `{"shot": "path.png"}` or
`{"steps": n}`. Use `--system-theme light|dark|none` to supply the initial OS appearance, and
script events such as `{"system_theme": "dark"}` to change it on subsequent frames.

Appearance defaults to Dark. `ui.theme {"mode":"system"}` follows the appearance supplied by
the native or web integration at launch and on later frames, while the saved choice stays System.
When the integration cannot report an appearance, System uses Dark. Manual Light and Dark stay
fixed when the OS appearance changes. An offscreen render without `--system-theme` supplies no
OS appearance and therefore resolves System to Dark.

## Themes

`ui.theme_*` commands edit a custom colour theme on top of Dark, Light or Midnight. The first
edit forks an unsaved "Custom (from Dark)" copy that is saved with the UI preferences.
`ui.theme_get {}` returns the name, base, `scope`, `dirty`, every colour as `#RRGGBB`, the settings
and contrast warnings.
`ui.theme_set_color {key, color}` (an unknown key lists the valid ones),
`ui.theme_set_param {key, value}` (`clip_opacity`, `clip_opacity_selected`, `grid_strength`,
`track_tint`, `grid_beat_always`), `ui.theme_set_track_color {index 0-15, color}` and
`ui.theme_reset {key?}` change the working copy.

Themes live in scopes: `builtin` (Dark, Light, Midnight, Colour-safe), `shipped` (the repository's
`themes/` folder, embedded in every build, read-only), `shared` (a folder of theme files meant to
be committed to git), `user` (the `themes` folder next to `ui.json`, shown as "This computer") and
`memory` (kept until quit, only when there is no folder to write to). Shipped and built-in themes
cannot be saved over, renamed or deleted. A shared theme with the same name hides the shipped one;
a shared and a user theme may share a name (both are listed; `ui.theme_load` without a scope
prefers shared). Names compare case-insensitively.

- `ui.theme_list {}`: `themes` (each with `name`, `scope`, `read_only`, `builtin`, `active`,
  `path` for files), `active`, `shared_dir`, `user_dir` (`themes_dir` is the same), `default_scope`,
  `prefs_enabled` and `skipped` (`[{scope, file, reason}]`: files that were not accepted, because
  they are larger than 256 KiB, not UTF-8, not valid JSON, not a SoundCraft theme file, have a bad
  name or repeat a name; a newer version still loads).
- `ui.theme_load {name, scope?}`.
- `ui.theme_save {name?, scope?}`: scope `shared` or `user`; without it the active theme's scope
  if it is shared or user, else the `theme_default_scope` UI preference, else shared when a shared
  folder is set, else user. A built-in name is refused; a name that exists in the same scope is
  overwritten.
- `ui.theme_duplicate {name, to, scope?}`, `ui.theme_import {path?, scope?}`,
  `ui.theme_rename {from, to, scope?}`, `ui.theme_delete {name, scope?}`,
  `ui.theme_export {path?}` (exports the working or active theme).
- `ui.theme_move {name, from, to}`: copy a saved theme between `shared` and `user` (or from
  `memory`), then remove the original; refuses when the target already has that name.
- `ui.theme_folder {path?, detect?}`: no arguments reports `{shared_dir, detected, source}` with
  `source` `pref`, `detected` or `off`. `path` sets the shared folder (a folder that may not exist
  yet; a file is refused), `path: ""` turns it off, `path: null` or `detect: true` goes back to
  the detected folder. The setting is the `shared_themes_folder` UI preference.

Save, duplicate, import and move report `saved_to` (`shared`, `user` or `memory`), `path` (files)
and `warning`. With no folder to write to (`SOUNDCRAFT_NO_PREFS`, tests, the web) the theme is kept
in memory, the call succeeds with `saved_to: "memory"` and a warning that it is NOT saved to disk.
A failed write to a folder (the shared folder included) is an error naming the folder; nothing is
redirected elsewhere and the working copy stays unsaved.

Files are canonical so that git diffs stay small: `<slug>.json` (lowercase name, letters and digits
kept, space, `-` and `_` become `-`, at most 64 characters, `-2`, `-3` when another theme has the
slug), sorted keys, 2-space indent, unset settings left out, LF line ends and one trailing newline;
saving a theme twice gives identical bytes. An older `<Name>.json` still loads and is replaced by
the slug file the next time it is saved. The desktop app detects the shared folder only when it runs
from a cargo build folder of a source checkout that has a `themes/` folder (never for installed
apps), and not at all under `SOUNDCRAFT_NO_PREFS`. A theme's track palette is pushed to the engine
with `track_palette.set` when the theme applies and cleared for built-in themes. The
Settings window (see Settings) is the UI for these commands.

## Settings

The Settings window has three pages: `general`, `appearance` and `tracks` (track colours).

- `window.settings {tab?: "general"|"appearance"|"tracks", value?: bool}` shows, hides or toggles the
  window. With `tab` it opens on that page; without `tab` a closed window opens on General. An
  unknown tab is an error. Returns `{value, tab}`. Menu: Setup > Preferences... and Window > Settings...
  (the engine command `setup.preferences` still returns the edit state for scripts).
- `window.ui_customization {tab?, value?}` is the same command with `appearance` as the default page
  (Setup > Appearance).
- `ui.prefs {ui_scale?: 0.75-1.5, reduce_motion?: bool, tooltips?: bool, reset?: true}` sets the
  interface preferences (scale values outside the range are clamped; wrong types are errors and
  change nothing) and returns all three. They are saved with the UI preferences (`ui.json`) and
  restored at launch; no undo. `reset` returns them to 1.0, false and true.
  Key sets (Settings > Keys) use the same command: `keymap_preset: "default"|"alternative"`,
  `keymap_overrides: {action: [chord, ...]}` (an empty list unbinds the action, `null` restores its key
  set keys; an action is a command id, or `id#{json params}` for a fixed-parameter action such as
  `edit.tool#{"tool":"zoom"}`), `keymap_reassign: true` (take a chord another command holds; without it a
  taken chord is an error naming that command) and `keymap_reset: true` (drop every override). Chords
  are written `Cmd+Shift+K`, `Alt+G`, `F3`, `Delete` (`Cmd` is Ctrl off macOS). A chord that does not parse,
  an unknown command or a wrong type is an error and nothing changes. The reply adds `keymap_preset` and
  `keymap_overrides`. Both are saved with the UI preferences; saved keys for commands this version lacks
  are dropped at load.

The other General controls use `ui.theme`, `ui.menu_layout`, `ui.toolbar_style`,
`ui.header_style`, `ui.transport_position` and `window.config_auto_update` (not saved). The
Appearance and Track colours pages use the `ui.theme_*` commands described under Themes.

## Track colour, gloss and the Mixer pane

- `track.color {tracks?, color, apply_to_clips?}`: `color` is `[r,g,b]` (three whole numbers 0-255; anything else is an error), a `"#RRGGBB"` string, or a palette index. Indices 0-15 are the default set (`TRACK_COLORS`); 16-99 are the extended 84-colour grid (index minus 16); anything else is an error (an index no longer wraps). Tracks store plain RGB, so saved sessions are unchanged. `apply_to_clips: true` also clears the colour of clips that carry their own, so they follow the track; it is one undo step. Clips without their own colour follow the track colour either way.
- Theme keys: `ui.theme_set_param {key, value}` and `ui.theme_reset {key}` accept `clip_gloss` (0 to 1; 0 is flat, default 1.0 Dark, 0.8 Light, 0.6 Midnight), `clip_corner` and `header_corner` (points, 0 to 8; defaults 4 and 3). `ui.theme_get` reports them. They are saved in themes; older themes load with the palette defaults. Settings > Appearance has the "Glossy clips" switch and "Corner radius" slider.
- `window.mixer_panel {value?}` shows, hides or toggles the Mixer pane; `ui.dock {operation: "open", panel: "mixer"}` does the same. Panel id `mixer`, Edit window only (the Mix window refuses it like the Inspector and Master panels). The Editor (`midi`) and the Mixer share the lower zone as exclusive tabs: opening one shows it in place of the other, and `ui.dock {operation: "activate", panel}` switches between them. The pane's Compact switch and the Inspector collapse state are saved with the UI preferences.
- `window.inspector {value?}` shows, hides or toggles the Inspector (`value` true or false sets it, omitted toggles) and returns `{value}`; `ui.dock {operation: "open", panel: "inspector"}` does the same. Panel id `inspector`, Edit window only: the Mix window never shows it and `ui.dock` refuses the id there.
- `window.loudness_panel {value?}` shows, hides or toggles the Loudness pane (`value` true or false sets it, omitted toggles) and returns `{value}`; `ui.dock {operation: "open", panel: "loudness"}` does the same. Panel id `loudness`, Edit window only: the Mix window never shows it and `ui.dock` refuses the id there.
- `window.meter_panel {value?}` shows, hides or toggles the Meters pane (`value` true or false sets it, omitted toggles) and returns `{value}`; `ui.dock {operation: "open", panel: "meter"}` does the same. Panel id `meter`, Edit window only: the Mix window never shows it and `ui.dock` refuses the id there.
- `window.visibility_pane {value?}` shows, hides or toggles the Visibility pane (`value` true or false sets it, omitted toggles) and returns `{value}`; `ui.dock {operation: "open", panel: "visibility"}` does the same. Panel id `visibility`, Edit window only: the Mix window never shows it and `ui.dock` refuses the id there.
- `window.midi_editor {value?}` shows, hides or toggles the MIDI Editor pane (`value` true or false sets it, omitted toggles) and returns `{value}`; `ui.dock {operation: "open", panel: "midi"}` does the same. Panel id `midi`, Edit window only: the Mix window never shows it and `ui.dock` refuses the id there. It shares the lower zone with the Mixer as exclusive tabs (see below).

## Panel docking

Tracks and Groups, Clips, MIDI Editor, Memory Locations, and Undo History use movable panels. Commands target the active Edit or Mix view; floating panels stay inside the current viewport. Edit and Mix retain separate arrangements; Clips and MIDI Editor belong to Edit. Transport, plug-in, and specialist utility windows retain their existing hosts. The document/session view is protected. Layouts, active tabs, panel visibility, and floating rectangles are saved with the app's existing UI preferences. These operations do not edit document contents.

Panel IDs: `tracks`, `clips`, `midi`, `memory`, `history`, `master`, `visibility`, `meter`, `loudness`, `inspector`, `mixer`. The protected center is `canvas`.

`ui.dock` accepts `operation` and `panel`. Operations are `open`, `close`, `activate`, `float` (with finite positive `rect: [x, y, width, height]`), or `move` (with `target` and `zone: center|left|right|top|bottom`). Center groups with the target; side zones split beside it. `ui.dock.reset` restores the default arrangement. Invalid moves return an error without changing the layout.

Invoke these UI commands through `engine.execute`, for example `{"command":"ui.dock","params":{"operation":"float","panel":"tracks","rect":[100,100,320,440]}}`.

Advanced `ui.dock` operations use the same validated transaction as pointer gestures:
`moveFloating` updates `rect`, `resizeSplit` accepts a boolean child `path` and
`size` (`{"Ratio":0.5}`, `{"FixedFirst":280}`, or `{"FixedSecond":280}`),
`setStackOpen` takes `open`, and `resizeStack` takes a point `height` or null.
A center `move` can include `before` (panel ID or null to append) for tab order.
The equivalent `{"action": ...}` form accepts a serialized shared docking action;
this is also the form emitted by the renderer. Protected-canvas and app-specific panel
rules apply equally to both forms.

## Presentation commands (`ui.*`, `edit.grid_adaptive`, `track_palette.*`)

All of these run through `engine.execute`. None is part of the undo history; the `ui.*` ones are not stored in the session.

| Command | Params | Result |
|---|---|---|
| `ui.menu_layout` | `{layout?: "standard"\|"classic"}` (`value` is accepted too) | `{layout}`: sets the menu bar arrangement when given, always reports the current one. Standard groups the menus by task (File, Edit, Project, Audio, MIDI, Media, Transport, View, Setup, Window, Help); Classic is the catalog order. Every item keeps its command and path in both. An unknown name is an error. |
| `ui.toolbar_style` | `{value?: "standard"\|"classic"}` (`style` is accepted too) | `{style}`: sets the toolbar style when given, always reports the current one. Unknown names are an error. |
| `edit.grid_adaptive` | `{value?: bool}` | `{value}`: a session flag. When on, grid lines and snapping use a grid coarsened to the zoom so lines are never closer than 12 points; omit `value` to toggle. Off by default. |
| `track_palette.set` | `{colors: [[r,g,b] \| "#RRGGBB", ...]}` | `{colors}`: the colours new tracks cycle through, instead of the built-in list. Each entry is `[r,g,b]` (0-255) or a hex string (the leading `#` is optional). Any invalid entry is an error and nothing changes. |
| `track_palette.clear` | `{}` | `{colors: null}`: back to the built-in colours. |
| `track_palette.get` | `{}` | `{colors}`: the override, or `null` when none is set. |

## Automation drawing and MIDI controllers

Automation parameter ids (`param`) are `volume`, `pan`, `pan2`, `mute`, `send_a_level`, `send_a_pan`, `send_a_mute` (slot letter or number), and `plugin:<slot letter>:<param id>`. MIDI controller lanes, only valid on instrument and MIDI tracks (other tracks get an error), use:

- `cc0` ... `cc127`: a continuous controller, range 0..127. `cc64` to `cc69` are switches (they step between their limits instead of ramping).
- `pitch_bend`: range -8192..8191.
- `pressure`: channel pressure, range 0..127.

Ids are case-insensitive. A `cc` number above 127 is rejected.

| Command | Params | Result |
|---|---|---|
| `automation.draw` | `{track?, param, points: [{at, value}], replace?: true}` | `{written, removed}`: writes a freehand stroke into a lane. At most 8192 points per call (more is an error). Points are sorted, a repeated `at` keeps the last value, `at` is clamped to 0 or later, `value` is clamped to the lane's range, and switch-like lanes (mute, cc64-69, plugin toggles) are snapped to their limits with the step placed at the point. With `replace` (default true) the points inside the swept range are removed, and anchor points hold the old curve one sample before and after the stroke, so the curve outside is kept. With `replace: false` the points are only added. Targets `track` or the selected tracks. Refused when the lane is locked. |
| `automation.erase_range` | `{track?, param, start, end}` | `{removed}`: removes the points from `start` up to `end` (swapped if reversed; positions as in "Positions and references"). Anchors keep the curve outside the range. An empty range removes nothing. Refused when the lane is locked. |
| `automation.lane_draw` | `{track?, index, draw?: bool}` | `{index, draw}`: sets or toggles (when `draw` is omitted) the draw mode of the shown lane at `index` on the track. Errors when the lane does not exist. |
| `automation.midi_value` | `{track?, param: cc<n>\|pitch_bend\|pressure, value, at?}` | `{at, value}`: writes one breakpoint. `at` defaults to the playhead while playing and otherwise to the edit insertion; `value` is clamped to the controller's range. Errors on a non-MIDI `param`, on a non-MIDI track, or on a locked lane. |
| `automation.thin` | `{tracks?, tolerance?: 0.1, param?, start?, end?}` | `{removed}`: removes points that lie within `tolerance` of the curve. Without `param` every lane of the targeted tracks is thinned; with `param` only that lane. With both `start` and `end` only the points between them are considered. A locked lane named by `param` is refused. `automation.thin_all {tolerance?}` does the same over every track. |
| `automation.to_clip` | `{track?, param: cc<n>\|pitch_bend\|pressure, start?, end?, clear_lane?: true}` | `{clips, created, events, removed_points}`: writes the lane into the controller events of the MIDI clips under the range (default: the lane's first to last point; both ends included). A clip named "Automation" is created when no MIDI clip is under the range. Switch lanes write their held values; continuous lanes are sampled every 32nd note and thinned to within half a controller step. Events of that controller inside the range are replaced; those outside stay. Events take the track's channel override (`track.midi_params`). With `clear_lane` (default) the converted points are removed, and the lane goes away when none are left; points outside the range stay and still own the controller. One undo step. Errors: not a MIDI controller, an audio track, no such lane or an empty one, a locked lane. |
| `automation.merge_into_clip` | `{track?, param, clip, start?, end?, clear_lane?: false}` | `{events, removed_points}`: like `automation.to_clip` into one named MIDI clip of the track (range default: the clip), never creating a clip. |

**Locked lanes.** A lane locked with `automation.lane_lock` keeps its points. Single-lane commands that name the lane (`automation.to_clip`, `automation.merge_into_clip`, `automation.set_point`, `write_range`, `clear`, `thin` with `param`, `draw`, `erase_range`, `midi_value`) refuse it with the error "lane is locked". Bulk edits that touch several lanes skip the locked ones and still apply to the rest: Cut, Clear Automation, Paste and its automation variants, Duplicate, Insert Silence, clip moves and Shuffle ripples that carry automation, `event.insert_time` and `event.cut_time`, `automation.write_to_current` / `write_to_all`, `automation.clear` and `thin` without `param`, the glide and trim edits, and the trim and volume helpers (`track.trim_automation_write`, `track.coalesce_trim`, `track.clear_trim_automation`, `track.coalesce_vca`, the clip-gain-to-volume conversions; a helper that writes the Volume lane skips a track whose Volume lane is locked). Undo is never blocked by a lock.

## MIDI track parameters and audition

Playback settings of instrument and MIDI tracks, kept in the session's numeric settings (nothing is written into the clips; old sessions load unchanged):

| Command | Params | Result |
|---|---|---|
| `track.midi_params` | `{tracks?, transpose?: -48..48, velocity?: -127..127, channel?: 1..16 \| null}` | `{tracks: [{track, transpose, velocity, channel}]}`: transposes the track's notes by semitones, adds a velocity offset (the result stays 1..127), and sends everything on one MIDI channel (`null` or no value clears the override, default channel 1). Values are rounded and clamped; zero and `null` remove the setting. At least one of the three is required; audio tracks are refused. One undo step. While playing, a change of transpose or channel first releases the sounding notes with the values they started with, so no note is left hanging, and each note-off matches its note-on. The channel reaches notes of hosted plugins that take raw MIDI (CLAP in MIDI mode), and controllers and poly pressure of every instrument; built-in instruments and plugin formats without a channel on their note input ignore it. `transpose` and `velocity` are the same values `event.midi_rtp` sets. |
| `track.midi_params_get` | `{tracks?}` | `{tracks: [...]}`: the same entries for the MIDI tracks (a query). |
| `options.audition_tail_ms` | `{value?: 0..2000}` | `{value}`: how long an auditioned note's release keeps sounding after the key is released, in milliseconds (default 300, not stored while it is the default; no `value` reads it). Not an undo step. |

`file.export_midi` takes `lanes` (default false): MIDI controller automation lanes are also written as controller events, replacing the clips' own events of that controller between the lane's first and last point.

## Master loudness

| Command | Params | Result |
|---|---|---|
| `mix.loudness` | `{offline?: false}` | `{momentary, short_term, integrated, max_momentary, true_peak_db, source}`: BS.1770 K-weighted loudness of the master in LUFS, true peak in dBTP. `-144` means nothing has been measured. By default the live meter that playback feeds (`source: "live"`). With `offline: true` the session is rendered from 0 to its content end and measured whole (`source: "offline"`; an empty session reads `-144`). |
| `mix.loudness_reset` | `{}` | `{}`: restarts the live integrated, maximum momentary and true-peak measurement. |

## Templates

A template is a session file without clips or media: tracks, mixer, plugin slots and their saved state. User templates live in the per-user app folder, `templates` subfolder (macOS `~/Library/Application Support/SoundCraft/templates`, Windows `%APPDATA%\SoundCraft\templates`, elsewhere `$XDG_CONFIG_HOME/soundcraft/templates` or `~/.config/soundcraft/templates`). Templates shipped with the app are read from a `templates` folder beside the executable, or in the working directory. Every command below except `instrument_rack` also accepts `dir` to use one explicit folder instead. The two folders are the settled design: a template saved without `dir` always goes to the per-user folder, so shipped templates are never overwritten, and a name is looked up in the per-user folder before the shipped one.

| Command | Params | Result |
|---|---|---|
| `template.dir` | `{}` | `{user, shipped}`: the two folders (`null` when there is none; `shipped` is null when the folder does not exist). |
| `template.list` | `{dir?}` | `{templates: [{name, path, source: "user"\|"shipped"}]}`. With `dir` only that folder is listed, as `user`. |
| `template.save` | `{name, replace?: false, dir?}` | `{path}`: writes the open session without clips or media. `name` is reduced to letters, digits, space, `-`, `_` and `.` (at most 64 characters); an empty result is an error. An existing file is an error unless `replace` is true. The folder is created when needed. |
| `template.new_session` | `{name \| path, session_name?, dir?}` | `{name, tracks}`: replaces the open session with the template, unsaved (the document has no path, so Save asks for one). Like `session.new` it is journaled (replays and macros that start from a template reproduce the document) and not undoable. `path` is checked against the automation read root, as `session.open` does. `name` is looked up in the user folder, then the shipped one; `path` is a file. Any clips, sources or videos in the file are dropped. The session is called `session_name`, or "Untitled". An unknown template is an error. |
| `template.instrument_rack` | `{count?: 10, names?: [string], prefix?: "Instrument", instrument?: plugin id}` | `{tracks: [ids]}`: adds `count` (1 to 128) stereo instrument tracks in one undo step, each loaded with `instrument` (default `subtractive_synth`; a plugin that is not a known instrument is an error). Track *n* is named `names[n]`, or `<prefix> <n+1>` when that entry is missing or blank. `names` that is not an array of strings is an error. |

In the app, File has **Save As Template...**, **New from Template...** and **Instrument Rack...** (reached by `ui.menu.invoke`, which opens the matching dialog); calling the commands directly never opens a dialog. **New from Template...** asks to save first when the open session has unsaved changes.
