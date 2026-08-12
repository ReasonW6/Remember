# Remember

Remember records and replays user input while preserving the intended relationship between input and its targets.

## Language

**Window-relative recording**:
An optional version 2 recording mode in which pointer positions remain at the same relative location after a target window moves. It can reproduce recorded window move or resize gestures, but does not scale input for an arbitrary resize, DPI change, or internal layout change.
_Avoid_: Window positioning, adaptive coordinates, responsive coordinates

**Coordinate mode**:
The remembered preference applied when the next recording starts, selecting either version 1 screen-only recording or version 2 window-relative recording. A recording keeps the selected mode for its entire lifetime.
_Avoid_: Playback override, live coordinate switch

**Main window session state**:
The last selected compact or expanded main interface together with its last visible desktop position. Ordinary launch and administrator restart restore the same state, while an unavailable display position falls back to a visible monitor.
_Avoid_: Default window layout, administrator layout, temporary panel state

**Screen-relative position**:
A pointer position fixed to the virtual desktop, independent of any window. Desktop movement and movement that merely crosses background windows use this meaning.
_Avoid_: Unbound position, fallback position

**Window-relative position**:
A pointer position measured from a target window's client-area origin and valid only through a compatible window binding. Positions on window chrome or owned transient surfaces may fall outside the client-area bounds.
_Avoid_: Adjusted absolute position, scalable position

**Window adjustment gesture**:
A recorded pointer gesture that moves or resizes its target window. Every pointer step in the gesture uses the client-area origin captured when the button was pressed, preventing the moving window from feeding a new origin back into its own drag path; after release, the resulting client geometry becomes the target's current geometry.
_Avoid_: Drifting drag, scaled pointer path, ordinary content drag

**Coordinate transition**:
A recorded boundary where pointer movement changes between screen-relative and window-relative meaning. Playback may jump directly at this boundary and does not synthesize a smoothing path.
_Avoid_: Interpolated transition, corrected desktop path

**Loop interval delay**:
A user-selected wall-clock pause after one playback loop has fully ended and before the next loop begins. It applies only when another loop will run, is independent of playback speed, and is never added after the final loop.
_Avoid_: Start delay, step delay, trailing duration

**Target window**:
An operating-system window associated with targeted pointer or keyboard input in a window-relative recording. A recording may contain more than one target window.
_Avoid_: Application, screen, target coordinate

**Target window identity**:
The recorded executable path, window class, and title used to match a target window for playback. It is persistent metadata, unlike a temporary operating-system window handle.
_Avoid_: HWND, application name, target ID

**Target window instance**:
One continuous lifetime of an independent target window. Closing and reopening an otherwise identical window creates a new instance and requires a new binding.
_Avoid_: Process instance, reused window identity

**Independent target window**:
A top-level window or independently movable dialog that receives its own target identity and binding.
_Avoid_: Child control, menu surface

**Owned transient surface**:
A short-lived interactive surface such as a menu, dropdown, or context menu that uses its owning target window's coordinate reference rather than becoming a target itself.
_Avoid_: Deferred target window, independent popup

**System surface**:
Windows Shell UI such as the desktop, taskbar, Start menu, notification area, or task switcher. It always uses screen-relative input and never becomes a target window.
_Avoid_: Target window, owned transient surface

**Initial target window**:
A target window that already exists when recording begins. This is descriptive recording metadata only; like every target, it is bound when its first targeted playback action is about to run.
_Avoid_: Pre-opened application, startup window

**Deferred target window**:
A target window that first appears after recording begins. Recorded actions are responsible for making it appear, and playback waits for and binds it only before its first targeted action.
_Avoid_: Auto-launched window, missing initial target

**Foreground window**:
The currently active operating-system window. Pointer movement over it is eligible for window-relative recording, while movement over the desktop or background windows remains screen-relative.
_Avoid_: Focused application, hovered window

**Activation click**:
A click on a background or launcher surface intended to change the foreground context. It remains associated with the clicked surface for coordinate replay, but the resulting foreground may be that surface or a different window opened by it.
_Avoid_: Absolute focus click, same-window foreground requirement

**Drop target**:
A background window where a pointer drag is completed. Movement that merely crosses the window remains screen-relative, but the final release remains associated with the drop target.
_Avoid_: Hovered window, drag path window

**Targeted input**:
Recorded pointer or keyboard input whose effect belongs to a target window. A minimized or hidden target is restored only when its next targeted input is about to run.
_Avoid_: Global input, window wake-up request

**Keyboard sequence**:
A group beginning with the first key press and ending when every key has been released. It is associated with and checked against the foreground target only at its start, so a readable sequence continues normally through focus changes such as Alt+Tab; a sequence starting on an unreadable target is omitted as a whole.
_Avoid_: Individual targeted keystroke, text entry

**Just-in-time restoration**:
Restoring and activating a minimized or hidden target window immediately before its next targeted input, without opening or waking windows earlier in playback.
_Avoid_: Preflight wake-up, continuous focus enforcement

**Window binding**:
The automatically selected association between a recorded target window and one compatible target window currently available for playback. Every target is bound just before its first targeted action. With no held input, binding may wait a bounded time for the window; while input is held, it may only bind an already available compatible window without waiting, restoring, or changing focus, and otherwise stops playback. Binding never asks the user to choose a window.
_Avoid_: Manual binding, window guess, handle reuse

**Compatible target window**:
A currently available, unoccupied window whose executable path and class exactly match the recorded identity and whose client-area size and DPI match when binding begins. A position change alone does not make a target incompatible; a recorded window adjustment may change the bound window's current size, while DPI remains fixed.
_Avoid_: Filename match, scaled target, approximately matching window

**Target candidate ranking**:
The deterministic order of compatible target windows by recorded-title similarity and then stable window order. The highest-ranked candidate is bound automatically; no candidate means playback cannot continue, although a deferred target may first wait for one to appear.
_Avoid_: Manual disambiguation, arbitrary window choice

**Privileged target window**:
A target window running at a higher Windows integrity level than Remember. It is unavailable for playback until the user explicitly restarts Remember with sufficient privileges.
_Avoid_: Incompatible target, automatically elevated target

**Unreadable target window**:
A window whose identity or geometry Remember cannot read reliably. Pointer gestures encountering it and new keyboard sequences beginning on it are omitted without stopping or constraining the real input, while a reason-specific warning follows the pointer.
_Avoid_: Incompatible target, fatal recording error

**Skipped target interval**:
Elapsed recording time spent over an unreadable target while ordinary target input is omitted. Playback preserves the duration by holding the last valid pointer position and emitting no omitted keyboard or pointer actions until readable recording resumes.
_Avoid_: Deleted time, frozen recording pointer

**Boundary safety release**:
A synthetic release of mouse buttons that remain pressed when a pointer gesture enters an unreadable target interval. It preserves the recorded prefix while ending it safely at the last readable pointer position.
_Avoid_: Gesture rollback, release inside unreadable target

**Resumed pointer gesture**:
A new pointer gesture synthesized at the first readable position after a skipped target interval when a mouse button remains physically held. It is separate from the gesture ended by the boundary safety release.
_Avoid_: Continuous cross-window drag, resumed original gesture

**Reachable target point**:
A window-relative operation point that remains inside the current virtual desktop after applying its target window's position. A target window may be partly off-screen as long as every required point remains reachable.
_Avoid_: Clamped point, fully visible window
