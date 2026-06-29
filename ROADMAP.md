# Remember Roadmap

## Current Snapshot

Remember currently has a post-MVP desktop automation surface:

- Compact control window route: `#/control`.
- Workbench route: `#/workbench`.
- Tauri windows: `control` and `workbench`.
- Flow files: app-local `flows/*.remember.json` persistence.
- Recording session lifecycle: start / stop commands create a recorded flow and
  sync it across both windows.
- Recording starts directly from the control window; recording status still
  shows the sensitive-input warning.
- Active-window metadata: recording sessions capture foreground window title,
  process, and size as target-window metadata.
- Mouse-click capture: a Windows low-level mouse hook records discrete left and
  right clicks, converts them into click steps with wait timing, and filters
  clicks inside Remember app windows when recording stops.
- Keyboard capture: a Windows low-level keyboard hook records safe text input
  into `type` steps and modifier combinations into `hotkey` steps, preserving
  capture order with mouse actions.
- Playback state: a backend player module can run wait steps, guarded mouse
  click steps, guarded text input steps, guarded hotkey steps, and guarded
  scroll steps in a
  background thread, apply speed and loop count, refuse clearly unsafe input
  targets, and respond to stop or emergency-stop requests.
- Guarded click refusal has been desktop-smoke verified with a saved flow whose
  recorded target was `EXCEL.EXE` while the active window was Remember; the
  control window returned to stopped and the workbench showed the safety reason.
- Guarded text playback has been desktop-smoke verified with a temporary
  Notepad flow: after a safe wait, text was sent to the matched Notepad target
  and the workbench reported completion.
- Guarded hotkey playback has been desktop-smoke verified with a temporary
  Notepad flow: `Ctrl+A` selected existing text and the following type step
  replaced it.
- Guarded scroll playback has been desktop-smoke verified with a temporary
  Notepad flow: after a safe wait, the matched Notepad target scrolled from the
  first lines of a long text file down to later lines.
- The workbench now includes a compact run-log surface that shows recent
  playback start, stop, completion, emergency-stop, and safety-stop messages.
- The workbench can select a recorded step and edit its delay value against the
  real flow state; a desktop smoke check verified that saving writes the edited
  delay back to `.remember.json`.
- The workbench can delete the selected step from the real flow state; a
  desktop smoke check verified that saving persists the remaining step list to
  `.remember.json`.
- The workbench can edit selected `type` and `hotkey` step values from the
  real flow state; a desktop smoke check verified that saving writes edited
  text and hotkey keys back to `.remember.json`.
- The workbench can edit selected click step coordinates from the real flow
  state; a desktop smoke check verified that saving writes edited `x`, `y`, and
  target labels back to `.remember.json`.
- The workbench can insert a wait step after the selected step from the real
  flow state; a desktop smoke check verified that saving writes the inserted
  wait step back to `.remember.json`.
- Recording target-window metadata now comes from the first real recorded input
  event instead of the Remember control window that starts recording, so a
  normal Record-button workflow can produce a replayable target app flow.
- Target-window safety stops are now surfaced directly in the workbench target
  preview as a visible `安全停止` state with the mismatch reason, not only in
  the run log.
- The workbench now exposes basic target-window confirmation controls in the
  inspector and window preview; toggling the control updates
  `targetWindow.matched`, saves to `.remember.json`, and unconfirmed targets
  safety-stop instead of replaying input.
- Flow validation and storage tests now cover duplicate step IDs and wait-step
  timing consistency, so ambiguous edits and mismatched wait playback timing are
  rejected before a flow is saved or loaded.
- Global emergency stop hotkey: `Ctrl + Alt + S` is registered on Windows app
  startup and triggers the same emergency-stop playback path as the workbench
  command; a desktop smoke check verified that it interrupts a safe long-wait
  flow.
- Playback now waits an input step's `delayMs`, then tries to bring the
  recorded target window to the foreground before target-window validation and
  input injection.
- Windows packaging is enabled for NSIS; `npm run tauri build` produced
  `src-tauri/target/release/bundle/nsis/Remember_0.1.0_x64-setup.exe`, and the
  release executable launched successfully.
