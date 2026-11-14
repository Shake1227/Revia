import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

function dropLegacyFonts(): Plugin {
  return {
    name: "revia:drop-legacy-fonts",
    enforce: "pre",
    transform(code, id) {
      const file = id.split("?")[0];
      if (!file.includes("@fontsource") || !file.endsWith(".css")) return null;
      return { code: code.replace(/,\s*url\([^)]*\.woff\)\s*format\((['"])woff\1\)/g, ""), map: null };
    },
  };
}

export default defineConfig({
  plugins: [dropLegacyFonts(), react()],
  build: { chunkSizeWarningLimit: 1200 },
  server: {
    host: "127.0.0.1",
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});
