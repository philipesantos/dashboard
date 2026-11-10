import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// The development content policy (specs/008-tauri-shell/contracts/content-policy.md). On
// desktop, `tauri dev` loads the page straight from this server, and Tauri attaches no policy to
// it, so the server sends one: the release policy of tauri.conf.json, read here so the two
// cannot drift, with these sources added and nothing else:
// - a fixed development nonce in script-src and style-src. Vite puts it on the scripts and
//   styles it writes (`html.cspNonce`), and main.tsx hands it to the components that insert
//   <style> elements, as it hands them Tauri's per-response nonce in a release build;
// - Vite's hot-reload socket in connect-src.
const DEV_NONCE = "dashboard-development";
const DEV_ADDITIONS: Record<string, string> = {
  "script-src": `'nonce-${DEV_NONCE}'`,
  "style-src": `'nonce-${DEV_NONCE}'`,
  "connect-src": "ws://localhost:1420",
};

function readCsp(file: string): Record<string, string> | undefined {
  const config = JSON.parse(readFileSync(new URL(`./src-tauri/${file}`, import.meta.url), "utf8"));
  return config?.app?.security?.csp;
}

function developmentPolicy(): string {
  const release = readCsp("tauri.conf.json");
  if (typeof release !== "object" || release === null) {
    throw new Error("tauri.conf.json has no app.security.csp object to build the development policy from");
  }
  // On Windows, Tauri merges tauri.windows.conf.json over it: the IPC channel's http form
  const merged = process.platform === "win32" ? { ...release, ...readCsp("tauri.windows.conf.json") } : release;
  return Object.entries(merged)
    .map(([directive, sources]) =>
      DEV_ADDITIONS[directive] ? `${directive} ${sources} ${DEV_ADDITIONS[directive]}` : `${directive} ${sources}`,
    )
    .join("; ");
}

// The shell probe (specs/008-tauri-shell/contracts/probe.md): in the `probe` mode only, the page
// also loads src/probe/run.ts. No other mode bundles it.
function shellProbe(): Plugin {
  return {
    name: "shell-probe",
    transformIndexHtml: {
      order: "pre",
      handler: (html) =>
        html.replace(
          "</body>",
          '  <script type="module" src="/src/probe/run.ts"></script>\n  </body>',
        ),
    },
  };
}

// https://vite.dev/config/
export default defineConfig(({ command, mode }) => ({
  plugins: [react(), tailwindcss(), ...(mode === "probe" ? [shellProbe()] : [])],

  // Only for the development server: in a build, Tauri sets each response's nonce itself
  html: command === "serve" ? { cspNonce: DEV_NONCE } : undefined,

  // Fonts and images stay files, never data: URLs, which the content policy refuses
  // (specs/008-tauri-shell/contracts/content-policy.md)
  build: {
    assetsInlineLimit: 0,
  },

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    headers: { "Content-Security-Policy": developmentPolicy() },
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
      ignored: ["**/src-tauri/**"],
    },
  },
}));
