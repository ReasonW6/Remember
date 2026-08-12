import { WindowBinding } from "./WindowBinding";
import { mountReactApp } from "./mountReactApp";
import "./styles.css";

document.documentElement.classList.add("window-binding-page");
mountReactApp(<WindowBinding />);
