# Remember Project Goals

## Final Goal

Remember is a lightweight Windows desktop automation app for recording and
replaying repetitive user operations.

The finished product should let a normal Windows user turn repeated manual work
into a reusable local flow without writing code. It should feel compact,
practical, and safe: closer to TinyTask for daily use than to a large enterprise
RPA platform.

## Product Shape

- Platform: Windows first, Windows-only for the initial product.
- Stack: Tauri + Rust backend + React + TypeScript frontend.
- Default surface: a compact always-on-top control window.
- Advanced surface: a larger workbench opened from the control window.
- Visual direction: unified Windows 11 dark Mica/acrylic style for the current
  official theme.

The small control window is for daily operation:

- Choose current flow.
- Start recording.
- Replay flow.
- Stop current activity.
- Adjust speed.
- Adjust loop count.
- Show current status.
- Show hotkey hints.
- Open the workbench.

The workbench is for advanced control:

- Flow list and flow metadata.
- Step timeline.
- Step table.
- Step editing.
- Playback settings.
- Recording settings.
- Hotkey settings.
- Window matching.
- Safety controls.
- Run logs and history.

## Core MVP Loop

The first usable MVP is complete when this loop works end to end:

1. Open Remember and see the compact control window.
2. Start recording.
3. Capture key mouse and keyboard actions.
4. Stop recording.
5. Review the generated steps in the workbench.
6. Edit simple step properties.
7. Save the flow locally.
8. Load the flow later.
9. Replay it with speed and loop controls.
10. Stop it immediately with a global emergency hotkey.

## Core Features

### Recording

- Mouse clicks.
- Keyboard input.
- Hotkeys.
- Wait/delay between steps.
- Scroll events when needed.
- Active window metadata.

Full mouse movement recording is not part of the first MVP unless it becomes
necessary for a verified user flow.

### Playback

- Replay saved steps.
- Speed control.
- Loop count.
- Stop current playback.
- Emergency stop.
- Basic target-window check before playback.

### Flow Editing

- View steps in a timeline and table.
- Delete steps.
- Edit delays.
- Edit click coordinates.
- Edit typed text.
- Edit hotkey steps.
- Insert wait steps.
- Save/load `.remember.json` flows.

### Safety

- Emergency stop must exist before serious playback work.
- Infinite loop must require explicit confirmation.
- Sensitive input recording must be warned about.
- Password recording should not be intentionally supported.
- If target-window matching fails, the app should stop or ask instead of
  clicking blindly.

## Completion Standards

The project is considered product-complete when:

- A non-technical Windows user can record, edit, save, load, and replay a simple
  repetitive workflow.
- The compact control window remains useful without opening the workbench.
- The workbench supports enough editing to correct a recorded flow without
  touching code.
- Emergency stop works reliably during playback.
- Saved flows can be reused across app restarts.
- Basic target-window checks reduce accidental clicks in the wrong app.
- The app can be packaged as a Windows desktop installer.
- Core behavior has tests or repeatable manual verification steps.

## Development Principles

- Keep the app lightweight and local-first.
- Prefer small verified steps over large rewrites.
- Preserve the two-window product shape.
- Keep small and large windows visually unified.
- Do not add cloud sync, OCR, image recognition, AI planning, plugins, or
  cross-platform support before the MVP is stable.
- Keep Rust backend modules focused: storage, recording, playback, hotkeys,
  window inspection, and safety should have clear boundaries.
- Keep React components focused around product surfaces: control window,
  workbench, timeline, step table, inspector, and run controls.
- Use type-safe interfaces between frontend and Tauri commands.
- Verify user-facing UI in the running desktop app, not only with builds.

## Current Status

The current project has completed the first UI shell milestone:

- Tauri v2 + React + TypeScript app exists.
- Rust backend command skeleton exists.
- Compact control window exists.
- Large workbench window exists.
- Settings opens the workbench.
- Both windows share a unified dark UI style.
- Flow data uses real local `.remember.json` persistence.
- A flow name edited in the workbench can be saved and restored after app
  restart.
- Save-as creates a separate local flow copy.
- Record and Stop now use a Rust-owned recording session lifecycle.
- Stop returns a clearly marked safe placeholder recording flow when no input
  was captured.
- Recording start / stop events sync recorded flows across the control window
  and workbench.
- The control window warns before starting a recording session and explicitly
  reminds users not to record passwords or sensitive fields.
- Recording sessions capture foreground window title, process name, and window
  size as basic target-window metadata.
- Recording sessions now capture discrete left and right mouse clicks through a
  Windows low-level mouse hook.
- Recording sessions now capture safe keyboard text through a Windows low-level
  keyboard hook and merge consecutive characters into `type` steps.
- Recording sessions now capture modifier-based hotkeys such as `Ctrl + S` as
  explicit `hotkey` steps.
- Captured click steps include screen coordinates and wait timing between
  recorded clicks.
- Captured mouse, text, and hotkey events are sorted by capture time before
  being sent to the workbench.
