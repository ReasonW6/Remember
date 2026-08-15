<div align="center">
  <img src="src-tauri/icons/remember-icon.svg" width="88" alt="Remember icon">
</div>

<h1 align="center">Remember</h1>

<p align="center">A lightweight, portable, fully local keyboard and mouse recorder for Windows.</p>

<p align="center">
  <a href="https://github.com/ReasonW6/Remember/actions/workflows/windows-ci.yml"><img src="https://github.com/ReasonW6/Remember/actions/workflows/windows-ci.yml/badge.svg" alt="Windows CI"></a>
  <a href="https://github.com/ReasonW6/Remember/releases/latest"><img src="https://img.shields.io/github/v/release/ReasonW6/Remember" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/platform-Windows-0078D4?logo=windows" alt="Windows">
  <img src="https://img.shields.io/badge/code%20signing-unsigned-orange" alt="Unsigned">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/ReasonW6/Remember" alt="MIT License"></a>
</p>

<p align="center">
  <a href="https://github.com/ReasonW6/Remember/releases/latest">Download</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#recording-modes">Recording modes</a> ·
  <a href="#development-and-builds">Development</a> ·
  <a href="README.md">中文</a>
</p>

<div align="center">
  <img src="docs/images/remember-main.png" width="420" alt="Remember full interface">
</div>

Remember follows the familiar TinyTask workflow: press a hotkey to record keyboard and mouse input, press it again to stop, then replay the result whenever needed. Recordings stay on your computer. No account or network upload is required.

Remember is an original implementation. It does not contain TinyTask code, icons, names, binaries, or assets.

## Highlights

- Record and replay real keyboard and mouse input with global hotkeys.
- Choose between V1 screen coordinates and V2 window-relative coordinates.
- V2 finds, restores, and activates a target only when an operation is about to use it. It does not bring every application to the foreground when playback starts.
- Replay workflows that open new windows during recording; each deferred window is resolved when its first operation arrives.
- Select standard Windows `ComboBox` options by visible name. If “Mihomo” was recorded, playback still tries to select “Mihomo” after the list order changes.
- Show a cursor-side explanation while playback waits or stops. A privileged target produces an immediate administrator guidance message instead of a misleading missing-window wait.
- Manage a local recording library and configure finite or infinite loops, playback speed, and inter-loop delay.
- Switch between a compact floating window and the full interface. Remember restores the last interface mode and visible window position.

## Compact interface

<div align="center">
  <img src="docs/images/remember-compact.png" width="540" alt="Remember compact floating window">
</div>

The compact interface keeps recording selection, the V1/V2 toggle, Record, Play, and the administrator-mode entry available in a small desktop footprint.

## Quick start

