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

import { spawn } from 'node:child_process'
import http from 'node:http'

let backendProcess = null

function checkServerReady(port = 2241, timeoutMs = 15000) {
  const start = Date.now()
  return new Promise((resolve) => {
    const tryPing = () => {
      const req = http.get(`http://127.0.0.1:${port}/api/station`, (res) => {
        if (res.statusCode === 200) {
          resolve(true)
        } else {
          retry()
        }
      })
      req.on('error', () => retry())
      req.setTimeout(1000, () => {
        req.destroy()
        retry()
      })
    }
    const retry = () => {
      if (Date.now() - start > timeoutMs) {
        resolve(false)
      } else {
        setTimeout(tryPing, 300)
      }
    }
    tryPing()
  })
}

function findBackendBinary() {
  const binName = process.platform === 'win32' ? 'radio-backend.exe' : 'radio-backend'
  
  // 1. Packaged location in app resources: resources/bin/radio-backend.exe or resources/radio-backend.exe
  const inResourcesBin = resolveResource('bin', binName)
  if (fs.existsSync(inResourcesBin)) return inResourcesBin
  const inResources = resolveResource(binName)
  if (fs.existsSync(inResources)) return inResources

  // 2. Dev / Repo target directory
  const repoRoot = path.resolve(__dirname, '..')
  const releaseBin = path.join(repoRoot, 'radio-backend', 'target', 'release', binName)
  if (fs.existsSync(releaseBin)) return releaseBin
  const debugBin = path.join(repoRoot, 'radio-backend', 'target', 'debug', binName)
  if (fs.existsSync(debugBin)) return debugBin
  const distBin = path.join(repoRoot, 'dist', binName)
  if (fs.existsSync(distBin)) return distBin

  return null
}

async function ensureBackendRunning() {
  // Check if a server is already listening on 2241
  const alreadyRunning = await checkServerReady(2241, 1000)
  if (alreadyRunning) {
    console.log('[main] Existing backend detected on port 2241, connecting to it.')
    return true
  }

  const binaryPath = findBackendBinary()
  if (!binaryPath) {
    console.warn('[main] No radio-backend binary found, skipping auto-launch.')
    return false
  }

  // Determine working directory for backend
  let workDir
  if (app.isPackaged) {
    workDir = path.join(app.getPath('userData'), 'world')
  } else {
    workDir = path.resolve(__dirname, '../dist')
    if (!fs.existsSync(workDir)) {
      workDir = path.resolve(__dirname, '../radio-backend')
    }
  }

  fs.mkdirSync(path.join(workDir, 'data'), { recursive: true })
  fs.mkdirSync(path.join(workDir, 'media'), { recursive: true })

  const targetConfig = path.join(workDir, 'config.toml')
  if (!fs.existsSync(targetConfig)) {
    const exampleConfig = resolveResource('config.toml.example')
    const repoExample = path.resolve(__dirname, '../radio-backend/config.toml.example')
    const srcConfig = fs.existsSync(exampleConfig) ? exampleConfig : (fs.existsSync(repoExample) ? repoExample : null)
    if (srcConfig) {
      fs.copyFileSync(srcConfig, targetConfig)
    }
  }

  console.log(`[main] Launching integrated World server: ${binaryPath} in ${workDir}`)
  backendProcess = spawn(binaryPath, [], {
    cwd: workDir,
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      RADIO_SERVER_PORT: '2241',
      RADIO_DISCOVERY_ENABLED: 'true',
    },
  })

  backendProcess.on('exit', (code, signal) => {
    console.log(`[main] Backend process exited with code ${code}, signal ${signal}`)
    backendProcess = null
  })

  const ready = await checkServerReady(2241, 15000)
  if (ready) {
    console.log('[main] Integrated World server is ready!')
  } else {
    console.error('[main] Timeout waiting for backend server to become ready.')
  }
  return ready
}

function stopBackendProcess() {
  if (backendProcess) {
    console.log('[main] Stopping integrated World server...')
    try {
      if (process.platform === 'win32') {
        spawn('taskkill', ['/pid', backendProcess.pid.toString(), '/f', '/t'])
      } else {
        backendProcess.kill()
      }
    } catch (e) {
      console.error('[main] Error killing backend process:', e)
    }
    backendProcess = null
  }
}

app.whenReady().then(async () => {
  await ensureBackendRunning()
  createMainWindow()

  app.on('activate', () => {
    if (BrowserWindow.getAllWindows().length === 0) createMainWindow()
  })
})

app.on('window-all-closed', () => {
  // macOS 惯例：关窗不退出，保留 Dock 上的应用。
  if (process.platform !== 'darwin') {
    app.quit()
  }
})

app.on('will-quit', () => {
  stopBackendProcess()
})

