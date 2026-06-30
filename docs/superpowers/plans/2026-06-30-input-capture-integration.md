# Input Capture Integration Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Connect real Windows mouse and keyboard capture to the existing `Recorder` so manual recording produces replayable `.remember.json` steps.

**Architecture:** Keep `AppController` as the state boundary by adding a small capture method that ignores events unless the app is recording. Add a Windows-only hook runtime in `input.rs` that converts low-level hook messages into `RawInputEvent` values and forwards them to shared app state. Start the hook runtime once during Tauri setup.

**Tech Stack:** Rust, Tauri 2, Windows low-level keyboard/mouse hooks via the existing `windows` crate.

---

### Task 10: Wire Windows Input Capture

**Files:**
- Modify: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/tests/app_state.rs`
- Modify: `src-tauri/src/input.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/Cargo.toml` if additional Windows feature gates are required.

- [ ] **Step 1: Add failing state test**

Add a test proving `AppController` can accept captured input while recording and ignores it while idle.

- [ ] **Step 2: Implement controller capture boundary**

Add `AppController::capture_input(event: RawInputEvent)` that forwards to `Recorder::capture` only in recording mode.

- [ ] **Step 3: Add Windows hook runtime**

Add a small `InputCaptureRuntime` in `input.rs` that installs `WH_MOUSE_LL` and `WH_KEYBOARD_LL`, converts mouse/key messages into `RawInputEvent`, and forwards them to `Arc<Mutex<AppController>>`.

- [ ] **Step 4: Start capture runtime in Tauri setup**

Start and manage the capture runtime from `lib.rs` during setup.

- [ ] **Step 5: Verify and commit**

Run Rust and frontend checks, then commit as `feat: capture Windows input`.
