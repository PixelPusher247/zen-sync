// Runs the frontend in a plain browser with the Tauri backend stubbed.
//   npx vite --config .claude/mock/vite.mock.config.mjs
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

const mock = readFileSync(new URL("./tauri-mock.js", import.meta.url), "utf8");

export default defineConfig({
  root: fileURLToPath(new URL("../..", import.meta.url)),
  plugins: [
    react(),
    {
      name: "tauri-mock",
      transformIndexHtml: () => [
        { tag: "script", children: mock, injectTo: "head-prepend" },
      ],
    },
  ],
  server: { port: 1430, strictPort: true },
});
