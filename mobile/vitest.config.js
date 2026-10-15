import { defineConfig, mergeConfig } from 'vitest/config'
import viteConfig from './vite.config.js'

/* Merged with the app's own config so that the transport seam (the plugin that
   answers the stores' Tauri imports with the bridge) is the one the tests run
   through, not a copy of it. */
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: 'happy-dom',
      include: ['tests/**/*.test.js'],
      setupFiles: ['tests/support/setup.js'],
      globals: false,
      server: { deps: { inline: [/\/src\//] } }
    }
  })
)