- Final app icon assets replace the temporary icon: `src-tauri/icons/icon.ico`
  includes taskbar-optimized small-size frames plus 128 and 256 px entries,
  with `src-tauri/icons/remember-icon.svg` kept as the editable source.
- Tray and startup behavior were reviewed for the MVP and intentionally left
  out because the compact always-on-top control window and global emergency
  hotkey keep safety controls visible without adding hidden background behavior.
- User-facing documentation now covers installation, development startup,
  record/edit/save/replay, emergency stop, safety notes, flow files, and build
  verification commands.
- Final end-to-end acceptance was desktop-smoke verified on 2026-06-23 with the
  packaged release executable: record, stop, review, edit, save, restart, load,
  replay into Notepad, and interrupt playback with the global emergency hotkey.
- The default dev server port has moved to `1450` to avoid the local Windows
  reserved `1350-1449` range; `npm run tauri dev` has been verified with the
  updated default port.
- Recording now converts fast nearby left-click pairs into a single double-click
  step and captures vertical or horizontal mouse-wheel events as scroll steps.
- Recording now captures left or right mouse drags as explicit drag steps with
  start/end coordinates, drag duration, and sampled path points, instead of
  degrading them into ordinary clicks.
- Keyboard recording now preserves ordinary control keys such as Enter, Tab,
  Backspace, Delete, arrow keys, and Esc as explicit key steps.
- Playback now executes drag and ordinary key steps through the same
  target-window safety gate used by clicks, text, hotkeys, and scroll.
- Infinite-loop playback is now a real supported mode: `0` loop count means
  run until stopped, but both the UI and backend require explicit confirmation
  before it can start, and stop / emergency-stop interrupt it.
- Playback target-window safety now checks the window title when both recorded
  and active titles are known, so same-process but different-document windows
  are refused.
- The workbench can insert wait, text, hotkey, and ordinary key steps from the
  step list, shows all timeline and run-log entries through scrolling instead
  of hiding them, and keeps visible controls scoped to implemented behavior.
- Flow saving writes back to the selected file name, uses an atomic replacement
  path, keeps invalid flow files visible in the list, and rejects empty
  key/hotkey steps or high-risk global hotkeys.
- Recording stop filters keyboard input captured inside Remember windows,
  obvious sensitive windows, and high-risk global hotkeys before producing a
  flow.
- Long drag playback now checks stop and emergency-stop requests between drag
  movement slices and releases the mouse before exiting.
- The UI reads the real global emergency-hotkey registration status and shows
  when the shortcut is unavailable.
- Browser preview fallbacks no longer fabricate a duplicate sample flow; the
  app expects Tauri desktop commands for real flow data.
- Desktop initial flow loading no longer creates a first-run sample flow file
  when no saved flows exist.
- Release signing is now checked by `npm run verify:release-signature`, which
  fails if the release executable or NSIS installer is missing, unsigned, or not
  trusted.
- The compact control window grid has been tightened so the daily controls fit
  the current 520 px window without horizontal clipping.
- Visual target: unified dark Windows 11 Mica/acrylic style.

The app does not yet provide optional full mouse-trajectory recording,
IME-aware text capture, native password-field detection, or advanced
target-window matching controls.

## Phase 0: UI Shell

Status: Completed.

- [x] Create Tauri + Rust + React + TypeScript project.
- [x] Preserve project guidance in `AGENTS.md`.
- [x] Add compact control window.
- [x] Add large workbench window.
- [x] Add placeholder flow data.
- [x] Add placeholder status transitions.
- [x] Add dark unified UI style.
- [x] Verify frontend build and Rust check.
- [x] Verify settings opens the workbench in the desktop app.

## Phase 1: Real Local Flow State

Status: Completed.

Goal: replace placeholder flow behavior with real local flow persistence while
keeping the UI shell stable.

