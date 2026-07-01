const hotkeys = [
  ["录制", "Ctrl+Alt+R"],
  ["播放", "Ctrl+Alt+P"],
  ["停止", "Ctrl+Alt+Esc"]
];

export function HotkeyPanel() {
  return (
    <section className="panel hotkey-panel" aria-labelledby="hotkeys-title">
      <h2 id="hotkeys-title">快捷键</h2>
      <dl className="hotkey-list">
        {hotkeys.map(([label, shortcut]) => (
          <div key={shortcut}>
            <dt>{label}</dt>
            <dd>
              <kbd>{shortcut}</kbd>
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}
