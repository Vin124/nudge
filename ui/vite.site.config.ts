// Landing page build (site/), deployed on Vercel at the domain root. Lives here so it uses this
// package's vite. The demo imports the app's own sprite code from ./src so it
// can't drift from what the app draws.
//   npm --prefix ui run site:dev | site:build
import { defineConfig } from "vite";
import { resolve } from "node:path";

const site = resolve(__dirname, "../site");

export default defineConfig({
  root: site,
  // "/" on Vercel; set SITE_BASE for a sub-path host.
  base: process.env.SITE_BASE ?? "/",
  server: { port: 1432, strictPort: true, fs: { allow: [resolve(__dirname, "..")] } },
  build: { outDir: resolve(site, "dist"), emptyOutDir: true, target: "es2020" },
});
