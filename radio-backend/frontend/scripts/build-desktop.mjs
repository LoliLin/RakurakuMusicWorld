import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import builder from 'electron-builder'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const frontendDir = path.resolve(__dirname, '..')
const repoRoot = path.resolve(__dirname, '../../..')
const electronDir = path.resolve(repoRoot, 'electron')
const staticSrc = path.resolve(frontendDir, '../static')
const frontendDest = path.join(electronDir, 'frontend')
const outputDir = path.resolve(repoRoot, 'dist-desktop')

console.log('[*] Preparing frontend static assets into electron/frontend...')
if (fs.existsSync(frontendDest)) {
  fs.rmSync(frontendDest, { recursive: true, force: true })
}
fs.cpSync(staticSrc, frontendDest, { recursive: true })
console.log('[✓] Copied static frontend assets.')

const binSrc = path.resolve(repoRoot, 'dist/radio-backend.exe')
const electronBinDir = path.join(electronDir, 'bin')
fs.mkdirSync(electronBinDir, { recursive: true })
const electronBinDest = path.join(electronBinDir, 'radio-backend.exe')

if (fs.existsSync(binSrc)) {
  fs.copyFileSync(binSrc, electronBinDest)
  console.log('[✓] Copied radio-backend.exe to electron/bin/')
} else {
  console.warn('[!] dist/radio-backend.exe not found, backend will not be bundled.')
}

const configSrc = path.resolve(repoRoot, 'radio-backend/config.toml.example')
const electronConfigDest = path.join(electronDir, 'config.toml.example')
if (fs.existsSync(configSrc)) {
  fs.copyFileSync(configSrc, electronConfigDest)
}

console.log('[*] Packaging Electron desktop application with electron-builder...')
try {
  const result = await builder.build({
    projectDir: electronDir,
    targets: builder.Platform.WINDOWS.createTarget(['dir', 'portable']),
    config: {
      appId: 'in.kawaiis.RakurakuMusicWorld',
      productName: 'RakurakuMusicWorld',
      electronVersion: '44.2.0',
      directories: {
        output: outputDir,
      },
      files: [
        'main.mjs',
        'preload.mjs',
        'icon.ico',
        'icon.png',
        'frontend/**/*',
      ],
      extraResources: [
        {
          from: 'bin/radio-backend.exe',
          to: 'bin/radio-backend.exe',
        },
        {
          from: 'config.toml.example',
          to: 'config.toml.example',
        },
      ],
      win: {
        icon: 'icon.ico',
        target: ['dir', 'portable'],
      },
      portable: {
        artifactName: 'RakurakuMusicWorld-Portable-${version}.${ext}',
      },
    },
  })
  console.log('[✓] Desktop packaging complete!')
  console.log('Artifacts created in:', outputDir)
  for (const file of result) {
    console.log(' - ' + file)
  }
} catch (err) {
  console.error('[!] Electron packaging failed:', err)
  process.exit(1)
}
