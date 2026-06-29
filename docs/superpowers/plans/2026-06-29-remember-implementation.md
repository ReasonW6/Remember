# Remember Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `Remember`, a Windows portable Tauri desktop macro recorder that records real mouse/keyboard input, saves `.remember.json` recordings, and replays them with loop, speed, tray, and global hotkey controls.

**Architecture:** Rust is the source of truth for recording, playback, validation, storage, hotkeys, tray, and Windows input. React renders a compact utility control panel and calls Tauri commands; it does not own recorder/player state. The Rust core is split into pure, unit-tested modules first, then wired to Windows APIs and Tauri runtime.

**Tech Stack:** Tauri 2, Rust, `windows` crate, React, TypeScript, Vite, Vitest, React Testing Library, `@tauri-apps/api`, `tauri-plugin-dialog`, `tauri-plugin-global-shortcut`.

---

## File Structure

Create these files:

- `.gitignore`: exclude Node, Rust, Tauri, coverage, and build output.
- `README.md`: usage, development, build, and first-release limitations.
- `package.json`: frontend scripts and Tauri scripts.
- `index.html`: Vite entry.
- `tsconfig.json`: TypeScript config.
- `vite.config.ts`: Vite React config.
- `vitest.config.ts`: Vitest jsdom config.
- `src/main.tsx`: React bootstrap.
- `src/App.tsx`: top-level UI shell.
- `src/App.test.tsx`: app-level rendering and state tests.
- `src/components/Controls.tsx`: Record, Play, Stop, Save, Open buttons.
- `src/components/PlaybackSettings.tsx`: loop and speed inputs.
- `src/components/StatusPanel.tsx`: status, step count, duration, and messages.
- `src/components/HotkeyPanel.tsx`: default hotkey display.
- `src/lib/rememberApi.ts`: typed Tauri command wrapper.
- `src/types.ts`: frontend state and recording summary types.
- `src/test/setup.ts`: React Testing Library setup.
- `src-tauri/Cargo.toml`: Rust dependencies.
- `src-tauri/build.rs`: Tauri build hook.
- `src-tauri/tauri.conf.json`: Tauri app config for `Remember`.
- `src-tauri/src/main.rs`: executable entrypoint.
- `src-tauri/src/lib.rs`: module exports and Tauri app builder.
- `src-tauri/src/model.rs`: recording file data model and validation.
- `src-tauri/src/storage.rs`: JSON string/path load and save.
- `src-tauri/src/player.rs`: playback settings, timing, stop token, and executor contract.
- `src-tauri/src/recorder.rs`: recorder state machine and movement sampling.
- `src-tauri/src/app_state.rs`: shared app controller state.
- `src-tauri/src/commands.rs`: Tauri command wrappers.
- `src-tauri/src/input.rs`: Windows input capture/playback implementation and non-Windows errors.
- `src-tauri/src/hotkeys.rs`: global shortcut registration.
- `src-tauri/src/tray.rs`: system tray setup.
- `src-tauri/tests/model_storage.rs`: model and storage tests.
- `src-tauri/tests/player.rs`: playback validation and timing tests.
- `src-tauri/tests/recorder.rs`: recorder state and movement sampling tests.
- `src-tauri/tests/app_state.rs`: app controller tests.

## Dependency Baseline

Use these dependency sets unless a current Tauri template requires a version adjustment during install:

```json
{
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "@tauri-apps/plugin-dialog": "^2.0.0",
    "lucide-react": "^0.468.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@testing-library/jest-dom": "^6.4.8",
    "@testing-library/react": "^16.0.1",
    "@testing-library/user-event": "^14.5.2",
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.4",
    "jsdom": "^25.0.1",
    "vite": "^5.4.0",
    "vitest": "^2.0.5"
  }
}
```

```toml
[dependencies]
chrono = { version = "0.4", features = ["serde", "clock"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-dialog = "2"
tauri-plugin-global-shortcut = "2"
thiserror = "1"
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_WindowsAndMessaging"
] }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

Commands that fetch packages need network permission:

```powershell
npm install
cargo fetch --manifest-path src-tauri\Cargo.toml
```

---

### Task 1: Project Scaffold And Smoke Tests

**Files:**
- Create: `.gitignore`
- Create: `package.json`
- Create: `index.html`
- Create: `tsconfig.json`
- Create: `vite.config.ts`
- Create: `vitest.config.ts`
- Create: `src/test/setup.ts`
- Create: `src/main.tsx`
- Create: `src/App.test.tsx`
- Create: `src/App.tsx`
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add scaffold config files**

Create `.gitignore`:

```gitignore
node_modules/
dist/
coverage/
src-tauri/target/
*.log
*.tmp
```

Create `package.json`:

```json
{
  "name": "remember",
  "version": "0.1.0",
  "private": true,
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc && vite build",
    "test": "vitest run",
    "test:watch": "vitest",
    "tauri": "tauri"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "@tauri-apps/plugin-dialog": "^2.0.0",
    "lucide-react": "^0.468.0",
    "react": "^18.3.1",
    "react-dom": "^18.3.1"
  },
  "devDependencies": {
    "@tauri-apps/cli": "^2.0.0",
    "@testing-library/jest-dom": "^6.4.8",
    "@testing-library/react": "^16.0.1",
    "@testing-library/user-event": "^14.5.2",
    "@types/react": "^18.3.3",
    "@types/react-dom": "^18.3.0",
    "@vitejs/plugin-react": "^4.3.1",
    "typescript": "^5.5.4",
    "jsdom": "^25.0.1",
    "vite": "^5.4.0",
    "vitest": "^2.0.5"
  }
}
```

Create `index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Remember</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "useDefineForClassFields": true,
    "lib": ["DOM", "DOM.Iterable", "ES2020"],
    "allowJs": false,
    "skipLibCheck": true,
    "esModuleInterop": true,
    "allowSyntheticDefaultImports": true,
    "strict": true,
    "forceConsistentCasingInFileNames": true,
    "module": "ESNext",
    "moduleResolution": "Node",
    "resolveJsonModule": true,
    "isolatedModules": true,
    "noEmit": true,
    "jsx": "react-jsx"
  },
  "include": ["src"],
  "references": []
}
```

Create `vite.config.ts`:

```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true
  }
});
```

Create `vitest.config.ts`:

```ts
import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [react()],
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    globals: true
  }
});
```

Create `src/test/setup.ts`:

```ts
import "@testing-library/jest-dom/vitest";
```

- [ ] **Step 2: Add the failing React smoke test**

Create `src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { App } from "./App";

