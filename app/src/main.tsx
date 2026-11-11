import { setNonce } from "get-nonce";
import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";

// Radix's scroll lock inserts <style> elements at run time, through react-style-singleton. The
// content policy refuses a <style> without this response's nonce, which Tauri puts on the empty
// <style> of index.html (specs/008-tauri-shell/research.md, R3). `nonce` is read as a property:
// engines hide the attribute's value once the page has loaded.
const styleNonce = document.querySelector("style")?.nonce;
if (styleNonce) setNonce(styleNonce);

// The engine's own context menu would offer reload, inspect and the like. WebView2's is turned
// off in window.rs; the macOS and Linux engines have no setting, so the page cancels it in release builds
// (specs/008-tauri-shell/research.md, R6).
if (import.meta.env.PROD) {
  document.addEventListener("contextmenu", (event) => event.preventDefault());
}

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
