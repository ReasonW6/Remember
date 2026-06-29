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

The MVP product-complete state defined in this document has been reached.
Remember now has the two-window Tauri desktop shape, local `.remember.json`
storage, recording, editing, guarded playback, visible stop/emergency-stop
controls, app-local packaging, and user-facing documentation.

`ROADMAP.md` is the authoritative detailed progress log for implemented
milestones, verification notes, and current post-MVP hardening status. Keep this
file focused on product goals and acceptance standards so the two documents do
not drift.
