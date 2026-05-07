import { resolve } from "path";
import { defineConfig } from "vite";

// Vite output goes to ../dist; Rust serves it at /static/.
// Manifest is read at runtime so templates resolve hashed asset names.
// Four entry points mirror the original Django split: base (shared shell),
// pages (marketing static pages), properties (dashboard charts/map), and
// collector (the public embed script).

export default defineConfig({
  base: "/static/",
  build: {
    outDir: resolve(__dirname, "../dist"),
    emptyOutDir: true,
    manifest: true,
    rollupOptions: {
      input: {
        base: resolve(__dirname, "static_src/base/index.js"),
        pages: resolve(__dirname, "static_src/pages/index.js"),
        properties: resolve(__dirname, "static_src/properties/index.js"),
        collector: resolve(__dirname, "static_src/collector/index.js"),
      },
      output: {
        // Hashed asset filenames so we can cache them aggressively.
        entryFileNames: "assets/[name]-[hash].js",
        chunkFileNames: "assets/[name]-[hash].js",
        assetFileNames: (assetInfo) => {
          if (/\.(png|jpg|gif|svg|webp)$/.test(assetInfo.name || "")) {
            return "images/[name]-[hash][extname]";
          }
          return "assets/[name]-[hash][extname]";
        },
      },
    },
  },
  css: {
    preprocessorOptions: {
      scss: {
        quietDeps: true,
      },
    },
  },
});
