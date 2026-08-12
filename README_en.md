# Remember

[中文](README.md)

Remember is a lightweight Windows macro recorder inspired by the TinyTask workflow: press a hotkey to record keyboard and mouse input, then press it again to stop. The result is saved automatically to the local library and can also be exported as a `.remember.json` file for later replay.

Remember is an original implementation. It does not copy TinyTask code, icons, names, binaries, or assets.

## Features

- Record keyboard and mouse actions.
- Record in V1 screen coordinates by default for legacy behavior, or enable Window-relative recording before capture to create a V2 file that follows whole-window movement across multiple windows.
- Replay the current recording or choose a saved recording from the in-app list.
- Configure finite or infinite playback loops, playback speed, and an inter-loop delay.
- Customize hotkeys. Unmodified single-key shortcuts are limited to `F1`–`F24`; character, editing, and navigation keys require `Ctrl`, `Alt`, `Shift`, or `Win`.
- Remember the main window's compact or expanded interface and desktop position. Ordinary launch and “Restart as administrator” restore the same state; if the original monitor is gone, the window returns to a visible display.
- Use the same hotkey for recording and stopping. The default record/stop toggle is `F8`; during playback, both the play and stop hotkeys can stop the run.
- Play feedback tones when recording or playback starts and stops.
- Starting a recording from the main window does not minimize it automatically. Keyboard and mouse input inside Remember's own windows is filtered out of recordings.
- Start in a compact floating window on first use, with only recording selection, record, and play controls; use the titlebar button for a smooth transition to the full interface, and later launches restore the last choice. The transition is skipped when the system requests reduced motion.
- Save Advanced Settings with “Save and exit.” Closing the window directly discards unsaved edits, and the next open reloads persisted settings.
- Closing the main window safely stops any active recording or playback, then exits completely without staying in the background.
- Use a custom titlebar and localized Chinese interface.

## Default Hotkeys

- `F8`: start recording; press again while recording to stop; use it as the independent stop hotkey during playback.
- `F12`: start playback while idle; press it again during playback to stop.

The playback hotkey must be different from the record and stop hotkeys. The record and stop hotkeys may be the same. To avoid hijacking normal input, unmodified shortcuts are limited to `F1`–`F24`.

During playback, either `F8` or `F12` stops the run. Remember first releases any keys or mouse buttons that are still held down; the mode remains playing and reports “Stopping playback” until cleanup finishes, then returns to idle.

## Recording Files

Recording files are saved as `.remember.json`. Every time recording stops, Remember automatically saves the current recording to the local library. The Save button exports an additional copy to a user-selected location. The in-app list supports selecting, replaying, renaming, and deleting recordings. A normal delete requires confirmation; holding `Ctrl` while clicking delete permanently removes the recording immediately. Corrupt, oversized, or unreadable recording files remain visible with an error and cannot be loaded or replayed.

A recording can contain at most 250,000 steps, and a recording JSON file can be at most 64 MiB. When recording reaches the step limit, Remember truncates it, saves the captured prefix after stopping, and displays a warning.

The recording list marks each valid file as V1 or V2:

- **V1 (default)** preserves the original behavior and stores absolute screen coordinates. Playback does not match window identity, so the desktop layout, window position, and current focus can all affect the result.
- **V2 (optional)** stores in-window actions relative to the client-area origin of each target top-level window and can operate across multiple windows in one recording. During playback, Remember considers only unoccupied windows with an exact full-executable-path and window-class match and compatible initial client-area size and DPI, then automatically selects the highest-ranked candidate by recorded-title similarity and stable window order. It never opens a target-window picker. Once bound, coordinates are translated by the window's whole-movement offset.

V2 recognizes recorded window move or resize gestures. Every step in the gesture uses the client-area origin captured at button press, preventing the moving window from feeding displacement back into the pointer path; after release, the resulting client-area size becomes the current size for later actions. Client-area size and DPI must still strictly match when the target is first bound, and DPI or display-scaling changes and internal control reflow remain unsupported.

When V2 clicks a taskbar flyout, launcher, or another background surface, it does not require the clicked surface itself to become foreground. Playback completes the press and release so that surface can close or open another window, then validates the actual target at the next operation that requires a foreground window. This prevents launcher flows such as a network flyout opening Settings from being rejected as failed activation.

Recording metadata still distinguishes initial targets from deferred targets first seen later, but playback no longer requires initial targets during startup. Every target is waited for up to 30 seconds and bound automatically only when an operation first needs it; playback stops if the wait expires without a compatible candidate. This accommodates Windows reusing an existing host window for a page or dialog opened later during recording. A minimized or hidden target is restored and brought to the foreground only immediately before a click, wheel, keyboard, or other recorded operation that affects that window.

