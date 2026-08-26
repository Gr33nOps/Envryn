// vitest/config's defineConfig (not plain vite's) merges Vite's and
// Vitest's config types, so TypeScript recognises the `test` key below.
import { defineConfig } from "vitest/config";
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

  test: {
    // jsdom, not the vitest default (node): component tests below render
    // real React trees and need a DOM to render into.
    environment: "jsdom",
    setupFiles: ["./src/test-setup.ts"],
    coverage: {
      provider: "v8",
      // lcov is what SonarQube's generic JS/TS coverage import reads
      // (sonar.javascript.lcov.reportPaths in sonar-project.properties);
      // text is just for a human glancing at local `npm run test:coverage`
      // output.
      reporter: ["lcov", "text"],
      include: ["src/**/*.{ts,tsx}"],
      exclude: [
        "src/routeTree.gen.ts",
        // Generated directly from Rust by ts-rs -- see
        // packages/contract/index.ts's own doc comment. Nothing here is
        // hand-written, so coverage on it would not mean anything.
        "**/*.d.ts",
      ],
    },
  },
});
