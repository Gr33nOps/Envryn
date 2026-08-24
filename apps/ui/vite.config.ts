import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { tanstackRouter } from "@tanstack/router-plugin/vite";
import { fileURLToPath, URL } from "node:url";

// Envryn is a Tauri desktop/mobile app, not a website: this is a pure
// client-side SPA with no SSR, no server entry, and no remote asset hosts.
export default defineConfig({
  plugins: [
    // Must precede the React plugin -- it generates routeTree.gen.ts.
    tanstackRouter({ target: "react", autoCodeSplitting: true }),
    react(),
    tailwindcss(),
  ],

  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },

  // Tauri drives the dev server; a fixed port keeps tauri.conf.json honest.
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // The Rust core has its own rebuild loop; watching it churns the frontend.
      ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"],
    },
  },

  build: {
    // Matches the WebView2 / Android WebView floor from ARCHITECTURE.md.
    target: ["chrome105", "safari13"],
    sourcemap: false,
  },
});
