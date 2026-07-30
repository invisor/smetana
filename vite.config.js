import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [vue()],
  // Tauri handles terminal output itself
  clearScreen: false,
  server: {
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] }
  },
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) }
  },
  // Tauri runs in the system webview: WebKitGTK / WKWebView / WebView2.
  build: { target: ['es2021', 'chrome100', 'safari15'] }
})