- [x] Define the v1 `.remember.json` flow schema in shared TypeScript types.
- [x] Mirror the v1 flow schema in Rust or a Rust-owned storage model.
- [x] Add Tauri commands for listing, loading, saving, and creating flows.
- [x] Store flows in an app-local data directory.
- [x] Start with an empty unsaved flow when no saved flows exist.
- [x] Connect the control window flow selector to saved flows.
- [x] Connect workbench save and save-as buttons to real persistence.
- [x] Show save status and last-saved time in the workbench.
- [x] Add validation for malformed flow files.
- [x] Verify that a saved flow survives app restart.

Acceptance criteria:

- A flow edited in the UI can be saved locally.
- The same flow can be loaded after restarting the app.
- Invalid or missing flow files do not crash the app.
- `npm run build` and `cargo check --manifest-path src-tauri/Cargo.toml` pass.
- The desktop app is launched and visually checked after the changes.

## Phase 2: Recording MVP

Status: Completed.

Goal: record basic foreground user actions into the v1 flow model.

- [x] Add backend recorder module.
- [x] Add start-recording and stop-recording commands.
- [x] Capture mouse clicks.
- [x] Capture keyboard input and hotkeys.
- [x] Capture waits between actions.
- [x] Capture basic active-window metadata.
- [x] Do not record full mouse movement by default.
- [x] Send recorded steps to the frontend when recording stops.
- [x] Show a sensitive-input warning during recording.
- [x] Verify recording with a safe local test flow.

Acceptance criteria:

- Clicking Record starts a real recording session.
- Clicking Stop creates a step list in the workbench.
- Recorded steps can be saved through Phase 1 persistence.
- Recording does not continue after Stop.

## Phase 3: Playback MVP

Status: Completed.

Goal: replay saved click, type, hotkey, wait, and basic scroll steps safely.

- [x] Add backend player module.
- [x] Add run, pause or stop, and emergency-stop commands.
- [x] Replay click steps with a target-window safety gate.
- [x] Replay type steps with a target-window safety gate.
- [x] Replay hotkey steps with a target-window safety gate.
- [x] Replay basic scroll steps with a target-window safety gate.
- [x] Replay wait steps.
- [x] Apply speed control.
- [x] Apply loop count.
- [x] Support infinite loop playback only after explicit confirmation.
- [x] Verify emergency stop before broad playback testing.

Acceptance criteria:

- A saved simple flow can be replayed.
- Speed and loop count affect playback.
- Emergency stop interrupts playback reliably.
- Playback failures are visible in the UI log.

## Phase 4: Editing, Safety, And Reliability

Status: Completed.

Goal: make recorded flows correctable and reduce accidental actions.

- [x] Edit delay values.
- [x] Edit click coordinates.
- [x] Edit text input.
- [x] Edit hotkey steps.
- [x] Delete steps.
- [x] Insert wait steps.
- [x] Add basic target-window matching.
- [x] Add run log.
- [x] Add failure states for missing or mismatched target windows.
- [x] Add tests for flow validation and storage behavior.
- [x] Add global emergency stop hotkey.

Acceptance criteria:

- A user can correct common recording mistakes in the workbench.
- Playback refuses to run when target-window checks fail.
- Errors are visible without reading terminal output.

## Phase 5: Packaging And Daily-Use Polish

Status: Completed.

Goal: make Remember usable as a normal Windows desktop utility.

- [x] Configure app icon assets beyond the temporary icon.
- [x] Decide tray behavior is not needed for the MVP.
- [x] Add installer build configuration.
- [x] Decide startup behavior options are not needed for the MVP.
- [x] Add user-facing documentation for recording, replay, and emergency stop.
- [x] Build and verify a Windows package.

Acceptance criteria:

- Remember can be installed or launched as a packaged Windows app.
- A fresh user can complete the MVP loop from the UI.
- The app remains small, local-first, and safe by default.

## Current Priority

MVP product-complete. No Phase 1-5 blocker remains for the first deliverable.

Next work should be treated as post-MVP hardening, for example adding
native password-field detection, optional mouse-trajectory recording, expanding
manual QA coverage, or adding future advanced matching controls.
