import React from "react";
import ReactDOM from "react-dom/client";
import { TrayPanel } from "./TrayPanel";
import "./tray.css";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TrayPanel />
  </React.StrictMode>
);
