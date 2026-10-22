# Development handoff between machines (Windows and macOS)

This fork is developed from two machines. Git is the transport. Only one machine works on the trunk at a time. This file is the contract for putting work down on one machine and picking it up on the other. Keep it generic: no product names (the repository name check enforces this).

## 0. Two repositories
- Code: this repository (public fork). Branch model below.
- Planning notes: a separate PRIVATE repository, `mahadeveaswar/soundcraft-custom-plans` (plans, status logs, decision log, references, copies of the workspace files). It is cloned as the ignored `plans/` folder inside this checkout, so the two never interfere. Clone once per machine:
```
gh repo clone mahadeveaswar/soundcraft-custom-plans plans
```
Both repositories are pushed and pulled together at every put-down and pick-up (sections 3 and 4). Planning content (anything naming other products, reference screenshots) lives only in the private repository, never here.

## 1. Branches
| Branch | Purpose | Rules |
|---|---|---|
| `main` | mirror of upstream | never edited; fast-forward only from upstream |
| `custom-release` | integration branch: the combined build used day to day | both platform branches merge into it after their checks; never commit directly except merges and the handoff state block |
| `custom-windows` | all work done on the Windows machine | start each session by merging `custom-release` into it; merge back when tested |
| `custom-mac` | all work done on the macOS machine | start each session by merging `custom-release` into it; merge back when tested |

The old `custom` branch no longer exists: it became `custom-release` (same history). Shared code is the same on both platform branches, so merge `custom-release` into your platform branch often: small, frequent merges keep conflicts small. Do not rebase or force-push anything already pushed.
Tags: `rel-YYYY-MM-DD` on each merge into `custom-release` that passed the promotion checks (section 6).

## 2. The state block (update at every put-down)
Edit this block, commit it with the code.
```
last put-down: <date time> on <windows|macos>
branches pushed: <custom-windows|custom-mac|custom-release> at <sha>
tests run on that machine: <what passed, counts, known flakes seen>
not verified on the other OS: <list>
in progress / next tasks: <short list, pointers to task logs in the notes repo>
open questions for the user: <list>
```
Current entry:
```
last put-down: 2026-10-22, on macos (session: Inspector, track palette, glossy clips, Mixer pane)
branches pushed: nothing pushed from this session. Work is on the local topic branch mac/p10-b (based on custom-mac; code commit b3c098e plus a docs commit on top), to be merged into custom-mac by the owner.
tests run on that machine: cargo test --workspace --no-fail-fast 1554 passed, 0 failed; fmt, clippy -D warnings, cargo xtask layers and cargo xtask parity (77.5 %, 397/512) clean; name check clean. Headless kittest and unit tests only: the app was never launched.
not verified in the real app: gloss and corner look in Dark, Light and Midnight at small and large track heights and zoomed far out; selected and muted clip readability; no gaps between adjacent rounded clips; palette popup and its Apply to clips button; Colour... header menu item; Inspector at 220 pt width and inline rename focus; Mixer pane tabbed with the MIDI editor (resize, 60+ track horizontal scroll, meters animating, compact mode, selection sync); frame time with about 1000 visible clips (more than 1 ms worse than before is a fail). Not verified on Windows: everything at runtime.
in progress / next tasks: nothing in progress. Next: review of the P10 work, then the later items (Inspector EQ curve view, MIDI track transpose/velocity/channel, Mixer pane inside the Mix window workspace, side-by-side Editor and Mixer docking). Details in the notes repository (plans/status/RESUME-NEXT.md, plans/MAC-VERIFY.md, plans/10-track-inspector-palette-gloss-plan.md).
open questions for the user: none pending.
```