describe("App", () => {
  it("renders the Remember control panel title", () => {
    render(<App />);

    expect(screen.getByRole("heading", { name: "Remember" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /record/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 3: Run the React smoke test to verify it fails**

Run:

```powershell
npm install
npm test -- src/App.test.tsx
```

Expected: `npm install` succeeds, then Vitest fails because `src/App.tsx` does not exist.

- [ ] **Step 4: Add minimal React production code**

Create `src/App.tsx`:

```tsx
export function App() {
  return (
    <main className="app-shell">
      <header>
        <h1>Remember</h1>
        <p>Idle</p>
      </header>
      <button type="button">Record</button>
    </main>
  );
}
```

Create `src/main.tsx`:

```tsx
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

- [ ] **Step 5: Add the failing Rust smoke test**

Create `src-tauri/Cargo.toml`:

```toml
[package]
name = "remember"
version = "0.1.0"
description = "Portable Windows macro recorder"
edition = "2021"

[lib]
name = "remember"
crate-type = ["staticlib", "cdylib", "rlib"]

[[bin]]
name = "remember"
path = "src/main.rs"

[dependencies]
chrono = { version = "0.4", features = ["serde", "clock"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri = { version = "2", features = ["tray-icon"] }
tauri-plugin-dialog = "2"
tauri-plugin-global-shortcut = "2"
thiserror = "1"
windows = { version = "0.58", features = [
  "Win32_Foundation",
  "Win32_UI_Input_KeyboardAndMouse",
  "Win32_UI_WindowsAndMessaging"
] }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

Create `src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

Create `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Remember",
  "version": "0.1.0",
  "identifier": "com.remember.desktop",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Remember",
        "width": 420,
        "height": 520,
        "resizable": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": false,
    "targets": "all"
  }
}
```

Create `src-tauri/tests/smoke.rs`:

```rust
#[test]
fn exposes_product_name() {
    assert_eq!(remember::product_name(), "Remember");
}
```

- [ ] **Step 6: Run the Rust smoke test to verify it fails**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test smoke
```

Expected: compile fails because `src-tauri/src/lib.rs` does not exist or `product_name` is not defined.

- [ ] **Step 7: Add minimal Rust production code**

Create `src-tauri/src/lib.rs`:

```rust
pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
```

Create `src-tauri/src/main.rs`:

```rust
fn main() {
    remember::run();
}
```

- [ ] **Step 8: Verify scaffold tests pass**

Run:

```powershell
npm test -- src/App.test.tsx
cargo test --manifest-path src-tauri\Cargo.toml --test smoke
```

Expected: both commands pass.

- [ ] **Step 9: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add .
git -c safe.directory=D:/Code/Codex/remember commit -m "chore: scaffold Remember app"
```

---

### Task 2: Recording Model And JSON Storage

**Files:**
- Create: `src-tauri/tests/model_storage.rs`
- Create: `src-tauri/src/model.rs`
- Create: `src-tauri/src/storage.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing model and storage tests**

Create `src-tauri/tests/model_storage.rs`:

```rust
use remember::model::{KeyState, MacroStep, Recording};
use remember::storage::{recording_from_json, recording_to_json};

fn sample_recording() -> Recording {
    Recording {
        version: 1,
        name: "notepad smoke".to_string(),
        created_at: "2026-06-29T00:00:00Z".to_string(),
        duration_ms: 120,
        steps: vec![
            MacroStep::Key {
                elapsed_ms: 0,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 120,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Released,
            },
        ],
    }
}

#[test]
fn serializes_recording_with_stable_version() {
    let json = recording_to_json(&sample_recording()).expect("serialize");

    assert!(json.contains("\"version\": 1"));
    assert!(json.contains("\"kind\": \"key\""));
}

#[test]
fn deserializes_round_trip_recording() {
    let original = sample_recording();
    let json = recording_to_json(&original).expect("serialize");
    let loaded = recording_from_json(&json).expect("deserialize");

    assert_eq!(loaded, original);
}

#[test]
fn rejects_unsupported_version() {
    let json = r#"{
      "version": 99,
      "name": "bad",
      "created_at": "2026-06-29T00:00:00Z",
      "duration_ms": 0,
      "steps": []
    }"#;

    let error = recording_from_json(json).expect_err("unsupported version must fail");

    assert!(error.to_string().contains("unsupported recording version"));
}

#[test]
fn rejects_missing_required_fields() {
    let error = recording_from_json(r#"{"version":1}"#).expect_err("missing fields must fail");

    assert!(error.to_string().contains("invalid recording json"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test model_storage
```

Expected: compile fails because `model` and `storage` modules do not exist.

- [ ] **Step 3: Implement the model**

Create `src-tauri/src/model.rs`:

```rust
use serde::{Deserialize, Serialize};

pub const RECORDING_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    X1,
    X2,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ButtonState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    Pressed,
    Released,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MacroStep {
    MouseMove {
        elapsed_ms: u64,
        x: i32,
        y: i32,
    },
    MouseButton {
        elapsed_ms: u64,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    },
    MouseWheel {
        elapsed_ms: u64,
        x: i32,
        y: i32,
        delta: i32,
    },
    Key {
        elapsed_ms: u64,
        vk_code: u16,
        scan_code: u16,
        state: KeyState,
    },
    Wait {
        elapsed_ms: u64,
    },
}

impl MacroStep {
    pub fn elapsed_ms(&self) -> u64 {
        match self {
            MacroStep::MouseMove { elapsed_ms, .. }
            | MacroStep::MouseButton { elapsed_ms, .. }
            | MacroStep::MouseWheel { elapsed_ms, .. }
            | MacroStep::Key { elapsed_ms, .. }
            | MacroStep::Wait { elapsed_ms } => *elapsed_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Recording {
    pub version: u32,
    pub name: String,
    pub created_at: String,
    pub duration_ms: u64,
    pub steps: Vec<MacroStep>,
}

impl Recording {
    pub fn new(name: impl Into<String>, created_at: impl Into<String>, steps: Vec<MacroStep>) -> Self {
        let duration_ms = steps.last().map(MacroStep::elapsed_ms).unwrap_or(0);
        Self {
            version: RECORDING_VERSION,
            name: name.into(),
            created_at: created_at.into(),
            duration_ms,
            steps,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != RECORDING_VERSION {
            return Err(format!("unsupported recording version {}", self.version));
        }
        if self.name.trim().is_empty() {
            return Err("recording name cannot be empty".to_string());
        }
        if self.created_at.trim().is_empty() {
            return Err("created_at cannot be empty".to_string());
        }
        if self.duration_ms < self.steps.last().map(MacroStep::elapsed_ms).unwrap_or(0) {
            return Err("duration_ms cannot be shorter than the final step".to_string());
        }
        Ok(())
    }
}
```

- [ ] **Step 4: Implement JSON storage**

Create `src-tauri/src/storage.rs`:

```rust
use crate::model::Recording;
use std::{fs, path::Path};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("invalid recording json: {0}")]
    InvalidJson(#[from] serde_json::Error),
    #[error("{0}")]
    InvalidRecording(String),
    #[error("file error: {0}")]
    File(#[from] std::io::Error),
}

pub fn recording_to_json(recording: &Recording) -> Result<String, StorageError> {
    recording.validate().map_err(StorageError::InvalidRecording)?;
    serde_json::to_string_pretty(recording).map_err(StorageError::InvalidJson)
}

pub fn recording_from_json(json: &str) -> Result<Recording, StorageError> {
    let recording: Recording = serde_json::from_str(json).map_err(StorageError::InvalidJson)?;
    recording.validate().map_err(StorageError::InvalidRecording)?;
    Ok(recording)
}

pub fn save_recording(path: &Path, recording: &Recording) -> Result<(), StorageError> {
    let json = recording_to_json(recording)?;
    fs::write(path, json)?;
    Ok(())
}

pub fn load_recording(path: &Path) -> Result<Recording, StorageError> {
    let json = fs::read_to_string(path)?;
    recording_from_json(&json)
}
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod model;
pub mod storage;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
```

- [ ] **Step 5: Verify model and storage tests pass**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test model_storage
```

Expected: all four tests pass.

- [ ] **Step 6: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add src-tauri/src/model.rs src-tauri/src/storage.rs src-tauri/src/lib.rs src-tauri/tests/model_storage.rs
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: add recording model and storage"
```

---

### Task 3: Playback Validation And Timing Planner

**Files:**
- Create: `src-tauri/tests/player.rs`
- Create: `src-tauri/src/player.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing playback tests**

Create `src-tauri/tests/player.rs`:

```rust
use remember::model::{KeyState, MacroStep, Recording};
use remember::player::{build_playback_plan, scaled_delay_ms, PlaybackSettings, StopToken};

fn recording() -> Recording {
    Recording::new(
        "keys",
        "2026-06-29T00:00:00Z",
        vec![
            MacroStep::Key {
                elapsed_ms: 100,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Pressed,
            },
            MacroStep::Key {
                elapsed_ms: 250,
                vk_code: 0x41,
                scan_code: 0x1E,
                state: KeyState::Released,
            },
        ],
    )
}

#[test]
fn validates_loop_count_and_speed() {
    assert!(PlaybackSettings::new(1, 1.0).is_ok());
    assert!(PlaybackSettings::new(0, 1.0).is_err());
    assert!(PlaybackSettings::new(1, 0.0).is_err());
}

#[test]
fn scales_delay_by_speed_multiplier() {
    assert_eq!(scaled_delay_ms(200, 1.0), 200);
    assert_eq!(scaled_delay_ms(200, 2.0), 100);
    assert_eq!(scaled_delay_ms(200, 0.5), 400);
}

#[test]
fn builds_looped_playback_plan_with_step_deltas() {
    let settings = PlaybackSettings::new(2, 2.0).expect("settings");
    let plan = build_playback_plan(&recording(), settings);

    assert_eq!(plan.len(), 4);
    assert_eq!(plan[0].loop_index, 0);
    assert_eq!(plan[0].step_index, 0);
    assert_eq!(plan[0].delay_ms, 50);
    assert_eq!(plan[1].delay_ms, 75);
    assert_eq!(plan[2].loop_index, 1);
}

#[test]
fn stop_token_defaults_to_not_stopped() {
    let token = StopToken::default();
    assert!(!token.is_stopped());
    token.request_stop();
    assert!(token.is_stopped());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test player
```

Expected: compile fails because `player` module does not exist.

- [ ] **Step 3: Implement playback planner**

Create `src-tauri/src/player.rs`:

```rust
use crate::model::{MacroStep, Recording};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaybackSettings {
    pub loop_count: u32,
    pub speed_multiplier: f64,
}

impl PlaybackSettings {
    pub fn new(loop_count: u32, speed_multiplier: f64) -> Result<Self, String> {
        if loop_count == 0 {
            return Err("loop count must be at least 1".to_string());
        }
        if !speed_multiplier.is_finite() || speed_multiplier <= 0.0 {
            return Err("speed multiplier must be positive".to_string());
        }
        Ok(Self {
            loop_count,
            speed_multiplier,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaybackAction {
    pub loop_index: u32,
    pub step_index: usize,
    pub delay_ms: u64,
    pub step: MacroStep,
}

#[derive(Clone, Default)]
pub struct StopToken {
    stopped: Arc<AtomicBool>,
}

impl StopToken {
    pub fn request_stop(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::SeqCst)
    }
}

pub fn scaled_delay_ms(delay_ms: u64, speed_multiplier: f64) -> u64 {
    ((delay_ms as f64) / speed_multiplier).round().max(0.0) as u64
}

pub fn build_playback_plan(recording: &Recording, settings: PlaybackSettings) -> Vec<PlaybackAction> {
    let mut actions = Vec::new();
    for loop_index in 0..settings.loop_count {
        let mut previous_elapsed = 0;
        for (step_index, step) in recording.steps.iter().cloned().enumerate() {
            let elapsed = step.elapsed_ms();
            let raw_delay = elapsed.saturating_sub(previous_elapsed);
            previous_elapsed = elapsed;
            actions.push(PlaybackAction {
                loop_index,
                step_index,
                delay_ms: scaled_delay_ms(raw_delay, settings.speed_multiplier),
                step,
            });
        }
    }
    actions
}
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod model;
pub mod player;
pub mod storage;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
```

- [ ] **Step 4: Verify playback tests pass**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test player
```

Expected: all four tests pass.

- [ ] **Step 5: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add src-tauri/src/player.rs src-tauri/src/lib.rs src-tauri/tests/player.rs
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: add playback timing planner"
```

---

### Task 4: Recorder State And Movement Sampling

**Files:**
- Create: `src-tauri/tests/recorder.rs`
- Create: `src-tauri/src/recorder.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing recorder tests**

Create `src-tauri/tests/recorder.rs`:

```rust
use remember::model::{ButtonState, KeyState, MacroStep, MouseButton};
use remember::recorder::{RawInputEvent, Recorder};

#[test]
fn records_key_press_and_release() {
    let mut recorder = Recorder::new(50);
    recorder.start("keys", 1_000, "2026-06-29T00:00:00Z").expect("start");

    recorder.capture(RawInputEvent::Key {
        at_ms: 1_010,
        vk_code: 0x41,
        scan_code: 0x1E,
        state: KeyState::Pressed,
    });
    recorder.capture(RawInputEvent::Key {
        at_ms: 1_040,
        vk_code: 0x41,
        scan_code: 0x1E,
        state: KeyState::Released,
    });

    let recording = recorder.stop(1_050).expect("stop");

    assert_eq!(recording.steps.len(), 2);
    assert_eq!(recording.duration_ms, 50);
    assert!(matches!(recording.steps[0], MacroStep::Key { elapsed_ms: 10, state: KeyState::Pressed, .. }));
    assert!(matches!(recording.steps[1], MacroStep::Key { elapsed_ms: 40, state: KeyState::Released, .. }));
}

#[test]
fn samples_mouse_moves_at_configured_interval() {
    let mut recorder = Recorder::new(50);
    recorder.start("mouse", 1_000, "2026-06-29T00:00:00Z").expect("start");

    recorder.capture(RawInputEvent::MouseMove { at_ms: 1_010, x: 10, y: 10 });
    recorder.capture(RawInputEvent::MouseMove { at_ms: 1_020, x: 20, y: 20 });
    recorder.capture(RawInputEvent::MouseMove { at_ms: 1_061, x: 30, y: 30 });

    let recording = recorder.stop(1_070).expect("stop");

    assert_eq!(recording.steps.len(), 2);
    assert!(matches!(recording.steps[0], MacroStep::MouseMove { elapsed_ms: 10, x: 10, y: 10 }));
    assert!(matches!(recording.steps[1], MacroStep::MouseMove { elapsed_ms: 61, x: 30, y: 30 }));
}

#[test]
fn preserves_click_position_even_after_recent_move() {
    let mut recorder = Recorder::new(50);
    recorder.start("click", 1_000, "2026-06-29T00:00:00Z").expect("start");

    recorder.capture(RawInputEvent::MouseMove { at_ms: 1_010, x: 10, y: 10 });
    recorder.capture(RawInputEvent::MouseButton {
        at_ms: 1_020,
        x: 20,
        y: 20,
        button: MouseButton::Left,
        state: ButtonState::Pressed,
    });

    let recording = recorder.stop(1_030).expect("stop");

    assert!(matches!(
        recording.steps.last(),
        Some(MacroStep::MouseButton { elapsed_ms: 20, x: 20, y: 20, button: MouseButton::Left, state: ButtonState::Pressed })
    ));
}

#[test]
fn cannot_start_twice_without_stopping() {
    let mut recorder = Recorder::new(50);
    recorder.start("first", 1_000, "2026-06-29T00:00:00Z").expect("start");

    let error = recorder.start("second", 1_001, "2026-06-29T00:00:00Z").expect_err("second start fails");

    assert!(error.contains("already recording"));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test recorder
```

Expected: compile fails because `recorder` module does not exist.

- [ ] **Step 3: Implement recorder state machine**

Create `src-tauri/src/recorder.rs`:

```rust
use crate::model::{ButtonState, KeyState, MacroStep, MouseButton, Recording};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawInputEvent {
    MouseMove {
        at_ms: u64,
        x: i32,
        y: i32,
    },
    MouseButton {
        at_ms: u64,
        x: i32,
        y: i32,
        button: MouseButton,
        state: ButtonState,
    },
    MouseWheel {
        at_ms: u64,
        x: i32,
        y: i32,
        delta: i32,
    },
    Key {
        at_ms: u64,
        vk_code: u16,
        scan_code: u16,
        state: KeyState,
    },
}

#[derive(Debug, Default)]
pub struct Recorder {
    move_sample_ms: u64,
    active: Option<ActiveRecording>,
}

#[derive(Debug)]
struct ActiveRecording {
    name: String,
    created_at: String,
    started_at_ms: u64,
    last_kept_move_elapsed_ms: Option<u64>,
    steps: Vec<MacroStep>,
}

impl Recorder {
    pub fn new(move_sample_ms: u64) -> Self {
        Self {
            move_sample_ms,
            active: None,
        }
    }

    pub fn start(
        &mut self,
        name: impl Into<String>,
        started_at_ms: u64,
        created_at: impl Into<String>,
    ) -> Result<(), String> {
        if self.active.is_some() {
            return Err("already recording".to_string());
        }
        self.active = Some(ActiveRecording {
            name: name.into(),
            created_at: created_at.into(),
            started_at_ms,
            last_kept_move_elapsed_ms: None,
            steps: Vec::new(),
        });
        Ok(())
    }

    pub fn capture(&mut self, event: RawInputEvent) {
        let Some(active) = self.active.as_mut() else {
            return;
        };
        let step = match event {
            RawInputEvent::MouseMove { at_ms, x, y } => {
                let elapsed_ms = at_ms.saturating_sub(active.started_at_ms);
                if let Some(last) = active.last_kept_move_elapsed_ms {
                    if elapsed_ms.saturating_sub(last) < self.move_sample_ms {
                        return;
                    }
                }
                active.last_kept_move_elapsed_ms = Some(elapsed_ms);
                MacroStep::MouseMove { elapsed_ms, x, y }
            }
            RawInputEvent::MouseButton {
                at_ms,
                x,
                y,
                button,
                state,
            } => MacroStep::MouseButton {
                elapsed_ms: at_ms.saturating_sub(active.started_at_ms),
                x,
                y,
                button,
                state,
            },
            RawInputEvent::MouseWheel { at_ms, x, y, delta } => MacroStep::MouseWheel {
                elapsed_ms: at_ms.saturating_sub(active.started_at_ms),
                x,
                y,
                delta,
            },
            RawInputEvent::Key {
                at_ms,
                vk_code,
                scan_code,
                state,
            } => MacroStep::Key {
                elapsed_ms: at_ms.saturating_sub(active.started_at_ms),
                vk_code,
                scan_code,
                state,
            },
        };
        active.steps.push(step);
    }

    pub fn stop(&mut self, stopped_at_ms: u64) -> Result<Recording, String> {
        let active = self
            .active
            .take()
            .ok_or_else(|| "not recording".to_string())?;
        let mut recording = Recording::new(active.name, active.created_at, active.steps);
        recording.duration_ms = stopped_at_ms.saturating_sub(active.started_at_ms);
        Ok(recording)
    }

    pub fn is_recording(&self) -> bool {
        self.active.is_some()
    }
}
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod model;
pub mod player;
pub mod recorder;
pub mod storage;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
```

- [ ] **Step 4: Verify recorder tests pass**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test recorder
```

Expected: all four tests pass.

- [ ] **Step 5: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add src-tauri/src/recorder.rs src-tauri/src/lib.rs src-tauri/tests/recorder.rs
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: add recorder state machine"
```

---

### Task 5: App Controller State

**Files:**
- Create: `src-tauri/tests/app_state.rs`
- Create: `src-tauri/src/app_state.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write failing app state tests**

Create `src-tauri/tests/app_state.rs`:

```rust
use remember::app_state::{AppController, AppMode};
use remember::model::{KeyState, MacroStep, Recording};

fn recording() -> Recording {
    Recording::new(
        "loaded",
        "2026-06-29T00:00:00Z",
        vec![MacroStep::Key {
            elapsed_ms: 1,
            vk_code: 0x41,
            scan_code: 0x1E,
            state: KeyState::Pressed,
        }],
    )
}

#[test]
fn starts_and_stops_recording() {
    let mut app = AppController::new();

    app.start_recording("test", 100, "2026-06-29T00:00:00Z").expect("start");
    assert_eq!(app.mode(), AppMode::Recording);

    let saved = app.stop_recording(150).expect("stop");
    assert_eq!(app.mode(), AppMode::Idle);
    assert_eq!(saved.name, "test");
}

#[test]
fn rejects_play_without_recording() {
    let mut app = AppController::new();

    let error = app.start_playback(1, 1.0).expect_err("no recording");

    assert!(error.contains("no recording loaded"));
}

#[test]
fn loads_recording_and_starts_playback() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");

    let plan = app.start_playback(2, 1.0).expect("play");

    assert_eq!(app.mode(), AppMode::Playing);
    assert_eq!(plan.len(), 2);
}

#[test]
fn stop_playback_returns_to_idle() {
    let mut app = AppController::new();
    app.set_recording(recording()).expect("load");
    app.start_playback(1, 1.0).expect("play");

    app.stop_playback();

    assert_eq!(app.mode(), AppMode::Idle);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test app_state
```

Expected: compile fails because `app_state` module does not exist.

- [ ] **Step 3: Implement app controller**

Create `src-tauri/src/app_state.rs`:

```rust
use crate::{
    model::Recording,
    player::{build_playback_plan, PlaybackAction, PlaybackSettings, StopToken},
    recorder::Recorder,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    Idle,
    Recording,
    Playing,
}

#[derive(Debug, Clone, Serialize)]
pub struct UiState {
    pub mode: AppMode,
    pub recording_name: Option<String>,
    pub step_count: usize,
    pub duration_ms: u64,
    pub message: String,
}

pub struct AppController {
    mode: AppMode,
    recorder: Recorder,
    recording: Option<Recording>,
    stop_token: StopToken,
    message: String,
}

impl Default for AppController {
    fn default() -> Self {
        Self::new()
    }
}

impl AppController {
    pub fn new() -> Self {
        Self {
            mode: AppMode::Idle,
            recorder: Recorder::new(50),
            recording: None,
            stop_token: StopToken::default(),
            message: "Idle".to_string(),
        }
    }

    pub fn mode(&self) -> AppMode {
        self.mode
    }

    pub fn ui_state(&self) -> UiState {
        UiState {
            mode: self.mode,
            recording_name: self.recording.as_ref().map(|recording| recording.name.clone()),
            step_count: self.recording.as_ref().map(|recording| recording.steps.len()).unwrap_or(0),
            duration_ms: self.recording.as_ref().map(|recording| recording.duration_ms).unwrap_or(0),
            message: self.message.clone(),
        }
    }

    pub fn start_recording(
        &mut self,
        name: impl Into<String>,
        started_at_ms: u64,
        created_at: impl Into<String>,
    ) -> Result<(), String> {
        if self.mode == AppMode::Playing {
            return Err("cannot record while playing".to_string());
        }
        self.recorder.start(name, started_at_ms, created_at)?;
        self.recording = None;
        self.mode = AppMode::Recording;
        self.message = "Recording".to_string();
        Ok(())
    }

    pub fn stop_recording(&mut self, stopped_at_ms: u64) -> Result<Recording, String> {
        let recording = self.recorder.stop(stopped_at_ms)?;
        self.recording = Some(recording.clone());
        self.mode = AppMode::Idle;
        self.message = "Recording stopped".to_string();
        Ok(recording)
    }

    pub fn set_recording(&mut self, recording: Recording) -> Result<(), String> {
        recording.validate()?;
        self.recording = Some(recording);
        self.mode = AppMode::Idle;
        self.message = "Recording loaded".to_string();
        Ok(())
    }

    pub fn current_recording(&self) -> Option<&Recording> {
        self.recording.as_ref()
    }

    pub fn start_playback(
        &mut self,
        loop_count: u32,
        speed_multiplier: f64,
    ) -> Result<Vec<PlaybackAction>, String> {
        if self.mode == AppMode::Recording {
            return Err("cannot play while recording".to_string());
        }
        let recording = self
            .recording
            .as_ref()
            .ok_or_else(|| "no recording loaded".to_string())?;
        let settings = PlaybackSettings::new(loop_count, speed_multiplier)?;
        self.stop_token = StopToken::default();
        self.mode = AppMode::Playing;
        self.message = "Playing".to_string();
        Ok(build_playback_plan(recording, settings))
    }

    pub fn stop_playback(&mut self) {
        self.stop_token.request_stop();
        self.mode = AppMode::Idle;
        self.message = "Playback stopped".to_string();
    }

    pub fn stop_token(&self) -> StopToken {
        self.stop_token.clone()
    }
}
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod app_state;
pub mod model;
pub mod player;
pub mod recorder;
pub mod storage;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
```

- [ ] **Step 4: Verify app state tests pass**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test app_state
```

Expected: all four tests pass.

- [ ] **Step 5: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add src-tauri/src/app_state.rs src-tauri/src/lib.rs src-tauri/tests/app_state.rs
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: add app controller state"
```

---

### Task 6: Playback Executor Contract And Windows Input

**Files:**
- Modify: `src-tauri/src/player.rs`
- Create: `src-tauri/src/input.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/tests/player.rs`

- [ ] **Step 1: Add failing executor test**

Append this test to `src-tauri/tests/player.rs`:

```rust
use remember::model::{ButtonState, MouseButton};
use remember::player::{play_actions, StepExecutor};
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct FakeExecutor {
    calls: Arc<Mutex<Vec<String>>>,
}

impl StepExecutor for FakeExecutor {
    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("move:{x}:{y}"));
        Ok(())
    }

    fn mouse_button(&self, x: i32, y: i32, button: MouseButton, state: ButtonState) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("button:{x}:{y}:{button:?}:{state:?}"));
        Ok(())
    }

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("wheel:{x}:{y}:{delta}"));
        Ok(())
    }

    fn key(&self, vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String> {
        self.calls.lock().unwrap().push(format!("key:{vk_code}:{scan_code}:{state:?}"));
        Ok(())
    }
}

#[test]
fn play_actions_dispatches_steps_to_executor() {
    let fake = FakeExecutor::default();
    let calls = fake.calls.clone();
    let settings = PlaybackSettings::new(1, 1000.0).expect("settings");
    let plan = build_playback_plan(&recording(), settings);
    let token = StopToken::default();

    play_actions(&plan, &fake, &token).expect("play");

    assert_eq!(
        calls.lock().unwrap().as_slice(),
        ["key:65:30:Pressed", "key:65:30:Released"]
    );
}
```

- [ ] **Step 2: Run player tests to verify they fail**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test player
```

Expected: compile fails because `StepExecutor` and `play_actions` do not exist.

- [ ] **Step 3: Add executor contract and dispatcher**

Add this to `src-tauri/src/player.rs` below `build_playback_plan`:

```rust
use crate::model::{ButtonState, KeyState, MouseButton};
use std::{thread, time::Duration};

pub trait StepExecutor {
    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String>;
    fn mouse_button(&self, x: i32, y: i32, button: MouseButton, state: ButtonState) -> Result<(), String>;
    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String>;
    fn key(&self, vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String>;
}

pub fn play_actions(
    actions: &[PlaybackAction],
    executor: &dyn StepExecutor,
    stop_token: &StopToken,
) -> Result<(), String> {
    for action in actions {
        if stop_token.is_stopped() {
            return Ok(());
        }
        if action.delay_ms > 0 {
            thread::sleep(Duration::from_millis(action.delay_ms));
        }
        if stop_token.is_stopped() {
            return Ok(());
        }
        match action.step {
            MacroStep::MouseMove { x, y, .. } => executor.mouse_move(x, y)?,
            MacroStep::MouseButton { x, y, button, state, .. } => {
                executor.mouse_button(x, y, button, state)?
            }
            MacroStep::MouseWheel { x, y, delta, .. } => executor.mouse_wheel(x, y, delta)?,
            MacroStep::Key { vk_code, scan_code, state, .. } => executor.key(vk_code, scan_code, state)?,
            MacroStep::Wait { .. } => {}
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Implement system input executor**

Create `src-tauri/src/input.rs`:

```rust
use crate::{
    model::{ButtonState, KeyState, MouseButton},
    player::StepExecutor,
};

pub struct SystemInputExecutor;

impl StepExecutor for SystemInputExecutor {
    fn mouse_move(&self, x: i32, y: i32) -> Result<(), String> {
        system_mouse_move(x, y)
    }

    fn mouse_button(&self, x: i32, y: i32, button: MouseButton, state: ButtonState) -> Result<(), String> {
        system_mouse_button(x, y, button, state)
    }

    fn mouse_wheel(&self, x: i32, y: i32, delta: i32) -> Result<(), String> {
        system_mouse_wheel(x, y, delta)
    }

    fn key(&self, vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String> {
        system_key(vk_code, scan_code, state)
    }
}

#[cfg(not(target_os = "windows"))]
fn system_mouse_move(_x: i32, _y: i32) -> Result<(), String> {
    Err("Remember input playback is Windows-only".to_string())
}

#[cfg(not(target_os = "windows"))]
fn system_mouse_button(_x: i32, _y: i32, _button: MouseButton, _state: ButtonState) -> Result<(), String> {
    Err("Remember input playback is Windows-only".to_string())
}

#[cfg(not(target_os = "windows"))]
fn system_mouse_wheel(_x: i32, _y: i32, _delta: i32) -> Result<(), String> {
    Err("Remember input playback is Windows-only".to_string())
}

#[cfg(not(target_os = "windows"))]
fn system_key(_vk_code: u16, _scan_code: u16, _state: KeyState) -> Result<(), String> {
    Err("Remember input playback is Windows-only".to_string())
}

#[cfg(target_os = "windows")]
fn system_mouse_move(x: i32, y: i32) -> Result<(), String> {
    use windows::Win32::UI::WindowsAndMessaging::SetCursorPos;
    unsafe {
        SetCursorPos(x, y).map_err(|error| format!("SetCursorPos failed: {error}"))?;
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn system_mouse_button(x: i32, y: i32, button: MouseButton, state: ButtonState) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
        MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP,
        MOUSEEVENTF_XDOWN, MOUSEEVENTF_XUP, MOUSEINPUT, XBUTTON1, XBUTTON2,
    };
    system_mouse_move(x, y)?;
    let (flags, mouse_data) = match (button, state) {
        (MouseButton::Left, ButtonState::Pressed) => (MOUSEEVENTF_LEFTDOWN, 0),
        (MouseButton::Left, ButtonState::Released) => (MOUSEEVENTF_LEFTUP, 0),
        (MouseButton::Right, ButtonState::Pressed) => (MOUSEEVENTF_RIGHTDOWN, 0),
        (MouseButton::Right, ButtonState::Released) => (MOUSEEVENTF_RIGHTUP, 0),
        (MouseButton::Middle, ButtonState::Pressed) => (MOUSEEVENTF_MIDDLEDOWN, 0),
        (MouseButton::Middle, ButtonState::Released) => (MOUSEEVENTF_MIDDLEUP, 0),
        (MouseButton::X1, ButtonState::Pressed) => (MOUSEEVENTF_XDOWN, XBUTTON1.0 as u32),
        (MouseButton::X1, ButtonState::Released) => (MOUSEEVENTF_XUP, XBUTTON1.0 as u32),
        (MouseButton::X2, ButtonState::Pressed) => (MOUSEEVENTF_XDOWN, XBUTTON2.0 as u32),
        (MouseButton::X2, ButtonState::Released) => (MOUSEEVENTF_XUP, XBUTTON2.0 as u32),
    };
    let mut input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: mouse_data,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&mut [input], std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err("SendInput mouse button failed".to_string());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn system_mouse_wheel(x: i32, y: i32, delta: i32) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_WHEEL, MOUSEINPUT,
    };
    system_mouse_move(x, y)?;
    let mut input = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: delta as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&mut [input], std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err("SendInput mouse wheel failed".to_string());
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn system_key(vk_code: u16, scan_code: u16, state: KeyState) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VIRTUAL_KEY,
    };
    let flags = match state {
        KeyState::Pressed => Default::default(),
        KeyState::Released => KEYEVENTF_KEYUP,
    };
    let mut input = INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk_code),
                wScan: scan_code,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(&mut [input], std::mem::size_of::<INPUT>() as i32) };
    if sent == 0 {
        return Err("SendInput key failed".to_string());
    }
    Ok(())
}
```

Modify `src-tauri/src/lib.rs`:

```rust
pub mod app_state;
pub mod input;
pub mod model;
pub mod player;
pub mod recorder;
pub mod storage;

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
```

- [ ] **Step 5: Verify playback executor tests pass**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml --test player
cargo check --manifest-path src-tauri\Cargo.toml
```

Expected: tests and check pass on Windows.

- [ ] **Step 6: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add src-tauri/src/player.rs src-tauri/src/input.rs src-tauri/src/lib.rs src-tauri/tests/player.rs
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: add playback executor"
```

---

### Task 7: Tauri Commands, Hotkeys, And Tray

**Files:**
- Create: `src-tauri/src/commands.rs`
- Create: `src-tauri/src/hotkeys.rs`
- Create: `src-tauri/src/tray.rs`
- Modify: `src-tauri/src/lib.rs`
- Modify: `src-tauri/src/app_state.rs`

- [ ] **Step 1: Add command-facing state methods**

Modify `src-tauri/src/app_state.rs` by adding these methods to `impl AppController`:

```rust
pub fn saveable_recording(&self) -> Result<Recording, String> {
    self.recording
        .clone()
        .ok_or_else(|| "no recording loaded".to_string())
}

pub fn mark_idle(&mut self, message: impl Into<String>) {
    self.mode = AppMode::Idle;
    self.message = message.into();
}
```

- [ ] **Step 2: Create Tauri command wrappers**

Create `src-tauri/src/commands.rs`:

```rust
use crate::{
    app_state::{AppController, UiState},
    input::SystemInputExecutor,
    player::play_actions,
    storage::{load_recording, save_recording},
};
use chrono::Utc;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};
use tauri::{AppHandle, Emitter, State};

pub type SharedApp = Arc<Mutex<AppController>>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn emit_state(app: &AppHandle, state: UiState) -> Result<(), String> {
    app.emit("remember://state", state)
        .map_err(|error| format!("failed to emit state: {error}"))
}

#[tauri::command]
pub fn get_state(state: State<'_, SharedApp>) -> Result<UiState, String> {
    Ok(state.lock().map_err(|_| "state lock poisoned".to_string())?.ui_state())
}

#[tauri::command]
pub fn start_recording(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    let mut guard = state.lock().map_err(|_| "state lock poisoned".to_string())?;
    let name = format!("recording-{}", now_ms());
    guard.start_recording(name, now_ms(), Utc::now().to_rfc3339())?;
    let ui = guard.ui_state();
    emit_state(&app, ui.clone())?;
    Ok(ui)
}

#[tauri::command]
pub fn stop_recording(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    let mut guard = state.lock().map_err(|_| "state lock poisoned".to_string())?;
    guard.stop_recording(now_ms())?;
    let ui = guard.ui_state();
    emit_state(&app, ui.clone())?;
    Ok(ui)
}

#[tauri::command]
pub fn open_recording(app: AppHandle, state: State<'_, SharedApp>, path: PathBuf) -> Result<UiState, String> {
    let recording = load_recording(&path).map_err(|error| error.to_string())?;
    let mut guard = state.lock().map_err(|_| "state lock poisoned".to_string())?;
    guard.set_recording(recording)?;
    let ui = guard.ui_state();
    emit_state(&app, ui.clone())?;
    Ok(ui)
}

#[tauri::command]
pub fn save_current_recording(state: State<'_, SharedApp>, path: PathBuf) -> Result<(), String> {
    let recording = state
        .lock()
        .map_err(|_| "state lock poisoned".to_string())?
        .saveable_recording()?;
    save_recording(&path, &recording).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn start_playback(
    app: AppHandle,
    state: State<'_, SharedApp>,
    loop_count: u32,
    speed_multiplier: f64,
) -> Result<UiState, String> {
    let (plan, token, ui) = {
        let mut guard = state.lock().map_err(|_| "state lock poisoned".to_string())?;
        let plan = guard.start_playback(loop_count, speed_multiplier)?;
        let token = guard.stop_token();
        let ui = guard.ui_state();
        emit_state(&app, ui.clone())?;
        (plan, token, ui)
    };
    let app_for_thread = app.clone();
    let state_for_thread = state.inner().clone();
    thread::spawn(move || {
        let executor = SystemInputExecutor;
        let result = play_actions(&plan, &executor, &token);
        if let Ok(mut guard) = state_for_thread.lock() {
            let message = result
                .map(|_| "Playback finished".to_string())
                .unwrap_or_else(|error| error);
            guard.mark_idle(message);
            let _ = app_for_thread.emit("remember://state", guard.ui_state());
        }
    });
    Ok(ui)
}

#[tauri::command]
pub fn stop_playback(app: AppHandle, state: State<'_, SharedApp>) -> Result<UiState, String> {
    let mut guard = state.lock().map_err(|_| "state lock poisoned".to_string())?;
    guard.stop_playback();
    let ui = guard.ui_state();
    emit_state(&app, ui.clone())?;
    Ok(ui)
}
```

- [ ] **Step 3: Add hotkey registration**

Create `src-tauri/src/hotkeys.rs`:

```rust
use crate::commands::{start_recording, start_playback, stop_playback, stop_recording, SharedApp};
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

pub fn register_hotkeys(app: &AppHandle) -> Result<(), String> {
    let record = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyR);
    let play = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyP);
    let emergency = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::Escape);

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(record, move |_app, _shortcut, event| {
            if event.state() != ShortcutState::Pressed {
                return;
            }
            if let Some(shared) = app_handle.try_state::<SharedApp>() {
                let is_recording = shared
                    .lock()
                    .map(|state| state.ui_state().mode == crate::app_state::AppMode::Recording)
                    .unwrap_or(false);
                if is_recording {
                    let _ = stop_recording(app_handle.clone(), shared);
                } else {
                    let _ = start_recording(app_handle.clone(), shared);
                }
            }
        })
        .map_err(|error| format!("failed to register record hotkey: {error}"))?;

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(play, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(shared) = app_handle.try_state::<SharedApp>() {
                    let _ = start_playback(app_handle.clone(), shared, 1, 1.0);
                }
            }
        })
        .map_err(|error| format!("failed to register play hotkey: {error}"))?;

    let app_handle = app.clone();
    app.global_shortcut()
        .on_shortcut(emergency, move |_app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                if let Some(shared) = app_handle.try_state::<SharedApp>() {
                    let _ = stop_playback(app_handle.clone(), shared);
                }
            }
        })
        .map_err(|error| format!("failed to register emergency stop hotkey: {error}"))?;

    Ok(())
}
```

- [ ] **Step 4: Add tray setup**

Create `src-tauri/src/tray.rs`:

```rust
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn setup_tray(app: &AppHandle) -> Result<(), String> {
    let show = MenuItem::with_id(app, "show", "Show Remember", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)
        .map_err(|error| error.to_string())?;
    let menu = Menu::with_items(app, &[&show, &quit]).map_err(|error| error.to_string())?;

    TrayIconBuilder::new()
        .tooltip("Remember")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)
        .map_err(|error| error.to_string())?;
    Ok(())
}
```

- [ ] **Step 5: Wire Tauri app builder**

Modify `src-tauri/src/lib.rs`:

```rust
pub mod app_state;
pub mod commands;
pub mod hotkeys;
pub mod input;
pub mod model;
pub mod player;
pub mod recorder;
pub mod storage;
pub mod tray;