- Remember filters clicks inside its own windows when recording stops, so the
  Stop button is not saved as a user flow step.
- Full mouse movement is not recorded by default.
- Playback now has a Rust-owned player state and Tauri commands for run, stop,
  and emergency stop.
- Playback safely executes `wait` steps, guarded mouse click steps, and guarded
  text input steps in a background thread, applies speed and loop count, and
  checks target-window process metadata before input playback.
- Playback refuses guarded click and text steps when the recorded target window
  is missing or the current active window is clearly different, and reports the
  safety stop through the existing UI message path.
- The guarded click refusal path has been verified in the desktop app: a flow
  recorded for `EXCEL.EXE` safely stopped while Remember was the active window.
- Guarded text input playback has been verified in the desktop app with a
  temporary Notepad flow: after a safe wait, Remember typed `remember smoke`
  into the matched Notepad target and completed the run.
- Guarded hotkey playback has been verified in the desktop app with a temporary
  Notepad flow: `Ctrl+A` selected the existing text and the following type step
  replaced it with `hotkey smoke`.
- Guarded scroll playback has been verified in the desktop app with a temporary
  Notepad flow: after a safe wait, Remember scrolled a long text file from the
  first lines down to later lines while the Notepad target was active.
- The workbench now has a compact run-log surface for recent playback start,
  stop, completion, emergency-stop, and safety-stop messages; a desktop smoke
  check verified that a target-window mismatch appears in the UI log without
  reading terminal output.
- The workbench now supports basic step selection and delay editing against the
  real flow state; a desktop smoke check verified that a changed delay is saved
  back to `.remember.json`.
- The workbench now supports deleting the selected step; a desktop smoke check
  verified that deleting a recorded wait step and saving leaves only the
  remaining steps in `.remember.json`.
- The workbench now supports editing selected text and hotkey step values; a
  desktop smoke check verified that edited `type.text` and `hotkey.keys` values
  are saved back to `.remember.json`.
- The workbench now supports editing selected click step coordinates; a desktop
  smoke check verified that edited `x`, `y`, and target labels are saved back
  to `.remember.json`.
- The workbench now supports inserting a wait step after the selected step; a
  desktop smoke check verified that the inserted wait step is saved back to
  `.remember.json`.
- Recording target-window metadata now follows the first real recorded input
  event instead of the Remember control window that starts recording, so a flow
  started from the control window can still safely replay against the target
  app.
- Target-window safety stops now appear directly in the workbench target
  preview as a visible `安全停止` state with the mismatch reason, not only in
  the run log.
- The workbench now exposes basic target-window confirmation controls in the
  inspector and window preview; a desktop smoke check verified that toggling
  the control saves `targetWindow.matched` to `.remember.json` and that
  unconfirmed targets safety-stop instead of replaying input.
- Flow validation now rejects duplicate step IDs and wait steps whose
  `durationMs` and `delayMs` diverge; Rust storage tests cover these cases
  alongside existing save/load, malformed file, default-flow, and save-as
  behavior.
- A global emergency stop hotkey is registered on Windows startup as
  `Ctrl + Alt + S`; a desktop smoke check verified that it interrupts a safe
  long-wait playback flow through the same emergency-stop path as the workbench
  command.
- Playback now applies an input step's delay before checking the active target
  window, which gives the user time to switch focus back to the recorded target
  after pressing Replay while preserving the safety gate before input is sent.
- Windows packaging is enabled for an NSIS installer. `npm run tauri build`
  produced `src-tauri/target/release/bundle/nsis/Remember_0.1.0_x64-setup.exe`,
  and the release executable launched successfully.
- Final app icon assets replace the temporary icon. The bundled ICO contains
  16, 32, 48, 64, 128, and 256 px entries, and
  `src-tauri/icons/remember-icon.svg` is kept as the editable source.
- Tray and startup behavior were reviewed and intentionally left out of the MVP
  because the compact always-on-top control window and global emergency hotkey
  keep safety controls visible without adding hidden background behavior.
- `README.md` now documents installation, development startup, daily
  record/edit/save/replay usage, emergency stop, safety limits, flow files, and
  build verification commands.
- The default dev server port is now `1450`, avoiding the local Windows
  reserved `1350-1449` range so `npm run tauri dev` can launch again.
- Infinite loop playback is rejected until explicit confirmation is designed.
- The workbench exposes emergency stop in the top command bar.
- Final acceptance was desktop-smoke verified on 2026-06-23 with the packaged
  release executable: record a Notepad-backed local flow, stop, review steps,
  edit a type step, save to `.remember.json`, restart, load the saved flow from
  the control window, replay the edited text into Notepad, and interrupt a
  delayed replay with the global `Ctrl + Alt + S` emergency hotkey.
- Phase 5 packaging and daily-use polish are complete for the MVP.

The MVP product-complete state defined in this document has been reached. Future
work should be handled as post-MVP hardening or feature expansion, not as a
blocker for the first deliverable.
