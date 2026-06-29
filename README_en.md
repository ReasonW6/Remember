# Remember

Remember is a lightweight Windows desktop automation app built with Tauri,
Rust, React, and TypeScript. It records local foreground actions into
`.remember.json` flows and replays them with basic safety checks.

中文版本: [README.md](README.md)

## Current Scope

Remember currently supports the MVP workflow:

- Record mouse clicks, double-clicks, drags, scrolls, keyboard text, ordinary
  keys, hotkeys, waits, and active-window metadata.
- Review recorded steps in the workbench.
- Edit delays, click coordinates, typed text, hotkey combinations, and target
  window confirmation.
- Delete steps and insert wait, text, hotkey, and ordinary key steps.
- Save and load local `.remember.json` flows across app restarts.
- Replay wait, click, drag, type, key, hotkey, and scroll steps with speed and
  loop count.
- Stop playback from the UI or with the global emergency hotkey
  `Ctrl + Alt + S`.
- Show run logs and visible safety-stop messages when target-window checks
  fail.

Remember is Windows-first and local-first. It does not include cloud sync, OCR,
image recognition, AI planning, plugins, remote execution, or cross-platform
support.

## Install Or Run

Use the packaged Windows installer after a successful build:

```powershell
src-tauri\target\release\bundle\nsis\Remember_0.1.0_x64-setup.exe
```

For development:

```powershell
node --version  # requires Node.js 22.12 or newer
npm install
npm run tauri dev
```

The dev server uses `127.0.0.1:1450`.

## Daily Workflow

1. Open Remember. The compact control window appears first.
2. Select a saved flow, or start from the current default flow.
3. Click `录制`.
4. Confirm the recording warning.
5. Perform the safe local actions you want Remember to capture.
6. Click `停止`.
7. The workbench opens with the recorded steps.
8. Review and edit the steps as needed.
9. Click `保存流程` or `另存为`.
10. Click `运行` or `重放` to replay.
11. Press `Ctrl + Alt + S` at any time during playback for emergency stop.

## Supported Input

Remember records ordinary text, mouse actions, modifier hotkeys, and common
control keys. Function-key hotkeys such as `Ctrl + F10` are saved as flow steps
for the target app; plain `F10` is not recorded and is not a global shortcut for
starting playback.

The workbench is a desktop editing surface with a 1080 px minimum window width.
On smaller screens, use Windows display scaling or move to a wider display
before editing dense step lists.

## Safety Notes

- Do not record passwords, verification codes, private messages, payment
  details, or other sensitive fields.
- Playback input steps require target-window metadata. If the target is missing
  or clearly different, Remember safety-stops instead of clicking or typing.
- Infinite-loop playback requires explicit confirmation and must be stopped
  from the UI or with the emergency hotkey.
- The app is intended for foreground desktop automation. It is not a hidden
  background automation runner.

## Flow Files

Flows are stored as app-local `.remember.json` files. The v1 shape is small and
explicit:

```json
{
  "version": 1,
  "name": "daily-report",
  "displayName": "Daily Report",
  "targetWindow": {
    "title": "Report - Notepad",
    "process": "notepad.exe",
    "size": "1024 x 768",
    "matched": true
  },
  "steps": [
    { "type": "click", "id": 1, "action": "左键单击", "target": "(120, 240) [屏幕绝对]", "x": 120, "y": 240, "delayMs": 200, "note": "open menu" },
    { "type": "type", "id": 2, "action": "文本输入", "text": "Daily Report", "delayMs": 300, "note": "title" },
    { "type": "hotkey", "id": 3, "action": "快捷键", "keys": ["Ctrl", "S"], "delayMs": 100, "note": "save" },
    { "type": "wait", "id": 4, "action": "等待", "durationMs": 500, "delayMs": 500, "note": "pause" }
  ]
}
```

Storage validation rejects malformed files, unsupported versions, duplicate
step IDs, missing flow names, empty key or hotkey steps, high-risk global
hotkeys, obvious sensitive step text or step metadata, excessive step timings,
and wait steps whose `durationMs` and `delayMs` diverge. Broken flow files
remain visible in the flow list so they can be identified instead of being
silently hidden.

## Build And Verify

Run the standard checks:

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo check --manifest-path src-tauri\Cargo.toml
cargo test --manifest-path src-tauri\Cargo.toml
```

Build the Windows installer:

```powershell
npm run tauri build
```

Expected output:

```text
src-tauri\target\release\bundle\nsis\Remember_0.1.0_x64-setup.exe
```

The release executable is also produced at:

```text
src-tauri\target\release\remember.exe
```

Release artifacts must have a `Valid` Authenticode signature before being
published. `npm run verify:release-signature` checks both the release executable
and NSIS installer and fails if either artifact is unsigned, missing, or signed
with an untrusted certificate. The check runs with PowerShell 7 (`pwsh`). A real
Windows code-signing certificate is still required for a publishable signed
build.

Pre-publish signature check:

```powershell
npm run verify:release-signature
```

## Known Remaining Polish

- The app icon is stored at `src-tauri/icons/icon.ico`; the editable source is
  `src-tauri/icons/remember-icon.svg`.
- Tray behavior is intentionally not part of the current MVP because the
  compact always-on-top control window and global emergency hotkey keep safety
  controls visible.
- Advanced target-window matching controls are not part of the current MVP.
