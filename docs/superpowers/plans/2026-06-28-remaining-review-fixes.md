# Remaining Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the remaining non-signing, non-logo review findings with regression tests and full verification.

**Architecture:** Keep the app compact. Add small frontend state helpers instead of a new store, extend the existing JSON flow model for scroll coordinates, and harden existing storage/script boundaries without broad rewrites.

**Tech Stack:** Tauri v2, Rust, React, TypeScript, Node test runner, PowerShell release scripts.

---

### Task 1: Frontend State Gates And Cross-Window Draft Sync

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/ControlWindow.tsx`
- Modify: `src/components/WorkbenchWindow.tsx`
- Modify: `src/runLog.ts`
- Test: `src/runLog.test.ts`

- [x] **Step 1: Write failing run-log tests**

Add tests proving safety-stop lookup is tied to the current run and can be ignored after target reconfirmation:

```ts
test("finds safety stop only for the requested run id", () => {
  const safetyStop = {
    id: "playback-finished-7",
    time: Date.now(),
    level: "danger",
    title: "安全停止",
    detail: "target mismatch",
    flowName: "Daily Report",
    runId: 7,
    reason: "safetyStopped" as const,
  };
  assert.equal(findLatestSafetyStopLog([safetyStop], "Daily Report", 8), undefined);
  assert.equal(findLatestSafetyStopLog([safetyStop], "Daily Report", 7), safetyStop);
});
```

- [x] **Step 2: Verify red**

Run: `npm test`

Expected: fails because `findLatestSafetyStopLog` only accepts two arguments and does not filter by run id.

- [x] **Step 3: Implement minimal frontend fixes**

In `App.tsx`, add `isFlowLoading`, `lastSafetyStopRunId`, a current-flow draft event, and use them to disable replay/save/load while loading. Broadcast local draft edits with `emit("flow-draft-updated", savedFlow)` from edit handlers; listen for that event and apply the draft in the other window. Use command returns as the source of truth for save/load and keep backend events for cross-window updates only by making duplicate updates idempotent.

- [x] **Step 4: Verify green**

Run: `npm test`

Expected: all frontend tests pass.

### Task 2: Scroll Coordinates In Flow Model And Playback

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/recorder.rs`
- Modify: `src-tauri/src/player.rs`
- Modify: `src-tauri/src/playback_input.rs`
- Modify: `src-tauri/tests/recorder.rs`
- Modify: `src-tauri/tests/player.rs`
- Modify: `src/types.ts`
- Modify: `src/components/WorkbenchWindow.tsx`

- [x] **Step 1: Write failing Rust tests**

Add recorder/player tests proving scroll steps retain `x/y` and playback sends scroll at that point:

```rust
assert!(matches!(
    &stopped.flow.steps[0],
    FlowStep::Scroll { x: 220, y: 340, delta_y: -120, .. }
));
```

- [x] **Step 2: Verify red**

Run: `cargo test --manifest-path src-tauri\Cargo.toml --test recorder -- --nocapture`

Expected: fails because `FlowStep::Scroll` has no `x/y`.

- [x] **Step 3: Implement model extension**

Add `x` and `y` fields to Rust and TypeScript `Scroll` steps. Recorder writes captured scroll coordinates. Playback input gains `scroll_at(x, y, delta_x, delta_y)` that moves the cursor and checks `SetCursorPos` before sending wheel input.

- [x] **Step 4: Verify green**

Run: `cargo test --manifest-path src-tauri\Cargo.toml --test recorder -- --nocapture`

Expected: recorder tests pass.

### Task 3: Storage Metadata And Time Bounds

**Files:**
- Modify: `src-tauri/src/storage.rs`
- Modify: `src-tauri/tests/storage.rs`

- [x] **Step 1: Write failing storage tests**

Add tests rejecting sensitive metadata and excessive waits/drags:

```rust
let mut flow = sample_flow();
flow.display_name = "password reset".to_string();
let error = save_flow_to_dir(&root, &flow).expect_err("sensitive metadata should reject");
assert!(matches!(error, StorageError::InvalidFlow(message) if message.contains("displayName")));
```

- [x] **Step 2: Verify red**

Run: `cargo test --manifest-path src-tauri\Cargo.toml --test storage -- --nocapture`

Expected: fails because only typed text is checked and time fields have no upper bound.

- [x] **Step 3: Implement minimal validation**

Reuse the existing sensitive-text helper for display name, target window title, notes, and targets. Add product constants for maximum single-step delay and drag duration, and validate all relevant `delayMs`, `durationMs`, and drag duration fields.

- [x] **Step 4: Verify green**

Run: `cargo test --manifest-path src-tauri\Cargo.toml --test storage -- --nocapture`

Expected: storage tests pass.

### Task 4: Release Script Version Path

**Files:**
- Modify: `scripts/sign-release-artifacts.ps1`
- Modify: `scripts/verify-release-signature.ps1`

- [x] **Step 1: Replace hardcoded installer path**

Read `src-tauri/tauri.conf.json`, derive `productName` and `version`, and build `src-tauri\target\release\bundle\nsis\<Product>_<Version>_x64-setup.exe`.

- [x] **Step 2: Verify syntax without requiring signing**

Run: `pwsh -NoProfile -ExecutionPolicy Bypass -Command "$null = [scriptblock]::Create((Get-Content -Raw scripts/verify-release-signature.ps1)); $null = [scriptblock]::Create((Get-Content -Raw scripts/sign-release-artifacts.ps1))"`

Expected: command exits 0.

### Task 5: Full Verification

**Files:**
- No code changes.

- [x] **Step 1: Run frontend checks**

Run: `npm test`

Expected: 0 failures.

- [x] **Step 2: Run frontend build**

Run: `npm run build`

Expected: build exits 0.

- [x] **Step 3: Run Rust checks**

Run: `cargo fmt --manifest-path src-tauri\Cargo.toml --check`

Expected: exits 0.

Run: `cargo check --manifest-path src-tauri\Cargo.toml`

Expected: exits 0.

Run: `cargo test --manifest-path src-tauri\Cargo.toml`

Expected: 0 failures.

- [x] **Step 4: Run diff hygiene**

Run: `git diff --check`

Expected: no whitespace errors.
