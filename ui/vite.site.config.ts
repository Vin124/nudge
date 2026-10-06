// Landing page build for GitHub Pages (site/). Lives here so it uses this
// package's vite. The demo imports the app's own sprite code from ./src so it
// can't drift from what the app draws.
//   npm --prefix ui run site:dev | site:build
import { defineConfig } from "vite";
import { resolve } from "node:path";

const site = resolve(__dirname, "../site");

export default defineConfig({
  root: site,
  base: "/nudge/",
  server: { port: 1432, strictPort: true, fs: { allow: [resolve(__dirname, "..")] } },
  build: { outDir: resolve(site, "dist"), emptyOutDir: true, target: "es2020" },
});
