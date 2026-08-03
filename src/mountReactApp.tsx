import { StrictMode, type ReactNode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { flushSync } from "react-dom";

export function mountReactApp(content: ReactNode): Root {
  const container = document.getElementById("root");
  if (!container) {
    throw new Error("Remember root element is unavailable");
  }

  const root = createRoot(container);
  flushSync(() => {
    root.render(<StrictMode>{content}</StrictMode>);
  });
  return root;
}
