import React from "react";
import ReactDOM from "react-dom/client";
import { AdminApp } from "./App";
import "../shared/tokens.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <AdminApp />
  </React.StrictMode>,
);
