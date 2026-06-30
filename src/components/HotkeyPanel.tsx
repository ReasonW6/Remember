const hotkeys = [
  ["Record", "Ctrl+Alt+R"],
  ["Play", "Ctrl+Alt+P"],
  ["Stop", "Ctrl+Alt+Esc"]
];

export function HotkeyPanel() {
  return (
    <section className="panel hotkey-panel" aria-labelledby="hotkeys-title">
      <h2 id="hotkeys-title">Hotkeys</h2>
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
