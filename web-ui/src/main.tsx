import React from "react";
import ReactDOM from "react-dom/client";
import { HashRouter } from "react-router-dom";
import App from "./App";
import "./styles.css";

// HashRouter (not BrowserRouter): this app is a single static bundle served
// by a loopback-only axum binary with no server-side routing. Hash-based
// routes (e.g. "#/sign") never hit the server at all, and the ?token=...
// query string — which sits before the "#" — survives every navigation
// untouched, exactly like the pre-React hand-rolled hash router this
// replaces.
ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <HashRouter>
      <App />
    </HashRouter>
  </React.StrictMode>
);
