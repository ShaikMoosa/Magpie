import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import "./index.css";
import { initPreviews } from "./lib/previews";

async function start() {
  // Outside Tauri (plain browser during development) use an in-memory backend.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window && (window as any).__TAURI_INTERNALS__?.invoke)) {
    const { installMockBackend } = await import("./dev/mockBackend");
    installMockBackend();
  }
  await initPreviews();
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void start();
