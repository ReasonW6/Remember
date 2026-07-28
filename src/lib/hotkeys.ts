export interface ShortcutEventLike {
  key: string;
  code?: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

export function shortcutFromEvent(event: ShortcutEventLike) {
  const key = keyLabel(event.key, event.code);
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
  const parts = shortcut.split("+");
  const key = parts.pop() ?? "";
  const modifiers = parts;
  if (
    !isSupportedKey(key) ||
    modifiers.some((modifier) => !["Ctrl", "Alt", "Shift", "Win"].includes(modifier)) ||
    new Set(modifiers).size !== modifiers.length
  ) {
    return false;
  }

  return modifiers.length > 0 || isFunctionKey(key);
}

function keyLabel(key: string, code?: string) {
  if (code && /^Key[A-Z]$/.test(code)) {
    return code.slice(3);
  }
  if (code && /^Digit[0-9]$/.test(code)) {
    return code.slice(5);
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

  if (labels[key]) {
    return labels[key];
  }

  if (isFunctionKey(key)) {
    return key;
  }

  if (/^[A-Za-z0-9]$/.test(key)) {
    return key.toUpperCase();
  }

  return key.length === 1 ? key : "";
}

function isModifierKey(key: string) {
  return key === "Control" || key === "Alt" || key === "Shift" || key === "Meta";
}

function isFunctionKey(key: string) {
  return /^F([1-9]|1[0-9]|2[0-4])$/.test(key);
}

function isSupportedKey(key: string) {
  return (
    /^[A-Z0-9]$/.test(key) ||
    isFunctionKey(key) ||
    [
      "Esc",
      "Space",
      "Tab",
      "Enter",
      "Backspace",
      "Delete",
      "Insert",
      "Home",
      "End",
      "PageUp",
      "PageDown",
      "ArrowUp",
      "ArrowDown",
      "ArrowLeft",
      "ArrowRight"
    ].includes(key)
  );
}
