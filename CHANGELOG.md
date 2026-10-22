# Changelog

Dated entries, newest first. Each entry groups changes as Added, Changed, Fixed and Decisions.

## 2026-10-22

### Added
- Keyboard icon (drawn in code) and, on instrument track headers, a key button that opens the loaded instrument's plugin window.
- Controller commands for the MIDI editor: `midi.ctrl_list`, `midi.ctrl_set`, `midi.ctrl_clear` (continuous controllers, pitch bend, pressure), with tests.
- MIDI note commands `midi.note_split` and `midi.notes_glue`, with tests.
- Piano roll: lane selector (velocity, two controllers, sustain, pitch bend) with draw, erase and clear; bar ruler; playhead; horizontal zoom (Ctrl/Cmd+wheel, Fit); horizontal and vertical scrollbars; Shift+wheel sideways scroll; Follow-playhead; Loop button.
- Piano roll tool palette: Select, Pencil, Erase, Split, plus a Glue action.
- Piano roll: a single click with the Select tool places a play-from position (editor state only; Space plays from it); double-click on empty roll adds a note; ruler click or drag locates the transport.
- Piano roll zoom buttons: H- and H+ (horizontal, 1.25x steps within the existing limits) and V- and V+ (vertical, row height 6 to 32 px in steps of 2) next to Fit, with hover text.
- MIDI editor panel header controls drawn in code: Float/Dock, Minimize (collapse to header) and Close.
- Tests: zoom step helper limits; zoom buttons change view state and leave the session unchanged; float, dock, minimize, close and reopen keep `show_midi_editor` in sync.
- Resizable velocity/controller lane: a drag handle (resize cursor, grip line) in the gap between the piano roll and the lane changes the lane height (default 64 px, minimum 40, maximum editor height minus 120); double-click resets it. Applies to the velocity lane and every controller lane; editor state only, no session change.
- Tests: lane height clamp helper; dragging the handle resizes the lane and leaves the session unchanged; double-click resets; header layout never overlaps and follows the priority order (unit test on a pure layout function); the rendered header at 760, 1100 and 1600 pt panel widths; minimized state resets on close.
- Note audition engine: live notes can be played into a track's instrument while the transport is stopped or playing. New `MixEngine::live_note`, `live_all_off` and `live_active` (`crates/mix/src/live.rs`), a lock-free single-producer single-consumer event queue of 256 slots (`crates/playback/src/live.rs`), and `Player::audition(track, pitch, velocity, on)` and `Player::audition_all_off()`. A held live note is released after 2 s if its release never arrives, and the instrument keeps rendering for a 1.5 s release tail.
- Command `midi.audition {track, pitch, velocity?: 100, on}` (no undo entry, no document change) and `TransportRequest::Audition`; the UI host forwards it to the player.
- Tests: queue order, wraparound, full queue and all-off round trip; live notes reaching a recording instrument while stopped; balanced on/off; timeout and tail; reset and instrument replacement clear live notes; unknown track, loading and failed instruments are ignored; the built-in synth is audible at a fixed position while stopped; clip notes are unchanged while playing; a fill-level test through the audio callback; `midi.audition` validation.