If the recording pointer enters a window or privilege boundary that Remember cannot inspect, the app shows the specific reason and stops capturing ordinary mouse and keyboard steps at that entry point. Recording continues after the pointer returns to a readable surface. Elapsed time across the unreadable interval is preserved, so playback holds the pointer at the last readable location, skips the unreadable input, and resumes at the next readable location.

The recording library is the `recordings` folder next to `remember.exe`:

```text
<application directory>\recordings
```

When the application directory is on drive D, recordings stay on drive D as well. The current user must have write access to the application directory, so do not place the portable build in a protected directory. Files in the legacy `%APPDATA%\com.remember.desktop\recordings` directory are not moved or deleted automatically.

Recording files are unencrypted JSON. They contain virtual key codes, scan codes, press/release timing, and mouse positions, so they may reveal passwords, tokens, or other sensitive input. V2 also stores each target window's full executable path, window class, and recorded title in plaintext; those fields may expose user names, installation locations, document names, or page titles. Handle every recording carefully and treat V2 files in particular as sensitive. Avoid recording secrets, inspect recordings before sharing, backing up, or uploading them, and delete recordings you no longer need.

## Playback Safety

- Loop count can be a finite integer of at least 1 or an explicitly selected infinite loop.
- Inter-loop delay is available only when the loop count is greater than 1 or infinite. It runs only between iterations, never after the final iteration, and can be interrupted with the play or stop hotkey.
- Infinite playback does not end by itself and must be stopped with the play or stop hotkey.
- V1 does not validate target-window identity and sends real input at absolute screen coordinates. V2 matches windows and checks client-area size and DPI, but this is not control-level safety validation: pop-ups, changed window content, or another program taking focus can still redirect real input.
- A non-elevated Remember process cannot reliably inspect or control an elevated window. After an explicit access-denied warning, the user may choose to restart as administrator; Remember does not automatically bypass the Windows privilege boundary.
- Before replaying an old or externally supplied recording, verify the focus, target window, and recording source.

## Requirements

- Windows
- Node.js 22.12+
- Rust stable
- Tauri 2 Windows build prerequisites

## Development

Install dependencies:

```powershell
npm install
```

Run the desktop app in development:

```powershell
npm run tauri dev
```

Development mode starts both the frontend dev server and the Tauri app. Release executables use the Windows GUI subsystem and should not open an extra console window.

## Tests and Build

Run frontend tests:

```powershell
npm test
```

Build the frontend:

```powershell
npm run build
```

Run Rust tests:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

Check Rust compilation:

```powershell
cargo check --manifest-path src-tauri\Cargo.toml
```

Check Rust formatting:

```powershell
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
```

Run Rust lint and dependency security checks:

```powershell
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features --locked -- -D warnings
npm audit
```

CI also checks `src-tauri\Cargo.lock` against RustSec advisories. If `cargo-audit` is installed locally, run the equivalent check with:

```powershell
cargo audit --file src-tauri\Cargo.lock
```

## Packaging

Create a release build:

```powershell
npm run tauri build
```

The release output is generated under:

```text
src-tauri\target\release
```

The Windows CI workflow runs frontend tests, npm audit, Rust tests, Clippy, and a RustSec audit, builds the portable `remember.exe`, and generates a SHA-256 checksum. CI artifacts are explicitly unsigned release candidates; SHA-256 detects file changes but does not authenticate the publisher.

Before a public release, sign and timestamp `remember.exe` with a real trusted Authenticode certificate, then verify its status:

```powershell
Get-AuthenticodeSignature .\remember.exe
```

Do not describe a self-signed binary or a checksum-only artifact as an officially signed release.

## Download and code signing policy

- Official downloads: [GitHub Releases](https://github.com/ReasonW6/Remember/releases)
- Full policy: [Code signing policy](CODE_SIGNING_POLICY.md)
- The SignPath Foundation application is currently pending; existing releases remain unsigned.
- If the application is accepted, future Windows release files will use the following statement: Free code signing provided by [SignPath.io](https://about.signpath.io), certificate by [SignPath Foundation](https://signpath.org).

Remember is licensed under the [MIT License](LICENSE).

## Current Limits

- Windows only.
- No AI automation or image recognition.
- V1 uses absolute screen coordinates without window-identity validation. V2 translates by the client-area origin and can reproduce recorded window move or resize gestures, but it does not support DPI changes, arbitrary resize adaptation, or control reflow.
- Both formats replay real keyboard and mouse input. Focus, pop-ups, and target-window content can still affect the result even with V2.
- A non-elevated process cannot reliably inspect or control elevated windows; restarting as administrator requires an explicit user choice.

Documents under `docs/superpowers` are historical design and implementation records and may retain older hotkeys or scope. Current behavior is defined by this README, the tests, and the source code.
