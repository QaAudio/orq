import path from "node:path";
import { fileURLToPath } from "node:url";
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

const root = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  base: "./",
  plugins: [vue()],
  resolve: {
    alias: {
      "@": path.resolve(root, "src"),
      "@mdi": path.resolve(root, "node_modules/vue-material-design-icons"),
    },
  },
  build: {
    outDir: "dashboard/dist",
    emptyOutDir: true,
  },
  server: {
    port: 5179,
    proxy: {
      "/data.json": "http://127.0.0.1:9847",
      "/api": "http://127.0.0.1:9847",
      "/canvas": "http://127.0.0.1:9847",
    },
  },
});