- Piano roll note audition (`crates/ui-egui/src/midi_audition.rs`): clicking a note, starting a note drag, drawing or double-clicking a new note, or pressing a key in the keyboard gutter sounds the track's instrument through `midi.audition`. A plain click holds the note about 150 ms; drags and gutter presses hold until release; dragging a note vertically (or sliding along the gutter keys) sends note-off for the old pitch and note-on for the new. Every on is released on pointer release, tool change, turning the toggle off, clip change, closing the editor, or by a per-frame safety. New `Audition` toggle in the editor header (preference `audition`, default on), part of the responsive header layout and its overflow menu.
- Panel discoverability (`crates/ui-egui/src/panel_menu.rs`): Window menu entries for every panel (Tracks and Groups, Clip List, MIDI Editor, Memory Locations, Undo History) with check marks that follow the panel state, `Window > Show All Panels` (command `window.show_all_panels`), a close button and a menu button (Hide, Float, Dock to default, panel list) on every dock tab strip (the MIDI editor keeps its own header controls; the Session tab has only the menu), the panel list in the tab right-click menu, and thin hint strips ("Tracks hidden - click to show" at the left edge, "MIDI editor hidden - click to show" at the bottom of the Edit window) that bring a hidden pane back.
- Tests: audition state machine (click, drag, vertical pitch drag, pencil, gutter drag, editor close, toggle off, clamping); UI tests for exactly one audition on then one off per note click, no audition with the toggle off, drag pitch changes, editor close, pencil and gutter; panel hide/show round trips for every panel, hint strips only while hidden and restoring on click, menu check marks, Show All Panels, old saved preferences load.
- Add Track menus (`track_add_menu.rs`): right-click the empty area under the tracks, or use "Add Track" in a track's context menu, for Audio, Instrument, MIDI, Aux and Folder tracks, plus "Add Multiple..." (the New Tracks dialog). A track added from a track's menu lands right after that track (after its folder contents when it is a folder). `track.new` gained an optional `after` parameter for this.
- Folder tracks (`folder_ui.rs`): a fold triangle on folder headers (down when open, right when closed, calls `track.folder_toggle`), thin colour guide bars down the left edge of child headers (one per enclosing folder), existing indent kept; the track menu keeps Move to Folder and Remove from Folder; children are hidden while the folder is closed.
- Piano roll toolbar: snap dropdown (Grid = follow the session grid, Off, Bar, 1/2 to 1/64, triplet and dotted variants) that drives note placement, drag and split snapping and default note length; clip selector for the MIDI clips on the edited clip's track; quantize strength (0 to 100) next to the Quantize button. All take part in the responsive header layout and the overflow menu. UI commands `ui.midi_tool {tool}` and `ui.midi_snap {snap}` (no undo, report the current value without a parameter).
- Piano roll notes are coloured by velocity (three-stop gradient from new theme tokens `midi_vel_lo/mid/hi`, Dark and Light values), selected notes get a white outline, pitch names are printed on notes at least 22 pt wide with rows at least 9 pt high; velocity lane bars use the same colours. View flag `view.midi_single_colour` (also a checkbox in the Snap menu) switches back to the single track-colour look.
- Keyboard gutter: octave label on every C, keys and rows tinted for notes sounding at the playhead (`active_pitches`, unit tested).
- Ruler: a coloured band over the clip span with the clip name, bar numbers below it.
- Info strip above the roll: "No note selected", or editable Start (bar.beat.tick), End, Length, Pitch (name or number), Velocity and Channel for the selected note; with several notes selected the fields show `*` and an entry is applied as a change relative to the first selected note. Invalid input is rejected with a short message. New command `midi.notes_adjust {clip, indices, absolute or *_delta values}` returns the new positions of the edited notes (one undo step). Unit tests for the bar.beat.tick format and parse round trips, pitch names, and rejection of bad input.
- Zoom sliders: horizontal slider with - and + at the bottom right of the roll and a vertical slider with + and - on the right edge (log mapping to pixels per tick and row height), in sync with H-/H+/V-/V+.
- Tests: Add Track menus, folder fold and remove, `track.new after`, `midi.notes_adjust`, snap ids and ticks, snap-driven pencil placement, clip selector, overflow menu, info strip edits and rejection, zoom state sharing, velocity colour flag, colour gradient, key highlight, zoom mapping.
- Selection-scoped MIDI operations: `event.quantize`, `event.transpose`, `event.change_velocity` and `event.change_duration` accept `indices: [n]` (notes of one clip) and report `indices` (the notes' new positions after re-sorting); without it they behave as before. New `event.fix_overlaps {clips?, indices?}` and `midi::ops::trim_overlaps` / `fix_overlaps` (trim to the next same-pitch same-channel note, drop zero-length ones).
- Note mute: `Sequence.muted` side table (serde default, skipped when empty), `midi.notes_mute {clip, indices, muted?}` (toggle when `muted` is absent), `midi.notes` lists `muted` and the clip `scale`; the mix engine skips muted notes; the editor draws them hollow at 40 percent strength.
- Scale assist: `soundcraft_midi::scale` (13 scale kinds, `contains`, `snap` with ties down) and commands `midi.scale_set {clip, root, kind|null}` and `midi.scale_snap {clip, indices?}`; the scale is stored per clip in `edit.values` key `midi.scale.<clip>`.
- Chord tools: `soundcraft_midi::chord` (maj, min, dim, aug, sus2, sus4, 7, maj7, m7 with inversions), `midi.chord_add` (one undo step) and the query `midi.chord_at`.
- Piano roll: eight tools drawn as icons (Select, Draw = the former Pencil, Erase, Split, Glue, Mute, Zoom, Line) plus the Glue action; Glue joins a note with the next same-pitch note, Mute toggles, Zoom clicks in (Alt-click out), Line drags a ramp in a controller lane (or in the roll for the active lane) with one `midi.ctrl_set`. `ui.midi_tool` accepts all (`draw` is an alias of `pencil`).
- Stacked controller lanes: each lane has a type dropdown label, its own splitter and right-click to remove; `+` at the bottom left adds the next unused lane. Commands `ui.midi_lane_add`, `ui.midi_lane_remove`, `ui.midi_lane_height`.
- Inspector strip inside the MIDI editor (170 pt, closed by default, `ui.midi_inspector {open}`, header button): Quantize, Transpose, Length (set, scale, legato, Fix Overlaps), Velocity (set, add, compress), Scale (root, kind, Highlight rows, Snap new notes, snap notes) and Chord (kind, inversion, Draw chords, chord at playhead). All act on the selected notes, else the whole clip. `ui.midi_assist` sets the three toggles.
- Out-of-scale rows are dimmed only when the clip has a scale; the Draw tool and pitch drags snap to it when Snap new notes is on; Draw chords places a chord per click.
- Tests: selection-scoped ops and undo; overlap cases; mute remap through delete, move, split, sort, duplicates, JSON round trip and render; scale snap table; chord pitches and one-undo; tool, lane, inspector and assist UI tests; default dock layout.

### Changed
- Piano roll row height is now state (`row_h`, default 9 px) instead of a constant; rows, notes, hit tests, scroll range and keys all use it.
- The piano roll header toolbar moved left to make room for the panel controls.
- The MIDI editor header is responsive: it lays out title, tool palette, panel glyphs, Loop/Follow, Fit and zoom, then Quantize/Vel/Legato from left to right without overlap. As the panel narrows the clip label hides first, then groups move into one overflow ("...") menu that runs the same actions.
- Default floating MIDI editor width is 960 pt (was 760), enough to show every header group.
- The dock runs panel operations requested by the MIDI editor header after it has drawn its frame.
- `event.all_notes_off` now silences live (audition) notes; before, the UI ignored the request. The app also releases live notes on exit.
- Stopped transport: instruments now render only while a live note or its release tail is active (before, nothing rendered while stopped except monitored inputs). Clip notes and controllers stay silent while stopped.

- Auditioned notes now move the track and master meters while the transport is stopped: `Player::live_active()` (an atomic fed by the audio callback) keeps the UI meters live during a live note and its release tail.
- Piano roll: a note drag now hit-tests at the press point instead of the point where egui starts the drag, so a quick drag off a thin note row still grabs the note.
- The velocity/controller lane maximum now leaves the roll a real 120 pt (the 6 pt handle and 2 pt gap are subtracted); at the largest lane the roll kept 112 pt.
- Piano roll layout: header, info strip (22 pt), ruler with clip band (26 pt, was 16), roll, lane, and a bottom strip (16 pt) for the horizontal zoom slider; an 18 pt column at the right edge holds the vertical slider. Row tint for sounding notes is subtle (16 percent).
- Header groups now are Snap, Clip selector, Quantize with its strength, Vel/Legato, Loop/Follow, Audition and Zoom; the default floating width is 1380 pt (was 960) so every group fits; priority when narrowing: Snap, Loop/Follow, Audition, Zoom, Clip, Quantize group.
- Folder headers draw a triangle instead of the round button; non-folder headers are unchanged.
- The floating MIDI editor opens at the window width less a margin when the window is narrower than 1460 pt (`float_rect`); groups that do not fit go to the overflow menu. `FLOAT_W` is now 1460 (the header gained an Inspector button).
- Mix engine, model and midi crate: `Sequence` gained the `muted` field; test literals use `..Default::default()`.
- Piano roll colours (lane background, roll rows, grid, octave and bar lines, lane zero line and label) come from new theme tokens (`lane_bg`, `roll_row_black`, `roll_row_white`, `roll_grid`, `roll_octave`, `roll_bar`, `lane_zero`, `lane_label`); Dark values equal the former fixed colours, Light has its own. The piano roll toggle buttons (Inspector, Loop, Follow, Audition, snap choices) use a helper that keeps their selected text white in Light.
- Tool tooltips use the platform modifier name (Option on macOS, Alt elsewhere); the Select tool icon is an outline path instead of a filled non-convex polygon.
- `folder_ui::ancestors` returns a fixed array and no longer allocates per header per frame; `event.fix_overlaps` uses binary search instead of linear `contains` (no quadratic cost on very large clips).

### Fixed
- Window > Reset Panel Layout did not bring back a closed track list or clip list: the reset re-applied the saved "hidden" flags to the fresh default layout. Reset now restores both panes (test added).
- A Windows path-separator test in the engine IO module.
- Upstream test `only_pencil_inserts_notes_in_the_midi_roll` now has idle frames between clicks, because a double-click now adds a note.
- The MIDI editor header controls overlapped at the default floating width and in narrow docked panels.
- The minimized state of the MIDI editor now resets when it is closed (by its Close button or any dock command), so it reopens expanded.
- Resizing the velocity/controller lane by its handle now follows the pointer exactly (position-based, grab offset kept) instead of lagging; double-click on the handle resets to the default without moving it; the maximum keeps about 120 pt for the roll below the header and ruler (one clamp helper for handle and layout).
- At very narrow MIDI editor widths the overflow button keeps its own space and the tool palette is truncated instead of overlapping it.
- While the transport was stopped, every render block looked like a seek and silenced held instrument notes. The seek reset now applies only while the transport runs.
- Info strip pitch field: a huge octave (for example `C9223372036854775807`) overflowed and would panic in debug builds; the pitch parser now uses checked arithmetic and rejects out-of-range input. Unit tests with huge bar, length and octave numbers.
- Pasting MIDI into a host clip (`edit.paste_merge_midi`) now keeps the mute flags of both the host notes and the pasted notes (test added).

### Decisions
- Plans are stored in `plans/`, which upstream ignores.
- The fork keeps new behaviour in new files with tiny hooks into existing files.
- Clicking in the piano roll must not change the session (upstream test), so the play-from position is editor state.
- A mute-note flag is deferred: `Note` has about 35 literal construction sites in 18 files.
- Rust toolchain updated to 1.99.1 with clippy and rustfmt.
- Test processes sometimes exit with STATUS_ACCESS_VIOLATION after all tests pass (upstream, uninvestigated).
- The floating MIDI editor is an in-app floating panel drawn inside the main window, not a separate OS window.
- Panel header controls cannot change the dock layout while the dock is drawing, so they queue a request that the dock applies right after the frame.
- `midi.audition` is declared with the journaling no-undo kind like other transport commands, so the command-reachability test applies; it is listed as a temporary known gap in `command_reachability.rs` until the piano-roll audition UI names it, and the test then forces the line's removal.
- Live notes are separate from clip-scheduled notes (a per-strip table of 128 timeout counters) and are not given the track's real-time transpose or velocity offsets yet.
- The Audition queue drops events when full (256); the engine's 2 s timeout covers a lost release.
- The `Audition` toggle is a user preference (`UiState.audition`, default on, old preferences load unchanged); the sounding note lives in editor state.
- Default floating MIDI editor width is now 1040 pt (was 960) so the new Audition group still fits with every other header group.
- Existing test `a_real_pointer_drag_up_100_points_grows_the_lane_by_about_100` became `..._80_points_..._80`: with the corrected lane maximum the test panel cannot grow the lane by 100 pt; the intent (drag distance equals growth) is unchanged. `lane_height_clamps...` expects 472 instead of 480 for the same reason. `midi.audition` was removed from the command-reachability known gaps (the UI now references it).
- Panel hint strips take their space from the dock area (the dock gets the remaining rectangle) rather than overlaying content; their clicks queue a dock request like the other panel buttons. The right-click menu is offered on tabs, tab-strip buttons and the hint strips; the Session canvas itself keeps its own right-click behaviour.
- Window menu entries reuse the existing window toggle commands (which sync through the dock) instead of adding a parallel set; only `window.show_all_panels` is new.
- The piano roll snap is the editor's own (default "Grid" follows the session edit grid, so nothing changes until the user picks another value); it never writes the session grid.
- Info strip edits go through one new command, `midi.notes_adjust`, instead of `midi.note_edit` plus `midi.notes_move`: the sequence re-sorts after every edit, so several index-based calls would act on the wrong notes, and one command is one undo step.
- Test changes (upstream tests adapted, not weakened): `tests/midi_panel.rs` now clicks `roll.top() + 22` instead of `body.top() + 60` (the same place inside the roll; the info strip and taller ruler moved the roll down), and the lane-drag test drags 40 pt instead of 80 because the extra chrome leaves 42 pt of lane growth in the 320 pt panel (the 120 pt roll minimum is unchanged); the in-file header layout test lists the new groups and the default floating width constant moved to 1380.
- Editor snap, strength and note-colour state are not saved between launches yet (the colour switch is a session view flag); persisting them is left for the editor-state work.
- Add Multiple... opens the existing New Tracks dialog, which appends at the end (it does not know the clicked track).
- Mute is a side table on the sequence, remapped by `Sequence::sort_tracked`, `retain_notes`, `remove_duplicates`, split, glue, adjust and clip split/trim; consolidating MIDI drops muted notes (they are silent).
- Existing tests adjusted: `midi.lane_h` became `midi.lanes[0].height` (same behaviour); the header layout test now includes the Inspector item and the clip label needs 1700 pt instead of 1600 (also in tests/midi_panel.rs).
- Lane minimum height stays 40 pt (plan said 24) to keep the V1 clamp rules; lanes, tool, snap and inspector state are not persisted between launches.
- Scale root/kind stored as one number per clip in `edit.values`; no model change.
- Recovery dialog investigation: after one unclean exit (taskkill) the app offers that one autosave; Discard removes it and the dialog does not return (reproduced in the real app). The reviewer saw a changing "autosaved N ago" because several autosave folders from earlier killed runs were present and the dialog offers them one at a time. Expected behaviour, no code change.
- A clip's scale (`edit.values` key `midi.scale.<clip>`) is not carried to copies or splits: new clips are created at about 59 call sites with no common helper, so a safe single fix is not possible. Left as a known gap.

