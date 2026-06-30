# Remember

Remember is a portable Windows macro recorder inspired by the TinyTask workflow: record a short keyboard and mouse sequence, replay it, and keep the resulting recording as a local `.remember.json` file.

Remember is an original implementation. It does not copy TinyTask code, icons, names, binaries, or assets.

## Requirements

- Windows
- Node.js 22.12+
- Rust stable
- Tauri Windows prerequisites

## Development

Install dependencies:

```powershell
npm install
```

Run the desktop app in development:

```powershell
npm run tauri dev
```

## Tests

Run the frontend tests:

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

## Portable Build

Create a release build:

```powershell
npm run tauri build
```

The executable is generated under:

```text
src-tauri\target\release
```

## Default Hotkeys

- `Ctrl+Alt+R`: start or stop recording
- `Ctrl+Alt+P`: start playback
- `Ctrl+Alt+Esc`: stop playback

## First Release Limits

- Windows only.
- No image recognition.
- No AI automation.
- No step editor.
- No target-window validation.
- Elevated windows may reject input from a non-elevated Remember process.
