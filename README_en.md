# Remember

[中文](README.md)

Remember is a lightweight Windows macro recorder inspired by the TinyTask workflow: press a hotkey to record keyboard and mouse input, press it again to stop, then save the result as a local `.remember.json` file and replay it later.

Remember is an original implementation. It does not copy TinyTask code, icons, names, binaries, or assets.

## Features

- Record keyboard and mouse actions.
- Replay the current recording or choose a saved recording from the in-app list.
- Configure playback loop count and playback speed.
- Customize hotkeys with either single keys or key combinations.
- Use the same hotkey for recording and stopping. The default record/stop toggle is `F8`.
- Play feedback tones when recording or playback starts and stops.
- Use a custom titlebar and localized Chinese interface.

## Default Hotkeys

- `F8`: start recording; press again while recording to stop; press while playing to stop playback.
- `F12`: start playback.

The playback hotkey must be different from the record and stop hotkeys. The record and stop hotkeys may be the same.

## Recording Files

Recording files are saved as `.remember.json`. The in-app recording list shows saved recordings and supports selecting, replaying, and deleting them.

Recordings saved to the library are stored in the app data `recordings` directory, for example:

```text
%APPDATA%\com.remember.desktop\recordings
```

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

## Packaging

Create a release build:

```powershell
npm run tauri build
```

The release output is generated under:

```text
src-tauri\target\release
```

## Current Limits

- Windows only.
- No AI automation or image recognition.
- Playback sends real keyboard and mouse input, so focus and target-window state affect results.
- Elevated windows may reject input from a non-elevated Remember process.
