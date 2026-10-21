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

### Fixed
- Window > Reset Panel Layout did not bring back a closed track list or clip list: the reset re-applied the saved "hidden" flags to the fresh default layout. Reset now restores both panes (test added).
- A Windows path-separator test in the engine IO module.
- Upstream test `only_pencil_inserts_notes_in_the_midi_roll` now has idle frames between clicks, because a double-click now adds a note.
- The MIDI editor header controls overlapped at the default floating width and in narrow docked panels.
- The minimized state of the MIDI editor now resets when it is closed (by its Close button or any dock command), so it reopens expanded.
- Resizing the velocity/controller lane by its handle now follows the pointer exactly (position-based, grab offset kept) instead of lagging; double-click on the handle resets to the default without moving it; the maximum keeps about 120 pt for the roll below the header and ruler (one clamp helper for handle and layout).
- At very narrow MIDI editor widths the overflow button keeps its own space and the tool palette is truncated instead of overlapping it.
- While the transport was stopped, every render block looked like a seek and silenced held instrument notes. The seek reset now applies only while the transport runs.

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
