import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import { VitePWA } from 'vite-plugin-pwa'
import { fileURLToPath, URL } from 'node:url'

const proxyTarget = process.env.VITE_PROXY_TARGET || 'http://localhost:2241'
const base = process.env.VITE_BASE_PATH || '/'

export default defineConfig({
  base,
  plugins: [
    react(),
    tailwindcss(),
    // PWA: 后端动态提供 /manifest.json（含站点图标），这里只生成 Service
    // Worker 预缓存静态产物，支撑安装与离线壳。
    VitePWA({
      registerType: 'autoUpdate',
      manifest: false,
      includeAssets: ['icon.svg'],
      workbox: {
        globPatterns: ['**/*.{js,css,html,woff2,svg}'],
        navigateFallback: `${base}index.html`,
        navigateFallbackDenylist: [/^\/api\//, /^\/ws/, /^\/stream/, /^\/manifest\.json/, /^\/site-icon/],
        cleanupOutdatedCaches: true,
      },
    }),
  ],
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
  server: {
    // 双栈 localhost 在部分 Windows 机器上 ::1 不可用（ping 一般故障），固定 IPv4。
    host: '127.0.0.1',
    strictPort: true,
    port: 5173,
    proxy: {
      '/api': { target: proxyTarget, changeOrigin: true },
      '/ws': { target: proxyTarget, ws: true, changeOrigin: true },
      '/stream': { target: proxyTarget, changeOrigin: true },
      '/manifest.json': { target: proxyTarget, changeOrigin: true },
      '/site-icon': { target: proxyTarget, changeOrigin: true },
    },
  },
  preview: {
    port: 4173,
    proxy: {
      '/api': { target: proxyTarget, changeOrigin: true },
      '/ws': { target: proxyTarget, ws: true, changeOrigin: true },
      '/stream': { target: proxyTarget, changeOrigin: true },
      '/manifest.json': { target: proxyTarget, changeOrigin: true },
      '/site-icon': { target: proxyTarget, changeOrigin: true },
    },
  },
  build: {
    // Web 构建写入后端 static/；Electron 打包用 VITE_ELECTRON_OUT=dist 写入本地 dist/。
    outDir: process.env.VITE_ELECTRON_OUT || '../static',
    emptyOutDir: true,
    chunkSizeWarningLimit: 1500,
  },
})
