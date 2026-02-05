import React from "react";
import ReactDOM from "react-dom/client";
import "../styles/base.css";
import "../styles/buttons.css";
import "../styles/main.css";
import "../styles/messages.css";
import "../styles/composer.css";
import "../styles/panel-tabs.css";
import "../styles/approval-toasts.css";
import "../styles/debug.css";
import "../styles/compact-base.css";
import "../styles/compact-tablet.css";
import "../styles/compact-phone.css";
import "../styles/request-user-input.css";
import "../styles/tabbar.css";
import "../features/life/styles/life-dashboard.css";
import { initCodexBridge } from "./bridge";
import { LifeStreamWebApp } from "./LifeStreamWebApp";

initCodexBridge();

const setViewportHeight = () => {
  const height = window.visualViewport?.height ?? window.innerHeight;
  document.documentElement.style.setProperty("--visual-viewport-height", `${height}px`);
};

setViewportHeight();
window.addEventListener("resize", setViewportHeight);
window.visualViewport?.addEventListener("resize", setViewportHeight);
window.visualViewport?.addEventListener("scroll", setViewportHeight);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LifeStreamWebApp />
  </React.StrictMode>,
);
