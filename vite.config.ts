import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { resolve } from "path";
import Components from "unplugin-vue-components/vite";
import { NaiveUiResolver } from "unplugin-vue-components/resolvers";

// https://vitejs.dev/config/
export default defineConfig(async () => ({
  plugins: [
    vue(),
    Components({
      resolvers: [NaiveUiResolver()],
      dts: "src/components.d.ts",
    }),
  ],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    watch: {
      // 3. tell vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },

  build: {
    // Tauri 使用系统 WebView2（现代 Chromium），可锁定高版本目标以获得更优 minify
    target: "es2022",
    // 生产包剥离 console/debugger（受 import.meta.env.DEV 守卫的开发日志仍保留）
    esbuild: {
      drop: ["console", "debugger"],
    },
    rollupOptions: {
      input: {
        main: resolve(__dirname, "index.html"),
        opgg: resolve(__dirname, "opgg.html"),
      },
      output: {
        manualChunks: {
          "vue-vendor": ["vue", "pinia"],
          "naive-ui": ["naive-ui"],
          "i18n": ["vue-i18n"],
        },
      },
    },
  },
}));
