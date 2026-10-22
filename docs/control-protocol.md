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

## Panel docking

Tracks and Groups, Clips, MIDI Editor, Memory Locations, and Undo History use movable panels. Commands target the active Edit or Mix view; floating panels stay inside the current viewport. Edit and Mix retain separate arrangements; Clips and MIDI Editor belong to Edit. Transport, plug-in, and specialist utility windows retain their existing hosts. The document/session view is protected. Layouts, active tabs, panel visibility, and floating rectangles are saved with the app's existing UI preferences. These operations do not edit document contents.

Panel IDs: `tracks`, `clips`, `midi`, `memory`, `history`. The protected center is `canvas`.

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

## Master loudness

| Command | Params | Result |
|---|---|---|
| `mix.loudness` | `{offline?: false}` | `{momentary, short_term, integrated, max_momentary, true_peak_db, source}`: BS.1770 K-weighted loudness of the master in LUFS, true peak in dBTP. `-144` means nothing has been measured. By default the live meter that playback feeds (`source: "live"`). With `offline: true` the session is rendered from 0 to its content end and measured whole (`source: "offline"`; an empty session reads `-144`). |
| `mix.loudness_reset` | `{}` | `{}`: restarts the live integrated, maximum momentary and true-peak measurement. |

## Templates

A template is a session file without clips or media: tracks, mixer, plugin slots and their saved state. User templates live in the per-user app folder, `templates` subfolder (macOS `~/Library/Application Support/SoundCraft/templates`, Windows `%APPDATA%\SoundCraft\templates`, elsewhere `$XDG_CONFIG_HOME/soundcraft/templates` or `~/.config/soundcraft/templates`). Templates shipped with the app are read from a `templates` folder beside the executable, or in the working directory. Every command below except `instrument_rack` also accepts `dir` to use one explicit folder instead.

| Command | Params | Result |
|---|---|---|
| `template.dir` | `{}` | `{user, shipped}`: the two folders (`null` when there is none; `shipped` is null when the folder does not exist). |
| `template.list` | `{dir?}` | `{templates: [{name, path, source: "user"\|"shipped"}]}`. With `dir` only that folder is listed, as `user`. |
| `template.save` | `{name, replace?: false, dir?}` | `{path}`: writes the open session without clips or media. `name` is reduced to letters, digits, space, `-`, `_` and `.` (at most 64 characters); an empty result is an error. An existing file is an error unless `replace` is true. The folder is created when needed. |
| `template.new_session` | `{name \| path, session_name?, dir?}` | `{name, tracks}`: replaces the open session with the template, unsaved (the document has no path, so Save asks for one). `name` is looked up in the user folder, then the shipped one; `path` is a file. Any clips, sources or videos in the file are dropped. The session is called `session_name`, or "Untitled". An unknown template is an error. |
| `template.instrument_rack` | `{count?: 10, names?: [string], prefix?: "Instrument", instrument?: plugin id}` | `{tracks: [ids]}`: adds `count` (1 to 128) stereo instrument tracks in one undo step, each loaded with `instrument` (default `subtractive_synth`; a plugin that is not a known instrument is an error). Track *n* is named `names[n]`, or `<prefix> <n+1>` when that entry is missing or blank. `names` that is not an array of strings is an error. |

In the app, File has **Save As Template...**, **New from Template...** and **Instrument Rack...** (reached by `ui.menu.invoke`, which opens the matching dialog); calling the commands directly never opens a dialog. **New from Template...** asks to save first when the open session has unsaved changes.
