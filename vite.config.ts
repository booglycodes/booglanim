import { defineConfig } from "vite";

import monacoEditorPlugin from 'vite-plugin-monaco-editor';

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
  },
  // 3. to make use of `TAURI_DEBUG` and other env variables
  // https://tauri.studio/v1/api/config#buildconfig.beforedevcommand
  envPrefix: ["VITE_", "TAURI_"],
  plugins: [(monacoEditorPlugin as any).default({})],
  build: {
    rollupOptions: {
        input: {
            index: "./index.html",
            main: "./src/main.html",
            animate_char: "./src/character_editors/animate.html",
            create_char: "./src/character_editors/create.html",
        },
    },
  },
}));
