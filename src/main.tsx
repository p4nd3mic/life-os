import React from "react";
import ReactDOM from "react-dom/client";
import * as Sentry from "@sentry/react";
import "./styles/base.css";
import "./styles/buttons.css";
import "./styles/main.css";
import "./styles/messages.css";
import "./styles/composer.css";
import "./styles/panel-tabs.css";
import "./styles/approval-toasts.css";
import "./styles/debug.css";
import "./styles/compact-base.css";
import "./styles/compact-tablet.css";
import "./styles/compact-phone.css";
import "./styles/request-user-input.css";
import "./styles/tabbar.css";
import "./features/life/styles/life-dashboard.css";
import { LifeStreamWebApp } from "./webview/LifeStreamWebApp";

const sentryDsn =
  import.meta.env.VITE_SENTRY_DSN ??
  "https://8ab67175daed999e8c432a93d8f98e49@o4510750015094784.ingest.us.sentry.io/4510750016012288";

Sentry.init({
  dsn: sentryDsn,
  enabled: Boolean(sentryDsn),
  release: __APP_VERSION__,
});

Sentry.metrics.count("app_open", 1, {
  attributes: {
    env: import.meta.env.MODE,
    platform: "macos",
  },
});

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <LifeStreamWebApp />
  </React.StrictMode>,
);
