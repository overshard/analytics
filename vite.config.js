import { resolve } from "path";
import { defineConfig } from "vite";

export default defineConfig({
  base: "/static/",
  build: {
    outDir: resolve(__dirname, "analytics/static"),
    emptyOutDir: true,
    rollupOptions: {
      input: {
        base: resolve(__dirname, "analytics/static_src/index.js"),
        pages: resolve(__dirname, "pages/static_src/index.js"),
        properties: resolve(__dirname, "properties/static_src/index.js"),
        collector: resolve(__dirname, "collector/static_src/index.js"),
      },
      output: {
        entryFileNames: "[name].js",
        assetFileNames: (assetInfo) => {
          if (/\.(png|jpg|gif|svg|webp)$/.test(assetInfo.name)) {
            return "images/[name][extname]";
          }
          return "[name][extname]";
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
