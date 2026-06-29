# Remember Project Agent Guide

This file is the local operating guide for agents working in
`D:\GitHub_Project\Remember`.

## Product North Star

Remember is a lightweight Windows desktop automation app. It records repetitive
foreground user operations and replays them reliably: mouse actions, keyboard
input, hotkeys, waits, loop count, playback speed, and safe stop controls.

The product should feel small, practical, and trustworthy. It should stay closer
to TinyTask in daily use than to a large enterprise RPA suite.

## Fixed Product Direction

- Platform: Windows first and Windows-only for the initial product.
- Tech stack: Tauri + Rust backend + React + TypeScript frontend.
- Default surface: compact always-on-top control window.
- Expanded surface: larger workbench/settings window opened from the control
  window.
- Visual style: compact Windows 11 desktop software, currently unified dark
  Mica/acrylic-inspired UI.
- Both windows must share one theme. Do not mix light and dark surfaces.
- Avoid landing pages, marketing layouts, decorative blobs, mascot visuals,
  cloud-first workflows, or large RPA-suite complexity.

## Current Product State

Treat the original Phase 0-5 MVP as product-complete. Current work should be
post-MVP hardening, reliability, simplification, or carefully scoped feature
expansion.

Current implemented capabilities include:

- Tauri v2 desktop app with `control` and `workbench` windows.
- Local `.remember.json` flow persistence under the app data `flows` directory.
- Save, save-as, list, load, and empty first-run flow behavior.
- Recording lifecycle owned by Rust commands.
- Mouse click, double-click, drag, and scroll recording through Windows hooks.
- Keyboard text, modifier hotkeys, and plain control-key recording.
- Wait timing derived from captured event timestamps.
- Basic target-window metadata from recorded input events.
- Filtering of mouse events inside Remember windows when recording stops.
- Playback of wait, click, drag, text, key, hotkey, and scroll steps.
- Playback speed and loop count, including confirmed infinite loop mode.
- Target-window focus attempt and safety gate using recorded and active
  process/title metadata.
- Stop and emergency-stop commands.
- Global emergency hotkey target: `Ctrl + Alt + S`.
- UI-visible emergency hotkey registration status.
- Workbench editing for delays, click coordinates, text, hotkeys, ordinary keys,
  deleting steps, and inserting wait/text/hotkey/key steps.
- Visible run-log surface and target safety-stop messages.
- Atomic local flow saves keyed by selected `.remember.json` file name.
- Flow listing that keeps invalid `.remember.json` files visible.
- Validation for empty key/hotkey steps and high-risk global hotkeys.
- Validation that rejects obviously sensitive typed text before it can be saved
  to a flow file.
- Recording-stop filtering for Remember-window keyboard input, obvious
  sensitive-window keyboard input, and high-risk global hotkeys.
- Tauri-only runtime behavior; initial loading no longer fabricates or writes a
  duplicate sample flow.
- Release signature verification script for checking Authenticode status before
  publishing.
- NSIS packaging and app icon assets.
- Default dev server port: `1450`.

Known current limitations:

- Native password-field detection is not implemented; only obvious sensitive
  window titles/processes are filtered.
- IME-aware text capture is not implemented.
- Optional full mouse-trajectory recording is not implemented.
- Advanced target-window matching controls are not implemented.
- Run history is currently a compact UI log, not a full persisted audit trail.

Do not add these before there is a specific request and a clear verification
path:

- OCR.
- Image recognition.
- AI planning.
- Cloud sync.
- Remote execution.
- Cross-platform support.
- Complex condition branches.
- Script language.
- Plugin system.

## Development Discipline

Before changing code:

1. Inspect the current files and project state.
2. Read this `AGENTS.md`.
3. Read `PROJECT_GOALS.md`, `ROADMAP.md`, and `README.md` when product scope or
   current status matters.
4. Identify the smallest useful change.
5. State assumptions when the request has multiple reasonable interpretations.

While coding:

- Prefer simple, focused modules.
- Match existing project style.
- Keep changes surgical. Every changed line should trace to the user request.
- Do not rewrite broad areas for aesthetics.
- Do not add abstractions for one use case.
- Do not add configurability or future-proofing that was not requested.
- Keep Rust backend boundaries clear: storage, recorder, player, hotkeys,
  windows, playback input, and Tauri commands should not become tangled.
- Keep React state predictable and testable.
- Use type-safe frontend/backend interfaces where practical.
- Keep emergency stop and visible stop controls first-class.

After coding:

1. Re-read the changed code.
2. Run the smallest meaningful verification command.
3. For UI behavior, launch the app and visually verify the relevant window when
   feasible.
4. Summarize what changed, what was verified, and what remains.

## Simplicity And Redundancy Review Rules

When asked to review code structure, overengineering, unnecessary code, or
redundancy:

1. Do not implement fixes silently.
2. Find concrete examples with file and line references.
3. Separate true waste from intentional product scaffolding.
4. Prefer smaller local simplifications over broad rewrites.
5. Call out code that duplicates behavior, preserves obsolete placeholder paths,
   implements one-use abstractions, or carries UI for unavailable features.
6. Do not flag code as unnecessary merely because it is verbose around Windows
   FFI, safety gates, explicit serialization, or user-visible state.

## Review Workflow

When the user says "审查", "审阅", "检查代码", "review", or asks whether prior
findings are real:

1. Use a review-first workflow.
2. Do not silently implement fixes.
3. Re-inspect the current code; do not assume previous audit findings are true.
4. Confirm each finding with direct code/config/test evidence.
5. Order findings by severity.
6. Include file and line references.
7. Mention missing tests or unverified behavior.
8. Mark suspected but unproven issues as "needs manual verification", not as
   confirmed defects.

## Shortcut Words For Future Work

When the user says "推进", "继续", "继续开发", or "完善":

1. Recover context by reading this file and existing local docs.
2. Inspect repository state.
3. Choose the smallest post-MVP hardening task unless the user specified a
   feature.
4. Implement only when the request is not review-only.
5. Verify the change.
6. Report the result and the next recommended step.

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
3. Run `npm run verify:release-signature`.
4. Report the artifact path, signature status, and any installer/runtime
   requirements.

## Verification Expectations

Use the smallest checks that prove the current change.

Common checks:

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
cargo check --manifest-path src-tauri\Cargo.toml
cargo test --manifest-path src-tauri\Cargo.toml
```

For recording/playback, global hotkeys, emergency stop, window matching, or
packaging claims, do not rely only on unit tests. Use a safe desktop smoke path
before claiming the behavior works end to end.

## Safety Rules

- Emergency stop must remain available before serious playback testing.
- Infinite loop playback must require explicit confirmation.
- Prefer foreground/active-window operation.
- Do not default to hidden background automation.
- Warn about sensitive input recording.
- Do not intentionally record passwords, verification codes, payment details,
  or private messages.
- If target-window matching fails, stop rather than clicking or typing blindly.
- Treat global hooks, injected input, unsigned installers, and app-data flow
  files as safety-sensitive surfaces.

## Suggested Structure

This is guidance, not a command to create files prematurely.

```text
src/
  components/
    ControlWindow.tsx
    WorkbenchWindow.tsx
  data/
  flowEditing.ts
  runLog.ts
  tauriApi.ts
  types.ts

src-tauri/
  src/
    hotkeys.rs
    keyboard.rs
    lib.rs
    mouse.rs
    playback_input.rs
    player.rs
    recorder.rs
    storage.rs
    windows.rs
```

Split modules only when there is a real reason. Keep the codebase compact.
