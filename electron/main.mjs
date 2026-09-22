import { app, BrowserWindow, shell } from 'electron'
import path from 'node:path'
import fs from 'node:fs'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))

// packaged: electron/resources/<relative>  |  dev: <repo>/electron/<relative>
const resourceRoot = app.isPackaged ? process.resourcesPath : __dirname
function resolveResource(...parts) {
  return path.join(resourceRoot, ...parts)
}

/** DEV_SERVER_URL 由 dev 脚本注入；生产默认加载本地 frontend/index.html。 */
const DEV_SERVER_URL = process.env.RAKURAKU_DEV_SERVER_URL || ''
const FRONTEND_INDEX = () => {
  const inApp = path.join(__dirname, 'frontend', 'index.html')
  if (fs.existsSync(inApp)) return inApp
  const inResources = resolveResource('frontend', 'index.html')
  if (fs.existsSync(inResources)) return inResources
  const backendStatic = path.join(__dirname, '../radio-backend/static/index.html')
  if (fs.existsSync(backendStatic)) return backendStatic
  return inApp
}

const isDev = DEV_SERVER_URL !== ''

function createMainWindow() {
  // 图标缺失时交给系统默认 — 不要让坏路径阻塞窗口创建。
  // Windows 上 BrowserWindow 只接受 .ico 作为窗口图标（PNG 会触发原生解析错误）。
  const iconFile = process.platform === 'win32' ? 'icon.ico' : 'icon.png'
  const iconInDir = path.join(__dirname, iconFile)
  const iconPath = fs.existsSync(iconInDir) ? iconInDir : resolveResource(iconFile)
  const win = new BrowserWindow({
    // 前端双栏布局需要 ≥1280 CSS px（xl 断点）；Windows DPI 缩放下 1280 会被
    // 压到断点以下退化成竖排，默认给足余量。
    width: 1440,
    height: 900,
    minWidth: 1280,
    minHeight: 720,
    autoHideMenuBar: true,
    title: 'RakurakuMusicWorld',
    ...(fs.existsSync(iconPath) ? { icon: iconPath } : {}),
    webPreferences: {
      preload: path.join(__dirname, 'preload.mjs'),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
      spellcheck: false,
    },
  })

  win.once('ready-to-show', () => win.show())

  // 外部链接交给系统默认浏览器，应用内只保留电台页面。
  win.webContents.setWindowOpenHandler(({ url }) => {
    if (url.startsWith('http://') || url.startsWith('https://')) {
      shell.openExternal(url)
    }
    return { action: 'deny' }
  })

  if (isDev) {
    win.loadURL(DEV_SERVER_URL)
  } else {
    win.loadFile(FRONTEND_INDEX())
  }
  return win
}

process.on('uncaughtException', (e) => {
  console.error('[main] uncaught:', e && e.stack || e)
})

app.whenReady().then(() => {
  createMainWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createMainWindow()
  })
})

app.on('window-all-closed', () => {
  // macOS 惯例：关窗不退出，保留 Dock 上的应用。
  if (process.platform !== 'darwin') app.quit()
})
