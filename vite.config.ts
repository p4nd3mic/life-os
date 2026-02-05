import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

const packageJson = JSON.parse(
  readFileSync(new URL("./package.json", import.meta.url), "utf-8"),
) as {
  version: string;
};

const webviewAliases = {
  "@tauri-apps/api/core": fileURLToPath(
    new URL("./src/webview/tauri/core.ts", import.meta.url),
  ),
  "@tauri-apps/api/event": fileURLToPath(
    new URL("./src/webview/tauri/event.ts", import.meta.url),
  ),
  "@tauri-apps/api/window": fileURLToPath(
    new URL("./src/webview/tauri/window.ts", import.meta.url),
  ),
  "@tauri-apps/api/menu": fileURLToPath(
    new URL("./src/webview/tauri/menu.ts", import.meta.url),
  ),
  "@tauri-apps/api/dpi": fileURLToPath(
    new URL("./src/webview/tauri/dpi.ts", import.meta.url),
  ),
  "@tauri-apps/api/app": fileURLToPath(
    new URL("./src/webview/tauri/app.ts", import.meta.url),
  ),
  "@tauri-apps/plugin-dialog": fileURLToPath(
    new URL("./src/webview/tauri/plugin-dialog.ts", import.meta.url),
  ),
  "@tauri-apps/plugin-opener": fileURLToPath(
    new URL("./src/webview/tauri/plugin-opener.ts", import.meta.url),
  ),
  "@tauri-apps/plugin-updater": fileURLToPath(
    new URL("./src/webview/tauri/plugin-updater.ts", import.meta.url),
  ),
  "@tauri-apps/plugin-process": fileURLToPath(
    new URL("./src/webview/tauri/plugin-process.ts", import.meta.url),
  ),
  "tauri-plugin-liquid-glass-api": fileURLToPath(
    new URL("./src/webview/tauri/liquid-glass.ts", import.meta.url),
  ),
};

// https://vite.dev/config/
export default defineConfig(async () => {
  const isWebviewBuild = process.env.VITE_BUILD_TARGET === "webview";

  return {
    plugins: [react()],
    worker: {
      format: "es",
    },
    resolve: isWebviewBuild ? { alias: webviewAliases } : undefined,
    base: isWebviewBuild ? "./" : undefined,
    build: isWebviewBuild
      ? {
          outDir: "dist-webview",
          emptyOutDir: true,
          rollupOptions: {
            input: {
              webview: fileURLToPath(
                new URL("./index.webview.html", import.meta.url),
              ),
            },
          },
        }
      : undefined,
    define: {
      __APP_VERSION__: JSON.stringify(packageJson.version),
    },
    test: {
      environment: "node",
      include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
      setupFiles: ["src/test/vitest.setup.ts"],
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // 1. prevent Vite from obscuring rust errors
    clearScreen: false,
    // 2. tauri expects a fixed port, fail if that port is not available
    server: {
      port: 1420,
      strictPort: true,
      host: host || false,
      hmr: host
        ? {
            protocol: "ws",
            host,
            port: 1421,
          }
        : undefined,
      watch: {
        // 3. tell Vite to ignore watching `src-tauri`
        ignored: ["**/src-tauri/**", "**/.codex-worktrees/**"],
      },
    },
  };
});
