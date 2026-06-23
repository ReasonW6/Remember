# Remember Project Agent Guide

## Product North Star

Remember is a lightweight Windows desktop automation app.

Its job is to record repetitive user operations and replay them reliably:
mouse clicks, keyboard input, hotkeys, waits, loop count, playback speed, and
safe stop controls.

The product must feel small, practical, and trustworthy. It should be closer to
TinyTask in everyday use than to a large enterprise RPA suite.

## Fixed Product Direction

- Platform: Windows first and Windows-only for the initial product.
- Tech stack: Tauri + Rust backend + React + TypeScript frontend.
- Default startup surface: a tiny control window.
- Expanded surface: a larger workbench/settings window opened from the small
  window.
- The small window is for daily operation: record, replay, stop, speed, count,
  selected flow, status, hotkey hint, and settings entry.
- The large window is for advanced work: flow editing, step timeline, hotkeys,
  window matching, safety settings, variables, logs, import/export, and history.
- Themes must be unified across windows. Do not mix a dark small window with a
  light large window, or the reverse. If the selected direction is light Mica,
  both windows are light Mica. If the selected direction is dark acrylic/Mica,
  both windows are dark acrylic/Mica.
- UI style should be compact Windows 11 desktop software with Mica/acrylic or
  frosted-glass feel. Avoid landing pages, oversized cards, decorative blobs,
  mascot-style visuals, or marketing layout.

## MVP Scope

Build the first usable version around this loop:

1. Open the tiny control window.
2. Start recording.
3. Capture key user actions.
4. Stop recording.
5. Review/edit the generated steps in the workbench.
6. Save the flow locally.
7. Replay the flow with speed and loop controls.
8. Stop immediately through a global emergency hotkey.

The MVP should support:

- Mouse click recording.
- Keyboard input and hotkey recording.
- Wait/delay between steps.
- Playback speed.
- Playback count.
- Start, pause/stop, and emergency stop.
- Save/load local flow files.
- Basic flow editing in the workbench.
- Basic run log.

Do not add these before the MVP is stable:

- OCR.
- Image recognition.
- AI planning.
- Cloud sync.
- Remote execution.
- Cross-platform support.
- Complex condition branches.
- Script language.
- Plugin system.

## Expected Data Model

Prefer a simple JSON flow format that can evolve:

```json
{
  "version": 1,
  "name": "Daily Report",
  "steps": [
    { "type": "click", "x": 120, "y": 240, "button": "left", "delayMs": 200 },
    { "type": "type", "text": "Daily Report", "delayMs": 300 },
    { "type": "hotkey", "keys": ["Ctrl", "S"], "delayMs": 1000 },
    { "type": "wait", "durationMs": 500 }
  ]
}
```

Keep the model small and explicit. Add fields only when a real feature needs
them.

## Development Discipline

Before changing code:

1. Inspect the current files and project state.
2. Read this `AGENTS.md`.
3. If roadmap, changelog, or design notes exist, read them before deciding the
   next step.
4. Identify the smallest useful next change.
5. If nearby broken behavior blocks the change, fix and verify it first.

While coding:

- Prefer simple, focused modules.
- Follow existing project patterns once the project exists.
- Do not rewrite large areas just to make them look cleaner.
- Keep UI compact and functional.
- Keep Rust backend boundaries clear: recording, playback, hotkeys, window
  inspection, file persistence, and app commands should not be tangled together.
- Keep React frontend state predictable and testable.
- Use type-safe interfaces between React and Tauri commands.
- Do not hide risky behavior behind automation. Emergency stop must remain a
  first-class feature.

After coding:

1. Re-read the changed code.
2. Run the smallest meaningful verification command.
3. For UI changes, launch the app and visually verify the relevant window.
4. Summarize what changed, what was verified, and what remains.

## Shortcut Words For Future Work

When the user says "推进", "继续", "继续开发", or "完善":

1. Recover context by reading `AGENTS.md` and existing local docs.
2. Inspect the repository state.
3. Choose the smallest next task that moves the MVP forward.
4. Implement it, unless the task is clearly blocked by a product decision.
5. Verify it.
6. Update any relevant local planning notes if they exist.
7. Report the result and the next recommended step.

When the user says "审查", "审阅", "检查代码", or "review":

1. Use a review-first workflow.
2. Do not silently implement fixes.
3. Inspect the relevant code and list findings first.
4. Order findings by severity.
5. Include file and line references when files exist.
6. Mention missing tests or unverified behavior.
7. Only implement fixes after the user asks for fixes or says to continue.

When the user says "设计", "UI", "界面", or "生成界面":

1. Stay in product/UI design mode unless implementation is explicitly requested.
2. Preserve the two-window product shape.
3. Keep small and large windows visually unified.
4. Prefer compact native Windows utility design over web-style layouts.

When the user says "停止":

1. Treat it as a request to stop running project processes.
2. Scope process termination to this project directory whenever possible.
3. Do not kill unrelated Node, Rust, or browser processes.

When the user says "打包", "发布", or "安装包":

1. Verify tests/build first.
2. Build the Windows desktop package.
3. Report the artifact path and any installer/runtime requirements.

## Verification Expectations

Use the most relevant checks for the current stage.

Before the project is scaffolded:

- Confirm files were created as expected.
- Confirm no accidental unrelated files were changed.

After Tauri scaffolding exists:

- Run frontend type/lint checks if configured.
- Run Rust formatting/checks if configured.
- Run the Tauri dev app for visible UI work.
- For recording/playback work, use a safe test flow before claiming success.

Never claim that global input recording, replay, hotkeys, or emergency stop are
working without running a real verification path.

## Safety Rules

- Emergency stop is required before serious playback work.
- Infinite loop playback must require explicit confirmation.
- Prefer foreground/active-window operation for MVP.
- Do not default to hidden background automation.
- Warn about sensitive input recording.
- Do not record passwords intentionally.
- If target-window matching fails, stop or ask rather than clicking blindly.

## Suggested Future Structure

This is guidance, not a command to create all files immediately.

```text
src/
  app/
  components/
    control-window/
    workbench/
    timeline/
    settings/
  state/
  types/

src-tauri/
  src/
    commands/
    recorder/
    player/
    hotkeys/
    windows/
    storage/
    safety/
```

Keep each module responsible for one thing. Split only when the code has a real
reason to split.

## Current First Milestone

The first milestone is not full automation. It is a polished shell that proves
the product shape:

- Tauri app opens a compact control window by default.
- Settings opens the larger workbench window.
- Both windows share one unified theme.
- Buttons and state transitions are wired with placeholder behavior.
- The app can be launched and visually checked on Windows.

After that, build recording, playback, and persistence in small verified steps.