1. Download `remember.exe` from [GitHub Releases](https://github.com/ReasonW6/Remember/releases/latest).
2. Put it in a user-writable directory such as `D:\Apps\Remember`. Remember is a portable application and does not need installation.
3. Run `remember.exe`. By default, `F8` starts or stops recording and `F12` starts or stops playback.
4. Enable Window-relative recording before capture when the replay should follow whole-window movement.
5. Before playback, verify the target windows, current focus, and recording source. Press `F8` or `F12` for an emergency stop.

Remember currently has no automatic updater. Use the Releases page to obtain newer versions.

## Default hotkeys

| Hotkey | Idle | Recording | Playing |
| --- | --- | --- | --- |
| `F8` | Start recording | Stop recording | Stop playback |
| `F12` | Start playback | No action | Stop playback |

Hotkeys can be changed in Advanced Settings. To avoid intercepting normal typing, unmodified single-key shortcuts are limited to `F1`–`F24`; character, editing, and navigation keys require `Ctrl`, `Alt`, `Shift`, or `Win`.

## Recording modes

| | V1 screen coordinates | V2 window-relative coordinates |
| --- | --- | --- |
| Default | Enabled by default | Opt in before recording |
| Coordinates | Absolute virtual-desktop position | Position relative to a target client area |
| Window movement | Can cause replay drift | Follows whole-window movement |
| Multi-window flows | Depends on the original layout and focus | Resolves multiple target windows per step |
| Window identity | Not checked | Checks full executable path, window class, initial client size, and DPI |
| Standard dropdowns | Coordinate replay | Can also select by visible option name |
| Privacy metadata | Input and timing | Also stores executable paths, classes, and recorded titles |

### How V2 handles windows

- A target is automatically bound only when its first effective operation is about to run. There is no target-window picker.
- Ordinary pointer motion with no held input never waits for, restores, wakes, activates, or foreground-checks a background window.
- Windows first opened during recording become deferred targets. Playback waits for up to 30 seconds when the corresponding step is reached.
- Short-lived menus and dropdown surfaces are not opened during playback startup. Legacy `ComboLBox` targets match only while the list is actually visible.
- If the target is already visible but has a higher Windows integrity level, playback stops immediately and asks the user to restart Remember as administrator.
- Initial client size and DPI must be compatible. V2 cannot understand internal control reflow or adapt to arbitrary resizing and display-scaling changes.

## Recording files and privacy

Every completed recording is saved automatically in the `recordings` folder next to `remember.exe`. The Save button exports an additional `.remember.json` copy to a location chosen by the user.

```text
<application directory>\recordings
```

- One recording may contain up to 250,000 steps; one JSON file may be up to 64 MiB.
- The in-app list supports selection, replay, rename, and delete. Normal deletion asks for confirmation; holding `Ctrl` while clicking Delete permanently removes the file immediately.
- Corrupt, oversized, or unreadable files remain visible with an error and cannot be loaded or replayed.
- Files in the legacy `%APPDATA%\com.remember.desktop\recordings` directory are not moved or deleted automatically.

Recordings are unencrypted JSON. They may contain virtual key codes, scan codes, press and release timing, mouse positions, and, for V2, full executable paths, window classes, and recorded titles. Do not record passwords, tokens, or other secrets. Inspect files before sharing, backing up, or uploading them.

## Playback safety and privileges

- Both V1 and V2 send real system keyboard and mouse input. Pop-ups, focus changes, or changed target content can still redirect an action.
- Infinite playback never ends by itself and must be stopped with the play or stop hotkey.
- When stopping playback, Remember releases any key or mouse button still held down before returning to idle.
- A non-elevated Remember process cannot reliably inspect or control an elevated window. After a privilege warning, the user must explicitly choose Restart as administrator. The UAC secure desktop still requires manual interaction.
- Do not replay `.remember.json` files from an untrusted source.

## Downloads, verification, and signing status

Official files are available only from [GitHub Releases](https://github.com/ReasonW6/Remember/releases):

- `remember.exe`: optimized Windows x64 portable build for normal use.
- `remember.exe.sha256`: SHA-256 checksum for the optimized build.
- `remember-debug.exe`: diagnostic debug build, not recommended for normal use.
- `remember-debug.exe.sha256`: SHA-256 checksum for the debug build.

**Current Windows executables are not Authenticode code-signed.** Windows may display an Unknown publisher or SmartScreen warning. SHA-256 verifies that a download matches the release asset, but it does not prove publisher identity.

Official release files are built by the repository's Windows CI from the corresponding source revision and include SHA-256 checksums and GitHub build provenance. Local builds are for development verification only and must not be uploaded to an official release.

## Current limitations

- Windows x64 only.
- No AI automation or image recognition. Remember does not decide actions from screen content.
- V1 depends on screen position and focus. V2 compensates for whole-window movement but does not support arbitrary resizing, DPI changes, or internal control reflow.
- Browsers, custom-drawn interfaces, and non-standard dropdowns generally fall back to coordinate replay.
- Recording contents are not encrypted; users are responsible for protecting recording files.

## Development and builds

### Prerequisites

- Windows
- Node.js 24.13.1
- Rust 1.94.1 stable
- Tauri 2 Windows build prerequisites

The repository CI pins the Node.js and Rust versions above. Other newer versions may work, but they are outside the current validation baseline.

### Run locally

```powershell
npm install
npm run tauri dev
```

### Verify

```powershell
npm test
npm run build
npm audit --audit-level=moderate
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo test --manifest-path src-tauri\Cargo.toml --locked
```

CI also runs RustSec auditing, coverage thresholds, and a real Windows input round trip.

### Build the local application

```powershell
npm run tauri build
```

The optimized local executable is written to `src-tauri\target\release\remember.exe`. It is intended only for local development verification.

## License

Remember is available under the [MIT License](LICENSE).
