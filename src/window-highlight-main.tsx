import { mountReactApp } from "./mountReactApp";
import "./styles.css";

document.documentElement.classList.add("window-highlight-page");
mountReactApp(<div className="window-highlight-frame" aria-hidden="true" />);