## 3. Put-down checklist (the machine you are leaving)
1. Stop or finish every running agent and close the app. Never leave half-edited files: either finish the task or commit the work as `wip:` on a topic branch.
2. `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, tests for the touched crates (full run before promoting), `cargo xtask layers`, `bash plans/check-names.sh` if the notes folder is present.
3. Update CHANGELOG.md (append-only, dated heading, tag entries with the machine, for example `(win)` or `(mac)`) and the state block above.
4. Commit in a small number of coherent commits (message style: short subject, generic bullets, co-author line).
5. Push your platform branch to the fork. Then commit and push the notes repository: `cd plans && git add -A && git commit -m "sync <machine> <date>" && git push` (copy any changed workspace files into `plans/workspace/` first).
6. Write one line in the state block saying exactly where to resume.

## 4. Pick-up checklist (the machine you are arriving on)
1. `git fetch --all --prune`, check out your platform branch, `git pull --ff-only`, then `git merge origin/custom-release` (resolve conflicts, never rebase pushed commits). Then `cd plans && git pull --ff-only` (clone it first if `plans/` is missing; see section 0).
2. Read the state block and the newest CHANGELOG entries; read the status files in `plans/status/`.
3. Build and run: `cargo build --release -p soundcraft`, then `target/release/soundcraft --demo` (Windows: `soundcraft.exe`). Run the touched crates' tests once to learn this machine's baseline (flakes differ per OS).
4. Verify what the other machine could not (the "not verified on the other OS" line): open the app and check those items; record results in the state block.
5. Only then start new work; create the topic branch if overlap is possible.

## 5. Per-OS reference
| Item | Windows | macOS |
|---|---|---|
| Toolchain | Rust stable via rustup, MSVC build tools | Rust stable via rustup, Xcode command line tools |
| Binary | `target\release\soundcraft.exe` | `target/release/soundcraft` |
| App data folder | `%APPDATA%\SoundCraft` | `~/Library/Application Support/SoundCraft` |
| Autosave folder (modal "Recover Unsaved Work" after an unclean exit) | `%APPDATA%\SoundCraft\Autosave` | `~/Library/Application Support/SoundCraft/Autosave` (move leftovers into a backup folder next to it, never delete) |
| Logs | `%APPDATA%\SoundCraft\logs\soundcraft.log` | `~/Library/Application Support/SoundCraft/logs/soundcraft.log` |
| Run without reading or writing preferences | set `SOUNDCRAFT_NO_PREFS=1` | same |
| Stop only your own build of the app | `Get-Process soundcraft \| Where Path -like '*target\release*' \| Stop-Process` | `pkill -f 'target/release/soundcraft'` |
| Control channel | `--control 0` prints the port; coordinates are window points | same; screenshots are 2x or more (Retina): divide pixels by the scale factor |
| Graphics | DX12 default; `WGPU_BACKEND=vulkan` if the window flashes and closes | Metal |
| Audio | WASAPI via the audio library | CoreAudio via the audio library |
| Plugins | VST3, CLAP | VST3, CLAP, Audio Units (signed release builds block third-party VST3, upstream issue; local builds expected to load them) |
| Menu bar | in-window menu | native menu bar at the top of the screen (rebuilt from the same menu tree) |
| Known test flakes | engine and mix test processes sometimes exit with an access violation after all tests pass; engine autosave and UI recovery timing tests | unknown until first run: record them in the state block |

## 6. Merging into `custom-release`
Merge a platform branch into `custom-release` only when all hold, then tag it:
- Windows: fmt, clippy `-D warnings`, full tests of the touched crates, `cargo xtask layers`, name check, app starts and the demo plays.
- macOS: `cargo build --release` succeeds, the app starts, the demo plays, the native menu bar shows and its items run, and an audio device is selected and sound is heard.
- No open "must fix" item in the state block.

## 7. Conflict rules
- CHANGELOG.md: append only, under a dated heading, entries tagged with the machine; never reorder or rewrite another entry.
- Status and task logs (local `plans/status/`): the machine currently working edits them; the other machine reads only; the notes repository carries them between machines (pull before editing, push at put-down).
- Do not rebase or force-push anything already pushed. If both machines changed the same file, merge `custom-release` into your platform branch, resolve, test, then merge back.
- Large refactors (for example moving hard-coded colours into theme tokens) run on one machine while the other stays on small, separate files.

## 8. What each machine uniquely verifies
- Windows: DX12 rendering, the Windows installer path, hosted VST3 behaviour with the installed plugins.
- macOS: Metal rendering at Retina scale, CoreAudio device and buffer behaviour, Audio Unit hosting, the native menu bar and its shortcuts (Cmd and Option labels), window and dock behaviour, trackpad scroll and pinch zoom.

## 9. Session format compatibility
- Sessions that contain MIDI controller automation lanes (MC1: `cc0`..`cc127`, `pitch_bend`, `pressure`) do not open in a build without commit 831ce28, because the older `AutoParam` enum rejects the unknown lane ids. Pull `custom-release` on the other machine before opening such a session there.
- The colour palette, the Inspector, the gloss tokens (`clip_gloss`, `clip_corner`, `header_corner`) and the Mixer pane (`show_mixer_panel`, `mixer_compact`, dock panel `mixer`) change no saved session format: track and clip colours stay plain RGB, and the new UI preference fields (`recent_track_colors`, `inspector_collapsed`, `show_inspector`, `show_mixer_panel`, `mixer_compact`) are serde defaults, so older preference files still load. A UI preference file saved with the `mixer` panel in its layout was not tried in an older build (unverified).
