# Remember

[中文](README.md)

Remember is a lightweight Windows macro recorder inspired by the TinyTask workflow: press a hotkey to record keyboard and mouse input, then press it again to stop. The result is saved automatically to the local library and can also be exported as a `.remember.json` file for later replay.

Remember is an original implementation. It does not copy TinyTask code, icons, names, binaries, or assets.

## Features

- Record keyboard and mouse actions.
- Replay the current recording or choose a saved recording from the in-app list.
- Configure finite or infinite playback loops and playback speed.
- Customize hotkeys. Unmodified single-key shortcuts are limited to `F1`–`F24`; character, editing, and navigation keys require `Ctrl`, `Alt`, `Shift`, or `Win`.
- Use the same hotkey for recording and stopping. The default record/stop toggle is `F8`; during playback, both the play and stop hotkeys can stop the run.
- Play feedback tones when recording or playback starts and stops.
- Use a custom titlebar and localized Chinese interface.

## Default Hotkeys

- `F8`: start recording; press again while recording to stop; use it as the independent stop hotkey during playback.
- `F12`: start playback while idle; press it again during playback to stop.

The playback hotkey must be different from the record and stop hotkeys. The record and stop hotkeys may be the same. To avoid hijacking normal input, unmodified shortcuts are limited to `F1`–`F24`.

During playback, either `F8` or `F12` stops the run. Remember first releases any keys or mouse buttons that are still held down; the mode remains playing and reports “Stopping playback” until cleanup finishes, then returns to idle.

## Recording Files

Recording files are saved as `.remember.json`. Every time recording stops, Remember automatically saves the current recording to the local library. The Save button exports an additional copy to a user-selected location. The in-app list supports selecting, replaying, renaming, and deleting recordings; hold `Ctrl` while clicking delete to skip confirmation. Corrupt or unreadable recording files remain visible with an error and cannot be loaded or replayed.

The recording library is the `recordings` folder next to `remember.exe`:

```text
<application directory>\recordings
```

When the application directory is on drive D, recordings stay on drive D as well. The current user must have write access to the application directory, so do not place the portable build in a protected directory. Files in the legacy `%APPDATA%\com.remember.desktop\recordings` directory are not moved or deleted automatically.

Recording files are unencrypted JSON. They contain virtual key codes, scan codes, press/release timing, and mouse positions, so they may reveal passwords, tokens, or other sensitive input. Avoid recording secrets, inspect recordings before sharing, backing up, or uploading them, and delete recordings you no longer need.

## Playback Safety

- Loop count can be a finite integer of at least 1 or an explicitly selected infinite loop.
- Infinite playback does not end by itself and must be stopped with the play or stop hotkey.
- Remember intentionally does not validate the target window. It sends real input to whichever window has focus, and mouse position and window layout affect the result.
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

## Current Limits

- Windows only.
- No AI automation or image recognition.
- Playback sends real keyboard and mouse input, so focus and target-window state affect results.
- Target-window validation is intentionally not implemented in the current portable tool.
- Elevated windows may reject input from a non-elevated Remember process.

Documents under `docs/superpowers` are historical design and implementation records and may retain older hotkeys or scope. Current behavior is defined by this README, the tests, and the source code.