use app_state::AppController;
use commands::SharedApp;
use std::sync::{Arc, Mutex};

pub fn product_name() -> &'static str {
    "Remember"
}

pub fn run() {
    let shared: SharedApp = Arc::new(Mutex::new(AppController::new()));
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(shared)
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::start_recording,
            commands::stop_recording,
            commands::open_recording,
            commands::save_current_recording,
            commands::start_playback,
            commands::stop_playback
        ])
        .setup(|app| {
            tray::setup_tray(app.handle()).map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            hotkeys::register_hotkeys(app.handle()).map_err(|error| Box::<dyn std::error::Error>::from(error))?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to run Remember");
}
```

- [ ] **Step 6: Verify Tauri Rust build**

Run:

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
cargo check --manifest-path src-tauri\Cargo.toml
```

Expected: all Rust tests pass and `cargo check` passes. If plugin APIs differ for the installed Tauri 2 minor version, adjust only the affected imports/calls while preserving command names and hotkey behavior.

- [ ] **Step 7: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add src-tauri/src
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: wire Tauri commands and hotkeys"
```

---

### Task 8: React Control Panel

**Files:**
- Create: `src/types.ts`
- Create: `src/lib/rememberApi.ts`
- Create: `src/components/Controls.tsx`
- Create: `src/components/PlaybackSettings.tsx`
- Create: `src/components/StatusPanel.tsx`
- Create: `src/components/HotkeyPanel.tsx`
- Modify: `src/App.tsx`
- Modify: `src/App.test.tsx`

- [ ] **Step 1: Replace the smoke test with UI behavior tests**

Replace `src/App.test.tsx`:

```tsx
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { App } from "./App";
import * as api from "./lib/rememberApi";

vi.mock("./lib/rememberApi");

const idleState = {
  mode: "idle" as const,
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle"
};

describe("App", () => {
  beforeEach(() => {
    vi.mocked(api.getState).mockResolvedValue(idleState);
    vi.mocked(api.subscribeToState).mockResolvedValue(() => undefined);
    vi.mocked(api.startRecording).mockResolvedValue({ ...idleState, mode: "recording", message: "Recording" });
    vi.mocked(api.stopPlayback).mockResolvedValue({ ...idleState, message: "Playback stopped" });
  });

  it("renders the Remember utility controls", async () => {
    render(<App />);

    expect(await screen.findByRole("heading", { name: "Remember" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /record/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /play/i })).toBeDisabled();
    expect(screen.getByText("Ctrl+Alt+R")).toBeInTheDocument();
  });

  it("starts recording from the Record button", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: /record/i }));

    expect(api.startRecording).toHaveBeenCalledOnce();
    expect(await screen.findByText("Recording")).toBeInTheDocument();
  });

  it("rejects invalid playback settings in the UI", async () => {
    const user = userEvent.setup();
    render(<App />);

    const loopInput = await screen.findByLabelText("Loop count");
    await user.clear(loopInput);
    await user.type(loopInput, "0");

    expect(screen.getByText("Loop count must be at least 1.")).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run frontend tests to verify they fail**

Run:

```powershell
npm test -- src/App.test.tsx
```

Expected: tests fail because `rememberApi`, components, and full UI behavior do not exist.

- [ ] **Step 3: Add frontend types and API wrapper**

Create `src/types.ts`:

```ts
export type AppMode = "idle" | "recording" | "playing";

export interface UiState {
  mode: AppMode;
  recording_name: string | null;
  step_count: number;
  duration_ms: number;
  message: string;
}
```

Create `src/lib/rememberApi.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { UiState } from "../types";

export function getState(): Promise<UiState> {
  return invoke<UiState>("get_state");
}

export function startRecording(): Promise<UiState> {
  return invoke<UiState>("start_recording");
}

export function stopRecording(): Promise<UiState> {
  return invoke<UiState>("stop_recording");
}

export function startPlayback(loopCount: number, speedMultiplier: number): Promise<UiState> {
  return invoke<UiState>("start_playback", {
    loopCount,
    speedMultiplier
  });
}

export function stopPlayback(): Promise<UiState> {
  return invoke<UiState>("stop_playback");
}

export async function subscribeToState(onState: (state: UiState) => void): Promise<() => void> {
  return listen<UiState>("remember://state", (event) => onState(event.payload));
}
```

- [ ] **Step 4: Add UI components**

Create `src/components/Controls.tsx`:

```tsx
import { FolderOpen, Play, Save, Square, Video } from "lucide-react";
import type { AppMode } from "../types";

interface ControlsProps {
  mode: AppMode;
  hasRecording: boolean;
  onRecord: () => void;
  onPlay: () => void;
  onStop: () => void;
  onSave: () => void;
  onOpen: () => void;
}

export function Controls({ mode, hasRecording, onRecord, onPlay, onStop, onSave, onOpen }: ControlsProps) {
  const isRecording = mode === "recording";
  const isPlaying = mode === "playing";
  return (
    <section className="toolbar" aria-label="Macro controls">
      <button type="button" onClick={onRecord} disabled={isPlaying} title="Record">
        <Video size={18} />
        {isRecording ? "Stop recording" : "Record"}
      </button>
      <button type="button" onClick={onPlay} disabled={!hasRecording || isRecording || isPlaying} title="Play">
        <Play size={18} />
        Play
      </button>
      <button type="button" onClick={onStop} disabled={!isPlaying && !isRecording} title="Stop">
        <Square size={18} />
        Stop
      </button>
      <button type="button" onClick={onSave} disabled={!hasRecording || isRecording || isPlaying} title="Save">
        <Save size={18} />
        Save
      </button>
      <button type="button" onClick={onOpen} disabled={isRecording || isPlaying} title="Open">
        <FolderOpen size={18} />
        Open
      </button>
    </section>
  );
}
```

Create `src/components/PlaybackSettings.tsx`:

```tsx
interface PlaybackSettingsProps {
  loopCount: number;
  speedMultiplier: number;
  onLoopCountChange: (value: number) => void;
  onSpeedMultiplierChange: (value: number) => void;
}

export function PlaybackSettings({
  loopCount,
  speedMultiplier,
  onLoopCountChange,
  onSpeedMultiplierChange
}: PlaybackSettingsProps) {
  const loopInvalid = loopCount < 1;
  const speedInvalid = speedMultiplier <= 0;
  return (
    <section className="settings" aria-label="Playback settings">
      <label>
        Loop count
        <input
          type="number"
          min={1}
          value={loopCount}
          onChange={(event) => onLoopCountChange(Number(event.target.value))}
        />
      </label>
      {loopInvalid ? <p role="alert">Loop count must be at least 1.</p> : null}

      <label>
        Speed
        <input
          type="number"
          min={0.1}
          step={0.1}
          value={speedMultiplier}
          onChange={(event) => onSpeedMultiplierChange(Number(event.target.value))}
        />
      </label>
      {speedInvalid ? <p role="alert">Speed must be greater than 0.</p> : null}
    </section>
  );
}
```

Create `src/components/StatusPanel.tsx`:

```tsx
import type { UiState } from "../types";

export function StatusPanel({ state }: { state: UiState }) {
  return (
    <section className="status" aria-label="Status">
      <p className={`status-pill status-${state.mode}`}>{state.message}</p>
      <dl>
        <div>
          <dt>Recording</dt>
          <dd>{state.recording_name ?? "None"}</dd>
        </div>
        <div>
          <dt>Steps</dt>
          <dd>{state.step_count}</dd>
        </div>
        <div>
          <dt>Duration</dt>
          <dd>{state.duration_ms} ms</dd>
        </div>
      </dl>
    </section>
  );
}
```

Create `src/components/HotkeyPanel.tsx`:

```tsx
export function HotkeyPanel() {
  return (
    <section className="hotkeys" aria-label="Hotkeys">
      <h2>Hotkeys</h2>
      <dl>
        <div>
          <dt>Record</dt>
          <dd>Ctrl+Alt+R</dd>
        </div>
        <div>
          <dt>Play</dt>
          <dd>Ctrl+Alt+P</dd>
        </div>
        <div>
          <dt>Emergency stop</dt>
          <dd>Ctrl+Alt+Esc</dd>
        </div>
      </dl>
    </section>
  );
}
```

- [ ] **Step 5: Replace App with integrated UI**

Replace `src/App.tsx`:

```tsx
import { useEffect, useMemo, useState } from "react";
import { Controls } from "./components/Controls";
import { HotkeyPanel } from "./components/HotkeyPanel";
import { PlaybackSettings } from "./components/PlaybackSettings";
import { StatusPanel } from "./components/StatusPanel";
import * as rememberApi from "./lib/rememberApi";
import type { UiState } from "./types";
import "./styles.css";

const initialState: UiState = {
  mode: "idle",
  recording_name: null,
  step_count: 0,
  duration_ms: 0,
  message: "Idle"
};

export function App() {
  const [state, setState] = useState<UiState>(initialState);
  const [loopCount, setLoopCount] = useState(1);
  const [speedMultiplier, setSpeedMultiplier] = useState(1);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let unsubscribe: undefined | (() => void);
    rememberApi.getState().then(setState).catch((err) => setError(String(err)));
    rememberApi
      .subscribeToState(setState)
      .then((off) => {
        unsubscribe = off;
      })
      .catch((err) => setError(String(err)));
    return () => {
      if (unsubscribe) unsubscribe();
    };
  }, []);

  const hasRecording = useMemo(() => state.step_count > 0, [state.step_count]);

  async function run(action: () => Promise<UiState>) {
    setError(null);
    try {
      setState(await action());
    } catch (err) {
      setError(String(err));
    }
  }

  async function handleRecord() {
    if (state.mode === "recording") {
      await run(rememberApi.stopRecording);
    } else {
      await run(rememberApi.startRecording);
    }
  }

  async function handlePlay() {
    await run(() => rememberApi.startPlayback(loopCount, speedMultiplier));
  }

  async function handleStop() {
    if (state.mode === "recording") {
      await run(rememberApi.stopRecording);
    } else {
      await run(rememberApi.stopPlayback);
    }
  }

  return (
    <main className="app-shell">
      <header className="app-header">
        <h1>Remember</h1>
        <p>Portable macro recorder</p>
      </header>

      <Controls
        mode={state.mode}
        hasRecording={hasRecording}
        onRecord={handleRecord}
        onPlay={handlePlay}
        onStop={handleStop}
        onSave={() => setError("Save dialog wiring follows the command layer.")}
        onOpen={() => setError("Open dialog wiring follows the command layer.")}
      />

      <PlaybackSettings
        loopCount={loopCount}
        speedMultiplier={speedMultiplier}
        onLoopCountChange={setLoopCount}
        onSpeedMultiplierChange={setSpeedMultiplier}
      />

      <StatusPanel state={state} />
      <HotkeyPanel />

      {error ? <p className="error" role="alert">{error}</p> : null}
    </main>
  );
}
```

Create `src/styles.css`:

```css
:root {
  color: #202124;
  background: #f6f7f8;
  font-family: "Segoe UI", Arial, sans-serif;
}

body {
  margin: 0;
}

button,
input {
  font: inherit;
}

.app-shell {
  min-height: 100vh;
  padding: 20px;
  box-sizing: border-box;
  display: grid;
  gap: 16px;
}

.app-header h1 {
  margin: 0;
  font-size: 28px;
}

.app-header p {
  margin: 4px 0 0;
  color: #5f6368;
}

.toolbar {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 8px;
}

.toolbar button {
  min-height: 42px;
  border: 1px solid #c9cdd2;
  background: #ffffff;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
}

.toolbar button:disabled {
  opacity: 0.45;
}

.settings,
.status,
.hotkeys {
  display: grid;
  gap: 10px;
}

.settings label {
  display: grid;
  gap: 4px;
}

.settings input {
  min-height: 34px;
  padding: 0 8px;
}

.status-pill {
  margin: 0;
  font-weight: 600;
}

dl {
  margin: 0;
  display: grid;
  gap: 8px;
}

dl div {
  display: flex;
  justify-content: space-between;
  gap: 12px;
}

dt {
  color: #5f6368;
}

dd {
  margin: 0;
  text-align: right;
}

.error,
[role="alert"] {
  color: #b3261e;
  margin: 0;
}
```

- [ ] **Step 6: Verify frontend tests pass**

Run:

```powershell
npm test -- src/App.test.tsx
npm run build
```

Expected: frontend tests and production build pass.

- [ ] **Step 7: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add src package.json package-lock.json index.html tsconfig.json vite.config.ts vitest.config.ts
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: add React control panel"
```

---

### Task 9: Save/Open Dialogs And End-To-End Build

**Files:**
- Modify: `src/App.test.tsx`
- Modify: `src/lib/rememberApi.ts`
- Modify: `src/App.tsx`
- Create: `README.md`

- [ ] **Step 1: Write failing save/open UI tests**

Append these tests inside the existing `describe("App", () => { ... })` block in `src/App.test.tsx`:

```tsx
  it("opens a recording from the Open button", async () => {
    const user = userEvent.setup();
    const loadedState = {
      ...idleState,
      recording_name: "loaded",
      step_count: 2,
      duration_ms: 250,
      message: "Recording loaded"
    };
    vi.mocked(api.openRecording).mockResolvedValue(loadedState);
    render(<App />);

    await user.click(await screen.findByRole("button", { name: /open/i }));

    expect(api.openRecording).toHaveBeenCalledOnce();
    expect(await screen.findByText("loaded")).toBeInTheDocument();
  });

  it("saves the current recording from the Save button", async () => {
    const user = userEvent.setup();
    vi.mocked(api.getState).mockResolvedValue({
      ...idleState,
      recording_name: "current",
      step_count: 3,
      duration_ms: 500
    });
    vi.mocked(api.saveCurrentRecording).mockResolvedValue();
    render(<App />);

    await user.click(await screen.findByRole("button", { name: /save/i }));

    expect(api.saveCurrentRecording).toHaveBeenCalledOnce();
  });
```

- [ ] **Step 2: Run save/open UI tests to verify they fail**

Run:

```powershell
npm test -- src/App.test.tsx
```

Expected: tests fail because `openRecording` and `saveCurrentRecording` are not exported and the App does not call them.

- [ ] **Step 3: Add save/open API functions**

Modify `src/lib/rememberApi.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { UiState } from "../types";

export function getState(): Promise<UiState> {
  return invoke<UiState>("get_state");
}

export function startRecording(): Promise<UiState> {
  return invoke<UiState>("start_recording");
}

export function stopRecording(): Promise<UiState> {
  return invoke<UiState>("stop_recording");
}

export function startPlayback(loopCount: number, speedMultiplier: number): Promise<UiState> {
  return invoke<UiState>("start_playback", {
    loopCount,
    speedMultiplier
  });
}

export function stopPlayback(): Promise<UiState> {
  return invoke<UiState>("stop_playback");
}

export async function openRecording(): Promise<UiState | null> {
  const selected = await open({
    multiple: false,
    filters: [{ name: "Remember Recording", extensions: ["remember.json", "json"] }]
  });
  if (typeof selected !== "string") return null;
  return invoke<UiState>("open_recording", { path: selected });
}

export async function saveCurrentRecording(): Promise<void> {
  const selected = await save({
    filters: [{ name: "Remember Recording", extensions: ["remember.json", "json"] }]
  });
  if (!selected) return;
  await invoke("save_current_recording", { path: selected });
}

export async function subscribeToState(onState: (state: UiState) => void): Promise<() => void> {
  return listen<UiState>("remember://state", (event) => onState(event.payload));
}
```

- [ ] **Step 4: Wire save/open buttons in App**

Replace the `onSave` and `onOpen` props in `src/App.tsx`:

```tsx
onSave={async () => {
  setError(null);
  try {
    await rememberApi.saveCurrentRecording();
  } catch (err) {
    setError(String(err));
  }
}}
onOpen={async () => {
  setError(null);
  try {
    const loaded = await rememberApi.openRecording();
    if (loaded) setState(loaded);
  } catch (err) {
    setError(String(err));
  }
}}
```

- [ ] **Step 5: Verify save/open UI tests pass**

Run:

```powershell
npm test -- src/App.test.tsx
```

Expected: all App tests pass.

- [ ] **Step 6: Add README**

Create `README.md`:

```markdown
# Remember

Remember is a portable Windows macro recorder inspired by the TinyTask workflow. It records real mouse and keyboard input, saves recordings as `.remember.json`, and replays them with loop and speed controls.

This is an original implementation. It does not copy TinyTask code, icons, names, binaries, or visual assets.

## Requirements

- Windows
- Node.js 22.12 or newer
- Rust stable toolchain
- Tauri prerequisites for Windows

## Development

```powershell
npm install
npm run tauri dev
```

## Tests

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo check --manifest-path src-tauri\Cargo.toml
```

## Portable Build

```powershell
npm run tauri build
```

The portable executable is produced under:

```text
src-tauri\target\release
```

Installer bundles are not part of the first release target.

## Default Hotkeys

- Record or stop recording: `Ctrl+Alt+R`
- Play: `Ctrl+Alt+P`
- Emergency stop: `Ctrl+Alt+Esc`

## First Release Limits

- Windows only.
- No image recognition.
- No AI-driven automation.
- No step editor.
- No target-window validation.
- Elevated target windows can reject input from a non-elevated Remember process.
```

- [ ] **Step 7: Verify all automated checks**

Run:

```powershell
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo check --manifest-path src-tauri\Cargo.toml
```

Expected: all automated checks pass.

- [ ] **Step 8: Build Tauri executable**

Run:

```powershell
npm run tauri build
```

Expected: Tauri produces `src-tauri\target\release\remember.exe`.

- [ ] **Step 9: Manual acceptance test**

Run the built executable and verify:

1. Launch Remember.
2. Open Notepad.
3. Start recording.
4. Type a short phrase and perform one mouse click.
5. Stop recording.
6. Save the file as `.remember.json`.
7. Reopen the file in Remember.
8. Replay into Notepad.
9. Confirm the phrase and click are reproduced.
10. Confirm Stop and `Ctrl+Alt+Esc` interrupt playback.

- [ ] **Step 10: Commit**

```powershell
git -c safe.directory=D:/Code/Codex/remember add README.md src src-tauri package.json package-lock.json
git -c safe.directory=D:/Code/Codex/remember commit -m "feat: complete portable Remember MVP"
```

---

## Final Verification

Run:

```powershell
git -c safe.directory=D:/Code/Codex/remember status --short
npm test
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
cargo check --manifest-path src-tauri\Cargo.toml
npm run tauri build
```

Expected:

- Working tree is clean before final packaging, except for user-approved release artifacts.
- Frontend tests pass.
- Frontend build passes.
- Rust tests pass.
- Rust check passes.
- `src-tauri\target\release\remember.exe` exists and launches.
- Manual Notepad acceptance test passes.
