import { resolve } from "path";
import { defineConfig } from "vite";

// datamaps pulls in d3 v3, whose IIFE reads `this.document` expecting the
// global object. Under ESM/strict `this` is undefined, which crashes at load.
const fixD3v3GlobalThis = {
  name: "fix-d3-v3-global-this",
  transform(code, id) {
    if (id.includes("datamaps/node_modules/d3/d3.js")) {
      return code.replace("this.document", "globalThis.document");
    }
  },
};

export default defineConfig({
  plugins: [fixD3v3GlobalThis],
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
