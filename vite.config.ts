import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import type { Plugin } from "postcss";

// @ts-expect-error process is a nodejs global
const host = process.env.TAURI_DEV_HOST;

/**
 * Every `:hover` rule applies only where a pointer can hover.
 *
 * A touch screen keeps `:hover` on whatever was tapped last, until the next tap
 * lands elsewhere: a button stays lit as if still pressed, which is the first
 * thing that gives a web page away among native apps. Writing the media query
 * around each of the window's hover rules by hand is a rule nobody remembers on
 * the next one, so the build does it. Selectors of the same rule without
 * `:hover` stay where they were and keep working on touch.
 */
const hoverOnly: Plugin = {
  postcssPlugin: "yaca-hover-only",
  Rule(rule, { AtRule }) {
    const hovered = rule.selectors.filter((selector) => selector.includes(":hover"));
    if (hovered.length === 0) return;
    const parent = rule.parent;
    if (parent?.type === "atrule" && "params" in parent && parent.params === "(hover: hover)") return;
    const media = new AtRule({ name: "media", params: "(hover: hover)" });
    media.append(rule.clone({ selectors: hovered }));
    rule.after(media);
    const rest = rule.selectors.filter((selector) => !selector.includes(":hover"));
    if (rest.length > 0) rule.selectors = rest;
    else rule.remove();
  },
};

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [react()],

  css: {
    postcss: { plugins: [hoverOnly] },
  },

  // The feed reducer and the dictionaries are pure functions: they need neither
  // a browser nor a window, and are checked the same way the core is checked
  // without a webview.
  test: {
    environment: "node",
    include: ["src/**/*.test.ts"],
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    // By address, not by name. `localhost` resolves to `::1` first and vite
    // binds only to it, while the shell and the browser go to 127.0.0.1 and
    // wait for a server that is already up. `devUrl` in `tauri.conf.json` is
    // the same address.
    host: host || "127.0.0.1",
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // Cargo can lock its output while Vite is watching the frontend.
      ignored: ["**/src-tauri/**", "**/target/**"],
    },
  },
}));
