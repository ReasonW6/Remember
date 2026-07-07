import { Minus, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { MouseEvent } from "react";

function runWindowAction(action: () => Promise<void>) {
  void action().catch((error: unknown) => {
    console.warn("Remember window action failed", error);
  });
}

export function WindowTitlebar() {
  const appWindow = getCurrentWindow();

  function handleDrag(event: MouseEvent<HTMLDivElement>) {
    if (event.button !== 0) {
      return;
    }
    runWindowAction(() => appWindow.startDragging());
  }

  return (
    <div className="window-titlebar">
      <div className="window-titlebar-drag" data-tauri-drag-region onMouseDown={handleDrag}>
        <img
          className="window-titlebar-icon"
          src="/remember-icon.svg"
          alt=""
          aria-hidden="true"
          data-tauri-drag-region
        />
        <div className="window-titlebar-text" data-tauri-drag-region>
          <span className="window-titlebar-name" data-tauri-drag-region>
            Remember
          </span>
          <span className="window-titlebar-subtitle" data-tauri-drag-region>
            录制播放
          </span>
        </div>
      </div>
      <div className="window-titlebar-controls" role="toolbar" aria-label="窗口控制">
        <button
          className="window-control-button"
          type="button"
          aria-label="最小化"
          onClick={() => runWindowAction(() => appWindow.minimize())}
        >
          <Minus size={14} aria-hidden="true" />
        </button>
        <button
          className="window-control-button close-button"
          type="button"
          aria-label="关闭"
          onClick={() => runWindowAction(() => appWindow.close())}
        >
          <X size={14} aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}
