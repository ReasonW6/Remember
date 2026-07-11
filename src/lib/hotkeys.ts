export interface ShortcutEventLike {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

export function shortcutFromEvent(event: ShortcutEventLike) {
  const key = keyLabel(event.key);
  if (!key || isModifierKey(key)) {
    return "";
  }

  const parts = [];
  if (event.ctrlKey) {
    parts.push("Ctrl");
  }
  if (event.altKey) {
    parts.push("Alt");
  }
  if (event.shiftKey) {
    parts.push("Shift");
  }
  if (event.metaKey) {
    parts.push("Win");
  }
  parts.push(key);
  return parts.join("+");
}

export function isAllowedGlobalShortcut(shortcut: string) {
  if (shortcut.includes("+")) {
    return true;
  }
  return /^F([1-9]|1[0-9]|2[0-4])$/.test(shortcut);
}

function keyLabel(key: string) {
  if (key.length === 1) {
    return key.toUpperCase();
  }

  const labels: Record<string, string> = {
    " ": "Space",
    Escape: "Esc",
    ArrowUp: "ArrowUp",
    ArrowDown: "ArrowDown",
    ArrowLeft: "ArrowLeft",
    ArrowRight: "ArrowRight",
    Backspace: "Backspace",
    Delete: "Delete",
    Enter: "Enter",
    Home: "Home",
    End: "End",
    Insert: "Insert",
    PageUp: "PageUp",
    PageDown: "PageDown",
    Tab: "Tab"
  };

  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(key)) {
    return key;
  }

  return labels[key] ?? "";
}

function isModifierKey(key: string) {
  return key === "Control" || key === "Alt" || key === "Shift" || key === "Meta";
}
