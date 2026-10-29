import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { Dock } from "./components/dock/Dock";
import { currentView } from "./lib/view";
import "./styles.css";

const root = document.getElementById("root");
if (!root) throw new Error("missing #root element");

const view = currentView();
document.documentElement.dataset.view = view;

createRoot(root).render(<StrictMode>{view === "dock" ? <Dock /> : <App />}</StrictMode>);
