import type { CapacitorConfig } from '@capacitor/cli'

const config: CapacitorConfig = {
  appId: 'in.kawaiis.RakurakuMusicWorld',
  appName: 'RakurakuMusicWorld',
  webDir: '../static',
  server: {
    androidScheme: 'https',
    cleartext: true,
  },
  android: {
    allowMixedContent: true,
  },
}

export default config
