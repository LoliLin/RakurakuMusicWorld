// RakurakuMusicWorld — Electron dev runner.
//
// 职责：
//   1. 默认先起 frontend Vite dev server（可 --no-server 跳过，用于附着已有 dev server）
//   2. 等待 dev server 就绪
//   3. 以 RAKURAKU_DEV_SERVER_URL 启动 Electron，加载 dev server
//
// 后端（radio-backend）需另行运行在 :2241；Vite 已代理 /api /ws /stream。
import { spawn } from 'node:child_process'
import path from 'node:path'
import net from 'node:net'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const frontendDir = path.resolve(__dirname, '../../radio-backend/frontend')
const electronDir = path.resolve(__dirname, '..')

const args = process.argv.slice(2)
const startServer = !args.includes('--no-server')
const DEV_PORT = 5173
const DEV_URL = `http://127.0.0.1:${DEV_PORT}/`

function spawnWithLog(cmd, cmdArgs, opts) {
  const child = spawn(cmd, cmdArgs, { stdio: 'pipe', shell: process.platform === 'win32', ...opts })
  const tag = opts?.tag || cmd
  child.stdout.on('data', (d) => process.stdout.write(`[${tag}] ${d}`))
  child.stderr.on('data', (d) => process.stderr.write(`[${tag}] ${d}`))
  child.on('exit', (code) => {
    if (code !== 0 && code !== null) console.error(`[${tag}] exited with ${code}`)
  })
  return child
}

function waitForPort(port, host = '127.0.0.1', timeoutMs = 30000) {
  return new Promise((resolve, reject) => {
    const tryOnce = () => {
      const socket = net.connect({ port, host })
      socket.once('connect', () => {
        socket.destroy()
        resolve()
      })
      socket.once('error', () => {
        socket.destroy()
        if (Date.now() - started > timeoutMs) {
          reject(new Error(`timeout waiting for ${host}:${port}`))
        } else {
          setTimeout(tryOnce, 250)
        }
      })
    }
    tryOnce()
  })
}

let viteProc = null
let electronProc = null
let shuttingDown = false

function shutdown() {
  if (shuttingDown) return
  shuttingDown = true
  if (electronProc && electronProc.exitCode === null) electronProc.kill()
  if (viteProc && viteProc.exitCode === null) viteProc.kill()
  process.exit(0)
}
process.on('SIGINT', shutdown)
process.on('SIGTERM', shutdown)

if (startServer) {
  viteProc = spawnWithLog(process.platform === 'win32' ? 'npm.cmd' : 'npm', ['run', 'dev'], {
    cwd: frontendDir,
    tag: 'vite',
  })
}

try {
  await waitForPort(DEV_PORT)
  console.log(`[electron] dev server ready at ${DEV_URL}`)
} catch (err) {
  console.error('[electron] dev server did not become ready:', err.message)
  shutdown()
}

const electronBin = process.platform === 'win32'
  ? path.join(frontendDir, 'node_modules', 'electron', 'dist', 'electron.exe')
  : path.join(frontendDir, 'node_modules', '.bin', 'electron')

electronProc = spawnWithLog(electronBin, [electronDir], {
  cwd: frontendDir,
  tag: 'electron',
  env: { ...process.env, RAKURAKU_DEV_SERVER_URL: DEV_URL },
})
electronProc.on('exit', () => {
  // Electron 窗口关闭 → 结束整个 dev 会话。
  shutdown()
})
