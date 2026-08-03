import { Maximize2, Minimize2, Minus, X } from "lucide-react";

type CurrentWindow = ReturnType<
  (typeof import("@tauri-apps/api/window"))["getCurrentWindow"]
>;

function runWindowAction(action: (appWindow: CurrentWindow) => Promise<void>) {
  void import("@tauri-apps/api/window")
    .then(({ getCurrentWindow }) => action(getCurrentWindow()))
    .catch((error: unknown) => {
      console.warn("Remember window action failed", error);
    });
}

interface WindowTitlebarProps {
  showMinimize?: boolean;
  compact?: boolean;
  resizePending?: boolean;
  onToggleSize?: () => void;
}

export function WindowTitlebar({
  showMinimize = true,
  compact,
  resizePending = false,
  onToggleSize
}: WindowTitlebarProps) {
  return (
    <div className="window-titlebar">
      <div className="window-titlebar-drag" data-tauri-drag-region>
        <img
          className="window-titlebar-icon"
          src="/remember-icon.svg"
          alt=""
          aria-hidden="true"
          data-tauri-drag-region
        />
        <span className="window-titlebar-name" data-tauri-drag-region>
          Remember
        </span>
      </div>
      <div className="window-titlebar-controls" role="toolbar" aria-label="窗口控制">
        {onToggleSize ? (
          <button
            className="window-control-button"
            type="button"
            aria-label={compact ? "展开完整界面" : "切换到小悬浮窗"}
            title={compact ? "展开完整界面" : "切换到小悬浮窗"}
            onClick={onToggleSize}
            disabled={resizePending}
          >
            {compact ? (
              <Maximize2 size={14} aria-hidden="true" />
            ) : (
              <Minimize2 size={14} aria-hidden="true" />
            )}
          </button>
        ) : null}
        {showMinimize ? (
          <button
            className="window-control-button"
            type="button"
            aria-label="最小化"
            onClick={() => runWindowAction((appWindow) => appWindow.minimize())}
          >
            <Minus size={14} aria-hidden="true" />
          </button>
        ) : null}
        <button
          className="window-control-button close-button"
          type="button"
          aria-label="关闭"
          onClick={() => runWindowAction((appWindow) => appWindow.close())}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
