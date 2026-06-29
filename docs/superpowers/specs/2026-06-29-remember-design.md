# Remember Design Spec

Date: 2026-06-29

## Goal

Build `Remember`, a Windows desktop macro recorder similar in workflow to TinyTask. It records real mouse and keyboard input, saves the recording, and replays it later with loop and speed controls.

This is an original implementation. It must not copy TinyTask code, icons, names, binaries, or visual assets.

## Confirmed Decisions

- Product name: `Remember`
- Project path: `D:\Code\Codex\remember`
- Platform: Windows first
- App stack: Tauri + Rust + React
- First deliverable: portable executable
- First release scope: close to TinyTask, with global hotkeys and tray support
- Portable artifact: the built Windows executable under `src-tauri\target\release`; installer bundles are not required for the first release.

## First Release Scope

Included:

- Record mouse clicks, cursor movement, wheel scrolling, and keyboard press/release events.
- Stop recording and keep the captured sequence in memory.
- Save recordings as `.remember.json` files.
- Open `.remember.json` files.
- Replay a loaded or freshly recorded sequence.
- Configure playback loop count.
- Configure playback speed multiplier.
- Stop playback from the main window.
- Emergency stop playback from a global hotkey.
- Register global hotkeys for record, playback, stop, and emergency stop.
- Keep the app available from the system tray.
- Allow the main window to be minimized during recording or playback.

Excluded from the first release:

- Image recognition.
- AI-driven automation.
- Cross-platform support.
- Step-by-step macro editing.
- Conditional branching or scripting.
- Installer packaging.
- Copying TinyTask UI assets or exact branding.

## User Experience

The app opens to a compact control panel:

- Primary controls: Record, Play, Stop, Save, Open.
- Playback controls: loop count and speed multiplier.
- Status display: idle, recording, playing, stopped, error.
- Activity summary: current recording name, step count, duration, and last action.
- Hotkey summary: the currently assigned global shortcuts.

Default hotkeys:

- Record/stop recording: `Ctrl+Alt+R`
- Play/stop playback: `Ctrl+Alt+P`
- Emergency stop: `Ctrl+Alt+Esc`

The interface should stay quiet and operational. It should behave like a utility, not a marketing page.

Default behavior:

- Recording starts only after the user presses Record or its hotkey.
- Playback starts only after the user presses Play or its hotkey.
- A loaded recording is never run automatically.
- During playback, Stop and emergency stop must remain available.

## Architecture

Rust owns the behavior that touches the operating system. React owns presentation and user input.

Rust modules:

- `recorder`: manages recording state and converts low-level input events into macro steps.
- `player`: runs macro steps according to timing, speed multiplier, and loop count.
- `input`: wraps Windows APIs for input capture and playback.
- `hotkeys`: registers and dispatches global shortcuts.
- `storage`: reads and writes `.remember.json`.
- `tray`: creates tray menu actions and window restore/minimize behavior.
- `commands`: exposes Tauri commands consumed by React.

React modules:

- `App`: top-level layout and state wiring.
- `Controls`: Record, Play, Stop, Save, Open actions.
- `PlaybackSettings`: loop count and speed multiplier inputs.
- `StatusPanel`: current state, step count, duration, and recent messages.
- `HotkeyPanel`: read-only first-release hotkey display.

State ownership:

- Rust is the source of truth for recording and playback state.
- React requests actions through Tauri commands and listens for state update events.
- React must not simulate recorder or player state independently beyond optimistic button disabling.

## Recording Model

A recording is a sequence of timestamped steps.

Step categories:

- `mouse_move`: cursor position and elapsed time.
- `mouse_button`: button, pressed/released state, cursor position, and elapsed time.
- `mouse_wheel`: wheel delta, cursor position, and elapsed time.
- `key`: virtual key, scan code when available, pressed/released state, and elapsed time.
- `wait`: explicit timing gap when useful after compaction.

Mouse movement can produce high event volume. The recorder should keep movement steps at roughly 50 ms intervals during continuous movement, and it should always preserve the cursor position attached to clicks and wheel events.

## File Format

Recordings are stored as JSON with a stable version field.

Required top-level fields:

- `version`
- `name`
- `created_at`
- `duration_ms`
- `steps`

The first version is `1`.

The app should reject invalid files with a readable error. It should not silently run malformed data.

## Playback Model

Playback replays steps in recorded order.

Rules:

- Time gaps are scaled by the selected speed multiplier.
- Loop count must be at least `1`.
- Speed multiplier must be positive.
- Playback must stop promptly when the user presses Stop or the emergency hotkey.
- Playback should report progress back to the UI.

Windows input playback should use appropriate system APIs rather than UI tree automation. The first release does not attempt to understand application controls.

## Safety And Limits

Required safeguards:

- Emergency stop global hotkey.
- No automatic playback on app start or file open.
- Validation before loading a recording.
- Clear error if Windows denies input capture or playback.
- Clear limitation notice if elevated/admin target windows cannot be controlled from a non-elevated app.

First release does not need target-window validation. That can be added later if unsafe replay becomes a problem.

## Testing Strategy

Rust tests:

- JSON round-trip for a valid recording.
- Invalid JSON/schema rejection.
- Loop count validation.
- Speed multiplier validation.
- Playback timing calculation.
- Recorder state transitions.
- Player state transitions and stop request handling.

React tests:

- Buttons enable and disable based on app state.
- Loop count and speed controls reject invalid input.
- Status panel renders recording and playback updates.

Manual acceptance test:

1. Launch Remember.
2. Open Notepad.
3. Start recording.
4. Type a short phrase and perform one mouse click.
5. Stop recording.
6. Save the file as `.remember.json`.
7. Reopen the file in Remember.
8. Replay into Notepad.
9. Confirm the phrase and click are reproduced.
10. Confirm stop and emergency stop interrupt playback.

## Delivery Criteria

The first release is complete when:

- `npm test` passes.
- `npm run build` passes.
- Rust tests pass.
- Tauri build produces a runnable portable executable.
- The manual acceptance test succeeds on Windows.
- The repository includes a README with run, build, and usage instructions.
