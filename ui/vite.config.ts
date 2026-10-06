import { defineConfig } from "vite";
import { resolve } from "node:path";

const pages = ["notch", "glow", "avatar"];

export default defineConfig({
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2022",
    rollupOptions: {
      input: Object.fromEntries(pages.map((p) => [p, resolve(__dirname, `${p}.html`)])),
    },
  },
  test: { environment: "jsdom" },
} as any);
